//! Traps, mines, totems and minions in the real fight (`unit`): entities of the rules' world in id order, made by a
//! cast, acting on their own, struck and destroyed like bodies, each step told to the game.

mod common;

use arena_calc::device::Kind;
use arena_calc::loadout::{Loadout, Slot as LoadoutSlot};
use common::{always, duel_of, idle, loadout, run};
use pixfx_duel::duel::Duel;
use pixfx_duel::out::Out;
use pixfx_duel::unit::Unit;

/// `skill` with `supports` first in an otherwise plain loadout.
fn with(skill: &str, supports: &[&str]) -> Loadout {
    let mut l = loadout(&[skill], "tribulation");
    l.actives[0] = LoadoutSlot {
        skill: skill.to_string(),
        supports: supports.iter().map(|s| s.to_string()).collect(),
        fuse: None,
    };
    assert!(arena_calc::loadout::check(&l).is_ok(), "{l:?}");
    l
}

/// Seat 0 casts `skill` (made by `l`) again and again, seat 1 stands; `skill` is on seat 0's bar, both stand
/// `apart` pixels from each other, nobody dodges.
fn caster(l: Loadout, skill: &'static str, apart: f64) -> Duel {
    let mut d = duel_of(7, [always(skill), idle()], [l, loadout(&[], "tribulation")]);
    let s = d.state_mut();
    s.fighters[0].bar[0] = common::common(skill);
    s.fighters[0].c.souls = 30.0;
    s.fighters[1].body.x = s.fighters[0].body.x + apart;
    for f in &mut s.fighters {
        f.c.evasion = 0.0;
    }
    d
}

fn count(out: &[Out], f: impl Fn(&Out) -> bool) -> usize {
    out.iter().filter(|o| f(o)).count()
}

#[test]
fn a_minion_is_an_entity_that_walks_strikes_and_dies_for_a_soul() {
    let mut d = caster(loadout(&["beans"], "tribulation"), "beans", 500.0);
    let out = run(&mut d, 1.0);
    let spawned = count(&out, |o| {
        matches!(
            o,
            Out::Spawn {
                kind: Kind::Minion,
                ..
            }
        )
    });
    assert!(spawned >= 1, "the beans stand up");
    // in the world, in id order
    let mut q = d.world_mut().query::<&Unit>();
    let mut ids: Vec<u32> = q.iter(d.world()).map(|u| u.id).collect();
    assert!(!ids.is_empty());
    ids.sort_unstable();
    let state: Vec<u32> = d.state().units.iter().map(|u| u.id).collect();
    assert_eq!(ids, state);
    let x0 = d.state().units[0].x;
    let out = run(&mut d, 3.0);
    assert!(
        d.state().units.first().is_some_and(|u| u.x > x0),
        "it walks at the foe"
    );
    assert!(
        count(&out, |o| matches!(o, Out::UnitAct { .. })) >= 1
            && count(&out, |o| matches!(o, Out::Hit { spell: "beans", .. })) >= 1,
        "it strikes the foe"
    );
    // struck down, it dies and both fighters near it gain a soul
    let souls = d.state().fighters.each_ref().map(|f| f.c.souls);
    let id = d.state().units[0].id;
    let p = d.state().units[0].payload();
    {
        let s = d.state_mut();
        s.units[0].c.life = 1.0;
        let blow = pixfx_duel::strike::Payload {
            by: 1,
            unit: None,
            ..p
        };
        assert!(pixfx_duel::unit::hurt_unit(s, id, &blow));
    }
    assert!(!d.state().units.iter().any(|u| u.id == id));
    let near: Vec<bool> = d
        .state()
        .fighters
        .iter()
        .map(|f| (f.body.x - d.state().fighters[0].body.x).abs() < 4.0 * pixfx_duel::data::T)
        .collect();
    for (i, f) in d.state().fighters.iter().enumerate() {
        if near[i] {
            assert!(f.c.souls > souls[i], "seat {i} gains a soul");
        }
    }
    assert!(
        d.state()
            .out
            .iter()
            .any(|o| matches!(o, Out::UnitDie { id: x, .. } if *x == id))
    );
}

#[test]
fn a_totem_casts_again_and_again_on_its_cadence() {
    let mut d = caster(loadout(&["arsenal"], "tribulation"), "arsenal", 400.0);
    let out = run(&mut d, 8.0);
    assert!(
        count(&out, |o| matches!(
            o,
            Out::Spawn {
                kind: Kind::Totem,
                ..
            }
        )) >= 1
    );
    let acts = count(&out, |o| matches!(o, Out::UnitAct { .. }));
    assert!(acts >= 3, "a totem casts several times: {acts}");
}

#[test]
fn a_trap_springs_on_the_foe_that_stands_on_it() {
    let mut d = caster(with("fireball", &["trap"]), "fireball", 400.0);
    let out = run(&mut d, 2.0);
    assert!(
        count(&out, |o| matches!(
            o,
            Out::Spawn {
                kind: Kind::Trap,
                ..
            }
        )) >= 1
    );
    assert!(
        count(&out, |o| matches!(o, Out::UnitAct { .. })) >= 1,
        "it springs"
    );
    assert!(
        count(&out, |o| matches!(
            o,
            Out::Hit {
                spell: "fireball",
                by: 0,
                ..
            }
        )) >= 1
    );
}

#[test]
fn a_mine_goes_off_when_the_foe_stands_in_its_blast() {
    let mut d = caster(with("fireball", &["mine"]), "fireball", 400.0);
    let out = run(&mut d, 2.0);
    assert!(
        count(&out, |o| matches!(
            o,
            Out::Spawn {
                kind: Kind::Mine,
                ..
            }
        )) >= 1
    );
    assert!(
        count(&out, |o| matches!(o, Out::UnitAct { .. })) >= 1,
        "it is set off"
    );
    assert!(
        count(&out, |o| matches!(
            o,
            Out::Hit {
                spell: "fireball",
                by: 0,
                ..
            }
        )) >= 1
    );
}

#[test]
fn units_replay_the_same_from_a_seed() {
    let go = || {
        let mut d = caster(loadout(&["beans"], "tribulation"), "beans", 500.0);
        run(&mut d, 6.0)
    };
    assert_eq!(go(), go());
}

#[test]
fn a_new_round_clears_the_field() {
    let mut d = caster(loadout(&["beans"], "tribulation"), "beans", 500.0);
    run(&mut d, 1.0);
    let old: Vec<u32> = d.state().units.iter().map(|u| u.id).collect();
    assert!(!old.is_empty());
    d.state_mut().fighters[1].c.life = 0.0;
    while d.state().round < 2 {
        d.step(1.0 / 60.0);
    }
    assert!(d.state().units.iter().all(|u| !old.contains(&u.id)));
    let mut q = d.world_mut().query::<&Unit>();
    assert!(q.iter(d.world()).all(|u| !old.contains(&u.id)));
}

//! Lôi Bạo Trễ (`delayedthunder`) in the real fight: a bolt a guard, a ward, a barrier or a unit stops stays there
//! as its caster's mine (`unit::lay_stopped`, `device::STOPPED_MINE`), armed after `device::MINE_ARM`, at most
//! `device::STOPPED_CAP` standing, going off as a mine does with a share of the stopped blow.

mod common;

use arena_calc::device::{self, Kind};
use arena_calc::loadout::{Loadout, Slot as LoadoutSlot};
use arena_duel::duel::Duel;
use arena_duel::out::Out;
use common::{duel_of, idle, loadout, run};

/// `skill` with `supports` first in an otherwise plain loadout.
fn with(skill: &str, supports: &[&str]) -> Loadout {
    let mut l = loadout(&[skill]);
    l.actives[0] = LoadoutSlot {
        skill: skill.to_string(),
        supports: supports.iter().map(|s| s.to_string()).collect(),
        fuse: None,
    };
    assert!(arena_calc::loadout::check(&l).is_ok(), "{l:?}");
    l
}

/// Seat 0 casts one fireball of loadout `l` at seat 1 standing `apart` pixels off with `wall` (a ward's charges);
/// nobody dodges.
fn once(l: Loadout, apart: f64, wall: impl FnOnce(&mut arena_duel::fighter::Fighter)) -> Duel {
    let mut d = duel_of(7, [common::caster("fireball"), idle()], [l, loadout(&[])]);
    let s = d.state_mut();
    s.fighters[0].bar[0] = common::common("fireball");
    s.fighters[1].body.x = s.fighters[0].body.x + apart;
    for f in &mut s.fighters {
        f.c.evasion = 0.0;
    }
    wall(&mut s.fighters[1]);
    d
}

fn mines(out: &[Out]) -> Vec<u32> {
    out.iter()
        .filter_map(|o| match o {
            Out::Spawn {
                id,
                by: 0,
                kind: Kind::Mine,
                skill: "fireball",
                ..
            } => Some(*id),
            _ => None,
        })
        .collect()
}

fn fireball_hits(out: &[Out]) -> Vec<f64> {
    out.iter()
        .filter_map(|o| match o {
            Out::Hit {
                by: 0,
                spell: "fireball",
                damage,
                ..
            } => Some(*damage),
            _ => None,
        })
        .collect()
}

fn ward(f: &mut arena_duel::fighter::Fighter) {
    f.charges = 1.0;
    f.shield_t = 10.0;
}

#[test]
fn a_warded_bolt_stays_as_a_mine_that_goes_off_for_a_share_of_its_blow() {
    let mut d = once(with("fireball", &["delayedthunder"]), 400.0, ward);
    // the bolt in flight, and the blow it carries
    let mut out = Vec::new();
    while d.state().shots.is_empty() {
        out.extend(d.step(1.0 / 60.0));
    }
    let carried: f64 = d.state().shots[0].payload.hit.base.iter().sum();
    assert!(d.state().shots[0].mines);
    let life = d.state().fighters[1].c.life + d.state().fighters[1].c.shield;
    let mut laid_at = None;
    for i in 0..180 {
        let o = d.step(1.0 / 60.0);
        if laid_at.is_none() && !mines(&o).is_empty() {
            laid_at = Some(i);
            let u = &d.state().units[0];
            // the stopped blow, a share of it: its base kept, the share one more factor resolved by arena-calc
            let base: f64 = u.hit.base.iter().sum();
            assert!((base - carried).abs() < 1e-9, "{base} {carried}");
            assert_eq!(u.hit.more.last(), Some(&(device::STOPPED_SHARE - 1.0)));
            assert_eq!(u.owner, 0);
            assert!(
                (u.x - d.state().fighters[1].body.x).abs() < 80.0,
                "in the ward"
            );
        }
        out.extend(o);
    }
    let laid = mines(&out);
    assert_eq!(laid.len(), 1, "one bolt, one mine");
    let id = laid[0];
    // the ward took the bolt; the mine, once armed, went off on the foe standing in it
    let gone = out
        .iter()
        .position(|o| matches!(o, Out::UnitAct { id: x, .. } if *x == id))
        .unwrap_or_else(|| unreachable!("the mine goes off"));
    let spawned = out
        .iter()
        .position(|o| matches!(o, Out::Spawn { id: x, .. } if *x == id))
        .unwrap_or(0);
    assert!(gone > spawned);
    let hits = fireball_hits(&out);
    assert_eq!(hits.len(), 1, "only the mine's blast lands: {hits:?}");
    let after = d.state().fighters[1].c.life + d.state().fighters[1].c.shield;
    assert!(after < life, "the blast hurts");
    assert!(d.state().units.iter().all(|u| u.id != id));
}

#[test]
fn the_mine_arms_before_it_goes_off() {
    let mut d = once(with("fireball", &["delayedthunder"]), 400.0, ward);
    let (mut laid, mut blew) = (None, None);
    for i in 0..240 {
        for o in d.step(1.0 / 60.0) {
            match o {
                Out::Spawn {
                    kind: Kind::Mine, ..
                } => laid = laid.or(Some(i)),
                Out::UnitAct { .. } => blew = blew.or(Some(i)),
                _ => {}
            }
        }
    }
    let (laid, blew) = (
        laid.unwrap_or_else(|| unreachable!("laid")),
        blew.unwrap_or_else(|| unreachable!("set off")),
    );
    assert!(
        f64::from(blew - laid) / 60.0 >= device::MINE_ARM - 1.0 / 60.0,
        "{laid} {blew}"
    );
}

#[test]
fn an_unstopped_bolt_strikes_as_before_and_leaves_nothing() {
    let go = |supports: &[&str]| {
        let mut d = once(with("fireball", supports), 400.0, |_| {});
        run(&mut d, 3.0)
    };
    let (plain, supported) = (go(&[]), go(&["delayedthunder"]));
    assert!(mines(&supported).is_empty());
    assert_eq!(fireball_hits(&plain).len(), 1);
    assert_eq!(fireball_hits(&supported).len(), 1);
}

#[test]
fn a_plain_bolt_into_a_ward_leaves_nothing() {
    let mut d = once(with("fireball", &[]), 400.0, ward);
    let out = run(&mut d, 3.0);
    assert!(mines(&out).is_empty());
    assert!(fireball_hits(&out).is_empty());
}

#[test]
fn a_barrier_stops_the_bolt_into_a_mine_and_a_mirror_sends_it_back_whole() {
    let mut d = once(with("fireball", &["delayedthunder"]), 400.0, |f| {
        f.barrier = 5.0;
    });
    assert_eq!(mines(&run(&mut d, 1.0)).len(), 1);
    let mut d = once(with("fireball", &["delayedthunder"]), 400.0, |f| {
        f.barrier = 5.0;
        f.barrier_spell = Some("mirror");
    });
    let out = run(&mut d, 3.0);
    assert!(mines(&out).is_empty(), "a reflected bolt lays no mine");
}

#[test]
fn no_more_than_the_cap_stand_and_the_oldest_goes() {
    let mut d = once(with("fireball", &["delayedthunder"]), 400.0, |_| {});
    let p = arena_duel::strike::Payload {
        by: 0,
        skill: "fireball",
        hit: arena_calc::hit::Hit::split(100.0, &[(arena_calc::element::Element::Fire, 1.0)]),
        control: 1.0,
        knockback: 0.0,
        depth: 0,
        unit: None,
        power: 12.0,
        powers: arena_calc::projectile::Powers::NONE,
    };
    let m = arena_duel::data::spell("fireball").unwrap_or_else(|| unreachable!("fireball moves"));
    let s = d.state_mut();
    // far from the foe, so none goes off
    let x = s.fighters[0].body.x + 50.0;
    for i in 0..5 {
        arena_duel::unit::lay_stopped(s, &p, m, x + f64::from(i));
    }
    let standing: Vec<f64> = s
        .units
        .iter()
        .filter(|u| u.owner == 0 && u.kind == Kind::Mine)
        .map(|u| u.x - x)
        .collect();
    assert_eq!(standing, vec![2.0, 3.0, 4.0]);
    assert_eq!(standing.len(), device::STOPPED_CAP as usize);
    assert!(
        s.units
            .iter()
            .all(|u| u.hit.more == vec![device::STOPPED_SHARE - 1.0])
    );
    // into the rules' world, as the tick's last system does; the foe stays away: none goes off
    arena_duel::unit::scatter(d.world_mut());
    let out = run(&mut d, 1.0);
    assert!(!out.iter().any(|o| matches!(o, Out::UnitAct { .. })));
    assert_eq!(d.state().units.len(), device::STOPPED_CAP as usize);
}

#[test]
fn stopped_mines_replay_the_same_from_a_seed() {
    let go = || {
        let mut d = once(with("fireball", &["delayedthunder"]), 400.0, ward);
        run(&mut d, 4.0)
    };
    let a = go();
    assert!(!mines(&a).is_empty());
    assert_eq!(a, go());
}

#[test]
fn a_bolt_spent_on_a_foes_minion_stays_there_as_a_mine() {
    // seat 1 raises its beans at once; seat 0 casts one fireball a second later, into the minion walking at it
    let late: arena_duel::seat::MakeSeat = Box::new(|_, _| {
        let mut ticks = 0;
        Box::new(arena_duel::seat::Script(Box::new(move |_, _| {
            ticks += 1;
            (ticks == 60).then(|| arena_mind::Choice::Cast {
                skill: "fireball".to_string(),
                target: None,
            })
        })))
    });
    let mut d = duel_of(
        7,
        [late, common::caster("beans")],
        [with("fireball", &["delayedthunder"]), loadout(&["beans"])],
    );
    let s = d.state_mut();
    s.fighters[0].bar[0] = common::common("fireball");
    s.fighters[1].bar[0] = common::common("beans");
    s.fighters[1].c.souls = 30.0;
    s.fighters[1].body.x = s.fighters[0].body.x + 600.0;
    for f in &mut s.fighters {
        f.c.evasion = 0.0;
    }
    let out = run(&mut d, 3.0);
    let minion = out
        .iter()
        .find_map(|o| match o {
            Out::Spawn {
                id,
                by: 1,
                kind: Kind::Minion,
                ..
            } => Some(*id),
            _ => None,
        })
        .unwrap_or_else(|| unreachable!("the beans stand up"));
    let struck = out
        .iter()
        .position(|o| matches!(o, Out::UnitHurt { id, by: 0, .. } if *id == minion))
        .unwrap_or_else(|| unreachable!("the bolt strikes the minion"));
    let laid = out
        .iter()
        .position(|o| {
            matches!(
                o,
                Out::Spawn {
                    by: 0,
                    kind: Kind::Mine,
                    ..
                }
            )
        })
        .unwrap_or_else(|| unreachable!("and stays there as a mine"));
    assert!(laid > struck);
}

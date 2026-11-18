//! Casting, blows and shots: the cases a cast's release decides, every blow through the damage system. A strike here
//! is the cast's last windup tick (`advance`); the whole-fight cases come with `Duel::step`.

mod common;

use arena_duel::act::{advance, begin, clash, knock_out};
use arena_duel::data::{KNOCKBACK, spell};
use arena_duel::fighter::{Act, Fighter, Phase, State};
use arena_duel::js::{js_sign, or_one};
use arena_duel::out::Out;
use arena_duel::shot::shot;

/// The skills both fighters of `still` carry.
const KIT: [&str; 8] = [
    "palm", "bolt", "ward", "collapse", "fireball", "spark", "heal", "quake",
];

/// A duel of two fighters carrying `KIT`, where nobody dodges or crits and the shields are down, with every effect in
/// the registry.
fn still(seed: u32) -> State {
    let mut s = State::new(
        seed,
        0.0,
        880.0,
        600.0,
        384.0,
        common::all_effects(),
        [
            Fighter::new(0, common::loadout(&KIT)),
            Fighter::new(1, common::loadout(&KIT)),
        ],
    );
    for (i, at) in [(0, 0.22), (1, 0.78)] {
        s.fighters[i].body.start_round(880.0 * at);
    }
    for f in &mut s.fighters {
        f.c.evasion = 0.0;
        f.c.crit = 0.0;
        f.c.shield = 0.0;
    }
    s
}

/// Fighter `by` lands `name` on the other from 80 px: its cast reaches the end of its windup this tick.
fn strike(s: &mut State, by: usize, name: &'static str) -> Vec<Out> {
    let f = 1 - by;
    let windup = spell(name).map_or(0.0, |sp| sp.windup);
    let fx = s.fighters[f].body.x;
    let bx = s.fighters[by].body.x;
    s.fighters[by].act = Some(Act {
        spell: name,
        phase: Phase::Windup,
        t: windup,
        windup,
        recover: 0.3,
        target: Some(fx),
        circle: None,
        blocked: false,
    });
    s.fighters[by].body.x = fx - 80.0 * js_sign(or_one(fx - bx));
    s.fighters[by].body.face = js_sign(fx - s.fighters[by].body.x);
    s.out.clear();
    advance(s, by, 1.0 / 60.0);
    s.out.clone()
}

fn hits(out: &[Out], name: &str) -> bool {
    out.iter()
        .any(|o| matches!(o, Out::Hit { spell, .. } if *spell == name))
}

#[test]
fn a_failed_block_lets_the_whole_blow_through_and_the_guards_qi_is_paid_anyway() {
    let (mut through, mut held) = (0, 0);
    for seed in 1..=40 {
        let mut s = still(seed);
        s.fighters[1].body.face = js_sign(s.fighters[0].body.x - s.fighters[1].body.x);
        s.fighters[1].guard = 5.0;
        let qi = s.fighters[1].c.qi;
        let out = strike(&mut s, 0, "palm");
        assert!(s.fighters[1].c.qi < qi, "seed {seed}");
        if hits(&out, "palm") {
            through += 1
        } else {
            held += 1
        }
    }
    assert!(through > 4 && held > 4, "{through} {held}");
}

#[test]
fn a_guarded_blows_chip_goes_through_the_damage_system_and_armour_cuts_it() {
    let chip = |armour: f64| {
        let mut s = still(6);
        s.fighters[1].c.armour = armour;
        s.fighters[1].body.face = js_sign(s.fighters[0].body.x - s.fighters[1].body.x);
        s.fighters[1].guard = 5.0;
        let life = s.fighters[1].c.life;
        // the burst's guard chance is high: find a seed's roll that holds
        strike(&mut s, 0, "qiburst");
        life - s.fighters[1].c.life
    };
    let soft = chip(0.0);
    let hard = chip(20_000.0);
    assert!(soft > 0.0);
    assert!(hard < soft * 0.7, "{hard} {soft}");
}

#[test]
fn each_charge_erases_one_whole_blow() {
    let mut s = still(2);
    s.fighters[1].charges = 2.0;
    s.fighters[1].shield_t = 6.0;
    let life = s.fighters[1].c.life;
    strike(&mut s, 0, "qiburst");
    strike(&mut s, 0, "qiburst");
    assert_eq!(s.fighters[1].c.life, life);
    assert_eq!(s.fighters[1].charges, 0.0);
    strike(&mut s, 0, "qiburst");
    assert!(s.fighters[1].c.life < life);
}

#[test]
fn a_ward_holds_1_to_3_charges_and_washes_off_the_ailments() {
    let mut seen = Vec::new();
    for seed in 1..=30 {
        let mut s = still(seed);
        s.fighters[1]
            .c
            .ailments
            .add(arena_calc::ailment::Ailment::Jolt, 2.0, 1.0);
        let x = s.fighters[1].body.x;
        let windup = spell("ward").map_or(0.0, |sp| sp.windup);
        s.fighters[1].act = Some(Act {
            spell: "ward",
            phase: Phase::Windup,
            t: windup,
            windup,
            recover: 0.3,
            target: Some(x),
            circle: None,
            blocked: false,
        });
        advance(&mut s, 1, 1.0 / 60.0);
        assert_eq!(
            s.fighters[1]
                .c
                .ailments
                .stacks(arena_calc::ailment::Ailment::Jolt),
            0.0
        );
        let c = s.fighters[1].charges;
        if !seen.contains(&c) {
            seen.push(c);
        }
    }
    seen.sort_by(f64::total_cmp);
    assert_eq!(seen, [1.0, 2.0, 3.0]);
}

#[test]
fn clash_at_equal_x_pushes_as_js() {
    let mut s = still(1);
    let x = s.fighters[0].body.x;
    s.fighters[1].body.x = x;
    let (v0, v1) = (s.fighters[0].body.vx, s.fighters[1].body.vx);
    s.out.clear();
    clash(&mut s, x, 400.0, &[0, 1]);
    // JS: Math.sign(f.x - x || 1) is 1 when they stand on the spot
    assert_eq!(s.fighters[0].body.vx, v0 + KNOCKBACK);
    assert_eq!(s.fighters[1].body.vx, v1 + KNOCKBACK);
    assert!(matches!(s.out.first(), Some(Out::Clash { y, .. }) if *y == s.ground - 400.0));
}

#[test]
fn a_cast_pays_its_built_costs_and_spends_a_use() {
    let mut s = still(3);
    let qi = s.fighters[1].c.qi;
    let target = s.fighters[0].body.x;
    s.fighters[1].bar = vec![common::common("collapse")];
    begin(&mut s, 1, "collapse", Some(target));
    assert!(s.fighters[1].act.is_some());
    assert!(s.fighters[1].c.qi < qi);
    let k = s.fighters[1].kit.get("collapse");
    assert!(k.is_some_and(|k| k.count == 1 && !k.uses.ready()));
    // not ready again until its use comes back: a second cast does nothing
    s.fighters[1].act = None;
    let qi = s.fighters[1].c.qi;
    begin(&mut s, 1, "collapse", Some(target));
    assert!(s.fighters[1].act.is_none() && s.fighters[1].c.qi == qi);
}

#[test]
fn a_cast_still_gathering_in_its_rune_circle_when_the_foe_falls_loses_its_circle() {
    let mut s = still(3);
    let target = s.fighters[0].body.x;
    s.fighters[1].bar = vec![common::common("collapse")];
    begin(&mut s, 1, "collapse", Some(target));
    let circle = s.fighters[1].act.and_then(|a| a.circle);
    assert!(circle.is_some());
    let circle = circle.unwrap_or(0);
    s.out.clear();
    knock_out(&mut s, 0);
    assert!(s.out.contains(&Out::Stop { key: circle }));
    assert!(s.fighters[1].act.is_none());
    assert_eq!(s.winner, Some(1));
    assert_eq!(s.wins, [0, 1]);
}

/// Fighter `me`'s bolt-like skill `name` comes out now.
fn fire(s: &mut State, me: usize, name: &'static str) {
    let windup = spell(name).map_or(0.0, |sp| sp.windup);
    s.fighters[me].act = Some(Act {
        spell: name,
        phase: Phase::Windup,
        t: windup,
        windup,
        recover: 0.3,
        target: None,
        circle: None,
        blocked: false,
    });
    advance(s, me, 1.0 / 60.0);
}

fn fly(s: &mut State, ticks: usize) {
    for _ in 0..ticks {
        let ids: Vec<u32> = s.shots.iter().map(|x| x.id).collect();
        for id in ids {
            shot(s, id, 1.0 / 60.0);
        }
        if s.shots.is_empty() {
            break;
        }
    }
}

#[test]
fn a_bolt_flies_strikes_lays_its_ailment_and_ends_its_effect() {
    let mut s = still(4);
    s.fighters[1].body.x = s.fighters[0].body.x + 240.0;
    s.fighters[0].body.face = 1.0;
    fire(&mut s, 0, "bolt");
    assert_eq!(s.shots.len(), 1);
    let key = s.out.iter().find_map(|o| match o {
        Out::Play {
            key,
            fx: "projectiles/laser-bolt",
            ..
        } => Some(*key),
        _ => None,
    });
    assert!(key.is_some(), "no bolt played");
    let key = key.unwrap_or(0);
    s.out.clear();
    fly(&mut s, 60);
    assert!(hits(&s.out, "bolt"));
    assert!(s.out.contains(&Out::Stop { key }));
    let jolt = s.fighters[1]
        .c
        .ailments
        .stacks(arena_calc::ailment::Ailment::Jolt);
    assert!(jolt > 0.0, "a thunder bolt jolts");
}

#[test]
fn two_bolts_meeting_head_on_are_both_spent() {
    let mut s = still(5);
    s.fighters[0].body.x = 60.0;
    s.fighters[1].body.x = 460.0;
    s.fighters[0].body.face = 1.0;
    s.fighters[1].body.face = -1.0;
    fire(&mut s, 0, "bolt");
    fire(&mut s, 1, "spark");
    assert_eq!(s.shots.len(), 2);
    s.out.clear();
    fly(&mut s, 60);
    assert!(s.shots.is_empty());
    assert!(s.out.iter().any(|o| matches!(o, Out::Clash { .. })));
    assert!(!s.out.iter().any(|o| matches!(o, Out::Hit { .. })));
}

#[test]
fn a_bolt_with_pierce_flies_through_a_minion_to_the_foe() {
    use arena_calc::loadout::Slot as LoadoutSlot;
    let mut l = common::loadout(&["bolt"]);
    l.actives[0] = LoadoutSlot {
        skill: "bolt".to_string(),
        supports: vec!["piercing".to_string()],
        fuse: None,
    };
    assert!(arena_calc::loadout::check(&l).is_ok());
    let mut s = still(7);
    s.fighters[0] = Fighter::new(0, l);
    s.fighters[0].body.start_round(200.0);
    s.fighters[0].body.face = 1.0;
    s.fighters[1].body.x = 600.0;
    // a minion of the foe's stands between them
    s.unit_id = 1;
    s.units.push(arena_duel::unit::Unit {
        id: 1,
        owner: 1,
        skill: "beans",
        kind: arena_calc::device::Kind::Minion,
        device: 0,
        x: 400.0,
        c: arena_calc::cultivator::Cultivator {
            evasion: 0.0,
            ..arena_calc::cultivator::Cultivator::new()
        },
        hit: arena_calc::hit::Hit::default(),
        control: 1.0,
        powers: arena_calc::projectile::Powers::NONE,
        motion: arena_duel::data::motion("beans").unwrap_or_else(|| unreachable!()),
        next_strike: 99.0,
    });
    fire(&mut s, 0, "bolt");
    s.out.clear();
    fly(&mut s, 120);
    assert!(
        s.out
            .iter()
            .any(|o| matches!(o, Out::UnitHurt { id: 1, .. })),
        "the minion in the way is struck"
    );
    assert!(hits(&s.out, "bolt"), "and the bolt flies on to the foe");
}

#[test]
fn a_lying_fighter_is_struck_only_where_it_lies() {
    let lying = [-108.0, 0.0, 124.0, 23.0];
    let c = arena_duel::unit::circles(lying);
    assert!(!c.is_empty());
    for &(x, y, r) in &c {
        assert!(
            y + r <= lying[3] + 1e-9 && y - r >= lying[1] - 1e-9,
            "{x} {y} {r}"
        );
    }
    let (first, last) = (c[0], c[c.len() - 1]);
    assert!((first.0 - first.2 - lying[0]).abs() < 1e-9);
    assert!((last.0 + last.2 - lying[2]).abs() < 1e-9);
    // standing: a column as wide as the box, from the floor to its top
    let standing = [-20.0, 0.0, 20.0, 150.0];
    let c = arena_duel::unit::circles(standing);
    assert!(c.iter().all(|&(x, _, r)| x == 0.0 && r == 20.0));
    assert!((c[0].1 - 20.0).abs() < 1e-9 && (c[c.len() - 1].1 - 130.0).abs() < 1e-9);
}

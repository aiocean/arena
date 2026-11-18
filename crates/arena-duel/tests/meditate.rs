//! Meditation (`data::MEDITATE`): sitting down, sitting, getting up; qi in blocks while it sits; a blow on a
//! meditating fighter breaks it and is worse than a counter.

mod common;

use arena_calc::cultivator::{BASE, Sitting};
use arena_calc::stat::Stat;
use arena_duel::act::{advance, pose};
use arena_duel::data::{MEDITATE, POWER, body_spec, spell};
use arena_duel::duel::apply;
use arena_duel::fighter::{Act, Fighter, Meditation, Phase, Sit, State};
use arena_duel::js::{js_sign, or_one};
use arena_duel::out::{How, Out};
use arena_duel::pose::Pose;
use arena_duel::sense::sense_of;
use arena_duel::strike::wear;
use arena_mind::Choice;

const DT: f64 = 1.0 / 60.0;

/// A duel where nobody dodges or crits and the shields are down, with every effect in the registry.
fn still(seed: u32) -> State {
    let kit = ["lotus", "palm", "qiburst", "bell"];
    let mut s = State::new(
        seed,
        0.0,
        880.0,
        600.0,
        384.0,
        common::all_effects(),
        [
            Fighter::new(0, common::loadout(&kit)),
            Fighter::new(1, common::loadout(&kit)),
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

/// `secs` seconds of time wearing the fighters, in ticks.
fn wear_for(s: &mut State, secs: f64) {
    for _ in 0..(secs / DT + 0.5) as u32 {
        wear(s, DT);
    }
}

fn phase(s: &State, f: usize) -> Option<Sit> {
    s.fighters[f].medit.map(|m| m.phase)
}

fn can_act(s: &State, f: usize) -> bool {
    sense_of(s, f).me.can_act
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.abs().max(1.0)
}

#[test]
fn it_sits_down_sits_and_gets_up() {
    let mut s = still(1);
    assert!(can_act(&s, 1));
    apply(&mut s, 1, &Choice::Meditate);
    assert_eq!(phase(&s, 1), Some(Sit::Down));
    assert!(!can_act(&s, 1));
    pose(&mut s, 1);
    assert_eq!(s.fighters[1].body.pose, Pose::Meditate);
    // sitting down, every choice is dropped
    let x = s.fighters[1].body.x;
    apply(&mut s, 1, &Choice::Move { dir: -1.0 });
    assert_eq!(s.fighters[1].intent, 0.0);
    apply(&mut s, 1, &Choice::Guard { hold: 1.0 });
    assert_eq!(s.fighters[1].guard, 0.0);
    wear_for(&mut s, MEDITATE.sit_down - DT);
    assert_eq!(phase(&s, 1), Some(Sit::Down));
    wear_for(&mut s, DT);
    assert_eq!(phase(&s, 1), Some(Sit::Sitting));
    assert!(can_act(&s, 1));
    assert!(s.fighters[1].c.sitting.is_some());
    // meditating or waiting, it stays
    apply(&mut s, 1, &Choice::Wait);
    apply(&mut s, 1, &Choice::Meditate);
    assert_eq!(phase(&s, 1), Some(Sit::Sitting));
    // anything else gets it up, and the choice is dropped
    apply(&mut s, 1, &Choice::Move { dir: -1.0 });
    assert_eq!(phase(&s, 1), Some(Sit::Up));
    assert_eq!(s.fighters[1].intent, 0.0);
    assert!(s.fighters[1].c.sitting.is_none());
    assert!(!can_act(&s, 1));
    pose(&mut s, 1);
    assert_eq!(s.fighters[1].body.pose, Pose::Kneel);
    apply(
        &mut s,
        1,
        &Choice::Cast {
            skill: "palm".into(),
            target: None,
        },
    );
    assert!(s.fighters[1].act.is_none());
    wear_for(&mut s, MEDITATE.get_up);
    assert_eq!(phase(&s, 1), None);
    assert!(can_act(&s, 1));
    assert_eq!(s.fighters[1].body.x, x);
}

#[test]
fn it_sits_down_only_when_free_on_the_ground() {
    let mut s = still(1);
    s.fighters[1].body.air = 30.0;
    apply(&mut s, 1, &Choice::Meditate);
    assert_eq!(phase(&s, 1), None);
    let mut s = still(1);
    s.fighters[1].fly = true;
    apply(&mut s, 1, &Choice::Meditate);
    assert_eq!(phase(&s, 1), None);
    let mut s = still(1);
    s.fighters[1].act = Some(Act {
        spell: "palm",
        phase: Phase::Recover,
        t: 0.0,
        windup: 0.3,
        recover: 0.3,
        target: None,
        circle: None,
        blocked: false,
    });
    apply(&mut s, 1, &Choice::Meditate);
    assert_eq!(phase(&s, 1), None);
}

/// Qi gained over `sat` seconds of sitting after sitting down, from empty with room for it, and the normal and the
/// meditating rate through its table.
fn qi_after(sat: f64) -> (f64, f64, f64) {
    let mut s = still(2);
    let f = &mut s.fighters[1];
    f.c.qi_max = 10_000.0;
    f.c.qi = 0.0;
    let t = f.c.bearer_table(&f.sheet, false);
    let (rate, med) = (
        t.get(Stat::QiRegen, BASE.qi_regen).max(0.0),
        t.get(Stat::QiRegen, BASE.meditate_regen).max(0.0),
    );
    apply(&mut s, 1, &Choice::Meditate);
    wear_for(&mut s, MEDITATE.sit_down);
    assert_eq!(phase(&s, 1), Some(Sit::Sitting));
    wear_for(&mut s, sat);
    (s.fighters[1].c.qi, rate, med)
}

#[test]
fn sitting_pays_qi_in_blocks_after_the_first_second() {
    let block = MEDITATE.blocks.block;
    let normal = |sat: f64, rate: f64| rate * (MEDITATE.sit_down + sat);
    // nothing extra before a whole second
    let (qi, rate, _) = qi_after(0.9);
    assert!(close(qi, normal(0.9, rate)), "{qi}");
    // at a second, two blocks at once
    let (qi, rate, med) = qi_after(1.0);
    assert!(med > rate);
    assert!(
        close(qi, normal(1.0, rate) + 2.0 * (med - rate) * block),
        "{qi}"
    );
    // a block still filling pays nothing
    let (qi, rate, med) = qi_after(1.4);
    assert!(
        close(qi, normal(1.4, rate) + 2.0 * (med - rate) * block),
        "{qi}"
    );
    let (qi, rate, med) = qi_after(1.5);
    assert!(
        close(qi, normal(1.5, rate) + 3.0 * (med - rate) * block),
        "{qi}"
    );
}

/// Fighter 0 lands a burst on fighter 1 from 80 px: its cast reaches the end of its windup this tick.
fn strike(s: &mut State) -> Vec<Out> {
    let name = "qiburst";
    let windup = spell(name).map_or(0.0, |sp| sp.windup);
    let fx = s.fighters[1].body.x;
    let bx = s.fighters[0].body.x;
    s.fighters[0].act = Some(Act {
        spell: name,
        phase: Phase::Windup,
        t: windup,
        windup,
        recover: spell(name).map_or(0.5, |sp| sp.recover),
        target: Some(fx),
        circle: None,
        blocked: false,
    });
    s.fighters[0].body.x = fx - 80.0 * js_sign(or_one(fx - bx));
    s.fighters[0].body.face = js_sign(fx - s.fighters[0].body.x);
    s.fighters[1].body.face = -s.fighters[0].body.face;
    s.out.clear();
    advance(s, 0, DT);
    s.out.clone()
}

fn burst(out: &[Out]) -> Option<(f64, How)> {
    out.iter().find_map(|o| match o {
        Out::Hit {
            spell: "qiburst",
            damage,
            how,
            ..
        } => Some((*damage, *how)),
        _ => None,
    })
}

#[test]
fn a_blow_breaks_a_meditation_and_is_worse_than_a_counter() {
    const { assert!(MEDITATE.struck.power > arena_duel::data::FRAMES.counter.power) };
    let (plain, how) = burst(&strike(&mut still(3))).unwrap_or((0.0, How::Plain));
    assert!(plain > 0.0);
    assert_eq!(how, How::Plain);
    for phase in [Sit::Down, Sit::Sitting, Sit::Up] {
        let mut s = still(3);
        s.fighters[1].medit = Some(Meditation { phase, t: 0.1 });
        if phase == Sit::Sitting {
            s.fighters[1].c.sitting = Some(Sitting::new(MEDITATE.blocks));
        }
        let out = strike(&mut s);
        let (damage, how) = burst(&out).unwrap_or((0.0, How::Plain));
        assert_eq!(how, How::Meditate, "{phase:?}");
        assert!(
            close(damage, plain * MEDITATE.struck.power),
            "{damage} {plain}"
        );
        // broken outright: no getting up
        assert_eq!(s.fighters[1].medit, None);
        assert!(s.fighters[1].c.sitting.is_none());
        let b = &s.fighters[1].body;
        let spec = body_spec();
        let stagger = spec.hitstun_base + damage / POWER * spec.hitstun_per;
        assert!(
            b.air > 0.0 || b.vy > 0.0 || close(b.stun, stagger + MEDITATE.struck.stagger),
            "{}",
            b.stun
        );
    }
}

//! Frame data (`data::FRAMES`): a blow that catches a fighter gathering a cast is a COUNTER, one that catches it
//! recovering is a PUNISH (the damage system's step 10, `hit::Frame`), an unsafe skill whose blow is guarded recovers
//! longer, and a fighter casting is not guarding. None of it draws a die of its own.

mod common;

use arena_calc::hit::FRAME_MORE;
use arena_mind::Outcome;
use arena_duel::act::{advance, begin};
use arena_duel::body::BodySpec;
use arena_duel::data::{FRAMES, POWER, body_spec, spell};
use arena_duel::fighter::{Act, Fighter, Phase, State, Told};
use arena_duel::js::{js_sign, or_one};
use arena_duel::out::{How, Out};

const DT: f64 = 1.0 / 60.0;

/// A duel where nobody dodges or crits and the shields are down, both carrying the skills these cases cast, with
/// every effect in the registry.
fn still(seed: u32) -> State {
    let kit = [
        "lotus", "palm", "step", "void", "bell", "heal", "jet", "burst",
    ];
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

fn act(name: &'static str, phase: Phase, t: f64) -> Act {
    let sp = spell(name);
    Act {
        spell: name,
        phase,
        t,
        windup: sp.map_or(0.5, |s| s.windup),
        recover: sp.map_or(0.5, |s| s.recover),
        target: None,
        circle: None,
        blocked: false,
    }
}

/// Fighter `by` lands `name` on the other from 20 px: its cast reaches the end of its windup this tick. The one
/// struck faces it.
fn strike(s: &mut State, by: usize, name: &'static str) -> Vec<Out> {
    let f = 1 - by;
    let windup = spell(name).map_or(0.0, |sp| sp.windup);
    let fx = s.fighters[f].body.x;
    let bx = s.fighters[by].body.x;
    s.fighters[by].act = Some(Act {
        target: Some(fx),
        ..act(name, Phase::Windup, windup)
    });
    s.fighters[by].body.x = fx - 80.0 * js_sign(or_one(fx - bx));
    s.fighters[by].body.face = js_sign(fx - s.fighters[by].body.x);
    s.fighters[f].body.face = -s.fighters[by].body.face;
    s.out.clear();
    advance(s, by, DT);
    s.out.clone()
}

/// The `Hit` of `name` among `out`: the damage it dealt and how it caught its target.
fn hit(out: &[Out], name: &str) -> Option<(f64, How)> {
    out.iter().find_map(|o| match o {
        Out::Hit {
            spell, damage, how, ..
        } if *spell == name => Some((*damage, *how)),
        _ => None,
    })
}

/// A slash on seat 1 standing idle: what it dealt, how, and the stagger it leaves.
fn plain_slash(seed: u32) -> (f64, How, f64) {
    let mut s = still(seed);
    let out = strike(&mut s, 0, "slash");
    let (damage, how) = hit(&out, "slash").unwrap_or((0.0, How::Plain));
    (damage, how, s.fighters[1].body.stun)
}

/// The stagger a blow that dealt `damage` leaves (its power is what it dealt over `POWER`).
fn stagger(damage: f64) -> f64 {
    let spec = body_spec();
    let _: BodySpec = spec;
    spec.hitstun_base + damage / POWER * spec.hitstun_per
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * b.abs().max(1.0)
}

#[test]
fn a_fighter_that_guards_then_casts_is_not_guarding_while_it_gathers() {
    let mut s = still(1);
    s.fighters[1].guard = 5.0;
    s.fighters[1].bar = vec![common::common("lotus")];
    begin(&mut s, 1, "lotus", None);
    assert_eq!(s.fighters[1].guard, 0.0);
    // a slash is always turned aside by a guard: here it lands
    let out = strike(&mut s, 0, "slash");
    assert!(hit(&out, "slash").is_some(), "{out:?}");
    assert!(!out.iter().any(|o| matches!(o, Out::Clash { .. })));
}

#[test]
fn a_blow_on_a_fighter_standing_idle_is_plain() {
    let (damage, how, stun) = plain_slash(3);
    assert_eq!(how, How::Plain);
    assert!(damage > 0.0);
    assert!(close(stun, stagger(damage)), "{stun} {}", stagger(damage));
}

#[test]
fn a_blow_on_a_fighter_gathering_a_cast_is_a_counter_harder_and_staggering_longer() {
    let mut s = still(3);
    s.fighters[1].act = Some(act("lotus", Phase::Windup, 0.1));
    let out = strike(&mut s, 0, "slash");
    let (damage, how) = hit(&out, "slash").unwrap_or((0.0, How::Plain));
    assert_eq!(how, How::Counter);
    let (plain, _, _) = plain_slash(3);
    assert!(close(damage, plain * FRAME_MORE), "{damage} {plain}");
    assert!(close(
        s.fighters[1].body.stun,
        stagger(damage) + FRAMES.counter.stagger
    ));
    // and the cast it was gathering is lost
    assert!(s.fighters[1].act.is_none());
}

#[test]
fn a_blow_on_a_fighter_recovering_is_a_punish_harder_and_staggering_longer() {
    let mut s = still(3);
    s.fighters[1].act = Some(act("lotus", Phase::Recover, 0.1));
    let out = strike(&mut s, 0, "slash");
    let (damage, how) = hit(&out, "slash").unwrap_or((0.0, How::Plain));
    assert_eq!(how, How::Punish);
    let (plain, _, _) = plain_slash(3);
    assert!(close(damage, plain * FRAME_MORE), "{damage} {plain}");
    assert!(close(
        s.fighters[1].body.stun,
        stagger(damage) + FRAMES.punish.stagger
    ));
    assert!(s.told.iter().any(|(f, t)| *f == 1
        && *t
            == Told::Outcome(Outcome::Punished {
                skill: "lotus".to_string()
            })));
}

#[test]
fn a_counter_draws_exactly_the_dice_a_plain_hit_draws() {
    for seed in 1..=20 {
        let mut plain = still(seed);
        let mut counter = still(seed);
        let mut punish = still(seed);
        counter.fighters[1].act = Some(act("lotus", Phase::Windup, 0.1));
        punish.fighters[1].act = Some(act("lotus", Phase::Recover, 0.1));
        for s in [&mut plain, &mut counter, &mut punish] {
            s.fighters[1].c.crit = 0.2;
            s.fighters[0].c.crit = 0.2;
            let out = strike(s, 0, "slash");
            assert!(hit(&out, "slash").is_some(), "seed {seed}");
        }
        assert_eq!(plain.rng, counter.rng, "seed {seed}");
        assert_eq!(plain.rng, punish.rng, "seed {seed}");
    }
}

#[test]
fn a_fields_strike_is_never_a_counter_or_a_punish() {
    use arena_duel::strike::{Payload, hurt};
    for phase in [Phase::Windup, Phase::Recover] {
        let mut s = still(5);
        s.fighters[0].act = Some(act("dive", phase, 0.1));
        s.out.clear();
        let p = Payload {
            by: 1,
            skill: "void",
            hit: arena_calc::hit::Hit::split(100.0, &[(arena_calc::element::Element::Yin, 1.0)]),
            control: 1.0,
            knockback: 0.0,
            depth: 0,
            unit: None,
            power: 1.0,
            powers: arena_calc::projectile::Powers::NONE,
        };
        assert!(hurt(&mut s, 0, &p, 1.0));
        assert_eq!(hit(&s.out, "void").map(|h| h.1), Some(How::Plain));
    }
}

#[test]
fn a_golden_bells_answer_counters_the_striker_still_in_its_windup() {
    let mut s = still(2);
    s.fighters[1].barrier = 2.0;
    s.fighters[1].barrier_spell = Some("bell");
    let out = strike(&mut s, 0, "slash");
    let (damage, how) = hit(&out, "bell").unwrap_or((0.0, How::Plain));
    assert_eq!(how, How::Counter);
    assert!(damage > 0.0);
}

#[test]
fn a_top_tier_skill_gathering_is_countered_like_any_other() {
    let mut s = still(4);
    s.fighters[1].act = Some(act("tribulation", Phase::Windup, 0.05));
    let out = strike(&mut s, 0, "slash");
    assert_eq!(hit(&out, "slash").map(|h| h.1), Some(How::Counter));
    assert!(s.fighters[1].act.is_none());
}

#[test]
fn close_heavies_dive_dash_heal_and_the_top_tier_recover_slowly_and_pokes_stay_safe() {
    let recover = |name: &str| spell(name).map_or(0.0, |sp| sp.recover);
    for (name, want) in [
        ("slash", 0.32),
        ("dive", 0.5),
        ("step", 0.45),
        ("palm", 0.5),
        ("burst", 0.5),
        ("lotus", 0.45),
        ("jet", 0.35),
        ("heal", 0.45),
        ("tribulation", 0.8),
        ("bolt", 0.22),
    ] {
        assert_eq!(recover(name), want, "{name}");
    }
}

/// Fighter 0 casts `name` on a guarding Frost until a seed turns it aside; its act, just blocked.
fn blocked(name: &'static str) -> Option<State> {
    (1..=40).find_map(|seed| {
        let mut s = still(seed);
        s.fighters[1].guard = 5.0;
        let out = strike(&mut s, 0, name);
        (hit(&out, name).is_none() && out.iter().any(|o| matches!(o, Out::Clash { .. })))
            .then_some(s)
    })
}

/// How long fighter 0 stays in its recovery from here, in ticks.
fn recovery_ticks(s: &mut State) -> usize {
    let mut n = 0;
    while s.fighters[0].act.is_some() && n < 600 {
        advance(s, 0, DT);
        n += 1;
    }
    n
}

#[test]
fn an_unsafe_blow_turned_aside_leaves_its_striker_open_longer() {
    for name in ["palm", "step", "dive"] {
        let s = blocked(name);
        assert!(s.is_some(), "no seed blocks {name}");
        let Some(mut s) = s else { return };
        let a = s.fighters[0].act;
        assert_eq!(
            a.map(|a| (a.phase, a.blocked)),
            Some((Phase::Recover, true))
        );
        let want = spell(name).map_or(0.0, |sp| sp.recover) + FRAMES.blocked_recover;
        let ticks = recovery_ticks(&mut s) as f64;
        assert!(
            (ticks * DT - want).abs() <= DT,
            "{name}: {ticks} ticks, want {want} s"
        );
    }
}

#[test]
fn a_safe_blow_turned_aside_recovers_as_ever() {
    let s = blocked("slash");
    assert!(s.is_some(), "no seed blocks slash");
    let Some(mut s) = s else { return };
    let want = spell("slash").map_or(0.0, |sp| sp.recover);
    let ticks = recovery_ticks(&mut s) as f64;
    assert!(
        (ticks * DT - want).abs() <= DT,
        "{ticks} ticks, want {want} s"
    );
}

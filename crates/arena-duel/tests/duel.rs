//! The duel's rules and minds, driven step after step, ported from `site/tests/duel.test.js`, with the plan's
//! whole-tick and missing-effect cases.

mod common;

use arena_duel::data::spell;
use arena_duel::duel::Duel;
use arena_duel::fighter::Slot;
use arena_duel::out::Out;
use common::{all_effects, config, fight, run};

const PLAN2: [&str; 16] = [
    "fire/candle",
    "fire/campfire",
    "fire/brazier",
    "impacts/parry",
    "explosions/plasma-explosion",
    "explosions/implosion",
    "explosions/small-explosion",
    "arcane/rune-circle",
    "lightning/ball-lightning",
    "projectiles/laser-bolt",
    "ice/icicle-fall",
    "shadow/void-orb",
    "holy/feather-burst",
    "spawn/poof",
    "fire/spirit-flame",
    "weather/fireflies",
];

fn plan2(seed: u32) -> Duel {
    Duel::with_brains(config(seed, all_effects()))
}

/// Each round's wins as it ended, over `seconds` at 60 Hz.
fn rounds(duel: &mut Duel, seconds: f64) -> Vec<[u32; 2]> {
    let mut out = Vec::new();
    let mut round = duel.state().round;
    for _ in 0..(seconds * 60.0) as usize {
        duel.step(1.0 / 60.0);
        if duel.state().round != round {
            out.push(duel.state().wins);
            round = duel.state().round;
        }
    }
    out
}

#[test]
fn a_seed_replays_the_same_fight() {
    let a = run(&mut plan2(7), 20.0);
    let b = run(&mut plan2(7), 20.0);
    assert!(a.len() > 20);
    assert_eq!(a, b);
}

#[test]
fn fights_end_in_a_knockout_and_the_next_round_starts() {
    // a property over seeds, not lucky seeds: each round that ends is counted to its winner and the next starts, and
    // most fights reach a knockout in four minutes (some stall: no free finisher since the ultimate went)
    let mut knockouts = 0;
    for seed in 1..=12 {
        let r = rounds(&mut fight(seed), 240.0);
        if let Some(last) = r.last() {
            knockouts += 1;
            assert_eq!((last[0] + last[1]) as usize, r.len(), "seed {seed}");
        }
    }
    assert!(
        knockouts >= 6,
        "only {knockouts} of 12 seeds reached a knockout"
    );
}

#[test]
fn a_heavy_blow_throws_its_target_which_lands_lies_and_gets_up() {
    let mut duel = common::duel_of(
        2,
        [common::idle(), common::idle()],
        [common::loadout(&["nineturn"]), common::loadout(&[])],
    );
    for f in &mut duel.state_mut().fighters {
        f.c.evasion = 0.0;
        f.c.shield = 0.0;
    }
    duel.state_mut().fighters[0].body.x = 400.0;
    duel.state_mut().fighters[1].body.x = 464.0;
    duel.state_mut().fighters[0].body.face = 1.0;
    duel.state_mut().fighters[1].body.face = -1.0;
    let windup = spell("nineturn").map_or(0.0, |s| s.windup);
    duel.state_mut().fighters[0].act = Some(arena_duel::fighter::Act {
        spell: "nineturn",
        phase: arena_duel::fighter::Phase::Windup,
        t: windup - 1.0 / 60.0,
        windup,
        recover: 0.5,
        target: None,
        circle: None,
        blocked: false,
    });
    duel.step(1.0 / 60.0);
    assert!(duel.state().fighters[1].body.down.is_some());
    let mut lands = 0;
    let mut up = false;
    for _ in 0..240 {
        lands += duel
            .step(1.0 / 60.0)
            .iter()
            .filter(|o| matches!(o, Out::Land { .. }))
            .count();
        if duel.state().fighters[1].body.down.is_none() {
            up = true;
            break;
        }
    }
    assert!(up);
    assert!(lands >= 1);
}

#[test]
fn fights_end_in_knockouts_and_each_seat_wins_at_least_30_percent_of_rounds() {
    let mut wins = [0_u32; 2];
    for seed in 1..=40 {
        let r = rounds(&mut fight(seed), 120.0);
        if let Some(last) = r.last() {
            wins[0] += last[0];
            wins[1] += last[1];
        }
    }
    let total = f64::from(wins[0] + wins[1]);
    assert!(total >= 40.0, "{wins:?}");
    for w in wins {
        assert!(f64::from(w) / total >= 0.3, "{wins:?}");
    }
}

#[test]
fn a_cast_spends_a_stored_use_and_a_bar_skills_cast_empties_its_slot() {
    let mut checked = 0;
    for seed in 1..=4 {
        let mut duel = plan2(seed);
        let mut was: [Option<&str>; 2] = [None, None];
        for _ in 0..90 * 60 {
            let before: Vec<Vec<Slot>> = duel
                .state()
                .fighters
                .iter()
                .map(|f| f.bar.clone())
                .collect();
            duel.step(1.0 / 60.0);
            for f in &duel.state().fighters {
                let name = f.act.map(|a| a.spell);
                let fresh = name.is_some()
                    && name != was[f.index]
                    && f.act.is_some_and(|a| a.t < 1.0 / 30.0);
                was[f.index] = name;
                let Some(name) = name.filter(|_| fresh) else {
                    continue;
                };
                let k = f
                    .kit
                    .get(name)
                    .unwrap_or_else(|| unreachable!("{name} is in its kit"));
                assert!(k.count >= 1, "{name}");
                assert!(
                    k.uses.cooldown <= 0.0 || k.uses.stored < k.uses.max,
                    "{name}: a use spent"
                );
                let Some(j) = before[f.index].iter().position(|s| s.spell() == Some(name)) else {
                    continue;
                };
                assert!(matches!(f.bar[j], Slot::Empty { .. }), "{name}");
                checked += 1;
            }
        }
    }
    assert!(checked > 10, "{checked}");
}

/// What decides a fight after `seconds` of seed 5 stepped at `hz`.
fn fought(hz: f64, seconds: f64) -> ([u32; 2], u32, Vec<(f64, f64)>) {
    let mut duel = plan2(5);
    for _ in 0..(seconds * hz) as usize {
        duel.step(1.0 / hz);
    }
    (
        duel.state().wins,
        duel.state().round,
        duel.state()
            .fighters
            .iter()
            .map(|f| (f.hp(), f.body.x))
            .collect(),
    )
}

#[test]
fn a_seed_fights_the_same_at_60_hz_and_144_hz() {
    assert_eq!(fought(144.0, 60.0), fought(60.0, 60.0));
}

#[test]
fn step_0_moves_nothing() {
    let mut duel = plan2(5);
    run(&mut duel, 1.5);
    let before = (duel.state().time, duel.state().fighters.clone());
    assert!(duel.step(0.0).is_empty());
    assert_eq!((duel.state().time, duel.state().fighters.clone()), before);
}

#[test]
fn zero_dt_holds_and_big_dt_is_capped() {
    let mut duel = plan2(5);
    for _ in 0..100 {
        assert!(duel.step(0.0).is_empty());
    }
    assert_eq!(duel.state().time, 0.0);
    // one huge step runs at most a quarter second of whole ticks
    duel.step(10.0);
    assert!(
        (duel.state().time - 0.25).abs() < 1e-9,
        "{}",
        duel.state().time
    );
    // NaN and negative steps hold too
    duel.step(f64::NAN);
    duel.step(-1.0);
    assert!(
        (duel.state().time - 0.25).abs() < 1e-9,
        "{}",
        duel.state().time
    );
    // and the next real step still moves it on
    duel.step(1.0 / 60.0);
    assert!(duel.state().time > 0.26);
}

#[test]
fn missing_effects_leave_their_skills_uncast() {
    let gone = ["projectiles/laser-bolt", "lightning/ball-lightning"];
    let effects: Vec<String> = all_effects()
        .into_iter()
        .filter(|e| !gone.contains(&e.as_str()))
        .collect();
    for seed in 1..=10 {
        let mut duel = Duel::with_brains(config(seed, effects.clone()));
        for f in &duel.state().fighters {
            for k in &f.kit.skills {
                let missing = k.motion.fx.is_some_and(|fx| gone.contains(&fx));
                assert_eq!(k.usable, !missing, "{}", k.gem.id);
            }
        }
        for o in run(&mut duel, 30.0) {
            if let Out::Play { fx, .. } = o {
                assert!(effects.iter().any(|e| e == fx), "{fx}");
            }
        }
    }
    // every skill is a spell with its effect: an empty registry leaves every skill uncast, and plays nothing
    let mut bare = Duel::with_brains(config(1, Vec::new()));
    assert!(
        run(&mut bare, 20.0)
            .iter()
            .all(|o| !matches!(o, Out::Play { .. }))
    );
    for f in &bare.state().fighters {
        for k in &f.kit.skills {
            assert!(!k.usable, "{}", k.gem.id);
        }
    }
}

#[test]
fn explicit_loadouts_are_checked_and_fought() {
    let ok = [
        common::loadout(&["fireball"]),
        common::loadout(&["bolt", "daofusion"]),
    ];
    let duel = common::duel_of(1, [common::idle(), common::idle()], ok.clone());
    assert_eq!(duel.state().fighters[0].loadout, ok[0]);
    assert!(duel.state().fighters[1].kit.get("daofusion").is_some());
    let mut bad = ok.clone();
    bad[1].actives[1].skill = "blink".to_string();
    assert!(
        Duel::with_loadouts(
            config(1, all_effects()),
            [common::idle(), common::idle()],
            bad
        )
        .is_err()
    );
}

#[test]
fn random_loadouts_come_from_dice_of_their_own() {
    // the loadouts are legal, differ by seed, and drawing them moves nothing else: the first rolls of the rules'
    // dice are the seed's own
    for seed in 1..=20 {
        for l in arena_duel::duel::loadouts(seed) {
            assert!(arena_calc::loadout::check(&l).is_ok(), "seed {seed}: {l:?}");
        }
    }
    assert_ne!(arena_duel::duel::loadouts(1), arena_duel::duel::loadouts(2));
    // a duel drawn at random fights exactly as one given the same loadouts: drawing them moved no rule's dice
    let mut drawn = plan2(4);
    let _ = PLAN2;
    let mut given = Duel::with_loadouts(
        config(4, all_effects()),
        [
            arena_duel::seat::Brain::make(),
            arena_duel::seat::Brain::make(),
        ],
        arena_duel::duel::loadouts(4),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(run(&mut drawn, 10.0), run(&mut given, 10.0));
}

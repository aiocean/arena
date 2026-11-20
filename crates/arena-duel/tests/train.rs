//! The training environment: one round a episode, decisions in, rewards out, the same every time.

mod common;

use arena_duel::style::Style;
use arena_duel::train::{Env, EnvConfig, Opponent, Step};
use arena_mind::Rng;
use arena_mind::policy::{ACTIONS, Act};
use common::all_effects;

fn cfg(seed: u32, learner: usize, opponent: Opponent) -> EnvConfig {
    EnvConfig {
        seed,
        learner,
        style: Style::Balanced,
        foe_style: Style::Balanced,
        opponent,
        limit: 90.0,
        effects: all_effects(),
    }
}

/// A random legal action from the env's mask.
fn random_legal(env: &Env, rng: &mut Rng) -> usize {
    let (_, legal) = env.observe();
    let ok: Vec<usize> = (0..ACTIONS).filter(|&i| legal[i]).collect();
    ok.get((rng.next() * ok.len() as f64) as usize)
        .copied()
        .unwrap_or(0)
}

/// Play `env` to its end with `pick`; every step's result.
fn play(env: &mut Env, shaping: f32, mut pick: impl FnMut(&Env) -> usize) -> Vec<Step> {
    let mut steps = vec![];
    for _ in 0..5000 {
        let a = pick(env);
        let s = env.step(a, shaping);
        steps.push(s);
        if s.done {
            break;
        }
    }
    steps
}

#[test]
fn an_episode_pays_plus_or_minus_one_at_its_end() {
    // a property over seeds, not a lucky seed: every episode that ends in a knockout pays +1 won or -1 lost, and a
    // random hand still knocks out or is knocked out in most of them (a cut, at the time limit, pays no ±1)
    let mut rng = Rng::new(1);
    let (mut ends, mut tried) = (0, 0);
    for seed in 1..=6 {
        for (learner, opp) in [
            (0, Opponent::Brain),
            (1, Opponent::First),
            (0, Opponent::Aggressive),
        ] {
            tried += 1;
            let mut env = Env::new(cfg(seed, learner, opp));
            let steps = play(&mut env, 0.0, |e| random_legal(e, &mut rng));
            let Some(last) = steps.last().copied().filter(|s| s.done && s.won.is_some()) else {
                continue;
            };
            ends += 1;
            let won = last.won.unwrap_or(false);
            let want = if won { 1.0 } else { -1.0 };
            assert!(
                (last.reward - want).abs() < 0.05,
                "seed {seed}: {} for won {won}",
                last.reward
            );
            assert!(last.length > 0.0 && last.length < 90.0);
        }
    }
    assert!(
        ends * 2 >= tried,
        "only {ends} of {tried} episodes ended in a knockout"
    );
}

#[test]
fn an_episode_cut_at_its_limit_is_a_loss() {
    let mut env = Env::new(EnvConfig {
        limit: 3.0,
        ..cfg(4, 0, Opponent::Turtle)
    });
    let steps = play(&mut env, 0.0, |_| 0);
    let last = steps.last().copied();
    assert!(
        last.is_some_and(|s| s.done && s.won.is_none() && (s.reward + 1.0).abs() < 0.05),
        "{last:?}"
    );
    assert!(
        steps.len() >= 2,
        "it decided along the way: {}",
        steps.len()
    );
}

#[test]
fn the_same_config_and_actions_replay_the_same_episode() {
    let run = || {
        let mut env = Env::new(cfg(5, 1, Opponent::Brain));
        let mut rng = Rng::new(9);
        let mut seen = vec![];
        let steps = play(&mut env, 1.0, |e| {
            seen.push(e.observe().0.to_vec());
            random_legal(e, &mut rng)
        });
        (steps, seen)
    };
    assert_eq!(run(), run());
}

#[test]
fn an_action_outside_the_mask_is_a_wait() {
    let run = |illegal: bool| {
        let mut env = Env::new(cfg(6, 0, Opponent::Aggressive));
        play(&mut env, 1.0, |e| {
            let (_, legal) = e.observe();
            if illegal {
                (0..ACTIONS).rev().find(|&i| !legal[i]).unwrap_or(0)
            } else {
                0
            }
        })
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn without_shaping_only_the_outcome_and_small_penalties_pay() {
    let mut rng = Rng::new(3);
    let mut env = Env::new(cfg(7, 0, Opponent::Brain));
    let steps = play(&mut env, 0.0, |e| random_legal(e, &mut rng));
    for s in steps.iter().filter(|s| !s.done) {
        assert!(s.reward <= 0.0 && s.reward >= -0.004 - 1e-6, "{s:?}");
    }
}

#[test]
fn shaping_pays_for_damage() {
    // the same fight paid with and without shaping: the damage either way is paid only when shaping is on
    let paid = |seed, shaping| {
        let mut rng = Rng::new(4);
        let mut env = Env::new(cfg(seed, 0, Opponent::Turtle));
        play(&mut env, shaping, |e| random_legal(e, &mut rng))
            .iter()
            .map(|s| s.reward)
            .sum::<f32>()
    };
    // a random hand deals damage in some fights: in those, and only with shaping, it is paid
    assert!((8..=20).any(|seed| paid(seed, 1.0) != paid(seed, 0.0)));
}

#[test]
fn a_bot_opponent_plays_its_answers() {
    let mut rng = Rng::new(5);
    let mut env = Env::new(cfg(9, 1, Opponent::Bot(common::pick())));
    let steps = play(&mut env, 0.0, |e| random_legal(e, &mut rng));
    assert!(steps.last().is_some_and(|s| s.done));
}

#[test]
fn an_act_that_fills_its_recent_decisions_pays_for_the_habit() {
    // meditate and guard in turn never repeat one act six times running, yet meditate is half of what it does
    let mut env = Env::new(cfg(9, 0, Opponent::Turtle));
    let mut turn = 0;
    let steps = play(&mut env, 0.0, |e| {
        turn += 1;
        let want = if turn % 2 == 0 {
            Act::Meditate
        } else {
            Act::Guard
        }
        .index();
        if e.observe().1[want] { want } else { 0 }
    });
    let worst = steps
        .iter()
        .filter(|s| !s.done)
        .map(|s| s.reward)
        .fold(0.0_f32, f32::min);
    assert!(
        worst <= -0.004,
        "the worst of {} steps paid {worst}",
        steps.len()
    );
}

#[test]
fn a_cast_that_lands_nothing_is_a_whiff_and_pays_the_whiff_cost() {
    // a hand that casts the first of its bar's slots it may, from wherever it stands, against a turtle (seed 16, whose
    // draws whiff): the same fight paid with and without the whiff cost differs by that cost for every whiff counted
    let hand = |e: &Env| {
        let (_, legal) = e.observe();
        (3..6)
            .map(|slot| {
                Act::Cast {
                    slot,
                    aim: arena_mind::policy::Aim::Foe,
                }
                .index()
            })
            .chain([Act::Toward.index()])
            .find(|&a| legal[a])
            .unwrap_or(0)
    };
    let run = |whiff: f32| {
        let mut env = Env::new(cfg(16, 0, Opponent::Turtle));
        let mut paid = 0.0_f32;
        for _ in 0..5000 {
            let s = env.step_with(hand(&env), 0.0, whiff);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        (paid, env.take_casts())
    };
    let (free, casts) = run(0.0);
    let (costly, again) = run(0.01);
    assert_eq!(
        casts, again,
        "the cost changes what is paid, not what happens"
    );
    let total = |f: fn(&arena_duel::train::CastLog) -> u32| casts.kinds.iter().map(f).sum::<u32>();
    let (n, far, whiffs) = (
        total(|c| c.casts),
        total(|c| c.out_of_reach),
        total(|c| c.whiffs),
    );
    assert!(n > 0 && whiffs > 0 && whiffs <= n && far <= n, "{casts:?}");
    let want = free - 0.01 * whiffs as f32;
    assert!(
        (costly - want).abs() < 1e-3,
        "{costly} against {want} ({whiffs} whiffs)"
    );
}

#[test]
fn the_far_cost_pays_only_for_whiffs_begun_out_of_reach() {
    // the hand above, against the aggressive script, who closes in, so some blows are thrown in reach and some from afar: the far
    // cost takes exactly its price for each far whiff and nothing for a blow thrown in reach
    let hand = |e: &Env| {
        let (_, legal) = e.observe();
        // the first of its skills it may cast (inside its band): thrown in reach or from afar
        (3..6)
            .map(|slot| {
                Act::Cast {
                    slot,
                    aim: arena_mind::policy::Aim::Foe,
                }
                .index()
            })
            .chain([Act::Wait.index()])
            .find(|&a| legal[a])
            .unwrap_or(0)
    };
    let run = |seed: u32, far: f32| {
        let mut env = Env::new(cfg(seed, 0, Opponent::Aggressive));
        let mut paid = 0.0_f32;
        for _ in 0..5000 {
            let s = env.step_costs(hand(&env), 0.0, 0.0, far);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        (paid, env.take_casts())
    };
    let tally = |casts: &arena_duel::train::Casts| {
        let total =
            |f: fn(&arena_duel::train::CastLog) -> u32| casts.kinds.iter().map(f).sum::<u32>();
        (
            total(|c| c.out_of_reach),
            total(|c| c.whiffs),
            total(|c| c.far_whiffs),
        )
    };
    // the range bands keep most blows in reach: the first seed whose fight has whiffs of both kinds
    let seed = (1..60)
        .find(|&seed| {
            let (_, whiffs, far_whiffs) = tally(&run(seed, 0.0).1);
            far_whiffs > 0 && far_whiffs < whiffs
        })
        .unwrap_or(0);
    assert!(seed > 0, "a seed with whiffs in reach and from afar");
    let (free, casts) = run(seed, 0.0);
    let (costly, again) = run(seed, 0.01);
    assert_eq!(casts, again);
    let (far, whiffs, far_whiffs) = tally(&casts);
    assert!(
        far_whiffs > 0 && far_whiffs < whiffs && far_whiffs <= far,
        "{casts:?}"
    );
    let want = free - 0.01 * far_whiffs as f32;
    assert!(
        (costly - want).abs() < 1e-3,
        "{costly} against {want} ({far_whiffs} far whiffs)"
    );
}

#[test]
fn variety_pays_for_what_the_actives_deal_and_nothing_for_the_basics() {
    // a random hand, which casts its actives as often as its basics: the same fight paid with and without the
    // variety term differs by that term times what the actives dealt, and by nothing else
    let run = |seed: u32, variety: f32| {
        let mut rng = Rng::new(seed);
        let mut env = Env::new(cfg(seed, 0, Opponent::Turtle));
        let pay = arena_duel::train::Pay {
            variety,
            ..Default::default()
        };
        let mut paid = 0.0_f32;
        for _ in 0..5000 {
            let s = env.step_paid(random_legal(&env, &mut rng), &pay);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        (paid, env.take_casts())
    };
    // over seeds, not one lucky one: every fight pays exactly what its actives dealt, and some deal something
    let mut dealt = 0.0;
    for seed in 14..24 {
        let (plain, casts) = run(seed, 0.0);
        let (varied, again) = run(seed, 1.0);
        assert_eq!(casts, again, "seed {seed}");
        let want = plain + (casts.active_damage / 100.0) as f32;
        assert!(
            (varied - want).abs() < 1e-3,
            "seed {seed}: {varied} against {want}"
        );
        dealt += casts.active_damage;
    }
    assert!(dealt > 0.0, "no active dealt anything in ten fights");
}

#[test]
fn idle_costs_a_decision_that_could_cast_and_does_not() {
    // a hand that never casts: paid with the idle cost, it loses that cost once a decision a cast was legal in
    let is_cast = |i: usize| matches!(Act::from_index(i), Some(Act::Cast { .. }));
    let run = |seed: u32, idle: f32| {
        let mut rng = Rng::new(seed);
        let mut env = Env::new(cfg(seed, 1, Opponent::Turtle));
        let pay = arena_duel::train::Pay {
            idle,
            ..Default::default()
        };
        let (mut paid, mut could) = (0.0_f32, 0_u32);
        for _ in 0..3000 {
            let legal = *env.observe().1;
            could += u32::from((0..ACTIONS).any(|i| legal[i] && is_cast(i)));
            let plain: Vec<usize> = (0..ACTIONS).filter(|&i| legal[i] && !is_cast(i)).collect();
            let a = plain[(rng.next() * plain.len() as f64) as usize];
            let s = env.step_paid(a, &pay);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        (paid, could)
    };
    let (plain, could) = run(5, 0.0);
    let (idle, again) = run(5, 0.01);
    assert_eq!(could, again);
    assert!(could > 0, "no cast was ever legal");
    let want = plain - 0.01 * could as f32;
    assert!((idle - want).abs() < 1e-3, "{idle} against {want}");
}

#[test]
fn land_pays_for_each_damaging_cast_that_lands() {
    // a random hand paid `land` a landed cast: the same fight paid with and without it differs by the casts that
    // landed (casts that struck, less the whiffs; one still in the air at the end is not paid)
    let run = |seed: u32, land: f32| {
        let mut rng = Rng::new(seed);
        let mut env = Env::new(cfg(seed, 0, Opponent::Turtle));
        let pay = arena_duel::train::Pay {
            land,
            ..Default::default()
        };
        let mut paid = 0.0_f32;
        for _ in 0..5000 {
            let s = env.step_paid(random_legal(&env, &mut rng), &pay);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        paid
    };
    let mut more = 0;
    for seed in 14..20 {
        let (plain, landed) = (run(seed, 0.0), run(seed, 1.0));
        let n = landed - plain;
        let whole = (n + 0.5) as i64;
        assert!(
            n >= -1e-3 && (n - whole as f32).abs() < 1e-3,
            "seed {seed}: {n}"
        );
        more += whole;
    }
    assert!(more > 0, "no cast landed in six fights");
}

#[test]
fn flicker_costs_a_move_undone_within_a_second_and_dropping_the_repeat_penalties_only_pays_more() {
    // duck, walk, duck, walk...: each act chosen again with the other between, inside a second, costs `flicker`;
    // the same fight paid without it differs by a whole number of flickers, and some are counted
    let run = |pay: arena_duel::train::Pay| {
        let mut env = Env::new(cfg(31, 0, Opponent::Turtle));
        let (duck, toward) = (Act::Duck.index(), Act::Toward.index());
        let mut paid = 0.0_f32;
        for i in 0..400 {
            let (_, legal) = env.observe();
            let want = if i % 2 == 0 { duck } else { toward };
            let s = env.step_paid(if legal[want] { want } else { 0 }, &pay);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        paid
    };
    let plain = run(arena_duel::train::Pay::default());
    let costly = run(arena_duel::train::Pay {
        flicker: 1.0,
        ..Default::default()
    });
    let flickers = plain - costly;
    assert!(flickers >= 5.0, "{flickers} flickers");
    let frac = flickers.rem_euclid(1.0);
    assert!(!(1e-3..=1.0 - 1e-3).contains(&frac), "{flickers}");
    // one act held: no flicker, and the repeat and habit penalties are all it pays beyond the fight
    let held = |repeat: f32| {
        let mut env = Env::new(cfg(31, 0, Opponent::Turtle));
        let duck = Act::Duck.index();
        let pay = arena_duel::train::Pay {
            flicker: 1.0,
            repeat,
            habit: repeat,
            ..Default::default()
        };
        let mut paid = 0.0_f32;
        for _ in 0..400 {
            let (_, legal) = env.observe();
            let s = env.step_paid(if legal[duck] { duck } else { 0 }, &pay);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        paid
    };
    assert!(held(0.0) > held(1.0), "{} {}", held(0.0), held(1.0));
    assert!(held(1.0) > -50.0, "no flicker when one act is held");
}

#[test]
fn a_meditation_left_at_once_costs_short_sit_and_one_sat_through_does_not() {
    // meditate then walk at once, over and over: each costs `short_sit`; meditate and wait out two seconds first:
    // nothing
    let run = |hold: usize, short_sit: f32| {
        let mut env = Env::new(cfg(33, 0, Opponent::Turtle));
        let (sit, toward) = (Act::Meditate.index(), Act::Toward.index());
        let pay = arena_duel::train::Pay {
            short_sit,
            ..Default::default()
        };
        let (mut paid, mut i) = (0.0_f32, 0usize);
        let mut sat_at = 0.0;
        for _ in 0..600 {
            let (_, legal) = env.observe();
            let t = env.brief().time;
            let a = match i {
                0 if legal[sit] => {
                    sat_at = t;
                    i = 1;
                    sit
                }
                1 if t - sat_at >= hold as f64 => {
                    i = 0;
                    toward
                }
                _ => 0,
            };
            let s = env.step_paid(a, &pay);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        paid
    };
    let short = run(0, 0.0) - run(0, 1.0);
    assert!(short >= 3.0, "{short} short sits");
    let frac = short.rem_euclid(1.0);
    assert!(!(1e-3..=1.0 - 1e-3).contains(&frac), "{short}");
    assert!(
        (run(2, 0.0) - run(2, 1.0)).abs() < 1e-3,
        "a whole sit costs nothing"
    );
}

#[test]
fn each_duck_costs_duck() {
    // duck whenever it may: the cost is `duck` times the ducks chosen, and nothing else changes
    let run = |duck: f32| {
        let mut env = Env::new(cfg(35, 0, Opponent::Turtle));
        let act = Act::Duck.index();
        let pay = arena_duel::train::Pay {
            duck,
            ..Default::default()
        };
        let (mut paid, mut ducks) = (0.0_f32, 0);
        for _ in 0..400 {
            let (_, legal) = env.observe();
            let a = if legal[act] { act } else { 0 };
            ducks += usize::from(a == act);
            let s = env.step_paid(a, &pay);
            paid += s.reward;
            if s.done {
                break;
            }
        }
        (paid, ducks)
    };
    let (plain, ducks) = run(0.0);
    let (costly, _) = run(1.0);
    assert!(ducks > 0);
    assert!(
        ((plain - costly) - ducks as f32).abs() < 1e-2,
        "{plain} {costly} {ducks}"
    );
}

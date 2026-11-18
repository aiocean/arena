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
    // a hand that casts its first slot whenever it may, from wherever it stands, against a turtle: the same fight
    // paid with and without the whiff cost differs by that cost for every whiff counted
    let hand = |e: &Env| {
        let (_, legal) = e.observe();
        [
            Act::Cast {
                slot: 0,
                aim: arena_mind::policy::Aim::Foe,
            }
            .index(),
            Act::Toward.index(),
        ]
        .into_iter()
        .find(|&a| legal[a])
        .unwrap_or(0)
    };
    let run = |whiff: f32| {
        let mut env = Env::new(cfg(11, 0, Opponent::Turtle));
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
        [
            // the second basic, the close burst: thrown in reach or from afar
            Act::Cast {
                slot: 1,
                aim: arena_mind::policy::Aim::Foe,
            }
            .index(),
            Act::Wait.index(),
        ]
        .into_iter()
        .find(|&a| legal[a])
        .unwrap_or(0)
    };
    let run = |far: f32| {
        let mut env = Env::new(cfg(12, 0, Opponent::Aggressive));
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
    let (free, casts) = run(0.0);
    let (costly, again) = run(0.01);
    assert_eq!(casts, again);
    let total = |f: fn(&arena_duel::train::CastLog) -> u32| casts.kinds.iter().map(f).sum::<u32>();
    let (far, whiffs, far_whiffs) = (
        total(|c| c.out_of_reach),
        total(|c| c.whiffs),
        total(|c| c.far_whiffs),
    );
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

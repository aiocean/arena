//! A training env copied at a decision (`Env::fork`, `restore_from`): the copy fights the original's fight bit for bit
//! until it is reseeded (`Env::reseed`), then a fight of its own, the same for the same salt; and the search on it
//! (`Env::lookahead`).

mod common;

use arena_duel::seat::{Decider, Fault, SharedDecider};
use arena_duel::style::Style;
use arena_duel::train::{Env, EnvConfig, Opponent, Pay, Step};
use arena_mind::Rng;
use arena_mind::policy::{ACTIONS, Act, OBS};
use common::all_effects;
use std::cell::RefCell;
use std::rc::Rc;

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

/// A bot that answers from what it is handed only: the first legal cast, else a step toward.
struct Plain {
    stateless: bool,
}

impl Decider for Plain {
    fn start(&mut self, _: u32) -> Result<(), Fault> {
        Ok(())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Ok(())
    }
    fn decide(&mut self, _: &[f32; OBS], legal: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        let cast = (0..ACTIONS)
            .find(|&i| legal[i] && matches!(Act::from_index(i), Some(Act::Cast { .. })));
        Ok(cast.unwrap_or(1) as i32)
    }
    fn stateless(&self) -> bool {
        self.stateless
    }
}

fn plain(stateless: bool) -> SharedDecider {
    Rc::new(RefCell::new(Plain { stateless }))
}

fn opponents() -> Vec<(&'static str, Opponent)> {
    vec![
        ("brain", Opponent::Brain),
        ("first", Opponent::First),
        ("aggressive", Opponent::Aggressive),
        ("turtle", Opponent::Turtle),
        ("bot", Opponent::Bot(plain(true))),
    ]
}

/// A random legal act on `rng`.
fn pick(rng: &mut Rng, legal: &[bool; ACTIONS]) -> usize {
    let ok: Vec<usize> = (0..ACTIONS).filter(|&i| legal[i]).collect();
    if ok.is_empty() {
        return 0;
    }
    ok[((rng.next() * ok.len() as f64) as usize).min(ok.len() - 1)]
}

fn shaped() -> Pay {
    Pay {
        shaping: 1.0,
        whiff: 0.01,
        land: 0.01,
        ..Pay::default()
    }
}

/// `n` decisions answered on `rng`: what each observed, allowed and earned.
type Trace = Vec<(Vec<u32>, [bool; ACTIONS], Step)>;

fn play(env: &mut Env, rng: &mut Rng, n: usize) -> Trace {
    let mut out = Vec::new();
    for _ in 0..n {
        let (obs, legal) = env.observe();
        let (obs, legal) = (obs.iter().map(|x| x.to_bits()).collect(), *legal);
        let s = env.step_paid(pick(rng, &legal), &shaped());
        out.push((obs, legal, s));
        if s.done {
            break;
        }
    }
    out
}

#[test]
fn a_fork_fights_the_originals_fight_bit_for_bit() -> Result<(), &'static str> {
    for (name, foe) in opponents() {
        for seed in 1..=4u32 {
            let mut env = Env::new(cfg(seed, (seed % 2) as usize, foe.clone()));
            let mut rng = Rng::new(seed);
            play(&mut env, &mut rng, 25);
            let mut copy = env.fork().ok_or("every built-in seat forks")?;
            // a spare from another fight, restored into
            let mut spare = Env::new(cfg(seed + 50, 1 - (seed % 2) as usize, Opponent::Brain));
            play(&mut spare, &mut Rng::new(9), 10);
            assert!(spare.restore_from(&env));
            assert_eq!(copy.state(), env.state(), "{name} {seed}");
            let (a, b, c) = (
                play(&mut env, &mut rng.clone(), 200),
                play(&mut copy, &mut rng.clone(), 200),
                play(&mut spare, &mut rng.clone(), 200),
            );
            assert_eq!(a, b, "{name} seed {seed}: fork");
            assert_eq!(a, c, "{name} seed {seed}: restored");
            assert_eq!(env.state(), copy.state(), "{name} {seed}");
            assert_eq!(env.state(), spare.state(), "{name} {seed}");
        }
    }
    Ok(())
}

#[test]
fn a_bot_that_keeps_state_is_not_forked() {
    let env = Env::new(cfg(3, 0, Opponent::Bot(plain(false))));
    assert!(env.fork().is_none());
    let mut spare = Env::new(cfg(3, 0, Opponent::Brain));
    let before = spare.state().clone();
    assert!(!spare.restore_from(&env));
    assert_eq!(spare.state(), &before);
    let mut none = None;
    assert!(
        env.lookahead(&mut none, 4, 1, 1.0, 7, &shaped(), &mut |_, _, _| 0)
            .is_none()
    );
}

#[test]
fn a_reseeded_fork_fights_a_fight_of_its_own_the_same_for_the_same_salt() -> Result<(), &'static str>
{
    for (name, foe) in opponents() {
        let mut env = Env::new(cfg(5, 0, foe));
        let mut rng = Rng::new(5);
        play(&mut env, &mut rng, 10);
        let (mut a, mut b, mut c) = (
            env.fork().ok_or("a fork")?,
            env.fork().ok_or("a fork")?,
            env.fork().ok_or("a fork")?,
        );
        a.reseed(11);
        b.reseed(11);
        c.reseed(12);
        let (ta, tb, tc, t0) = (
            play(&mut a, &mut rng.clone(), 300),
            play(&mut b, &mut rng.clone(), 300),
            play(&mut c, &mut rng.clone(), 300),
            play(&mut env, &mut rng.clone(), 300),
        );
        assert_eq!(ta, tb, "{name}: one salt, one fight");
        assert_eq!(a.state(), b.state(), "{name}");
        assert_ne!(
            ta, t0,
            "{name}: a reseeded copy fights the original's fight"
        );
        assert_ne!(ta, tc, "{name}: two salts, one fight");
    }
    Ok(())
}

#[test]
fn the_search_values_every_legal_act_and_no_illegal_one() -> Result<(), &'static str> {
    for (name, foe) in opponents() {
        let mut env = Env::new(cfg(2, 1, foe));
        play(&mut env, &mut Rng::new(2), 8);
        let mut spare = None;
        let before = env.state().clone();
        let w = env
            .lookahead(&mut spare, 6, 2, 0.99, 3, &shaped(), &mut |_, l, _| {
                (0..ACTIONS).find(|&i| l[i]).unwrap_or(0)
            })
            .ok_or("a search")?;
        let (_, legal) = env.observe();
        for a in 0..ACTIONS {
            assert_eq!(legal[a], w[a].is_finite(), "{name}: act {a} {}", w[a]);
        }
        assert_eq!(
            env.state(),
            &before,
            "{name}: the search leaves the env alone"
        );
        // the same salt, the same values
        let again = env
            .lookahead(&mut spare, 6, 2, 0.99, 3, &shaped(), &mut |_, l, _| {
                (0..ACTIONS).find(|&i| l[i]).unwrap_or(0)
            })
            .ok_or("a search")?;
        assert_eq!(w.map(f32::to_bits), again.map(f32::to_bits), "{name}");
    }
    Ok(())
}

#[test]
fn against_the_turtle_the_search_puts_a_damaging_cast_above_waiting() -> Result<(), &'static str> {
    let pay = Pay {
        shaping: 1.0,
        ..Pay::default()
    };
    let mut found = 0;
    for seed in 1..=3u32 {
        let mut env = Env::new(cfg(seed, 0, Opponent::Turtle));
        let mut spare = None;
        for _ in 0..80 {
            let w = env
                .lookahead(
                    &mut spare,
                    8,
                    1,
                    1.0,
                    u64::from(seed),
                    &pay,
                    &mut |_, _, _| 0,
                )
                .ok_or("a search")?;
            let best = (0..ACTIONS)
                .filter(|&a| w[a].is_finite())
                .max_by(|&a, &b| w[a].total_cmp(&w[b]))
                .unwrap_or(0);
            if matches!(Act::from_index(best), Some(Act::Cast { .. })) && w[best] > w[0] + 0.01 {
                found += 1;
            }
            // play on with the search's choice
            if env.step_paid(best, &pay).done {
                break;
            }
        }
    }
    assert!(found >= 3, "a cast beat waiting at {found} decisions");
    Ok(())
}

//! How fast the training env runs, on one thread: the learner answers each decision with a random legal act (fixed
//! dice) against each opponent, and the decisions a second are printed with a fingerprint of everything the learner
//! saw and earned (each observation, its legal acts, each reward), so a change meant to be faster only can be
//! checked to change nothing.
//!
//! `cargo run --release -p arena-bot --features onnx --example env_speed -- [--decisions 20000] [first brain ...]`
//! (the opponents: `first`, `brain`, `aggressive`, `turtle`, `onnx` for the house AI; all but `turtle` by default).
//!
//! With `--lookahead DEPTH` it measures the search instead (`Env::lookahead`, `docs/plans/2026-11-20-smarter-ai.md`,
//! G): at each of `--roots` decisions (default 200, one every 5th of the learner's), every legal act rolled out
//! `DEPTH` decisions, `--samples` times (default 1), the learner answered after the first by the house AI; it prints
//! the time a root decision and a rollout.

use arena_bot::onnx::Onnx;
use arena_duel::style::Style;
use arena_duel::train::{Env, EnvConfig, Opponent, Pay};
use arena_mind::policy::ACTIONS;
use std::time::Instant;

/// The learner's dice: xorshift64.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn act(&mut self, legal: &[bool; ACTIONS]) -> usize {
        let n = legal.iter().filter(|&&l| l).count().max(1);
        let k = (self.next() % n as u64) as usize;
        legal
            .iter()
            .enumerate()
            .filter(|(_, l)| **l)
            .nth(k)
            .map_or(0, |(i, _)| i)
    }
}

/// FNV-1a over 64-bit words.
struct Print(u64);

impl Print {
    fn add(&mut self, x: u64) {
        self.0 ^= x;
        self.0 = self.0.wrapping_mul(0x100_0000_01b3);
    }
}

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn opponent(name: &str, house: &Option<Onnx>) -> Res<Opponent> {
    Ok(match name {
        "brain" => Opponent::Brain,
        "first" => Opponent::First,
        "aggressive" => Opponent::Aggressive,
        "turtle" => Opponent::Turtle,
        "onnx" => Opponent::Bot(house.as_ref().ok_or("the house AI")?.decider()),
        _ => return Err(format!("no opponent {name}").into()),
    })
}

fn run(name: &str, house: &Option<Onnx>, decisions: u64) -> Res<()> {
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    let mut print = Print(0xcbf2_9ce4_8422_2325);
    let pay = Pay::default();
    let start = Instant::now();
    let (mut n, mut seed, mut rounds) = (0u64, 1u32, 0u32);
    while n < decisions {
        let mut env = Env::new(EnvConfig {
            seed,
            learner: (seed % 2) as usize,
            style: Style::Balanced,
            foe_style: Style::Balanced,
            opponent: opponent(name, house)?,
            limit: 90.0,
            effects: arena_duel::effects::all(),
        });
        seed += 1;
        rounds += 1;
        loop {
            let (obs, legal) = env.observe();
            for x in obs {
                print.add(u64::from(x.to_bits()));
            }
            for &l in legal {
                print.add(u64::from(l));
            }
            let a = dice.act(legal);
            let s = env.step_paid(a, &pay);
            print.add(u64::from(s.reward.to_bits()));
            print.add(u64::from(s.done));
            n += 1;
            if s.done {
                break;
            }
        }
    }
    let s = start.elapsed().as_secs_f64();
    println!(
        "{name:>10}: {n} decisions, {rounds} rounds in {s:.2} s = {:.0} decisions/s; fingerprint {:016x}",
        n as f64 / s,
        print.0
    );
    Ok(())
}

/// The search's cost: `roots` root decisions against `name`, each act rolled out `depth` decisions `samples` times.
fn search(name: &str, house: &Onnx, depth: u32, samples: u32, roots: u32) -> Res<()> {
    let mut dice = Dice(0x9e37_79b9_7f4a_7c15);
    let pay = Pay {
        shaping: 1.0,
        ..Pay::default()
    };
    let mut policy = |obs: &[f32; arena_mind::policy::OBS], legal: &[bool; ACTIONS], u: f32| {
        house
            .logits(obs)
            .map_or(0, |l| arena_mind::policy::sample(&l, legal, u))
    };
    let some = Some(house.clone());
    let (mut seed, mut done, mut acts, mut busy) = (1u32, 0u32, 0u64, 0.0f64);
    let mut rollouts = 0u64;
    while done < roots {
        let mut env = Env::new(EnvConfig {
            seed,
            learner: (seed % 2) as usize,
            style: Style::Balanced,
            foe_style: Style::Balanced,
            opponent: opponent(name, &some)?,
            limit: 90.0,
            effects: arena_duel::effects::all(),
        });
        seed += 1;
        let mut spare = None;
        for k in 0u32.. {
            if k % 5 == 0 && done < roots {
                let t = Instant::now();
                let w = env
                    .lookahead(
                        &mut spare,
                        depth,
                        samples,
                        1.0,
                        u64::from(k),
                        &pay,
                        &mut policy,
                    )
                    .ok_or("a seat that cannot be copied")?;
                busy += t.elapsed().as_secs_f64();
                let legal = w.iter().filter(|v| v.is_finite()).count() as u64;
                acts += legal;
                rollouts += legal * u64::from(samples.max(1));
                done += 1;
            }
            let (_, legal) = env.observe();
            let a = dice.act(legal);
            if env.step_paid(a, &pay).done {
                break;
            }
        }
    }
    println!(
        "{name:>10}: depth {depth} x {samples}: {roots} roots, {:.1} legal acts a root: {:.2} ms a root, {:.0} µs a rollout",
        acts as f64 / f64::from(roots),
        busy * 1e3 / f64::from(roots),
        busy * 1e6 / rollouts as f64
    );
    Ok(())
}

fn main() -> Res<()> {
    let mut decisions = 20_000u64;
    let (mut depth, mut samples, mut roots) = (None, 1u32, 200u32);
    let mut names = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--decisions" {
            decisions = args.next().ok_or("--decisions N")?.parse()?;
        } else if a == "--lookahead" {
            depth = Some(args.next().ok_or("--lookahead DEPTH")?.parse()?);
        } else if a == "--samples" {
            samples = args.next().ok_or("--samples N")?.parse()?;
        } else if a == "--roots" {
            roots = args.next().ok_or("--roots N")?.parse()?;
        } else {
            names.push(a);
        }
    }
    if names.is_empty() {
        names = ["first", "brain", "aggressive", "onnx"]
            .map(String::from)
            .to_vec();
    }
    if let Some(depth) = depth {
        let house = Onnx::house().map_err(|f| f.0)?;
        for name in &names {
            search(name, &house, depth, samples, roots)?;
        }
        return Ok(());
    }
    let house = if names.iter().any(|n| n == "onnx") {
        Some(Onnx::house().map_err(|f| f.0)?)
    } else {
        None
    };
    for name in &names {
        run(name, &house, decisions)?;
    }
    Ok(())
}

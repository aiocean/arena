//! How fast the training env runs, on one thread: the learner answers each decision with a random legal act (fixed
//! dice) against each opponent, and the decisions a second are printed with a fingerprint of everything the learner
//! saw and earned (each observation, its legal acts, each reward), so a change meant to be faster only can be
//! checked to change nothing.
//!
//! `cargo run --release -p arena-bot --features onnx --example env_speed -- [--decisions 20000] [first brain ...]`
//! (the opponents: `first`, `brain`, `aggressive`, `turtle`, `onnx` for the house AI; all but `turtle` by default).

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

fn main() -> Res<()> {
    let mut decisions = 20_000u64;
    let mut names = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--decisions" {
            decisions = args.next().ok_or("--decisions N")?.parse()?;
        } else {
            names.push(a);
        }
    }
    if names.is_empty() {
        names = ["first", "brain", "aggressive", "onnx"]
            .map(String::from)
            .to_vec();
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

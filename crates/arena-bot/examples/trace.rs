//! Every tick of a set of fights, one JSON line each (the page's view, the commands and events, what each mind
//! senses), to compare two builds of the rules: `compare` reads two traces and says, for every number, whether it
//! stayed the same or was scaled by one factor, by its path. A change of the world's unit must scale every length
//! and leave everything else as it was.
//!
//! `cargo run --release -p arena-bot --features trace --example trace -- write <file>`
//! `cargo run --release -p arena-bot --features trace --example trace -- compare <old> <new> [factor=2]`
//!
//! The policy rows play the house AI (`weights/policy.onnx`) natively.

use arena_bot::onnx::Onnx;
use pixfx_duel::duel::{Duel, DuelConfig, TICK};
use pixfx_duel::ladder::Bot;
use pixfx_duel::sense::sense_of;
use pixfx_duel::view::view;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufWriter, Write};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The fights: two contestants, a seed, and how long.
fn fights() -> Res<Vec<(Bot, Bot, u32, f64)>> {
    let house = Onnx::house().map_err(|e| e.0)?;
    let policy = || -> Res<Bot> {
        let onnx = house.clone();
        Ok(Bot::Decider(std::rc::Rc::new(move || onnx.decider())))
    };
    let mut out = Vec::new();
    for seed in 1..=6 {
        out.push((Bot::Brain, Bot::Brain, seed, 90.0));
    }
    for seed in 1..=3 {
        out.push((policy()?, policy()?, seed, 90.0));
        out.push((Bot::First, Bot::Aggressive, seed, 60.0));
        out.push((Bot::Turtle, policy()?, seed, 60.0));
        out.push((Bot::Brain, Bot::First, seed, 60.0));
    }
    Ok(out)
}

fn write(file: &str, unit: f64) -> Res<()> {
    let effects: Vec<String> = pixfx_duel::effects::all();
    let mut w = BufWriter::new(std::fs::File::create(file)?);
    for (a, b, seed, secs) in fights()? {
        let config = DuelConfig {
            seed,
            left: 0.0,
            right: 220.0 * unit,
            ground: 150.0 * unit,
            sky: 96.0 * unit,
            effects: effects.clone(),
        };
        let mut duel = Duel::new(config, [a.maker(), b.maker()]);
        for _ in 0..(secs / TICK) as u32 {
            let out = duel.step(TICK);
            let senses: Vec<Value> = (0..2)
                .map(|me| serde_json::to_value(sense_of(duel.state(), me)))
                .collect::<Result<_, _>>()?;
            let line = json!({ "view": view(duel.state()), "out": out, "sense": senses });
            writeln!(w, "{line}")?;
        }
    }
    Ok(())
}

/// For every path (array indices dropped), the factors seen between old and new numbers, and the first mismatch.
#[derive(Default)]
struct Seen {
    same: u64,
    scaled: u64,
    zero: u64,
    other: Vec<String>,
}

fn walk(
    path: &str,
    a: &Value,
    b: &Value,
    factor: f64,
    line: usize,
    seen: &mut BTreeMap<String, Seen>,
    shape: &mut Vec<String>,
) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (
                x.as_f64().unwrap_or(f64::NAN),
                y.as_f64().unwrap_or(f64::NAN),
            );
            let s = seen.entry(path.to_string()).or_default();
            if x == 0.0 && y == 0.0 {
                s.zero += 1;
            } else if x == y {
                s.same += 1;
            } else if x * factor == y {
                s.scaled += 1;
            } else if s.other.len() < 3 {
                s.other.push(format!("line {line}: {x} -> {y}"));
            } else {
                s.other.push(String::new());
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (u, v) in x.iter().zip(y) {
                walk(&format!("{path}[]"), u, v, factor, line, seen, shape);
            }
        }
        (Value::Object(x), Value::Object(y)) if x.len() == y.len() => {
            for (k, u) in x {
                match y.get(k) {
                    Some(v) => walk(&format!("{path}.{k}"), u, v, factor, line, seen, shape),
                    None => shape.push(format!("line {line}: {path}.{k} gone")),
                }
            }
        }
        _ if a == b => {}
        _ => {
            if shape.len() < 20 {
                shape.push(format!("line {line}: {path}: {} -> {}", short(a), short(b)));
            }
        }
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 160 {
        format!("{}...", &s[..160])
    } else {
        s
    }
}

fn compare(old: &str, new: &str, factor: f64) -> Res<()> {
    let a = std::io::BufReader::new(std::fs::File::open(old)?).lines();
    let b = std::io::BufReader::new(std::fs::File::open(new)?).lines();
    let mut seen = BTreeMap::new();
    let mut shape = Vec::new();
    let mut n = 0;
    for (i, (x, y)) in a.zip(b).enumerate() {
        let (x, y): (Value, Value) = (serde_json::from_str(&x?)?, serde_json::from_str(&y?)?);
        walk("", &x, &y, factor, i + 1, &mut seen, &mut shape);
        n = i + 1;
        if shape.len() >= 20 {
            break;
        }
    }
    println!("{n} lines compared");
    for s in &shape {
        println!("SHAPE {s}");
    }
    for (path, s) in &seen {
        let mixed = s.same > 0 && s.scaled > 0;
        let tag = if !s.other.is_empty() {
            "WRONG"
        } else if mixed {
            "MIXED"
        } else if s.scaled > 0 {
            "x"
        } else {
            "="
        };
        println!(
            "{tag:5} {path}  same {} scaled {} zero {} other {}",
            s.same,
            s.scaled,
            s.zero,
            s.other.len()
        );
        for o in s.other.iter().filter(|o| !o.is_empty()) {
            println!("        {o}");
        }
    }
    Ok(())
}

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["write", file] => write(file, 1.0),
        ["write", file, unit] => write(file, unit.parse()?),
        ["compare", old, new] => compare(old, new, 2.0),
        ["compare", old, new, factor] => compare(old, new, factor.parse()?),
        _ => Err("usage: trace write <file> [unit] | trace compare <old> <new> [factor]".into()),
    }
}

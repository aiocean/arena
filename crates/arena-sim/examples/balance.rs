//! The balance tool: random legal loadout pairs, each played both ways over a set of seeds, measured per skill,
//! support and keystone (`arena_sim::balance`), printed as a tier table and written as JSON.
//!
//! ```text
//! cargo run --release -p arena-sim --example balance -- --seeds 20 --pairs 200 [--seed 1] [--threads N] [--out table.json]
//! ```

use std::time::Instant;

use arena_sim::balance::{Config, run};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut config = Config {
        seeds: 4,
        pairs: 20,
        seed: 1,
        threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
    };
    let mut out: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        let v = args
            .get(i + 1)
            .ok_or(format!("{} needs a value", args[i]))?;
        let num = |v: &str| v.parse::<u64>().map_err(|_| format!("not a number: {v}"));
        match args[i].as_str() {
            "--seeds" => config.seeds = num(v)? as u32,
            "--pairs" => config.pairs = num(v)? as u32,
            "--seed" => config.seed = num(v)?,
            "--threads" => config.threads = num(v)? as usize,
            "--out" => out = Some(v.clone()),
            f => return Err(format!("unknown flag {f}")),
        }
        i += 2;
    }
    let start = Instant::now();
    let table = run(config);
    let took = start.elapsed().as_secs_f64();
    print!("{}", table.text());
    println!(
        "\n{:.1} s on {} threads ({:.1} ms a match)",
        took,
        config.threads,
        1000.0 * took * config.threads as f64 / f64::from(table.matches.max(1))
    );
    if let Some(path) = out {
        // validate before writing: the JSON must parse
        let json = table.json();
        serde_json::from_str::<serde_json::Value>(&json).map_err(|e| format!("bad JSON: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("{path}: {e}"))?;
        println!("wrote {path}");
    }
    Ok(())
}

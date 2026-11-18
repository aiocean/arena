//! The balance tool: random legal loadout pairs, each played both ways over a set of seeds, measured per skill,
//! support and keystone (`arena_sim::balance`), printed as a tier table and written as JSON.
//!
//! ```text
//! cargo run --release -p arena-sim --example balance -- --seeds 20 --pairs 200 [--seed 1] [--threads N] [--out table.json] [--telemetry-dir DIR]
//! ```
//!
//! `--telemetry-dir DIR` (or `ARENA_TELEMETRY_DIR` set) sends the run and every row (each gem, support and keystone: its
//! win rate, use, damage) to the telemetry files as `arena::ladder` lines under one `run_id` (`ops/telemetry/README.md`).

use std::time::Instant;

use arena_sim::balance::{Config, Table, run};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _export = arena_telemetry::requested(&args)?.map(|dir| {
        arena_telemetry::init(
            "arena-ladder",
            arena_telemetry::Options {
                dir: Some(dir),
                ..arena_telemetry::Options::tool(env!("CARGO_PKG_VERSION"))
            },
        )
    });
    let args = arena_telemetry::strip(args);
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
    export(&table, took);
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

/// The run and its rows as `arena::ladder` events (nothing when telemetry is off).
fn export(table: &Table, took: f64) {
    let run = arena_telemetry::run_id("balance");
    let c = &table.config;
    tracing::info!(target: "arena::ladder", run_id = %run, seeds = c.seeds, pairs = c.pairs, seed = c.seed,
        matches = table.matches, seat0_wins = table.seat0_wins, drawn = table.drawn, rounds = table.rounds,
        cut = table.cut, round_seconds = table.round_seconds, wall_seconds = took, "balance.run");
    for r in &table.rows {
        tracing::info!(target: "arena::ladder", run_id = %run, kind = %format!("{:?}", r.kind).to_lowercase(),
            id = %r.id, tier = %r.tier, picks = r.picks, games = r.games, win = r.wr(), used = r.used,
            used_rate = r.used_rate(), damage = r.damage, damage_per_game = r.avg_damage(), casts = r.casts,
            hits = r.hits, cast_distance = r.avg_cast(), hit_distance = r.avg_hit(), "balance.row");
    }
}

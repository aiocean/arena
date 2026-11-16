//! `arena-bot check FILE` | `ladder FILE [--vs FILE] [--seeds N]` | `fight A B --seed N [--out LOG]` | `verify LOG A B`:
//! exit 0 on a pass, 1 with the reason on stderr otherwise.

use std::error::Error;
use std::process::ExitCode;

use arena_bot::run::{LIMIT, ROUNDS, Verdict, bot, entry, fight, verify};
use arena_bot::{check, engine};
use pixfx_duel::first_mind::FirstMind;
use pixfx_duel::ladder::{Match, Record, match_up};
use pixfx_duel::log::ActLog;
use pixfx_duel::scripted::Scripted;
use pixfx_duel::seat::Brain;
use pixfx_duel::style::Style;
use wasmtime::Engine;

const USAGE: &str = "usage: arena-bot check FILE | ladder FILE [--vs FILE] [--seeds N] | fight A B --seed N [--out LOG] | verify LOG A B";

/// The seeds of the ladder and of the entry bar.
const SEEDS: u32 = 150;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match command(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}

/// The value after `name`, if the flag is there.
fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

/// The arguments that are neither a flag nor a flag's value.
fn files(args: &[String]) -> Vec<&String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if a.starts_with("--") {
            skip = true;
        } else {
            out.push(a);
        }
    }
    out
}

fn command(args: &[String]) -> Result<(), Box<dyn Error>> {
    let engine = engine()?;
    let rest = args.get(1..).unwrap_or_default();
    let files = files(rest);
    match (args.first().map(String::as_str), files.as_slice()) {
        (Some("check"), [file]) => {
            let bytes = std::fs::read(file)?;
            match entry(&engine, &bytes, 1..SEEDS + 1) {
                verdict @ Verdict::Pass { .. } => {
                    println!("{verdict}");
                    Ok(())
                }
                verdict => Err(verdict.to_string().into()),
            }
        }
        (Some("ladder"), [file]) => {
            ladder(&engine, file, flag(rest, "--vs"), flag(rest, "--seeds"))
        }
        (Some("fight"), [a, b]) => {
            let seed = flag(rest, "--seed")
                .ok_or("fight needs --seed N")?
                .parse()?;
            let log = fight(
                &engine,
                &std::fs::read(a)?,
                &std::fs::read(b)?,
                seed,
                ROUNDS,
                LIMIT,
            )?;
            let json = serde_json::to_string(&log)?;
            match flag(rest, "--out") {
                Some(path) => std::fs::write(path, json)?,
                None => println!("{json}"),
            }
            Ok(())
        }
        (Some("verify"), [log, a, b]) => {
            let log: ActLog = serde_json::from_slice(&std::fs::read(log)?)?;
            verify(&engine, &log, &std::fs::read(a)?, &std::fs::read(b)?)?;
            println!("the fight of seed {} plays again act for act", log.seed);
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

/// The module against the house opponents, itself and `--vs`, printed as the example ladder prints its rows.
fn ladder(
    engine: &Engine,
    file: &str,
    vs: Option<String>,
    seeds: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let seeds: u32 = seeds.map_or(Ok(SEEDS), |s| s.parse())?;
    let module = check(engine, &std::fs::read(file)?)?;
    let balanced = bot(engine, &module, Some(Style::Balanced));
    let drawn = bot(engine, &module, None);
    let m = Match {
        seeds: 1..seeds + 1,
        rounds: ROUNDS,
        limit: LIMIT,
        effects: pixfx_effects::EFFECTS
            .iter()
            .map(|e| e.meta().id.to_string())
            .collect(),
    };
    println!("ladder: {seeds} seeds x both ways x {ROUNDS} rounds, rounds cut at {LIMIT} s\n");
    println!(
        "{:<34} {:>7} {:>9} {:>7} {:>7} {:>5} {:>7}",
        "match", "win", "thunder", "frost", "len s", "cut", "faults"
    );
    let row = |name: &str, r: Record| {
        let pct = |x: f64| format!("{:.1}%", x * 100.0);
        println!(
            "{:<34} {:>7} {:>9} {:>7} {:>7.1} {:>5} {:>7}",
            name,
            pct(r.win_rate()),
            pct(r.as_school(0)),
            pct(r.as_school(1)),
            r.mean_length(),
            r.timeouts,
            r.faults[0],
        );
    };
    row("balanced vs brain", match_up(&balanced, &Brain::make, &m));
    row(
        "balanced vs first mind",
        match_up(&balanced, &FirstMind::make, &m),
    );
    row(
        "balanced vs aggressive script",
        match_up(&balanced, &Scripted::aggressive, &m),
    );
    row(
        "balanced vs turtle script",
        match_up(&balanced, &Scripted::turtle, &m),
    );
    row("mirror, styles drawn", match_up(&drawn, &drawn, &m));
    if let Some(other) = vs {
        let theirs = bot(
            engine,
            &check(engine, &std::fs::read(&other)?)?,
            Some(Style::Balanced),
        );
        row(
            &format!("balanced vs {other}"),
            match_up(&balanced, &theirs, &m),
        );
    }
    Ok(())
}

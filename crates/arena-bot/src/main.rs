//! `arena-bot check FILE [--known FILE]` | `ladder FILE [--vs FILE] [--seeds N]` | `match A B [--seeds N] [--from N]` |
//! `match A --bar [--seeds N] [--from N]` | `fight A B --seed N [--out LOG]` | `verify LOG A B`: exit 0 on a pass, 1 with the
//! reason on stderr otherwise. A bot of `match` is a module (`.wasm`) or what `pixfx-duel`'s `match` example takes: a
//! weights file, `shipped` or a built-in seat.

use std::error::Error;
use std::process::ExitCode;

use arena_bot::run::{LIMIT, ROUNDS, Verdict, bot, effects, entry, fought, rows, verify};
use arena_bot::{check, engine};
use pixfx_duel::first_mind::FirstMind;
use pixfx_duel::ladder::{Bot, Match, Record, bout, entry_bar, match_up};
use pixfx_duel::log::ActLog;
use pixfx_duel::scripted::Scripted;
use pixfx_duel::seat::Brain;
use wasmtime::Engine;

const USAGE: &str = "usage: arena-bot check FILE [--known FILE] | ladder FILE [--vs FILE] [--seeds N] | match A B [--seeds N] [--from N] | match A --bar [--seeds N] [--from N] | fight A B --seed N [--out LOG] | verify LOG A B";

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

/// The arguments that are neither a flag nor a flag's value (`--bar` has none).
fn files(args: &[String]) -> Vec<&String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
        } else if a.starts_with("--") {
            skip = a != "--bar";
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
            let known: Vec<String> = match flag(rest, "--known") {
                Some(path) => std::fs::read_to_string(path)?
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect(),
                None => Vec::new(),
            };
            match entry(&engine, &bytes, 1..SEEDS + 1, &known) {
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
        (Some("match"), bots @ ([_] | [_, _])) => {
            let seeds: u32 = flag(rest, "--seeds").map_or(Ok(SEEDS), |s| s.parse())?;
            let from: u32 = flag(rest, "--from").map_or(Ok(1), |s| s.parse())?;
            let m = Match {
                seeds: from..from + seeds,
                rounds: ROUNDS,
                limit: LIMIT,
                effects: effects(),
            };
            let bar = rest.iter().any(|a| a == "--bar");
            match (bots, bar) {
                ([a], true) => {
                    let b = entry_bar(&contestant(&engine, a)?, &m);
                    print_record("vs brain", &b.vs_brain);
                    print_record("vs first mind", &b.vs_first);
                    for (name, rate, min) in rows(&b) {
                        println!(
                            "{name:<28} {:>5.1}% (bar {:.0}%)",
                            rate * 100.0,
                            min * 100.0
                        );
                    }
                    if b.passed() {
                        println!("entry bar: PASS");
                        Ok(())
                    } else {
                        Err("entry bar: FAIL".into())
                    }
                }
                ([a, b], false) => {
                    let r = bout(&contestant(&engine, a)?, &contestant(&engine, b)?, &m);
                    print_record(&format!("{a} vs {b}"), &r);
                    Ok(())
                }
                _ => Err(USAGE.into()),
            }
        }
        (Some("fight"), [a, b]) => {
            let seed = flag(rest, "--seed")
                .ok_or("fight needs --seed N")?
                .parse()?;
            let (log, faults) = fought(
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
            let ended: Vec<String> = faults
                .iter()
                .enumerate()
                .filter_map(|(corner, f)| f.as_ref().map(|f| format!("corner {corner}: {f}")))
                .collect();
            if ended.is_empty() {
                Ok(())
            } else {
                Err(format!("the fight ended by a fault, {}", ended.join("; ")).into())
            }
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

/// The bot `spec` names: a module (`.wasm`, checked), or a weights file, `shipped` or a built-in seat.
fn contestant(engine: &Engine, spec: &str) -> Result<Bot, Box<dyn Error>> {
    if spec.ends_with(".wasm") {
        let module =
            check(engine, &std::fs::read(spec)?).map_err(|why| format!("{spec}: {why}"))?;
        return Ok(bot(engine, &module));
    }
    Ok(Bot::of(spec)?)
}

/// `r`, from the first bot's side, as `pixfx-duel`'s `match` example prints it, with the faults.
fn print_record(name: &str, r: &Record) {
    let pct = |x: f64| format!("{:.1}%", x * 100.0);
    println!(
        "{name}: won {} (as Thunder {}, as Frost {}), {} of {} rounds; mean round {:.1} s; {} fights cut; faults {}/{}",
        pct(r.win_rate()),
        pct(r.as_school(0)),
        pct(r.as_school(1)),
        r.won[0] + r.won[1],
        r.won[0] + r.won[1] + r.lost[0] + r.lost[1],
        r.mean_length(),
        r.timeouts,
        r.faults[0],
        r.faults[1]
    );
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
    let me = bot(engine, &module);
    let balanced = || me.balanced();
    let drawn = || me.maker();
    let m = Match {
        seeds: 1..seeds + 1,
        rounds: ROUNDS,
        limit: LIMIT,
        effects: effects(),
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
            format!("{}/{}", r.faults[0], r.faults[1]),
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
        let theirs = bot(engine, &check(engine, &std::fs::read(&other)?)?);
        row(
            &format!("balanced vs {other}"),
            match_up(&balanced, &|| theirs.balanced(), &m),
        );
    }
    Ok(())
}

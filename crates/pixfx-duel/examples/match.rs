//! Two bots fight a match and the first one's record is printed: each a weights file or a built-in seat
//! (`brain`, `first`, `aggressive`, `turtle`, or `shipped` for the trained policy in `weights/policy.bin`).
//! Every seed is fought both ways, so neither school's edge decides it. With `--bar`, the first bot is measured
//! against the tournament's entry bar instead. A bot module (`.wasm`) runs in wasmtime: `arena-bot match`.
//!
//! `cargo run --release -p pixfx-duel --example match -- A [B] [--seeds 150] [--from 1] [--rounds 3] [--limit 120] [--bar]`

use pixfx_duel::ladder::{Bot, EntryBar, Match, Record, bout, entry_bar};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let seeds: u32 = arg("--seeds").map_or(Ok(150), |s| s.parse())?;
    let from: u32 = arg("--from").map_or(Ok(1), |s| s.parse())?;
    let rounds: u32 = arg("--rounds").map_or(Ok(3), |s| s.parse())?;
    let limit: f64 = arg("--limit").map_or(Ok(120.0), |s| s.parse())?;
    let bar = args.iter().any(|a| a == "--bar");
    // the bots: every argument that is neither a flag nor a flag's value
    let mut bots = Vec::new();
    let mut i = 0;
    while let Some(a) = args.get(i) {
        if a.starts_with("--") {
            i += if a == "--bar" { 1 } else { 2 };
        } else {
            bots.push(a.clone());
            i += 1;
        }
    }
    let m = Match {
        seeds: from..from + seeds,
        rounds,
        limit,
        effects: pixfx_effects::EFFECTS
            .iter()
            .map(|e| e.meta().id.to_string())
            .collect(),
    };
    println!(
        "seeds {}..={}, both ways, {rounds} rounds, rounds cut at {limit} s",
        from,
        from + seeds - 1
    );
    match (bots.as_slice(), bar) {
        ([a], true) => {
            let b = entry_bar(&Bot::of(a)?, &m);
            print_record("vs brain", &b.vs_brain);
            print_record("vs first mind", &b.vs_first);
            println!(
                "entry bar (vs brain >= {:.0}%, >= {:.0}% as each school; vs first mind >= {:.0}%): {}",
                EntryBar::BRAIN * 100.0,
                EntryBar::BRAIN_EACH_SCHOOL * 100.0,
                EntryBar::FIRST * 100.0,
                if b.passed() { "PASS" } else { "FAIL" }
            );
            if !b.passed() {
                std::process::exit(1);
            }
        }
        ([a, b], false) => print_record(
            &format!("{a} vs {b}"),
            &bout(&Bot::of(a)?, &Bot::of(b)?, &m),
        ),
        _ => return Err("usage: match A B [flags] | match A --bar [flags]".into()),
    }
    Ok(())
}

fn print_record(name: &str, r: &Record) {
    let pct = |x: f64| format!("{:.1}%", x * 100.0);
    println!(
        "{name}: won {} (as Thunder {}, as Frost {}), {} of {} rounds; mean round {:.1} s; {} fights cut",
        pct(r.win_rate()),
        pct(r.as_school(0)),
        pct(r.as_school(1)),
        r.won[0] + r.won[1],
        r.won[0] + r.won[1] + r.lost[0] + r.lost[1],
        r.mean_length(),
        r.timeouts
    );
}

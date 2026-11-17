//! The submission checks of `docs/tournament.md` on a bot module, as the arena runs them: it is admitted, it answers
//! the ABI, it plays fixed decisions twice the same within its fuel, and it clears the entry bar.
//!
//! `cargo run --release -p arena-bot --features host --example check -- BOT.wasm [--seeds 150] [--from 1]`

use arena_bot::host::Runner;
use arena_bot::{ABI, ACTIONS, DECIDE_FUEL, OBS, admit};
use arena_duel::ladder::{Bot, EntryBar, Match, Record, entry_bar_of};
use arena_duel::seat::Decider;
use serde_json::Value;
use std::rc::Rc;

/// Observations from real fights and their masks (`train/export.py` wrote them).
const FIXED: &str = include_str!("../../arena-duel/tests/fixtures/policy_check.json");

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
    let Some(path) = args.first().filter(|a| !a.starts_with("--")) else {
        return Err("usage: check BOT.wasm [--seeds 150] [--from 1]".into());
    };

    // 1. admitted
    let bytes = std::fs::read(path)?;
    let admitted = admit(&bytes).map_err(|e| format!("refused: {e}"))?;
    println!("1. admitted: {} bytes, hash {}", bytes.len(), admitted.hash);
    let runner = Runner::new(&admitted).map_err(|e| format!("refused: {}", e.0))?;

    // 2. the ABI
    let mut bot = runner
        .instance()
        .map_err(|e| format!("refused: it does not start: {}", e.0))?;
    let abi = bot
        .abi()
        .map_err(|e| format!("refused: arena_abi: {}", e.0))?;
    if abi != ABI {
        return Err(format!("refused: arena_abi() is {abi}, not {ABI}").into());
    }
    println!("2. arena_abi() = {ABI}");

    // 3. fixed decisions, twice, each in a fresh instance
    let cases = fixed()?;
    let play = || -> Result<(Vec<i32>, i64), String> {
        let mut bot = runner.instance().map_err(|e| e.0)?;
        bot.start(1).map_err(|e| format!("arena_start: {}", e.0))?;
        bot.round().map_err(|e| format!("arena_round: {}", e.0))?;
        let acts = cases
            .iter()
            .enumerate()
            .map(|(k, (obs, legal))| {
                let u = (k as f32 + 0.5) / cases.len() as f32;
                bot.decide(obs, legal, u)
                    .map_err(|e| format!("decision {k}: {}", e.0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((acts, bot.most_fuel))
    };
    let (a, fuel) = play().map_err(|e| format!("refused: {e}"))?;
    let (b, _) = play().map_err(|e| format!("refused: {e}"))?;
    if a != b {
        return Err(format!("refused: the same decisions gave {a:?}, then {b:?}").into());
    }
    let illegal = a
        .iter()
        .zip(&cases)
        .filter(|(act, (_, legal))| !usize::try_from(**act).is_ok_and(|i| i < ACTIONS && legal[i]))
        .count();
    println!(
        "3. {} fixed decisions twice, the same acts; at most {fuel} fuel a decision ({:.1}% of {DECIDE_FUEL}); {illegal} not legal (played as a wait)",
        cases.len(),
        100.0 * fuel as f64 / DECIDE_FUEL as f64
    );

    // 4. the entry bar
    let m = Match {
        seeds: from..from + seeds,
        rounds: 3,
        limit: 120.0,
        effects: arena_duel::effects::all(),
    };
    println!(
        "4. the entry bar: seeds {from}..={}, both ways, 3 rounds",
        from + seeds - 1
    );
    let r = runner.clone();
    let bar = entry_bar_of(&Bot::Decider(Rc::new(move || r.decider())), &m);
    print_record("vs brain", &bar.vs_brain);
    print_record("vs first mind", &bar.vs_first);
    let passed = bar.passed();
    println!(
        "entry bar (vs brain >= {:.0}%, >= {:.0}% in each seat; vs first mind >= {:.0}%): {}",
        EntryBar::BRAIN * 100.0,
        EntryBar::BRAIN_EACH_SEAT * 100.0,
        EntryBar::FIRST * 100.0,
        if passed { "PASS" } else { "FAIL" }
    );
    if !passed {
        std::process::exit(1);
    }
    Ok(())
}

/// A decision: what the bot sees and what it may do.
type Case = ([f32; OBS], [bool; ACTIONS]);

fn fixed() -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let check: Value = serde_json::from_str(FIXED)?;
    let mut out = Vec::new();
    for case in check["cases"].as_array().cloned().unwrap_or_default() {
        let mut obs = [0.0_f32; OBS];
        let mut legal = [false; ACTIONS];
        for (o, v) in obs
            .iter_mut()
            .zip(case["obs"].as_array().cloned().unwrap_or_default())
        {
            *o = v.as_f64().unwrap_or(0.0) as f32;
        }
        for (l, v) in legal
            .iter_mut()
            .zip(case["mask"].as_array().cloned().unwrap_or_default())
        {
            *l = v.as_bool().unwrap_or(false);
        }
        out.push((obs, legal));
    }
    Ok(out)
}

fn print_record(name: &str, r: &Record) {
    let pct = |x: f64| format!("{:.1}%", x * 100.0);
    println!(
        "   {name}: won {} (in seat 0 {}, in seat 1 {}), {} of {} rounds; mean round {:.1} s; {} fights cut",
        pct(r.win_rate()),
        pct(r.as_seat(0)),
        pct(r.as_seat(1)),
        r.won[0] + r.won[1],
        r.won[0] + r.won[1] + r.lost[0] + r.lost[1],
        r.mean_length(),
        r.timeouts
    );
}

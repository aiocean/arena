//! Fights matches of two brains with the duel observer on: what a live match sends, without the relay.
//!
//! ```text
//! ARENA_TELEMETRY_DIR=/tmp/arena-tel cargo run --release -p arena-telemetry --example observe -- [--seeds 3] [--wins 2] [--limit 120]
//! ```

use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_telemetry::duel::Observer;
use arena_telemetry::{Options, init};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str, default: f64| -> Result<f64, String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map_or(Ok(default), |v| {
                v.parse().map_err(|_| format!("{name}: not a number"))
            })
    };
    let (seeds, wins, limit) = (
        arg("--seeds", 3.0)? as u32,
        arg("--wins", 2.0)? as u32,
        arg("--limit", 120.0)?,
    );
    let _telemetry = init(
        "arena-ladder",
        Options {
            version: env!("CARGO_PKG_VERSION"),
            ..Options::default()
        },
    );
    for seed in 1..=seeds {
        let config = DuelConfig {
            seed,
            left: 88.0,
            right: 1192.0,
            ground: 632.0,
            sky: 196.0,
            effects: arena_duel::effects::all(),
        };
        let mut duel = Duel::with_brains(config);
        let mut watch = Observer::start(&duel, format!("observe-{seed}"), ["brain", "brain"]);
        let mut cut = false;
        while duel.state().wins.iter().all(|w| *w < wins) {
            watch.step(&mut duel, TICK);
            if duel.state().time > limit * f64::from(wins * 2) {
                cut = true;
                break;
            }
        }
        let s = duel.state();
        tracing::info!(
            seed,
            wins_0 = s.wins[0],
            wins_1 = s.wins[1],
            seconds = s.time,
            cut,
            "observed match"
        );
        watch.end(&duel, cut);
    }
    Ok(())
}

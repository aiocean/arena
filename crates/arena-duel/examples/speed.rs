//! Ticks a second of brain-vs-brain duels: `cargo run --release -p arena-duel --example speed`.

use arena_duel::duel::{Duel, DuelConfig, TICK};
use std::time::Instant;

fn main() {
    let start = Instant::now();
    let mut ticks = 0u64;
    for seed in 1..=40 {
        let mut duel = Duel::with_brains(DuelConfig {
            seed,
            left: 0.0,
            right: 880.0,
            ground: 600.0,
            sky: 384.0,
            effects: arena_duel::effects::all(),
        });
        for _ in 0..3600 {
            duel.step(TICK);
            ticks += 1;
        }
    }
    let s = start.elapsed().as_secs_f64();
    println!(
        "{ticks} ticks in {s:.2} s: {:.0} ticks/s, {:.2} us a tick",
        ticks as f64 / s,
        s * 1e6 / ticks as f64
    );
}

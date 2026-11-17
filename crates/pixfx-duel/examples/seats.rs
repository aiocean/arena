//! Seat 0 against seat 1: the same mind in both seats on each seed's random loadouts, rounds counted by the seat that
//! won them. With identical seats and loadouts drawn alike, seat 0 should win about half.
//!
//! `cargo run -p pixfx-duel --example seats -- [--seeds N] [--rounds R] [--mind first|brain|aggressive|turtle]`

use pixfx_duel::duel::{Duel, DuelConfig, TICK};
use pixfx_duel::first_mind::FirstMind;
use pixfx_duel::scripted::Scripted;
use pixfx_duel::seat::{Brain, MakeSeat};

fn make(mind: &str) -> MakeSeat {
    match mind {
        "brain" => Brain::make(),
        "aggressive" => Scripted::aggressive(),
        "turtle" => Scripted::turtle(),
        _ => FirstMind::make(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let seeds: u32 = arg("--seeds").and_then(|v| v.parse().ok()).unwrap_or(150);
    let rounds: u32 = arg("--rounds").and_then(|v| v.parse().ok()).unwrap_or(3);
    let mind = arg("--mind").unwrap_or_else(|| "first".to_string());
    // a round past this many seconds is cut and counts for neither seat
    let limit = 120.0;
    let (mut wins, mut cut, mut seconds, mut ended) = ([0_u32; 2], 0_u32, 0.0, 0_u32);
    for seed in 1..=seeds {
        let config = DuelConfig {
            seed,
            left: 0.0,
            right: 880.0,
            ground: 600.0,
            sky: 384.0,
            effects: pixfx_duel::effects::all(),
        };
        let mut duel = Duel::new(config, [make(&mind), make(&mind)]);
        for _ in 0..rounds {
            let round = duel.state().round;
            let start = duel.state().time;
            while duel.state().winner.is_none() && duel.state().time - start < limit {
                duel.step(TICK);
            }
            match duel.state().winner {
                Some(w) => {
                    wins[w] += 1;
                    seconds += duel.state().time - start;
                    ended += 1;
                }
                None => {
                    cut += 1;
                    break;
                }
            }
            while duel.state().round == round {
                duel.step(TICK);
            }
        }
    }
    let total = f64::from(wins[0] + wins[1]).max(1.0);
    println!(
        "{mind} mirror, {seeds} seeds, random loadouts: seat 0 won {} rounds, seat 1 {} ({:.1}% seat 0); {cut} cut; mean round {:.1} s",
        wins[0],
        wins[1],
        100.0 * f64::from(wins[0]) / total,
        seconds / f64::from(ended.max(1)),
    );
}

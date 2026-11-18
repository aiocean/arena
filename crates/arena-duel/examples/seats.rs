//! Seat 0 against seat 1: the same mind in both seats on each seed's random loadouts, rounds counted by the seat that
//! won them and by the look (Vương Lâm or Thất Thải Đạo Nhân) that won them. Each seed is fought twice, the looks swapped
//! between the seats, so a seat's edge and a look's edge are told apart: with fair rules and fair art, seat 0 and
//! Vương Lâm each win about half.
//!
//! `cargo run -p arena-duel --example seats -- [--seeds N] [--rounds R] [--mind first|brain|aggressive|turtle]`

use arena_duel::data::Look;
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::first_mind::FirstMind;
use arena_duel::scripted::Scripted;
use arena_duel::seat::{Brain, MakeSeat};

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
    // rounds won by seat 0, by Vương Lâm, and decided
    let (mut seat0, mut vuong_lam, mut decided) = (0_u32, 0_u32, 0_u32);
    let (mut cut, mut seconds) = (0_u32, 0.0);
    for seed in 1..=seeds {
        for looks in [
            [Look::VuongLam, Look::ThatThai],
            [Look::ThatThai, Look::VuongLam],
        ] {
            let config = DuelConfig {
                seed,
                left: 0.0,
                right: 880.0,
                ground: 600.0,
                sky: 384.0,
                effects: arena_duel::effects::all(),
            };
            let mut duel = Duel::new_with_looks(config, [make(&mind), make(&mind)], looks);
            for _ in 0..rounds {
                let round = duel.state().round;
                let start = duel.state().time;
                while duel.state().winner.is_none() && duel.state().time - start < limit {
                    duel.step(TICK);
                }
                match duel.state().winner {
                    Some(w) => {
                        decided += 1;
                        seat0 += u32::from(w == 0);
                        vuong_lam += u32::from(looks[w] == Look::VuongLam);
                        seconds += duel.state().time - start;
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
    }
    let share = |n: u32| 100.0 * f64::from(n) / f64::from(decided.max(1));
    println!(
        "{mind} mirror, {seeds} seeds x both looks, random loadouts: {decided} rounds; seat 0 won {:.1}%, Vương Lâm won {:.1}%; {cut} cut; mean round {:.1} s",
        share(seat0),
        share(vuong_lam),
        seconds / f64::from(decided.max(1)),
    );
}

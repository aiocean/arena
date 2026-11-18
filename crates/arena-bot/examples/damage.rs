//! Where a bot's damage comes from: the share of the life it took from its foes by the skill whose blow struck
//! (`Out::Hit`), by its bleeds and burns (`Note::Ticked`), and the rest (curses, chip through a guard, fields: what
//! the foe lost that neither of those names).
//!
//! `cargo run --release -p arena-bot --features host,onnx --example damage -- [--seeds 150] [--rounds 3]
//! [--limit 120] [--onnx FILE]`
//!
//! It fights the house AI (or `--onnx`'s model in its place) in its own mirror, styles drawn, and against the brain
//! at the balanced style, each seed both ways and in the looks `ladder::looks_of` draws.

use arena_bot::onnx::Onnx;
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::ladder::looks_of;
use arena_duel::out::{Note, Out};
use arena_duel::seat::{BotSeat, Brain, MakeSeat};
use arena_duel::style::Style;
use std::collections::BTreeMap;

/// One striker's damage by source, in % of a foe's life.
#[derive(Default)]
struct Tally {
    by: BTreeMap<String, f64>,
    total: f64,
}

impl Tally {
    fn add(&mut self, key: &str, share: f64) {
        *self.by.entry(key.to_string()).or_default() += share;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let seeds: u32 = arg("--seeds").map_or(Ok(150), |s| s.parse())?;
    let rounds: u32 = arg("--rounds").map_or(Ok(3), |s| s.parse())?;
    let limit: f64 = arg("--limit").map_or(Ok(120.0), |s| s.parse())?;
    let onnx = match arg("--onnx") {
        Some(path) => Onnx::load(&std::fs::read(path)?).map_err(|e| e.0)?,
        None => Onnx::house().map_err(|e| e.0)?,
    };
    let house = |style: Option<Style>| -> MakeSeat { BotSeat::make(onnx.decider(), style) };
    let effects = arena_duel::effects::all();
    for (row, mirror) in [("house mirror", true), ("house vs brain", false)] {
        // the house's tally (both seats in the mirror) and its foe's
        let mut tallies = [Tally::default(), Tally::default()];
        for seed in 1..=seeds {
            for corner in 0..2_usize {
                let make = |house_seat: bool| -> MakeSeat {
                    match (mirror, house_seat) {
                        (true, _) => house(None),
                        (false, true) => house(Some(Style::Balanced)),
                        (false, false) => Brain::make(),
                    }
                };
                let seats = if corner == 0 {
                    [make(true), make(false)]
                } else {
                    [make(false), make(true)]
                };
                // which tally each seat's damage goes to
                let side = |seat: usize| usize::from(!mirror && seat != corner);
                let config = DuelConfig {
                    seed,
                    left: 0.0,
                    right: 880.0,
                    ground: 600.0,
                    sky: 384.0,
                    effects: effects.clone(),
                };
                let mut duel = Duel::new_with_looks(config, seats, looks_of(seed + corner as u32));
                let mut started = duel.state().time;
                let mut round = duel.state().round;
                while duel.state().wins[0] + duel.state().wins[1] < rounds {
                    let before = duel.state().fighters.each_ref().map(|f| f.hp());
                    let fighting = duel.state().winner.is_none();
                    let outs = duel.step(TICK);
                    let s = duel.state();
                    if s.round != round {
                        started = s.time;
                        round = s.round;
                    } else if fighting {
                        let mut named = [0.0_f64; 2];
                        for o in &outs {
                            if let Out::Hit {
                                by, spell, share, ..
                            } = o
                            {
                                tallies[side(*by)].add(spell, *share);
                                named[*by] += share;
                            }
                        }
                        for (_, n) in &s.notes {
                            if let Note::Ticked { by, cause, share } = n {
                                tallies[side(*by)].add(cause, *share);
                                named[*by] += share;
                            }
                        }
                        for by in 0..2 {
                            let lost = (before[1 - by] - s.fighters[1 - by].hp()).max(0.0);
                            let t = &mut tallies[side(by)];
                            t.total += lost;
                            t.add("(rest)", lost - named[by]);
                        }
                    }
                    if s.winner.is_none() && s.time - started > limit {
                        break;
                    }
                }
            }
        }
        let names: &[&str] = if mirror {
            &["house"]
        } else {
            &["house", "brain"]
        };
        for (name, t) in names.iter().zip(&tallies) {
            let share = |k: &str| 100.0 * t.by.get(k).copied().unwrap_or(0.0) / t.total.max(1e-9);
            let blades = share("dive") + share("slash");
            println!(
                "{row:<15} {name:<6} dealt {:>8.0}% of a life over the fights: dive {:.1}%, slash {:.1}%, bleed {:.1}% \
                 (dive+slash+bleed {:.1}%), burn {:.1}%, rest {:.1}%",
                t.total,
                share("dive"),
                share("slash"),
                share("bleed"),
                blades + share("bleed"),
                share("burn"),
                share("(rest)")
            );
            let mut top: Vec<(&String, &f64)> =
                t.by.iter()
                    .filter(|(k, _)| {
                        !["dive", "slash", "bleed", "burn", "(rest)"].contains(&k.as_str())
                    })
                    .collect();
            top.sort_by(|a, b| b.1.total_cmp(a.1));
            let top: Vec<String> = top
                .iter()
                .take(6)
                .map(|(k, v)| format!("{k} {:.1}%", 100.0 * **v / t.total.max(1e-9)))
                .collect();
            println!("{:<22} other skills: {}", "", top.join(", "));
        }
    }
    Ok(())
}

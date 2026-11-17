//! One match, logged: casts, hits with their damage by kind, ailments laid, controls, deaths and each round's end.
//!
//! ```text
//! cargo run --release -p arena-sim --example duel -- <seed> [--loadout-a <json or file>] [--loadout-b <json or file>]
//! ```
//!
//! A loadout not given is drawn at random (`loadout::random`) from the seed. A loadout is `arena_calc::loadout::
//! Loadout` as JSON, inline or in a file: `{"actives":[{"skill":"fireball","supports":[],"fuse":null}, ...],
//! "ultimate":"tribulation","keystones":[],"root":null}`.

use arena_calc::loadout::{self, Loadout};
use arena_sim::fight::Fight;
use arena_sim::rng::Rng;

fn read_loadout(arg: &str) -> Result<Loadout, String> {
    let text = if arg.trim_start().starts_with('{') {
        arg.to_string()
    } else {
        std::fs::read_to_string(arg).map_err(|e| format!("{arg}: {e}"))?
    };
    let l: Loadout = serde_json::from_str(&text).map_err(|e| format!("{arg}: {e}"))?;
    loadout::check(&l).map_err(|e| format!("illegal loadout: {e:?}"))?;
    Ok(l)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed = 1u64;
    let mut given: [Option<Loadout>; 2] = [None, None];
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--loadout-a" | "--loadout-b" => {
                let v = args.get(i + 1).ok_or("a loadout flag needs a value")?;
                let side = usize::from(args[i] == "--loadout-b");
                given[side] = Some(read_loadout(v)?);
                i += 2;
            }
            s => {
                seed = s.parse().map_err(|_| format!("not a seed: {s}"))?;
                i += 1;
            }
        }
    }
    let mut dice = Rng::salted(seed, 99);
    let mut draw = || loadout::random(&mut || dice.next_f64()).ok_or("no legal loadout");
    let a = match given[0].take() {
        Some(l) => l,
        None => draw()?,
    };
    let b = match given[1].take() {
        Some(l) => l,
        None => draw()?,
    };
    for (name, l) in [("A", &a), ("B", &b)] {
        let skills: Vec<String> = l
            .actives
            .iter()
            .map(|s| {
                if s.supports.is_empty() {
                    s.skill.clone()
                } else {
                    format!("{}[{}]", s.skill, s.supports.join("+"))
                }
            })
            .collect();
        println!(
            "{name}: {} | ult {} | keystones {:?} | root {:?}",
            skills.join(", "),
            l.ultimate,
            l.keystones,
            l.root
        );
    }
    let m = Fight::new(seed, &a, &b, true).play();
    for line in &m.log {
        println!("{line}");
    }
    println!(
        "match: {}",
        m.winner
            .map_or("drawn".to_string(), |w| format!("{} wins", ["A", "B"][w]))
    );
    for (name, side) in [("A", &m.sides[0]), ("B", &m.sides[1])] {
        println!(
            "{name}: dealt {:.0} ({:.0} over time)",
            side.damage, side.dot
        );
        for s in side.skills.iter().filter(|s| s.used()) {
            println!(
                "  {:<18} casts {:>3} (+{} triggered) landed {:>3} missed {:>3} damage {:>7.0}",
                s.id, s.casts, s.triggered, s.landed, s.missed, s.damage
            );
        }
    }
    Ok(())
}

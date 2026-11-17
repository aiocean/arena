//! One match, logged: casts, hits with their damage by kind, ailments laid, controls, deaths and each round's end.
//!
//! ```text
//! cargo run --release -p arena-sim --example duel -- <seed> [--loadout-a <json or file>] [--loadout-b <json or file>] [--telemetry-otlp]
//! ```
//!
//! `--telemetry-otlp` (or `ARENA_OTLP_ENDPOINT` set) sends the match's summary, each round and each used skill's
//! line to the telemetry store as `arena_ladder` events (`ops/observe/README.md`).
//!
//! A loadout not given is drawn at random (`loadout::random`) from the seed. A loadout is `arena_calc::loadout::
//! Loadout` as JSON, inline or in a file: `{"actives":[{"skill":"fireball","supports":[],"fuse":null}, ...],
//! "keystones":[],"root":null}`.

use arena_calc::loadout::{self, Loadout};
use arena_sim::fight::{Fight, MatchResult};
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
    let _export = arena_telemetry::requested(&args)?.then(|| {
        arena_telemetry::init(
            "arena-ladder",
            arena_telemetry::Options::tool(env!("CARGO_PKG_VERSION")),
        )
    });
    let args: Vec<String> = args
        .into_iter()
        .filter(|a| a != arena_telemetry::FLAG)
        .collect();
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
            "{name}: {} | keystones {:?} | root {:?}",
            skills.join(", "),
            l.keystones,
            l.root
        );
    }
    let m = Fight::new(seed, &a, &b, true).play();
    export(seed, [&a, &b], &m);
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

/// The match, its rounds and each used skill as `arena_ladder` events (nothing when telemetry is off).
fn export(seed: u64, loadouts: [&Loadout; 2], m: &MatchResult) {
    let id = format!(
        "sim-{seed}-{}",
        arena_telemetry::run_id("").trim_start_matches('-')
    );
    for (seat, l) in loadouts.iter().enumerate() {
        let (actives, supports, keystones, root) = arena_telemetry::duel::loadout_fields(l);
        let side = &m.sides[seat];
        tracing::info!(target: "arena::ladder", match_id = %id, seed, seat, actives = %actives,
            supports = %supports, keystones = %keystones, root = %root,
            won = m.winner == Some(seat), drawn = m.winner.is_none(), rounds = m.rounds.len(),
            damage = side.damage, dot = side.dot, "sim.match");
        for s in side.skills.iter().filter(|s| s.used()) {
            tracing::info!(target: "arena::ladder", match_id = %id, seed, seat, skill = s.id, casts = s.casts,
                triggered = s.triggered, landed = s.landed, missed = s.missed, damage = s.damage,
                unit_damage = s.unit_damage, won = m.winner == Some(seat), "sim.skill");
        }
    }
    for (i, r) in m.rounds.iter().enumerate() {
        tracing::info!(target: "arena::ladder", match_id = %id, seed, round = i + 1,
            winner = r.winner.map_or(-1, |w| w as i64), cut = r.cut, seconds = r.seconds, life_0 = r.life[0],
            life_1 = r.life[1], "sim.round");
    }
}

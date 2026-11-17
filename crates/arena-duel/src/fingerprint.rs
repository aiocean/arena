//! A fight's fingerprint: every command and event of a duel of two brains on the landing arena, hashed. The goldens pin
//! it natively (`tests/goldens.rs`, `goldens/event_hashes.json`), and `examples/wasm_hashes.rs` works out the same
//! hashes in WebAssembly, the build a page runs: the two agreeing is the proof that a page replays, bit for bit, a
//! match the server fought natively.

use crate::duel::{Duel, DuelConfig};
use crate::js::js_round;
use core::fmt::Write as _;

/// The landing arena (0 to 880, ground 600, sky 384) with every effect in the registry, for seed `seed`.
pub fn landing(seed: u32) -> DuelConfig {
    DuelConfig {
        seed,
        left: 0.0,
        right: 880.0,
        ground: 600.0,
        sky: 384.0,
        effects: crate::effects::all(),
    }
}

/// FNV-1a over `bytes`.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The hash of every command and event of `seconds` of seed `seed`'s duel of two brains, stepped at 60 Hz, each
/// output written on a line as `Debug` writes it.
pub fn event_hash(seed: u32, seconds: f64) -> u64 {
    let mut duel = Duel::with_brains(landing(seed));
    let mut log = String::new();
    for _ in 0..js_round(seconds * 60.0) as usize {
        for o in duel.step(1.0 / 60.0) {
            let _ = writeln!(log, "{o:?}");
        }
    }
    fnv1a(log.as_bytes())
}

/// Seeds `1..=seeds`' hashes of `seconds` each, as `goldens/event_hashes.json` holds them.
pub fn event_hashes_json(seeds: u32, seconds: f64) -> String {
    let mut out = String::from("{\n");
    for seed in 1..=seeds {
        let comma = if seed < seeds { "," } else { "" };
        let _ = writeln!(
            out,
            "  \"{seed}\": \"{:016x}\"{comma}",
            event_hash(seed, seconds)
        );
    }
    out.push_str("}\n");
    out
}

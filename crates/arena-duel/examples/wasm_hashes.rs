//! The fights' fingerprints worked out in the browser's WebAssembly (`wasm32-unknown-unknown`, the build a page runs),
//! for the native = wasm check: `tests/wasm/native_wasm.mjs` runs this under node and holds its hashes to
//! `tests/goldens/event_hashes.json`, which the native tests pin on Linux and macOS. The page replays the matches the
//! server fights natively, so the two must agree bit for bit.
//!
//! ```text
//! cargo build --release -p arena-duel --example wasm_hashes --target wasm32-unknown-unknown
//! wasm-bindgen --target nodejs --out-dir target/wasm-hashes target/wasm32-unknown-unknown/release/examples/wasm_hashes.wasm
//! node crates/arena-duel/tests/wasm/native_wasm.mjs target/wasm-hashes
//! ```

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

/// Seeds `1..=seeds`' fingerprints of `seconds` each, as `goldens/event_hashes.json` holds them.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn event_hashes(seeds: u32, seconds: f64) -> String {
    arena_duel::fingerprint::event_hashes_json(seeds, seconds)
}

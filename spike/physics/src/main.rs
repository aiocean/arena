//! The physics spike (`docs/specs/2026-11-18-physics-design.md`): each backend's hash over every query of seeds
//! `1..=seeds` (`seconds` each), its cost per tick and its answers that differ from the rules' hand-written ones.
//!
//! ```text
//! cargo run --release --features parry,rapier,rapier32,avian -- 20 120
//! cargo build --release --lib --features parry,rapier,avian --target wasm32-unknown-unknown
//! wasm-bindgen --target nodejs --out-dir pkg target/wasm32-unknown-unknown/release/physics_spike.wasm
//! node run.mjs $PWD/pkg 20 120      # the same hashes in wasm32
//! ```

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seeds = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let secs = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(120.0);
    print!("{}", physics_spike::report(seeds, secs));
}

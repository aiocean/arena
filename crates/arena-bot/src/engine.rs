use wasmtime::{Config, Engine};

use crate::Refusal;

/// What a bot may spend.
pub struct Limits {
    /// Bytes of module.
    pub module: usize,
    /// Bytes of every memory together.
    pub memory: usize,
    /// Elements of every table together.
    pub table_elements: usize,
    /// Fuel per `arena_decide`.
    pub fuel_decide: u64,
    /// Fuel per `arena_start`, `arena_round` and instantiation.
    pub fuel_setup: u64,
}

pub const LIMITS: Limits = Limits {
    module: 32 << 20,
    memory: 256 << 20,
    table_elements: 1 << 20,
    fuel_decide: 20_000_000,
    fuel_setup: 200_000_000,
};

/// The season's engine: the accepted features on, GC off, one result for every relaxed SIMD instruction and every NaN,
/// fuel counted. Threads are off because wasmtime's `threads` feature is not built.
pub fn engine() -> Result<Engine, Refusal> {
    let mut config = Config::new();
    config
        .relaxed_simd_deterministic(true)
        .cranelift_nan_canonicalization(true)
        .consume_fuel(true)
        .wasm_simd(true)
        .wasm_relaxed_simd(true)
        .wasm_tail_call(true)
        .wasm_exceptions(true)
        .wasm_function_references(true)
        .wasm_extended_const(true)
        .wasm_memory64(true)
        .wasm_multi_memory(true)
        .wasm_wide_arithmetic(true)
        .wasm_gc(false);
    Engine::new(&config).map_err(|e| Refusal::Invalid(format!("{e:#}")))
}

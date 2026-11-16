//! Checks a bot's WebAssembly module (ABI `arena-bot-1`) and runs it as a `Decider` in the season's wasmtime.

mod bot;
mod check;
mod engine;
pub mod run;

pub use bot::WasmBot;
pub use check::{Refusal, check, hash};
pub use engine::{LIMITS, Limits, engine};

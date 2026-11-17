//! Builds a bot module from an ONNX policy with the `bots/onnx` template, then admits it as a submission would.
//!
//! `cargo run -p arena-bot --example onnx -- MODEL.onnx BOT.wasm`
//!
//! The model takes the observation, float32 `[1, 354]`, and gives the 30 acts' logits (`docs/bot.md`). Building
//! needs the repo's toolchain, which has the `wasm32-unknown-unknown` target.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, out] = args.as_slice() else {
        return Err("usage: onnx MODEL.onnx BOT.wasm".into());
    };
    let model = std::fs::canonicalize(model).map_err(|e| format!("{model}: {e}"))?;
    let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bots/onnx");
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(&template)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .env("ARENA_ONNX", &model)
        // the outer cargo's settings are for the host, not the module
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTFLAGS")
        .status()?;
    if !status.success() {
        return Err("building the bot failed".into());
    }
    let built: PathBuf = template.join("target/wasm32-unknown-unknown/release/onnx_bot.wasm");
    let bytes = std::fs::read(&built)?;
    std::fs::write(out, &bytes)?;
    match arena_bot::admit(&bytes) {
        Ok(a) => println!("{out}: {} bytes, admitted, hash {}", bytes.len(), a.hash),
        Err(e) => return Err(format!("{out} is built but would be refused: {e}").into()),
    }
    println!("next: cargo run --release -p arena-bot --features host --example check -- {out}");
    Ok(())
}

//! Embeds the model named by `ARENA_ONNX` (`cargo run -p arena-bot --example onnx -- MODEL.onnx BOT.wasm` sets it).

fn main() {
    println!("cargo::rerun-if-env-changed=ARENA_ONNX");
    let out =
        std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("model.onnx");
    match std::env::var("ARENA_ONNX") {
        Ok(path) => {
            println!("cargo::rerun-if-changed={path}");
            if let Err(e) = std::fs::copy(&path, &out) {
                println!("cargo::error=ARENA_ONNX={path}: {e}");
            }
        }
        Err(_) => println!("cargo::error=set ARENA_ONNX to the .onnx file to build into the bot"),
    }
}

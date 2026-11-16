#!/bin/sh
# Builds the mlp bot for wasm32 with SIMD and copies it to the house bot's place. Run from anywhere.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
RUSTFLAGS="-C target-feature=+simd128" cargo build --release --target wasm32-unknown-unknown --manifest-path "$here/Cargo.toml"
cp "$here/target/wasm32-unknown-unknown/release/mlp.wasm" "$here/../../crates/pixfx-duel/weights/house.wasm"
ls -l "$here/../../crates/pixfx-duel/weights/house.wasm"

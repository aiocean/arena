# onnx-bot

A bot module of the `arena-bot-1` ABI (`docs/bot.md`, "The bot module") that runs an ONNX model with
[tract](https://github.com/sonos/tract). The model takes the observation, float32 `[1, 354]`, and gives the 30 acts'
logits, float32 `[1, 30]`; the bot samples the act from the legal ones with the arena's draw, as the shipped bot does.

```sh
cargo run -p arena-bot --example onnx -- MODEL.onnx BOT.wasm          # from the repo's root
cargo run --release -p arena-bot --features host --example check -- BOT.wasm
```

The first builds this crate for `wasm32-unknown-unknown` with `ARENA_ONNX=MODEL.onnx` (the model is embedded) and
admits the result as a submission would; the second runs the submission checks and the entry bar.

It is its own workspace, built only for wasm32. Two things keep the module free of imports: getrandom's `custom`
source that always fails (tract reaches getrandom; its `js` source would import from JavaScript), and the input
buffer as atomics, so there is no `unsafe`. Change `arena_decide` in `src/lib.rs` to pick an act another way.

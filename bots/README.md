# Bots

A bot is a WebAssembly module speaking ABI `arena-bot-1` (`crates/arena-bot`: `check` says what it accepts). Each
folder here is a template or a bot, a crate of its own workspace built for `wasm32-unknown-unknown`.

## mlp

A multilayer perceptron over the observation, sampled as the arena's policy samples: the shipped net's architecture
around the weights in `mlp/src/lib.rs` (`WEIGHTS`). Replace the file with your own export from `train/export.py` to
field your net.

The **house bot** is `mlp` around the shipped `policy.bin`; it plays the shipped policy's fight bit for bit
(`crates/arena-bot/tests/house.rs`). Rebuild it after a new `policy.bin`:

```
bots/mlp/build.sh    # writes crates/pixfx-duel/weights/house.wasm
```

`build.sh` builds with `-C target-feature=+simd128`, which the season's engine accepts.

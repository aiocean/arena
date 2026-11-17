# arena-duel-py

For the trainer. Read the root `AGENTS.md` ("Where code goes") before changing anything here; this file only says what this crate holds. The design is
`docs/specs/2026-11-15-duel-rules-in-rust-design.md` (Part 2, "Training").

## Job

`import arena_duel` for the PPO trainer in `train/`: `VecEnv(n, threads)` runs `n` training episodes
(`arena_duel::train::Env`) on worker threads and hands numpy arrays back: `set_model(id, bytes)` for `policy`
opponents (an ONNX model's bytes, run natively; no wasm host here, so the build stays light), `reset(i, seed, learner, style, foe_style,
opponent, model_id, limit)`, `observe()` →
`(obs float32[n, OBS], mask bool[n, ACTIONS])`, `step(actions int64[n], shaping)` →
`(reward, done, won, length)`; `bout`, `entry_bar`; `OBS`, `ACTIONS`, `HOUSE` (the house AI's ONNX bytes) and `BOTS` (the built-in seats' names). Each env is built and stepped on one thread (it holds `Rc`s
and seats); only plain arrays cross threads.

## Belongs here

- Translating `arena_duel::train` to Python and spreading envs over threads.

## Does not belong here

| If the code... | It goes to |
|---|---|
| decides a reward, an episode's end, an opponent's play | `arena-duel` (`train`, `seat`, `scripted`) |
| encodes what the policy sees or may do | `arena-mind` (`policy`) |
| is the trainer | `train/` |

## Dependencies

`pyo3`, `numpy`, `arena-duel`, `arena-mind`, `arena-bot` (`onnx` only: a bot's model). It is its own workspace
(excluded from the root), so pyo3 never builds with the rest.

## Traps

- `unsafe_code` is `deny`, not `forbid`: pyo3's macros emit `unsafe` marked `allow`. This crate's code has none.
- The `#[pymodule]` function must not be named `arena_duel` (it would shadow the crate); its Python name is set
  with `#[pyo3(name = "arena_duel")]`.

## Before you finish

```
cd train && uv run maturin develop --release -m ../crates/arena-duel-py/Cargo.toml && uv run pytest
PYO3_PYTHON=$PWD/train/.venv/bin/python cargo clippy --manifest-path crates/arena-duel-py/Cargo.toml -- -D warnings
```

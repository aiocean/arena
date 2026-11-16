# Bots as WASM modules: design

Status: approved 2026-11-16. Nothing here is built yet.

## Intent

A player owns their whole bot: the model, its size, its memory, how it picks an act, and how it was trained. The
arena fixes only the game: what a bot sees, what it can do, when it decides, and how much it may compute per
decision. A bot is one WebAssembly module. The arena runs every official fight natively, in one pinned wasmtime in
its deterministic mode, and publishes each fight's act log; the page plays fights from those logs, so every device
can watch, and anyone can check a fight by running it again with the arena's CLI.

## What exists today

- A bot is a weights file for one fixed network: about 198K parameters, MLP, no memory (`docs/bot.md`, "The weights
  file"; `pixfx-mind/src/policy/net.rs`). The arena samples the act from the network's logits (`net::sample`).
- The policy's timing, reaction delay and legality live in `PolicyMind` (`pixfx-mind/src/policy/mind.rs`), split
  so the network can be swapped out: `begin` hands back the act queued last tick, `due` returns the observation,
  the legal mask and the dice draw `u` when a decision is due, and `queue` takes the answer.
- The policy seat (`crates/pixfx-duel/src/seat.rs`, `PolicySeat`) runs `net.logits` + `sample` between `due`
  and `queue`. The wasm `Duel` seats it through `SeatKind::Policy` (`crates/arena-wasm/src/duel.rs:33-51`); the
  trainer seats an opponent policy through `set_weights` (`crates/pixfx-duel-py/src/lib.rs:173`); the ladder through
  `--weights` (`examples/ladder.rs:35`).

## The game stays fixed

The 354 inputs, the 30 acts, the legality rules, the reaction delay and the think clock stay as `docs/bot.md`
describes them. They are the game, the same for every bot. The bot only answers `due`.

## The bot ABI (`arena-bot-1`)

A bot is a WebAssembly 3.0 module, limited to the features under "Determinism". It **imports nothing** and exports:

| Export | Signature | What |
|---|---|---|
| `memory` | memory | its linear memory (memory 0) |
| `arena_abi` | `() -> i32` | returns `1` |
| `arena_input` | `() -> i32` | the address of a buffer the arena writes before each decision: 354 `f32` LE (the observation), then 30 bytes (the legal mask, 0 or 1) |
| `arena_start` | `(seed: i32)` | a new fight; `seed` is the bot's own dice for the fight |
| `arena_round` | `()` | a new round of the same fight |
| `arena_decide` | `(u: f32) -> i32` | the act index, 0..30; `u` is the seat's dice draw in [0, 1) |

- One instance per fight (one seed, one way, three rounds). Its memory lives across the rounds of that fight and is
  thrown away after: a bot remembers a foe within a fight, never between fights.
- The bot picks its act itself: argmax, sampling with `u`, a search, rules, anything. An act outside 0..30 or not
  legal is played as a wait.
- A trap, or a call that runs out of fuel, makes the bot lose the fight.

## Determinism

An official fight runs in the arena's pinned wasmtime, configured for WebAssembly 3.0's deterministic profile:
`Config::relaxed_simd_deterministic(true)` (one fixed result for every relaxed SIMD instruction on every CPU) and
`Config::cranelift_nan_canonicalization(true)` (every NaN canonical). The same module, seed and inputs give the same
bits on ARM and x86. The wasmtime version is part of the season, beside the rules' tag.

| Feature | Accepted | Why |
|---|---|---|
| Everything in 2.0, 128-bit SIMD, relaxed SIMD | yes | deterministic in the pinned wasmtime |
| Tail calls, exception handling (`exnref`), typed function references, extended constants | yes | deterministic |
| Memory64, multiple memories, wide arithmetic | yes | deterministic; the memory budget covers every memory |
| GC | no | its heap is outside linear memory, so the memory budget cannot hold it |
| Threads, shared memory | no | the order of threads is not deterministic |

The pinned wasmtime, configured with exactly the accepted set, validates a submission. The set is part of the season.

**Fuel** is wasmtime's own (`Config::consume_fuel`), set before each call. It counts the module's operators, the
same on every CPU for one wasmtime version, so an out-of-fuel loss happens on the same decision everywhere.

## The act log

Each fight is published as: the season tag, the seed, both modules' hashes, and the act log, every choice each seat
handed the rules, with its tick. The rules are deterministic given the seed and those choices, so a `LogSeat` that
hands back the logged choices replays the fight exactly, without running a bot. The page plays every published
fight that way, on any device. A check runs the fight again in the CLI and compares its log with the published one.

## Budgets

| Budget | Value | Why |
|---|---|---|
| Module size | 32 MiB | room for a model of a few million f32 parameters |
| Memory, all memories together | 256 MiB | wasmtime's `ResourceLimiter`; a `memory.grow` past it fails |
| Fuel per `arena_decide` | 20M | about 100x the shipped network (~200K multiply-adds); a few ms natively |
| Fuel per `arena_start` / `arena_round` | 200M | room to unpack a model once |

A fight has about 1,000 decisions per bot per 120 s round, so a full-budget bot costs at most about 20G operators a
round: the runner's size depends on this number.

## Where the code goes

| Code | Place |
|---|---|
| Validate a module, hash it, run it in wasmtime with fuel and the memory limit | new crate `crates/arena-bot` |
| `BotSeat`: `PolicyMind`'s `begin` / `due` / `queue` around a `Decider`; `LogSeat`; recording the act log | `crates/pixfx-duel/src/seat.rs` (the `Decider` trait keeps wasmtime out of the rules) |
| Playing an act log on the page | `crates/arena-wasm` (`Duel.fromLog`), `site/assets/duel.js` and the watch page |
| An opponent bot in training | `crates/pixfx-duel-py`: `env.set_bot(id, bytes)`, opponent `"bot"` |
| The arena's CLI: the submission checks, the ladder, a fight with its act log, checking a log | `crates/arena-bot` binary (`check`, `ladder`, `fight`, `verify`); `pixfx_duel.check(bytes)` in Python |
| Templates a player starts from | `bots/`: `mlp` (Rust: the shipped network as a bot), `onnx` (Rust: tract runs an embedded ONNX file) |

The engine (`pixfx-mind`) does not change: `PolicyMind` already exposes `begin`, `due` and `queue`. The house bot
ships as the `mlp` template built with `weights/policy.bin`; the home page's live duel keeps seating the shipped
weights file in the browser, as it does today.

## What a player does

1. Trains anything in Python against `VecEnv` (`observe` gives 354 floats and the mask, `step` takes an act).
   Opponents can be the house bots or any `.wasm` through `set_bot`.
2. Builds a module: the ONNX template (export ONNX from PyTorch, build), the MLP template, or their own code in any
   language that compiles to WebAssembly.
3. Checks it: `pixfx_duel.check(bytes)` runs the submission checks and the ladder locally.
4. Submits the `.wasm` and a name.

## Checks on submission

1. It validates: the accepted features, no imports, the exports with their signatures, `arena_abi() == 1`, within
   size and memory.
2. It runs: a fixed set of 64 decisions from real fights, twice, gives the same acts both times and within fuel.
3. It is good enough: the entry bar of `docs/tournament.md` (75% against the brain, 65% each school, 80% against the
   first mind).
4. A module with the same hash as one already entered is refused.

## What changes in the docs

- `docs/bot.md`: "The bot module" (the ABI, the features, the budgets, the templates) in place of "The weights file"
  and "How the network picks".
- `docs/tournament.md`: what a player submits, why a module, the checks, the act log, how to check a fight.
- `docs/training.md`: building a module; `set_bot`.
- `docs/roadmap.md`: items 3, 6 and 7 follow this design.

## Spikes before building (the plan's first tasks)

1. **Cross-CPU bits (MEDIUM).** wasmtime's deterministic mode returns the fused result for `relaxed_madd`, where the
   spec's deterministic profile is unfused (wasmtime #14934): consistent, but off the spec. Run one module that uses
   relaxed SIMD and NaNs on an ARM machine and an x86 machine (CI on both); the act logs and fuel must match bit for
   bit.
2. **ONNX template (MEDIUM).** Export the shipped network to ONNX, build it with tract into a bot for
   `wasm32-unknown-unknown`, and match the Rust `Net`'s acts on the 32 fixtures of `policy_check.json`.

## Training opponents

The trainer's built-in opponents (the latest self, past selves, the shipped policy) stay weights files run by
`Net`, the fastest path for millions of steps; a module joins as an opponent through `set_bot`.

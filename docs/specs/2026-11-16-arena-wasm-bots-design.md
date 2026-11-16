# Bots as WASM modules: design

Status: proposed 2026-11-16, waiting for the owner's review. Nothing here is built.

## Intent

A player owns their whole bot: the model, its size, its memory, how it picks an act, and how it was trained. The
arena fixes only the game: what a bot sees, what it can do, when it decides, and how much it may compute per
decision. A bot is submitted as one WebAssembly module. The arena runs every module in a sandbox with no I/O, counts
its instructions, and gets the same bits on every machine and in the browser, so a fight is still a seed, two files
and a season tag, and anyone can replay it.

## What exists today

- A bot is a weights file for one fixed network: about 198K parameters, MLP, no memory (`docs/bot.md`, "The weights
  file"; `pixfx-mind/src/policy/net.rs`). The arena samples the act from the network's logits (`net::sample`).
- The policy's timing, reaction delay and legality live in `PolicyMind` (`pixfx-mind/src/policy/mind.rs`), which
  is already split so the network can be swapped out: `begin` hands back the act queued last tick, `due` returns
  the observation, the legal mask and the dice draw `u` when a decision is due, and `queue` takes the answer.
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
| `memory` | memory | its linear memory |
| `arena_abi` | `() -> i32` | returns `1` |
| `arena_input` | `() -> i32` | the address of a buffer the arena writes before each decision: 354 `f32` LE (the observation), then 30 bytes (the legal mask, 0 or 1) |
| `arena_start` | `(seed: i32)` | a new fight; `seed` is the bot's own dice for the fight |
| `arena_round` | `()` | a new round of the same fight |
| `arena_decide` | `(u: f32) -> i32` | the act index, 0..30; `u` is the seat's dice draw in [0, 1) |

- One instance per fight (one seed, one way, three rounds). Its memory lives across the rounds of that fight and is
  thrown away after: a bot remembers a foe within a fight, never between fights.
- The bot picks its act itself: argmax, sampling with `u`, a search, rules, anything. An act outside 0..30 or not
  legal is played as a wait, as today.
- Every call is metered (below). A trap, or a call that runs out of fuel, makes the bot **lose the fight**. A wait
  instead would let a bot gamble on an over-budget search.

## Determinism

WebAssembly 3.0 (released 2025-09-17) defines a deterministic profile: canonical NaNs and one fixed result for each
relaxed SIMD instruction. Only wasmtime implements profiles; no browser does (`WebAssembly/website` `features.json`,
"profiles"). The arena must replay a fight in a browser, so it accepts only the features that give the same bits on
wasmtime and on Chrome, Firefox and Safari without that profile. The one difference left is the bits inside a NaN.

| Feature (Wasm 3.0 or proposal) | Accepted | Why |
|---|---|---|
| Everything in 2.0, fixed-width SIMD (128-bit) | yes | deterministic; in every engine (Safari 16.4) |
| Tail calls, exception handling (`exnref`), typed function references, extended constants | yes | deterministic; in every engine (Safari 18.4 for `exnref`) |
| Relaxed SIMD | no | its results depend on the CPU outside the deterministic profile; behind a flag in Safari |
| GC | no | its heap is outside linear memory, so the memory budget cannot hold it the same way on every host |
| Memory64, multiple memories | no | not shipped in Safari (memory64 behind a flag, multi-memory absent) |
| Threads, shared memory | no | the order of threads is not deterministic |
| Wide arithmetic (phase 4), half precision (phase 1) | no, until shipped | behind flags in Chrome, Firefox and Safari as of 2026-11 |

- Refused at submission: any import and every feature marked "no" (`wasmparser` validates with exactly the
  accepted set). The set is part of the season and is re-read each season against the engines.
- Fuel is **injected into the module**, not taken from the engine: on submission the arena rewrites the module so
  every basic block subtracts its instruction count from a global and traps at zero. The rewritten module is the one
  stored and hashed, and the same counter runs in wasmtime and in the browser, so an out-of-fuel loss happens on the
  same decision everywhere. (wasmtime's own fuel does not exist in a browser.)
- NaN bits: the only way out of the module is an `i32`, so a NaN can only change a fight if the bot branches on a
  NaN's bits. The arena also publishes each match's act log, so a fight can be checked two ways: run it again, or play
  the log.

## Budgets (proposed numbers, the owner's call)

| Budget | Proposed | Why |
|---|---|---|
| Module size | 32 MiB | room for a model of a few million f32 parameters |
| Linear memory | 256 MiB max | declared max checked at submission; `memory.grow` past it fails |
| Fuel per `arena_decide` | 20M instructions | about 100x the shipped network (~200K multiply-adds); about 2-5 ms native |
| Fuel per `arena_start` / `arena_round` | 200M | room to unpack a model once |

A fight has about 1,000 decisions per bot per 120 s round, so a full-budget bot costs at most about 20G instructions
a round: the runner's size depends on this number.

## Where the code goes

| Code | Place |
|---|---|
| Validate, inject fuel, hash a module | new crate `crates/arena-bot` (pure: bytes in, metered bytes or a named refusal out) |
| Run a module natively (wasmtime) | `crates/arena-bot`, behind a `host` feature |
| `BotSeat`: `PolicyMind`'s `begin` / `due` / `queue` around `arena_decide` | `crates/pixfx-duel/src/seat.rs`, over a small `Decider` trait so the duel does not depend on wasmtime |
| The bot seat on the page | `crates/arena-wasm` takes a JS decider; `site/assets/duel.js` instantiates the module with `WebAssembly.instantiate` and calls the same exports |
| An opponent bot in training | `crates/pixfx-duel-py`: `env.set_bot(id, bytes)`, opponent `"bot"` |
| The ladder and the submission checks | `examples/ladder.rs --bot FILE` (and a second `--bot` for two files) |
| Templates a player starts from | `bots/`: `mlp` (Rust: the shipped network as a bot), `onnx` (Rust: tract runs an embedded ONNX file) |

The engine (`pixfx-mind`) does not change: `PolicyMind` already exposes `begin`, `due` and `queue`.

## What a player does

1. Trains anything in Python against `VecEnv` (unchanged: `observe` gives 354 floats and the mask, `step` takes an
   act). Their opponents can be the house bots or any `.wasm` through `set_bot`.
2. Exports to a module: the ONNX template (export ONNX from PyTorch, build), the MLP template, or their own code in
   any language that compiles to WebAssembly.
3. Checks it: `pixfx_duel.check(bytes)` runs the submission checks and the ladder locally.
4. Submits the `.wasm` and a name.

## Checks on submission

1. It validates: the features above, no imports, the exports with their signatures, `arena_abi() == 1`, within size
   and memory.
2. The arena injects fuel; that module is the bot from now on, and its hash is its identity.
3. It runs: a fixed set of 64 decisions from real fights, twice, gives the same acts both times and within fuel.
4. It is good enough: the entry bar of `docs/tournament.md` (75% against the brain, 65% each school, 80% against the
   first mind).
5. A module with the same hash as one already entered is refused.

## What changes in the docs

- `docs/bot.md`: "The weights file" and "How the network picks" give way to "The bot module" (the ABI, the
  budgets, the templates).
- `docs/tournament.md`: what a player submits, why a module, the checks, and a published act log per match.
- `docs/training.md`: export to a module; `set_bot`.
- `docs/roadmap.md`: items 3 and 6 change; the runner hosts modules.

The house bot ships as the `mlp` template built with `weights/policy.bin`, so the page and the tournament seat one
kind of bot.

## Risks

- **tract on wasm32 (MEDIUM).** The ONNX template depends on tract building for `wasm32-unknown-unknown` without
  threads and running PyTorch-exported models. The plan's first task is a spike: export the shipped network to ONNX,
  build it with tract into a bot, and match the Rust `Net`'s acts on the 32 fixtures.
- **Fuel injection.** Writing the rewriter with `wasmparser` + `wasm-encoder` is ours to maintain; an existing
  crate (`wasm-instrument`'s gas metering) is an option to measure first.
- **Speed on the page.** A 20M-instruction decision eight times a second is fine natively; on a slow phone the page
  may fall behind and should play tournament fights from the act log.
- **A training env that runs bots.** wasmtime in `pixfx-duel-py` adds a large dependency to the trainer's build;
  prebuilt wheels (roadmap 2) matter more.

## Open questions for the owner

1. The budgets: module size, memory, fuel per decision.
2. Losing the fight on a trap or out of fuel, or losing only the round.
3. Memory per fight (proposed), per round, or across the whole match.
4. Whether the weights-file seat stays for training speed (the shipped trainer's opponents) or every seat becomes a
   module.

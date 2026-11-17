# Bots as WASM modules: design

Status: approved 2026-11-17 with the owner's answers ("Decisions" below). Built so far: admission, the host, the seat and
the forfeit rule, the `onnx` template and the checks; the plan `docs/plans/2026-11-17-arena-wasm-bots.md` lists what
is left.

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
| `arena_input` | `() -> i32` | the address of a buffer the arena writes before each decision: 354 `f32` LE (the observation), then 30 bytes (the legal mask, 0 or 1); read once, when the instance is made |
| `arena_start` | `(seed: i32)` | a new fight; `seed` is the bot's own dice for the fight |
| `arena_round` | `()` | a new round of the same fight |
| `arena_decide` | `(u: f32) -> i32` | the act index, 0..30; `u` is the seat's dice draw in [0, 1) |

- One instance per **match** (every seed of the match, both ways, in the order the runner publishes). Its memory
  lives across every round and fight of the match and is thrown away after: a bot may learn its foe over a match,
  never carry anything between matches. `arena_start` opens each fight of the match, `arena_round` each round.
- The bot picks its act itself: argmax, sampling with `u`, a search, rules, anything. An act outside 0..30 or not
  legal is played as a wait, as today.
- Every call is metered (below). A trap, or a call that runs out of fuel, makes the bot **lose the round** it
  happened in (the foe wins it at once; a trap in `arena_start` loses the fight's first round). A wait instead would
  let a bot gamble on an over-budget search. The instance and its memory stay as the trap left them, the same on
  every host, and the bot plays on from the next round.

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

The numbers above are the ones of the first season (decided 2026-11-17). Memory now lives across a match, so the
256 MiB cap holds for the whole match, not per fight.

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
3. It runs: a fixed set of 32 decisions from real fights (`tests/fixtures/policy_check.json`), twice in fresh
   instances, gives the same acts both times and within fuel.
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

## ONNX and modules only (owner, 2026-11-17, replacing decision 4 below)

The weights file is gone. Every bot is a module; most are built from an ONNX model with `bots/onnx`. The house AI is
`crates/pixfx-duel/weights/policy.onnx`, run natively with tract (`arena_bot::onnx`: the ladder, the tests, a
trainer's opponents through `set_model`) and as its module on the site and in the tournament. The trainer's network
has no fixed size (`--width`, `--hidden`, `--blocks`), and its snapshots fight the learner as ONNX. `PolicySeat`,
`Net` from a file and `policy.bin` are no longer used; the engine's `Net` stays in pixfx-mind for other games.

## Both kinds of file stay (decision 4, superseded)

What is easiest and freest for a player:

- **Submission takes either file.** A `.wasm` module, or a weights file as today. The arena wraps a weights file into
  the `mlp` template (a pure, published build: same weights, same module bytes), so a player who only runs `ppo.py`
  never touches WebAssembly, and the runner still hosts one kind of bot.
- **Training keeps the native weights seat** (`set_weights`, the league, the ladder's `--weights`): it is the fast
  path for the shipped trainer. `set_bot` adds any module as an opponent beside it.
- **The reward moves to Python** (roadmap 1) in the same piece of work: `step` also hands back each decision's facts,
  so a player changes how their bot is paid without rebuilding Rust.

## Risks

- **tract on wasm32: done, it works (spike 2026-11-17).** The shipped network exported to ONNX (opset 17, the
  legacy exporter), run by tract-onnx 0.22 in a `wasm32-unknown-unknown` cdylib: a 13.7 MB module that imports
  nothing (getrandom 0.2 needs its `custom` feature with a function that fails; its `js` feature would add imports).
  On the 32 fixtures it picks the same best legal act as PyTorch every time (worst logit difference 4.9e-6
  relative), and wasmi and JavaScriptCore give the same logit bits. Cost: about 2.4M wasmi fuel a decision (an
  eighth of the 20M budget), 51M to build the plan in `arena_start`, 0.35 ms a decision in Bun. Not yet run on V8
  or SpiderMonkey, and fuel was wasmi's count, not the injected one (Task 2).
- **Fuel injection.** Writing the rewriter with `wasmparser` + `wasm-encoder` is ours to maintain; an existing
  crate (`wasm-instrument`'s gas metering) is an option to measure first.
- **Speed on the page.** A 20M-instruction decision eight times a second is fine natively; on a slow phone the page
  may fall behind and should play tournament fights from the act log.
- **A training env that runs bots.** wasmtime in `pixfx-duel-py` adds a large dependency to the trainer's build;
  prebuilt wheels (roadmap 2) matter more.

## Decisions (owner, 2026-11-17)

1. Budgets: the proposed numbers (32 MiB module, 256 MiB memory, 20M fuel a decision, 200M for start and round),
   left to the implementer; revisit after measuring the runner.
2. A trap or running out of fuel loses the **round**, not the fight.
3. A bot's memory lives across the **whole match**.
4. Whatever is easiest and freest for players: both kinds of file are accepted, the weights seat stays for training,
   and the reward moves to Python ("Both kinds of file stay"). Superseded the same day: ONNX and modules only.

### What these decisions cost

- **A match is one sequence.** With memory across a match, one fight of a match can no longer be replayed alone from
  its seed: it needs every fight before it. The runner splits work across machines by match, never inside one, and
  publishes each fight's act log, so a single fight is still checked by playing its log.
- **The rules learn to forfeit a round.** Today a round ends only on a life at 0 or the clock; a seat must be able to
  say "I trapped", and the duel gives that round to the foe. That is a rule (`docs/rules.md`), deterministic, and
  only a module seat can trigger it.

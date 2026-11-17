# Bots as WASM modules Implementation Plan

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A player submits a WebAssembly module (or a weights file, wrapped for them) and owns their whole bot: the
model, its size, its memory across a match, how it picks an act. The arena runs it metered and deterministic,
natively and on the page.

**Spec:** `docs/specs/2026-11-16-arena-wasm-bots-design.md` (the ABI `arena-bot-1`, "Determinism", "Budgets",
"Decisions").

**Architecture:** A pure crate `crates/arena-bot` validates a module, injects fuel and hashes it; behind a `host`
feature it runs one with wasmtime. `pixfx-duel` gains a `Decider` trait and a `BotSeat` around `PolicyMind`'s
`begin` / `due` / `queue`, and a rule that gives a round to the foe when a seat forfeits it. Templates in `bots/`
(`mlp`, `onnx`) build modules; the house bot is the `mlp` template with `weights/policy.bin`. The page runs the same
module through `WebAssembly.instantiate` behind a JS decider.

## Global Constraints

- The engine (`pixfx-mind`) does not change.
- The 354 inputs, the 30 acts, legality, reaction delay and think clock do not change; the weights seat keeps
  playing the same fights bit for bit (`goldens/policy_fight.json` stays as it is).
- The forfeit rule is new: `docs/rules.md` in the same change; the brain-vs-brain and policy goldens do not move.
- Every script is Rust (`CLAUDE.md`, "Scripts"); `train/` stays Python.
- Each crate tests itself; finish with the commands of `CLAUDE.md`, "Before you finish".

## Review Focus

1. The same module, observations and `u` give the same acts and the same out-of-fuel decision in wasmtime and in the
   browser.
2. A module with an import, a refused feature, a wrong export signature or over budget is refused with a named reason.
3. A trap or out-of-fuel loses exactly that round; the instance plays on in the next round with its memory as left.
4. A weights file wrapped into the `mlp` template plays the same acts as `PolicySeat` with that file.

---

### Task 1: Spike, tract on wasm32 (go / no-go for the `onnx` template): done, go

- [x] Export the shipped network to ONNX from `train/net.py` (opset 17, no dynamic axes).
- [x] A throwaway bot crate: tract-onnx embedded model, built for `wasm32-unknown-unknown` without threads.
- [x] Run it (wasmi and Bun) on the 32 fixtures of `crates/pixfx-duel/tests/fixtures`: same acts as `Net::logits` +
  `sample`, and the instruction count per decision under 20M.
- [x] Write the result in the spec's "Risks". If it fails, the `onnx` template waits; `mlp` and own-code bots go on.

### Task 2: `crates/arena-bot`, validate, meter, hash: done

- [x] Failing tests: refuses an import, each refused feature, a missing or mistyped export, `arena_abi() != 1`, a
  module over 32 MiB, a memory max over 256 MiB (or none declared); accepts a minimal valid bot.
- [x] A rewriter on `wasmparser` + `wasm-encoder` (`wasm-instrument` rides on `parity-wasm`, which reads no
  bulk-memory or reference types, both on in rustc's wasm32 defaults). The
  counter is a mutable exported global the host sets before each call; it traps at zero.
- [x] `pub fn admit(bytes: &[u8]) -> Result<Admitted, Refusal>` (metered bytes + sha256), `Refusal` naming the reason.

### Task 3: `arena-bot` host (feature `host`): done

- [x] `Bot::new(&Admitted)`: one wasmtime instance per match; `start(seed)`, `round()`, `decide(obs, mask, u)`
  each set the fuel (200M / 200M / 20M) and return `Result<_, Fault>` (trap or out of fuel).
- [x] Tests: a looping bot runs out on the same decision every time; a trapping bot is usable on the next call with
  its memory as left.

### Task 4: `BotSeat` and the forfeit rule (`pixfx-duel`): done

- [x] `pub trait Decider { start, round, decide }` in `seat.rs`, so the duel does not depend on wasmtime.
- [x] `BotSeat` runs `PolicyMind::begin` / `due` / `queue` around `Decider::decide`; an act out of range or not legal
  is a wait; a fault marks the seat forfeit for the round.
- [x] `Seat::forfeit(&self) -> bool` (default false); the duel ends a round at once for the foe when a seat forfeits.
  Test it; `docs/rules.md` says it.
- [x] A match keeps one decider across its fights (the seat maker takes a shared decider).

### Task 5: The `mlp` template and the house bot: dropped (the weights file is gone; the house bot is `bots/onnx` over
`weights/policy.onnx`)

- [ ] `bots/mlp`: a Rust crate over `pixfx_mind::policy::Net`, the weights embedded, picking with `sample(u)`.
- [ ] `examples/wrap.rs FILE.bin -> FILE.wasm` in `arena-bot`: a weights file into the template (same input, same
  bytes).
- [ ] Test: the wrapped shipped weights in `BotSeat` play the same fights as `PolicySeat` over seeds 1-20.

### Task 6: The `onnx` template (if Task 1 went): done (`cargo run -p arena-bot --example onnx`)

- [x] `bots/onnx`: build from an `.onnx` file; README for a PyTorch export.

### Task 7: The ladder and the checks

- [x] The checks and the entry bar for a module: `cargo run --release -p arena-bot --features host --example check`
  (32 fixed decisions, twice). Not yet: a module in `examples/ladder.rs` / `match.rs` (they live in `pixfx-duel`,
  which does not depend on wasmtime).
- [ ] `pixfx_duel.check(bytes)` in the Python module: admit, 64 decisions twice, the entry bar.

### Task 8: Training (`pixfx-duel-py`)

- [x] `env.set_model(id, bytes)`: an ONNX model (tract) or a module (wasmtime) as a `policy` opponent; the league's
  snapshots go as ONNX.
- [ ] The reward in Python: `step` also returns each decision's facts (dealt, taken, blows landed, guarded,
  punished, statuses, round end); the Rust reward stays the default; `train/` gets a reward hook and a test.

### Task 9: The page: done (arena-dc wired modules; arena-e4 removed the weights path and pinned the house fight)

- [x] `crates/arena-wasm` takes a JS decider; `site/assets/duel.js` instantiates the module and meters it with the
  same injected counter.
- [x] `site/tests`: the house bot plays the same acts on the page as natively (the 32 fixtures and one
  whole fight).

### Task 10: Docs: done for what is built (`set_bot` and the Python reward wait for Task 8)

- [x] `docs/bot.md` ("The bot module": ABI, budgets, templates, both kinds of file), `docs/tournament.md` (what a
  player submits, a match as one sequence, the act log), `docs/training.md` (`set_bot`, the Python reward),
  `docs/roadmap.md` (items 1, 3, 6).

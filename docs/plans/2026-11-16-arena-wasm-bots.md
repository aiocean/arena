# Bots as WASM modules: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A player submits one `.wasm` bot that holds their whole model; the arena checks it, fights it natively and
deterministically, publishes each fight's act log, and the page replays fights from logs.

**Architecture:** The rules (`pixfx-duel`) gain a `Decider` trait and three seats: `BotSeat` (`PolicyMind`'s timing
around a decider), `Recorded` (writes an act log) and `LogSeat` (plays one back). A new crate `crates/arena-bot`
runs modules in a pinned wasmtime (deterministic relaxed SIMD, canonical NaNs, fuel, a memory limit) and is the
arena's CLI. Templates in `bots/` build a module from the shipped network or from an ONNX file.

**Tech Stack:** Rust (nightly-2026-09-07, `wasm32-unknown-unknown`), wasmtime (pinned `=`), tract-onnx, pyo3/maturin,
PyTorch (ONNX export), wasm-bindgen, bun tests.

**Spec:** `docs/specs/2026-11-16-arena-wasm-bots-design.md`

## Global Constraints

- ABI `arena-bot-1`: exports `memory`, `arena_abi() -> i32` (= 1), `arena_input() -> i32` (354 `f32` LE then 30
  bytes), `arena_start(seed: i32)`, `arena_round()`, `arena_decide(u: f32) -> i32`; **no imports**; extra exports
  are allowed and ignored.
- Accepted features: 2.0, SIMD128, relaxed SIMD, tail calls, exceptions (`exnref`), typed function references,
  extended constants, memory64, multi-memory, wide arithmetic. Refused: GC, threads, shared memory.
- wasmtime `Config`: `relaxed_simd_deterministic(true)`, `cranelift_nan_canonicalization(true)`,
  `consume_fuel(true)`, the features above on, the refused ones off. wasmtime is pinned with `=` to the newest
  release when Task 3 starts.
- Budgets: module 32 MiB; all memories together 256 MiB; fuel 20,000,000 per `arena_decide`; 200,000,000 per
  `arena_start` / `arena_round`.
- One instance per fight (one seed, one way, all its rounds). A trap or out-of-fuel: the bot loses the fight.
- An act outside 0..30 or not legal is played as a wait.
- Workspace lints: `unwrap_used`, `expect_used`, `panic` deny; `unsafe_code` forbid (`bots/*` are their own
  workspaces and allow `unsafe_code` only for the `#[unsafe(no_mangle)]` exports and the input buffer).
- The rules do not change: `tests/goldens/` must pass unchanged in every task.
- Public repo: no machine names, addresses or home paths in code, docs or commits.
- Before finishing each task: the "Before you finish" commands of `CLAUDE.md` that touch its crates.

## Review Focus

1. **The same module in both corners (a mirror)**: two instances, memories apart; one corner's state never leaks to the
   other. Test in Task 3.
2. **`arena_start` traps or runs out of fuel**: the bot loses the fight before its first decision; nothing panics.
   Test in Task 3 (fault) and Task 2 (scoring).
3. **`memory.grow` past the budget, across two memories**: the grow returns -1, the bot keeps running. Test in Task 3.
4. **An act log from another season or other modules**: `verify` refuses it, naming the mismatch, instead of
   replaying a different fight. Test in Task 6.
5. **A decision returning -1, 30 or 2^31-1**: a wait, never an index panic. Test in Task 1.

---

### Task 1: Deciders, bot seats and act logs in the rules

**Files:**
- Modify: `crates/pixfx-duel/src/seat.rs`
- Create: `crates/pixfx-duel/src/log.rs` (and `pub mod log;` in `lib.rs`)
- Test: `crates/pixfx-duel/tests/bots.rs`

**Interfaces:**
- Produces (in `seat`):
  - `pub enum Fault { Trap(String), OutOfFuel }` (`Clone, Debug, PartialEq`, `Display`)
  - `pub trait Decider { fn start(&mut self, seed: u32) -> Result<(), Fault>; fn round(&mut self) -> Result<(), Fault>;
    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault>; }`
  - `Seat::fault(&self) -> Option<&Fault>`, default `None`
  - `pub struct BotSeat`; `BotSeat::make(new: Rc<dyn Fn() -> Box<dyn Decider>>, style: Option<Style>) -> MakeSeat`
    (`None`: the style the fight drew). Uses `policy_mind`; the first `reset` calls `start(seed)` with
    `state.seed.wrapping_mul(2).wrapping_add(me) ^ 0x9e37_79b9`, later ones `round()`; between `due` and `queue` it
    calls `decide`; an index outside 0..30 queues 0 (wait). After a fault it returns `None` from `think` forever
    and `fault()` returns it.
- Produces (in `log`):
  - `pub struct ActLog { pub season: String, pub seed: u32, pub bots: [String; 2], pub acts: [Vec<(u32, Choice)>; 2] }`
    (serde behind the `serde` feature; `u32` = the index of the seat's `think` call in the fight)
  - `pub fn recorded(make: MakeSeat, log: Rc<RefCell<ActLog>>) -> MakeSeat` (records every `Some` its seat returns)
  - `pub fn replay(log: Rc<ActLog>) -> [MakeSeat; 2]` (`LogSeat`: counts its `think` calls, hands back the logged
    choice at each logged index, `None` otherwise)

- [ ] **Step 1: Write the failing tests** in `tests/bots.rs`, with a test-only `NetDecider` (`Net` + `net::sample` on
  `u`, exactly what `PolicySeat` does):
  - `bot_seat_plays_the_policys_fight`: seeds 1-5, 60 s each, `BotSeat(NetDecider(shipped))` vs brain gives the same
    `Out` stream (hash, as `tests/goldens.rs` hashes) as `PolicySeat::make(shipped)` vs brain.
  - `a_log_replays_its_fight`: seed 7, `recorded` both corners, 120 s; `replay` of that log for 120 s gives the same
    `Out` stream.
  - `an_act_out_of_range_is_a_wait`: deciders returning -1, 30 and `i32::MAX` play the same fight as one returning 0.
  - `a_fault_stops_the_seat`: a decider whose third `decide` returns `Err(Fault::OutOfFuel)`: `fault()` is
    `Some(&Fault::OutOfFuel)` and `decide` is never called again.
  - `start_then_round`: a counting decider sees `start` once, then `round` once per later round, with the seed above.
- [ ] **Step 2:** `cargo test -p pixfx-duel --test bots` fails (no `Decider`).
- [ ] **Step 3:** Implement the interfaces above.
- [ ] **Step 4:** `cargo test -p pixfx-duel --all-targets` passes, goldens unchanged; clippy clean.
- [ ] **Step 5: Commit** `duel: bot seats over a Decider, act logs and their replay`

### Task 2: A fault loses the fight in the ladder

**Files:**
- Modify: `crates/pixfx-duel/src/ladder.rs` (`match_up`, `ladder.rs:362`, and the fight loop it calls)
- Test: `crates/pixfx-duel/tests/ladder.rs`

**Interfaces:**
- Consumes: `Seat::fault` (Task 1).
- Produces: `Record` gains `pub faults: [u32; 2]` (fights each side lost to a fault); a faulted side loses every
  round of that fight not yet played, and the fight ends at once.

- [ ] **Step 1: Failing test** `a_fault_loses_the_fight`: `match_up` over seeds 1-3, 3 rounds, of a `BotSeat` whose
  decider faults on its first `start` against the brain: side 0 wins 0 rounds, `faults == [6, 0]` (3 seeds x both
  ways).
- [ ] **Step 2:** It fails. **Step 3:** Implement. **Step 4:** `cargo test -p pixfx-duel --all-targets` passes.
- [ ] **Step 5: Commit** `ladder: a bot that faults loses the fight`

### Task 3: `arena-bot`: check and run a module

**Files:**
- Create: `crates/arena-bot/Cargo.toml` (deps: `pixfx-duel`, `pixfx-mind`, `wasmtime` pinned, `sha2`, `wat` as a
  dev-dependency), `crates/arena-bot/src/lib.rs`, `src/engine.rs`, `src/check.rs`, `src/bot.rs`
- Test: `crates/arena-bot/tests/modules.rs` (modules written in WAT inside the test)

**Interfaces:**
- Consumes: `Decider`, `Fault` (Task 1).
- Produces:
  - `pub const LIMITS: Limits` with `module: 32 << 20`, `memory: 256 << 20`, `fuel_decide: 20_000_000`,
    `fuel_setup: 200_000_000`
  - `pub fn engine() -> Result<wasmtime::Engine, Refusal>` (the season's `Config`, Global Constraints)
  - `pub enum Refusal { TooBig { bytes: usize }, Invalid(String), Import(String), Export { name: String, why: String },
    Abi(i32) }` (`Display` names the cause)
  - `pub fn check(engine: &Engine, bytes: &[u8]) -> Result<Module, Refusal>`
  - `pub fn hash(bytes: &[u8]) -> String` (sha256, lowercase hex)
  - `pub struct WasmBot` implementing `Decider`; `WasmBot::new(engine: &Engine, module: &Module) -> Result<WasmBot,
    Fault>` (a fresh `Store` and instance); the memory limit is a `ResourceLimiter` summing every memory.

- [ ] **Step 1: Failing tests:** `refuses_an_import`, `refuses_gc`, `refuses_shared_memory`,
  `refuses_a_missing_export` (names it), `refuses_a_wrong_signature`, `refuses_abi_2`, `refuses_33_mib`;
  `accepts_relaxed_simd_memory64_and_two_memories`; `a_loop_runs_out_of_fuel` (`Err(Fault::OutOfFuel)` from
  `decide`); `unreachable_is_a_trap`; `grow_past_256_mib_across_two_memories_returns_minus_one`;
  `two_instances_keep_their_own_memory` (a bot counting its decisions: two instances, interleaved, each count 1, 2, 3);
  `a_trap_in_start_is_a_fault`; `the_input_reaches_the_bot` (a bot returning `trunc(obs[3] * 10)` and the first
  legal index).
- [ ] **Step 2:** `cargo test -p arena-bot` fails. **Step 3:** Implement. **Step 4:** passes; clippy clean.
- [ ] **Step 5: Commit** `arena-bot: check, hash and run a bot module in the season's wasmtime`

### Task 4: Spike: the same bits on ARM and x86

Gate: if the x86 run differs, stop and report to the owner before Task 5; the spec's determinism rests on this.

**Files:**
- Create: `crates/arena-bot/tests/cross_cpu.rs`, `crates/arena-bot/tests/goldens/cross_cpu.json`,
  `.github/workflows/cross-cpu.yml` (`cargo test -p arena-bot --test cross_cpu` on `ubuntu-latest` (x86_64) and
  `macos-latest` (arm64))

- [ ] **Step 1: Write the test:** a WAT bot whose `arena_decide` runs `f32x4.relaxed_madd`, `relaxed_min` / `max` on
  ±0 and NaN, `relaxed_trunc` past `i32` range, `relaxed_laneselect`, `relaxed_dot_i8x16_i7x16_add_s`, and a NaN
  made by `0/0` stored and reloaded as bits; it folds every result's bits into a running hash in memory and returns
  `hash % 30`. Over 2,000 decisions on fixed inputs, record the acts, the fuel left after each call and the final
  hash. The golden pins them (first run writes it, as `tests/goldens.rs` does).
- [ ] **Step 2:** Write the golden on this Mac (arm64); commit with the workflow; push the branch.
- [ ] **Step 3:** The workflow passes on both runners. `gh run watch` shows both jobs green.
- [ ] **Step 4: Commit** `arena-bot: one module, the same bits and fuel on ARM and x86`

### Task 5: The `mlp` template and the house bot

**Files:**
- Create: `bots/mlp/Cargo.toml` (own workspace, `crate-type = ["cdylib"]`, depends on `pixfx-mind` at the workspace's
  rev), `bots/mlp/src/lib.rs`, `bots/mlp/build.sh` (builds release for `wasm32-unknown-unknown` with
  `-C target-feature=+simd128`, copies to `crates/pixfx-duel/weights/house.wasm`), `bots/README.md`
- Modify: `Cargo.toml` (`exclude = ["crates/pixfx-duel-py", "bots"]`)
- Test: `crates/arena-bot/tests/house.rs`

**Interfaces:**
- Produces: `crates/pixfx-duel/weights/house.wasm`: the ABI around `Net::load(include_bytes!(policy.bin))` and
  `net::sample(&logits, &legal, u)`; it exports `arena_logits() -> i32` too (30 `f32` of its last decision).

- [ ] **Step 1: Failing test** `house_plays_the_shipped_policys_fight`: seeds 1-5, 60 s, `BotSeat(WasmBot(house.wasm))`
  vs brain gives the same `Out` hash as `PolicySeat::make(shipped)` vs brain; `check(house.wasm)` is `Ok`.
- [ ] **Step 2:** It fails (no file). **Step 3:** Write the template, run `bots/mlp/build.sh`. **Step 4:** passes.
- [ ] **Step 5: Commit** `bots: the mlp template; the house bot as a module`

### Task 6: The arena's CLI

**Files:**
- Create: `crates/arena-bot/src/main.rs`, `crates/arena-bot/src/run.rs`
- Test: `crates/arena-bot/tests/cli.rs`

**Interfaces:**
- Consumes: `check`, `WasmBot`, `BotSeat`, `recorded`, `replay`, `match_up`.
- Produces (`run`): `pub const SEASON: &str` (`"arena-s1"`); `pub fn fight(engine, a: &[u8], b: &[u8], seed: u32,
  rounds: u32, limit: f64) -> Result<ActLog, Refusal>` (a plays Thunder); `pub fn verify(engine, log: &ActLog, a:
  &[u8], b: &[u8]) -> Result<(), Mismatch>` with `Mismatch { Season { want, got }, Bot { corner, want, got }, Acts {
  corner, at } }`; `pub fn entry(engine, bytes: &[u8]) -> Verdict` (the four submission checks of the spec, the bar
  as `docs/tournament.md` states it, over ladder seeds 1-150).
- CLI: `arena-bot check FILE`, `arena-bot ladder FILE [--vs FILE] [--seeds N]`, `arena-bot fight A B --seed N [--out
  LOG]`, `arena-bot verify LOG A B`; exit 0 on pass, 1 on a refusal or mismatch, with the reason on stderr.

- [ ] **Step 1: Failing tests:** `fight_then_verify` (house vs house, seed 3: `verify` is `Ok`);
  `verify_refuses_another_season`; `verify_refuses_another_module` (corner and both hashes named);
  `verify_finds_a_changed_act` (one act edited: `Acts { corner, at }` names it); `entry_passes_the_house_bot`.
- [ ] **Step 2:** fails. **Step 3:** Implement. **Step 4:** `cargo test -p arena-bot` passes;
  `cargo run --release -p arena-bot -- ladder crates/pixfx-duel/weights/house.wasm --seeds 150` prints the same win
  rates as `examples/ladder.rs` for the policy rows.
- [ ] **Step 5: Commit** `arena-bot: the arena's CLI: check, ladder, fight, verify`

### Task 7: Bots in the trainer

**Files:**
- Modify: `crates/pixfx-duel-py/Cargo.toml` (depend on `arena-bot`), `crates/pixfx-duel-py/src/lib.rs`,
  `crates/pixfx-duel/src/train.rs` (`Opponent::Bot(Rc<dyn Fn() -> Box<dyn Decider>>)`)
- Test: `train/tests/test_bots.py`

**Interfaces:**
- Produces (Python): `VecEnv.set_bot(id: int, data: bytes)` (raises `ValueError` with the `Refusal` text), opponent
  `"bot"` with `weights` = that id; `pixfx_duel.check(data: bytes) -> dict` with keys `ok`, `hash`, `reason`,
  `brain`, `brain_thunder`, `brain_frost`, `first`. Each worker thread compiles its own `Module` from the stored
  bytes, as it does its own `Net`.

- [ ] **Step 1: Failing tests:** `test_house_bot_as_opponent` (64 envs vs `"bot"` = house.wasm run to 20 dones, no
  error); `test_set_bot_refuses_an_import`; `test_check_house` (`ok` true, `brain >= 0.75`).
- [ ] **Step 2:** `uv run pytest tests/test_bots.py` fails. **Step 3:** Implement; `uv run maturin develop --release -m
  ../crates/pixfx-duel-py/Cargo.toml`. **Step 4:** `uv run pytest` passes (all tests).
- [ ] **Step 5: Commit** `trainer: a bot module as an opponent; check from Python`

### Task 8: The `onnx` template

**Files:**
- Create: `train/export_onnx.py` (a `.pt` or `.bin` to `model.onnx`: input `obs` [1, 354], output `logits` [1, 30]),
  `bots/onnx/Cargo.toml` (own workspace, `tract-onnx` without threads), `bots/onnx/src/lib.rs` (loads
  `include_bytes!("../model.onnx")` once in `arena_start`; masks illegal acts; samples with `u` as `net::sample`
  does; exports `arena_logits`), `bots/onnx/build.sh`
- Test: `crates/arena-bot/tests/onnx.rs`, `train/tests/test_export_onnx.py`

- [ ] **Step 1: Failing tests:** `onnx_logits_match_the_net`: for the 32 observations of
  `tests/fixtures/policy_check.json`, the onnx bot built from the shipped weights gives logits (`arena_logits`)
  within 1e-4 of `Net::logits`; `check(onnx bot)` is `Ok` and its `arena_start` fits the setup fuel.
  `test_export_onnx`: onnxruntime-free check that the file loads in `onnx.checker` with those input/output names.
- [ ] **Step 2:** fails. **Step 3:** Implement; build. **Step 4:** passes. If tract cannot build for
  `wasm32-unknown-unknown` or a forward pass exceeds 20M fuel, stop and report to the owner (spec risk 2).
- [ ] **Step 5: Commit** `bots: the onnx template`

### Task 9: The page replays act logs

**Files:**
- Modify: `crates/arena-wasm/src/duel.rs` (`Duel.fromLog(config, log)`: both corners `replay(log)`; throws when
  the log's seed is not the config's), `site/assets/duel.js`, `site/watch.html` (open a log file: plays it)
- Create: `crates/arena-bot/tests/log_fight.rs` (writes `goldens/log_fight.json`: a house-vs-brain log of seed 3 and
  the native `Out` hash of its first 60 s), `site/tests/log.test.js`

- [ ] **Step 1: Failing test** `site/tests/log.test.js`: the wasm `Duel.fromLog` on `goldens/log_fight.json` gives
  the golden hash over 60 s; a log with another seed throws naming both seeds.
- [ ] **Step 2:** fails. **Step 3:** Implement. **Step 4:** `site/embed.sh /tmp/kit && bun test ./site/tests` and
  `cargo test -p arena-wasm` pass; open the watch page (`site/build.sh --serve`), load the golden log, see the fight
  play.
- [ ] **Step 5: Commit** `watch page: play a fight from its act log`

### Task 10: The player docs

**Files:**
- Modify: `docs/bot.md` ("The bot module": ABI, features, budgets, templates, in place of "The weights file" and
  "How the network picks"), `docs/tournament.md` (what a player submits and why a module, the checks, the act log,
  checking a fight with `arena-bot verify`, the season = repo tag + engine rev + wasmtime version),
  `docs/training.md` (building a module, `set_bot`, `pixfx_duel.check`), `docs/roadmap.md` (items 3, 6, 7 as built
  or left), the spec's Status line ("shipped", with what was built differently)

- [ ] **Step 1:** Write them from the code as built; every number matches `LIMITS` and the `Config`.
- [ ] **Step 2:** `site/build.sh` renders the docs pages without error; `rg -n "weights file" docs/tournament.md`
  only where a weights file is still meant (training opponents).
- [ ] **Step 3: Commit** `docs: bots are wasm modules`

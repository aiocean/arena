# arena

Allowed to use Nhĩ Căn's Content

The arena: the duel game on the pixfx engine, and the bot tournament built on it (arena.aiocean.io). Read `README.md`
for what is here and `docs/` for the game, the bot interface, training and the tournament. The engine is pixfx
(aiocean/pixfx-rs; its `AGENTS.md`: "The boundary", "Layers and where code goes"), pinned to one commit: the crates
take it from git at the rev in `Cargo.toml`, the site from `.pixfx/` at the same rev (`CONTRIBUTING.md`, also for
working on the engine beside the arena). The arena's crates are the **Game** layer above it. The engine never depends
on the arena; the pixfx site embeds the arena's duel through `site/embed.sh`.

## Where code goes

| If the code... | It goes to |
|---|---|
| is a rule of the fight: a number, a skill, a status, frame data, a round | `crates/pixfx-duel` (`data.rs` for numbers) |
| is the training env, its reward, an opponent, the ladder | `crates/pixfx-duel` (`train`, `seat`, `scripted`, `ladder`) |
| admits, meters or runs a bot module, or builds one from a model | `crates/arena-bot` (the duel's side is `seat::Decider` / `BotSeat` in `pixfx-duel`); templates in `bots/` |
| hands the env to Python | `crates/pixfx-duel-py` (translation only) |
| trains, exports or loads a model (ONNX) in Python | `train/` |
| is how any trained policy sees, acts or keeps its timing | the engine's `pixfx-mind` (`policy`, in pixfx-rs): it serves every game |
| runs a model natively (ONNX) or as a bot module | `crates/arena-bot` (`onnx`, `host`) |
| is how a fighter looks: which frame of its sprite sheet a pose shows, how that frame is drawn | `crates/arena-fighter` (the engine only says what the fighter is doing and draws the marks on it) |
| makes a fighter's sprite sheets from its PixelLab export | `art/<character>/tools/sheet.rs`, writing `site/assets/fighters/` |
| decides how a fighter is drawn: canvas, height, outline, colours, clips, the checks before it ships | `art/README.md` (the art direction; not a player doc) |
| is the duel for JS | `crates/arena-wasm` (translation only; the page loads it beside the engine's wasm) |
| draws or plays the duel on the page | `site/assets/duel*.js` (a shell: it decides nothing); `site/embed.sh` lists what the pixfx site embeds |
| is a page of the arena site (arena.aiocean.io): home, watch, the docs pages | this folder's `site/` (`build.sh`; the docs pages are rendered from `docs/*.md`, never edited by hand) |
| explains the game to a player | `docs/` |

## Traps

- The rules are deterministic from a seed; `crates/pixfx-duel/AGENTS.md` lists the goldens a change re-pins.
- A change to the inputs or the acts (`pixfx-mind/src/policy`), or to the bot ABI (`crates/arena-bot`), changes every
  bot: it needs a retrain and a new season (`docs/tournament.md`). Update `docs/bot.md` in the same change. The model
  inside a bot is free: its size or shape is no season's business.
- A change to a rule changes every bot's world: it waits for a new season once one has started. Update `docs/rules.md`
  in the same change.
- A new house AI: `train/export.py` (writes `crates/pixfx-duel/weights/policy.onnx` and its check), then
  `cargo test -p arena-bot --features onnx --test house`, `site/build.sh` (builds the house module), and re-pin
  `crates/arena-bot/tests/goldens/house_fight.json` (delete it, run `cargo test -p arena-bot --all-features`), which
  `site/tests/policy.test.js` checks the page against. There is no weights file (`.bin`) any more: every bot is ONNX or
  a module.
- Long training runs go on a desktop as launchd jobs, never on a laptop (`train/README.md`, "Training on a
  desktop").
- A new fighter is made to `art/README.md`: HD pixel art, a 256 px PixelLab canvas (v3), side view, about 240 px
  tall, the 20 clips of `pick::CLIPS` facing east, checked beside the reference before any animation is paid for
  (about 170 generations a character with its master).
- Rule and balance changes are the owner's call; measure them with the ladder (`--seeds 150 --telemetry`) before and
  after.

## Scripts

Every script is Rust, whatever it would have been (shell, Python, Go, one-off glue, tooling); no new `.sh`, `.py`
or `.go` unless a tool forces it (a hook that must be a shell one-liner). `train/` stays Python.

- First look for a crate or step that already does the job (the engine's `pixfx-pixel` steps, `pixfx-body`) and use
  it. When none fits, write an example in the crate that owns the logic (`crates/<crate>/examples/<name>.rs`, run
  with `cargo run -p <crate> --example <name> -- args`), against its library, not a copy of it.
- Anything outside a crate (art tooling under `art/`): a single-file cargo script, `cargo -Zscript <file>.rs args`
  (the pinned toolchain is nightly), its dependencies in a frontmatter block that also sets `edition = "2024"`.
  `art/cultivator/tools/split.rs` is the pattern.
- `std::process::Command` for external commands, `serde_json` instead of `jq`, `ureq` instead of `curl`. When a
  script grows or is needed a third time, move its logic into a crate with tests.

## Before you finish

```
cargo test -p pixfx-duel --all-targets
cargo clippy -p pixfx-duel --all-targets -- -D warnings
(cd train && uv run maturin develop --release -m ../crates/pixfx-duel-py/Cargo.toml && uv run pytest)
site/build.sh && bun test ./site/tests           # after a change the page sees: rebuilds both wasm modules first
cargo test -p arena-wasm && cargo clippy --workspace --all-targets -- -D warnings
```

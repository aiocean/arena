# arena

Allowed to use Nhĩ Căn's Content

The arena: the duel game, made with Bevy, and the bot tournament built on it (arena.aiocean.io). Read `README.md` for
what is here and `docs/` for the game, the bot interface, training and the tournament. The rules (`crates/pixfx-duel`)
are a `bevy_ecs` world whose tick is one single-threaded schedule; the game (`crates/arena-game`) is a Bevy app that
runs them, natively and in the browser. Nothing depends on the pixfx engine any more: what the arena took from it
(the mind and the policy, the body, the JS number helpers, the effect ids) lives in this repo, taken at pixfx-rs rev
ad1612b.

## Where code goes

| If the code... | It goes to |
|---|---|
| is a rule of the fight: a number, a skill, a status, frame data, a round | `crates/pixfx-duel` (`data.rs` for numbers) |
| is the training env, its reward, an opponent, the ladder | `crates/pixfx-duel` (`train`, `seat`, `scripted`, `ladder`) |
| admits, meters or runs a bot module, or builds one from a model | `crates/arena-bot` (the duel's side is `seat::Decider` / `BotSeat` in `pixfx-duel`); templates in `bots/` |
| hands the env to Python | `crates/pixfx-duel-py` (translation only) |
| trains, exports or loads a model (ONNX) in Python | `train/` |
| is how any trained policy sees, acts or keeps its timing, or how the rules' minds think | `crates/arena-mind` (`policy` for the trained policy) |
| is how a body moves (gravity, flight, blows, getting up), JS number semantics, the effect ids the rules may play | `crates/pixfx-duel` (`body`, `pose`, `js`, `effects`) |
| runs a model natively (ONNX) or as a bot module | `crates/arena-bot` (`onnx`, `host`) |
| is the game: the window or canvas, the camera, the fight stepped each frame, a corner's bot | `crates/arena-game` (`lib`, `layout`, `rules`, `bots`; `web` for the page's side) |
| is how a fighter looks: which frame of its sprite sheet a pose shows, how that frame is drawn | `crates/arena-game` (`pick`, `sheet`, `fighters`) |
| is how a spell's effect looks: a bolt, a blast, a seal, a fire (painted frame by frame on the CPU) | `crates/arena-vfx` (`recipes`, on the `kit`); baked into the sheets the game plays by `cargo run --release -p arena-vfx --example bake -- crates/arena-game/assets/vfx` |
| plays a spell's effect in the game: a baked sheet as sprites, an effect that reads the fighter painted live | `crates/arena-game/src/vfx` (`sprites`; `arena_vfx::SPRITE_AWARE` and `AMBIENT` are the live ones) |
| is the backdrop, the light, the fight's feel (hit-stop, shake, numbers), the HUD | `crates/arena-game` (`scene`, `light`, `feel`, `hud`) |
| makes a fighter's sprite sheets from its PixelLab export | `art/<character>/tools/sheet.rs`, writing `crates/arena-game/assets/fighters/` |
| decides how a fighter is drawn: canvas, height, outline, colours, clips, the checks before it ships | `art/README.md` (the art direction; not a player doc) |
| starts the game on the page, hands it a dropped bot module and a seed | `site/assets/arena.js` (it decides nothing) |
| is a page of the arena site (arena.aiocean.io): home (the duel and the bot slots), the docs pages | this folder's `site/` (`build.sh`; the docs pages are rendered from `docs/*.md`, never edited by hand) |
| explains the game to a player | `docs/` |

## Traps

- The rules are deterministic from a seed; `crates/pixfx-duel/AGENTS.md` lists the goldens a change re-pins.
- A change to the inputs or the acts (`crates/arena-mind/src/policy`), or to the bot ABI (`crates/arena-bot`), changes every
  bot: it needs a retrain (`docs/tournament.md`: one test season for now, no tags). Update `docs/bot.md` in the same
  change. The model inside a bot is free: its size or shape is nobody's business.
- A change to a rule changes every bot's world: while the arena is in testing it goes in on `main`. Update `docs/rules.md`
  in the same change.
- A new house AI: `train/export.py` (writes `crates/pixfx-duel/weights/policy.onnx` and its check), then
  `cargo test -p arena-bot --features onnx --test house`, `site/build.sh` (builds the house module), and re-pin
  `crates/arena-bot/tests/goldens/house_fight.json` (delete it, run `cargo test -p arena-bot --all-features`). CI does not run the `arena-bot` features (wasmtime and tract are
  too heavy for it): run them locally before a push that touches a bot, the house AI or the rules. There is no weights file (`.bin`) any more: every bot is ONNX or
  a module.
- Long training runs go on a desktop as launchd jobs, never on a laptop (`train/README.md`, "Training on a
  desktop").
- A new fighter is made to `art/README.md`: HD pixel art, a 256 px PixelLab canvas (v3), side view, about 240 px
  tall, the 20 clips of `pick::CLIPS` facing east, checked beside the reference before any animation is paid for
  (about 170 generations a character with its master).
- An effect is a sprite sheet unless it reads the fighter it plays on (its body, its outline: `SPRITE_AWARE`) or
  fills the scene (`AMBIENT`), as the pixfx engine keeps only sprite-aware effects. A change to an effect's painter,
  or to the colours and sizes the rules play it with, needs a re-bake (the bake writes `assets/vfx`;
  `every_played_effect_is_baked` fails on an effect with no sheet). The sheets are loaded through Bevy's asset
  server, beside the game's wasm on the page (`game/assets`).
- Rule and balance changes are the owner's call; measure them with the ladder (`--seeds 150 --telemetry`) before and
  after.

## Scripts

Every script is Rust, whatever it would have been (shell, Python, Go, one-off glue, tooling); no new `.sh`, `.py`
or `.go` unless a tool forces it (a hook that must be a shell one-liner). `train/` stays Python.

- First look for a crate or step that already does the job and use it. When none fits, write an example in the crate that owns the logic (`crates/<crate>/examples/<name>.rs`, run
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
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p arena-game --lib --target wasm32-unknown-unknown -- -D warnings
site/build.sh --serve                            # after a change the page sees: then look at it in a browser
ARENA_SHOTS=/tmp/arena:4,8 cargo run -p arena-game -- --seed 7   # natively, screenshots to look at
```

# arena

Allowed to use Nhĩ Căn's Content

The arena: the duel game, made with Bevy, and the bot tournament built on it (arena.aiocean.io). Read `README.md` for
what is here and `docs/` for the game, the bot interface, training and the tournament. The rules (`crates/arena-duel`)
are a `bevy_ecs` world whose tick is one single-threaded schedule; the game (`crates/arena-game`) is a Bevy app that
runs them, natively and in the browser. Nothing depends on the pixfx engine any more: what the arena took from it
(the mind and the policy, the body, the JS number helpers, the effect ids) lives in this repo, taken at pixfx-rs rev
ad1612b. How much a blow deals is the damage system, `crates/arena-calc`; the rules build every blow there and
resolve it there.

## Where code goes

| If the code... | It goes to |
|---|---|
| is how damage is worked out: the kinds of damage and which beats which, ailments, fusion, ranks, daos, curses, intents, domains, supports, keystones, the shared skill pool, loadouts, and what the field-bound stats do on plain geometry (projectiles, motion, stored uses, devices) | `crates/arena-calc` (pure, no fight in it; `docs/specs/2026-11-17-damage-system-design.md`) |
| measures loadouts, skills, supports and keystones against each other without the real fight (a headless 1D simulator for balance) | `crates/arena-sim` (`fight`, `units`, `balance`; it calls `arena-calc` as the fight does) |
| is a rule of the fight: how a skill travels, a body, a cast, a unit on the field, frame data, a round | `crates/arena-duel` (`data.rs` for numbers; `strike` calls `arena-calc`, `unit` holds traps, mines, totems and minions) |
| is the training env, its reward, an opponent, the ladder | `crates/arena-duel` (`train`, `seat`, `scripted`, `ladder`) |
| admits, meters or runs a bot module, or builds one from a model | `crates/arena-bot` (the duel's side is `seat::Decider` / `BotSeat` in `arena-duel`); templates in `bots/` |
| hands the env to Python | `crates/arena-duel-py` (translation only) |
| is a live match on the wire: the postcard frames a server publishes and a page folds, how they lie on MoQ | `crates/arena-link` (pure; `docs/specs/2026-11-17-live-matches-design.md`) |
| fights matches and publishes them live to the MoQ relay, or checks a published match by replaying it | `crates/arena-server` (`arena-server`, `arena-watch`); run on the kiosk by `io.aiocean.arena-live` (game-factory `backend/arena`) |
| sets up a process's logs, traces and metrics, exports them over OTLP, or turns a duel step into telemetry events (the duel observer: it only reads) | `crates/arena-telemetry` (`init`, `duel::Observer`; a process calls `init` once in `main`, game code only uses `tracing` on the `arena::*` targets) |
| runs or documents the telemetry store (OpenObserve), its streams, its retention and its queries | `ops/observe` (`compose.yaml`, `README.md`) |
| records a corner's decisions or replays them (the server's side and the page's) | `crates/arena-duel` (`seat::Recorder`, `seat::Replay`) |
| watches a live match in the game: the subscription, the replay seats, stepping only as far as the frames reach | `crates/arena-game` (`live.rs`, `rules.rs`) |
| trains, exports or loads a model (ONNX) in Python | `train/` |
| is how any trained policy sees, acts or keeps its timing, or how the rules' minds think | `crates/arena-mind` (`policy` for the trained policy) |
| is how a body moves (gravity, flight, blows, getting up), JS number semantics, the effect ids the rules may play | `crates/arena-duel` (`body`, `pose`, `js`, `effects`) |
| runs a model natively (ONNX) or as a bot module | `crates/arena-bot` (`onnx`, `host`) |
| is the game: the window or canvas, the camera, the fight stepped each frame, a corner's bot | `crates/arena-game` (`lib`, `layout`, `rules`, `bots`; `web` for the page's side) |
| is how a fighter looks: which frame of its sprite sheet a pose shows, how that frame is drawn | `crates/arena-game` (`pick`, `sheet`, `fighters`) |
| is how a spell's effect looks: a bolt, a blast, a seal, a fire (painted frame by frame on the CPU) | `crates/arena-vfx` (`recipes`, on the `kit`); baked into the sheets the game plays by `cargo run --release -p arena-vfx --example bake -- crates/arena-game/assets/vfx` |
| plays a spell's effect in the game: a baked sheet as sprites, an effect that reads the fighter painted live | `crates/arena-game/src/vfx` (`sprites`; `arena_vfx::SPRITE_AWARE` and `AMBIENT` are the live ones) |
| is a spell's effect drawn by an image model (smooth, no pixels) and moved by code: its frames, prompts and `motion.json` (spin, flight, scale-in, idle, tail, exit, its anchor) | `art/vfx/<name>/`, built by `cargo -Zscript art/vfx/tools/vfx.rs --all art/vfx crates/arena-game/assets/drawn`; read by `arena_vfx::drawn`, played by `crates/arena-game/src/vfx/drawn.rs`. An effect with a drawn sheet plays from it, the rest from their recipes. The sprite keeps only the drawn shapes; whatever code can do (a turn, a flight, a scale, a fade) is in `motion.json` |
| is the backdrop, the light, the fight's feel (hit-stop, shake, numbers), the HUD | `crates/arena-game` (`scene`, `light`, `feel`, `hud`) |
| makes a fighter's sprite sheets from its PixelLab export | `art/<character>/tools/sheet.rs`, writing `crates/arena-game/assets/fighters/` |
| decides how a fighter is drawn: canvas, height, outline, colours, clips, the checks before it ships | `art/README.md` (the art direction; not a player doc) |
| starts the game on the page, hands it a dropped bot module and a seed | `site/assets/arena.js` (it decides nothing) |
| is a page of the arena site (arena.aiocean.io): home (the duel and the bot slots), the docs pages | this folder's `site/` (`build.sh`; the docs pages are rendered from `docs/*.md`, never edited by hand) |
| explains the game to a player | `docs/` |

## Traps

- The rules are deterministic from a seed; `crates/arena-duel/AGENTS.md` lists the goldens a change re-pins.
- A change to the inputs or the acts (`crates/arena-mind/src/policy`), or to the bot ABI (`crates/arena-bot`), changes every
  bot: it needs a retrain (`docs/tournament.md`: one test season for now, no tags). Update `docs/bot.md` in the same
  change. The model inside a bot is free: its size or shape is nobody's business.
- The live page replays the server's matches from their decisions, so the page and `arena-server` must run one rules
  build. A change to the rules (or to a seat's `think` order) bumps `arena_link::RULES_VERSION` and ships the server
  with the page: build and restart `io.aiocean.arena-live` (game-factory `backend/arena/README.md`) in the same deploy.
  A page on other rules is told to reload.
- A change to a rule changes every bot's world: while the arena is in testing it goes in on `main`. Update `docs/rules.md`
  in the same change.
- A new house AI: `train/export.py` (writes `crates/arena-duel/weights/policy.onnx` and its check), then
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
cargo test -p arena-calc && cargo test -p arena-sim
cargo test -p arena-duel --all-targets
cargo clippy -p arena-duel --all-targets -- -D warnings
(cd train && uv run maturin develop --release -m ../crates/arena-duel-py/Cargo.toml && uv run pytest)
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p arena-game --lib --target wasm32-unknown-unknown -- -D warnings
site/build.sh --serve                            # after a change the page sees: then look at it in a browser
ARENA_SHOTS=/tmp/arena:4,8 cargo run -p arena-game -- --seed 7   # natively, screenshots to look at
```

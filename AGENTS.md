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
| is whether two shapes overlap or where a sweep first enters one (every hit test: boxes, circles, a bolt's flight; touching is not overlapping) | `arena_calc::collide` (on parry2d-f64, `enhanced-determinism`); the fight's shapes (a fighter's body, a probe, a field, a unit's body) in `arena_duel::collide`, which `debug::shapes` draws (`docs/specs/2026-11-18-physics-design.md`) |
| measures loadouts, skills, supports and keystones against each other without the real fight (a headless 1D simulator for balance) | `crates/arena-sim` (`fight`, `units`, `balance`; it calls `arena-calc` as the fight does) |
| measures physics engines (parry, rapier, Avian) against the rules' collision on real fights: answers, native = wasm32, cost, size | `spike/physics` (its own workspace, outside CI; `docs/specs/2026-11-18-physics-design.md`) |
| is where a fighter can be hit and where its blow or bolt comes from, frame by frame (hurtboxes, hitboxes, hands, each clip's active frame; which frame a pose shows) | `crates/arena-duel` (`frames`, `pick`); the table `frames_data.rs` is generated from the sprite sheets by `cargo run -p arena-game --example frame_boxes`: run it again after a sheet changes (a rule change: ladder, goldens) |
| is a rule of the fight: how a skill travels, a body, a cast, a unit on the field, frame data, a round | `crates/arena-duel` (`data.rs` for numbers; `strike` calls `arena-calc`, `unit` holds traps, mines, totems and minions) |
| is the training env, its reward, an opponent, the ladder | `crates/arena-duel` (`train`, `seat`, `scripted`, `ladder`) |
| admits, meters or runs a bot module, or builds one from a model | `crates/arena-bot` (the duel's side is `seat::Decider` / `BotSeat` in `arena-duel`); templates in `bots/` |
| hands the env to Python | `crates/arena-duel-py` (translation only) |
| is a live match on the wire: the postcard frames a server publishes and a page folds, how they lie on MoQ | `crates/arena-link` (pure; `docs/specs/2026-11-17-live-matches-design.md`) |
| fights matches and publishes them live to the MoQ relay, or checks a published match by replaying it | `crates/arena-server` (`arena-server`, `arena-watch`); run on the kiosk by `io.aiocean.arena-live` (game-factory `backend/arena`). Who sits in a live corner is `match_run::Bot` and the seat factory `Seats` (the house AI on ONNX against itself, owner 2026-11-18; its seat from the seed): a new kind of bot (an uploaded module) is a new `Bot` there |
| checks the live site after a deploy: the page, the game, the relay's live match, the HUD, the fight moving, the page's build against the server's | `crates/arena-smoke` (a headless Chrome; the page shows it `window.__arena.live` and `rules`, set in `crates/arena-game/src/live.rs`, and `window.__arena.build`, stamped into `index.html` by `site/build.sh`) |
| sets up a process's logs, spans and metrics, writes them as JSON lines to the telemetry files, reads them back (the query tool, the watchdog), or turns a duel step into telemetry events (the duel observer: it only reads) | `crates/arena-telemetry` (`init`, `read`, `duel::Observer`, examples `query` and `watchdog`; a process calls `init` once in `main`, game code only uses `tracing` on the `arena::*` targets) |
| documents the telemetry files (`ARENA_TELEMETRY_DIR`, `~/services/arena/telemetry` on the kiosk), their lines, their retention, the ready queries and the watchdog | `ops/telemetry` (`README.md`, the watchdog's plist) |
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
| makes a fighter's sprite sheets from its generated pictures (cut the grid, key the ground, find the feet, scale) | `art/tools/sheet.rs` (to be written), writing `crates/arena-game/assets/fighters/`; the PixelLab exports' own `art/<character>/tools/sheet.rs` built today's sheets until they are redrawn |
| decides how a fighter is drawn: canvas, height, outline, colours, clips, the checks before it ships | `art/README.md` (the art direction; not a player doc) |
| starts the game on the page, hands it a dropped bot module and a seed | `site/assets/arena.js` (it decides nothing) |
| is a page of the arena site (arena.aiocean.io): home (the duel and the bot slots), the docs pages | this folder's `site/` (`build.sh`; the docs pages are rendered from `docs/*.md`, never edited by hand) |
| is the game's wiki (arena.aiocean.io/wiki/): a page per skill, support, ailment, aura, domain, curse, keystone, unit and look, its pictures drawn offline from the sheets | `crates/arena-wiki` (generated from the code by `site/build.sh` into `dist/wiki/`; never edited by hand) |
| explains the game to a player | `docs/` |

## Traps

- The rules are deterministic from a seed; `crates/arena-duel/AGENTS.md` lists the goldens a change re-pins.
- A change to the inputs or the acts (`crates/arena-mind/src/policy`), or to the bot ABI (`crates/arena-bot`), changes every
  bot: it needs a retrain (`docs/tournament.md`: one test season for now, no tags). Update `docs/bot.md` in the same
  change. The model inside a bot is free: its size or shape is nobody's business.
- The live page replays the server's matches from their decisions, so the page and `arena-server` must run one rules
  build. A change to the rules, to a seat's `think` order or to a bot's inputs or acts bumps
  `arena_link::RULES_VERSION` to origin's number plus one at push time (`git pull --rebase` first: another change may
  have taken the next number) and ships the server with the page in one deploy ("Deploy", below). A page on other
  rules refuses the match and is told to reload; the smoke test fails it as stale. The page replays in WebAssembly
  what the server fought natively: run the native = wasm check (`CONTRIBUTING.md`) before pushing a rules change.
- A change to a rule changes every bot's world: while the arena is in testing it goes in on `main`. Update `docs/rules.md`
  in the same change.
- A new house AI: `train/export.py` (writes `crates/arena-duel/weights/policy.onnx` and its check), then
  `cargo test -p arena-bot --features onnx --test house`, `site/build.sh` (builds the house module), and re-pin
  `crates/arena-bot/tests/goldens/house_fight.json` and `house_module.sha256` (delete them, run `cargo test -p arena-bot --all-features`). CI does not run the `arena-bot` features (wasmtime and tract are
  too heavy for it): run them locally before a push that touches a bot, the house AI or the rules. There is no weights file (`.bin`) any more: every bot is ONNX or
  a module.
- The house module is byte-reproducible: any clone at any path builds the same `site/dist/bots/policy.wasm` on the pinned
  toolchain (the `onnx` example builds `bots/onnx` through the fixed link `/tmp/arena-onnx-bot/repo` with its own
  rustflags, paths remapped). `goldens/house_module.sha256` pins its bytes, `house_fight.rs` checks the built module
  against it, and arena-smoke's `module` check the deployed one. A new model, a change to `bots/onnx`, `arena-mind`,
  tract or the toolchain re-pins it; a deploy that ships another module fails the smoke. The `onnx` example is the one
  way to build it, and either profile gives the same bytes (`cargo run` or `--release`: the template keeps its own).
- Long training runs go on a desktop as launchd jobs, never on a laptop (`train/README.md`, "Training on a
  desktop").
- Pixel is how the arena is shown, not how it is drawn (owner, 2026-11-17). Art is generated smooth, a clean
  vector-style illustration, by image models through OpenRouter (game-factory `tools/openrouter-image`, Gemini for
  clips), never asked for as pixel art, and kept smooth in the game; the pixel layer at render makes the pixels.
  PixelLab is no longer used. A new fighter is made to `art/README.md`: a master in the fighting stance approved
  through the layer before any clip is paid for, then the 17 clips of `pick::CLIPS` facing east, 8 key poses a
  picture with the master as the reference (about $0.07 a picture). The PixelLab sheets on the page stay until each
  is redrawn.
- An effect is a sprite sheet unless it reads the fighter it plays on (its body, its outline: `SPRITE_AWARE`) or
  fills the scene (`AMBIENT`). A drawn sheet (`assets/drawn`, from `art/vfx/`) plays in place of the baked one; the
  rest are baked from their recipes. A change to an effect's painter, or to the colours and sizes the rules play it
  with, needs a re-bake (the bake writes `assets/vfx`; `every_played_effect_is_baked` fails on an effect with no
  sheet); a change to a drawn effect's frames or `motion.json` needs `art/vfx/tools/vfx.rs --all`. The sheets are
  loaded through Bevy's asset server, beside the game's wasm on the page (`game/assets`).
- An effect is a hybrid of code and sprite (owner, 2026-11-17). What can be reused, computed, or would make the
  sheet smaller is done in code: a spin or swirl, a flight, a scale in or out, a fade, a tail, an appear or a
  disappear. The sprite keeps only what code cannot make: the drawn shape itself, a few frames of it, with the
  motion, timing and transforms laid on by code (`motion.json`, `arena_vfx::drawn`).
- A playing effect ends: a one-shot when it has played out, a loop when the rules stop it. The game must never lose
  a stop (a play and its stop can land in one frame) and sweeps every effect when a fight begins; a drawn loop the
  rules forget still ends (`vfx/drawn.rs`).
- Work on `main` only, no side branches (owner, 2026-11-18): small commits, `cargo fmt --all`, `git pull --rebase`
  before each push, never force. Turn the hooks on once a clone (`git config core.hooksPath .githooks`: the pre-push
  hook refuses an unformatted push, CI's first check). A push cancels the CI run before it, so read the last push's
  run (`gh run watch`) before calling main green.
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

## Showing the owner

The agents run on the kiosk machine, not the owner's computer: a browser, `open` or a file path here shows them
nothing. Anything the owner should see is built and handed over as a link (owner, 2026-11-17):

- A change to the page: deployed ("Deploy", below), then https://arena.aiocean.io/ and where to look.
- Art, an effect, anything to review: a review page (a heading, what changed, every doubt, the pictures) in
  `~/services/arena/site/review/<name>/index.html` with its pictures, checked with `curl`, then the link
  https://arena.aiocean.io/review/<name>/. Deploys leave `review/` alone (`--exclude /review/`).
- Never paths to PNGs or GIFs in chat as the result.

## Before you finish

```
cargo fmt --all
cargo test -p arena-calc && cargo test -p arena-sim
cargo test -p arena-duel --all-targets
cargo clippy -p arena-duel --all-targets -- -D warnings
(cd train && uv run maturin develop --release -m ../crates/arena-duel-py/Cargo.toml && uv run pytest)
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude arena-game --all-targets   # CI's rules job; the arena-bot features run locally (Traps)
cargo clippy -p arena-game --lib --target wasm32-unknown-unknown -- -D warnings
site/build.sh --serve                            # after a change the page sees: then look at it in a browser
ARENA_SHOTS=/tmp/arena:4,8 cargo run -p arena-game -- --seed 7   # natively, screenshots to look at
```

## Deploy

One session deploys (the deployer; ask it, never rsync the site yourself), from `projects/arena-deploy`, a clone used
for nothing else, after the commit is on `origin/main`. The steps (game-factory `backend/arena/README.md`, "Deploy"):

1. `git pull --rebase` in the deploy clone; the commit asked for is an ancestor of its `HEAD`.
2. `ARENA_LIVE_URL` set from `ops/arena/moq-relay.env` (the relay's subscribe token, never echoed): without it the
   page fights on its own instead of watching the live matches.
3. Build both from that checkout, the server first: `cargo build --release -p arena-server --bin arena-server -p
   arena-smoke`, then `site/build.sh` (it must exit 0, and `site/dist/live.json` must not be empty).
4. Only once both built, swap them back to back: `rsync -a --delete --exclude /review/ site/dist/
   ~/services/arena/site/`, then the server binary into `~/services/arena/live/` and `launchctl kickstart -k
   gui/$(id -u)/io.aiocean.arena-live` (keep the old binary to roll back).
5. Check `/` and `/live.json` answer 200 and the server's log says it publishes and a new match began; then the
   smoke test (below), which must pass.

The smoke test of the live site: a headless
Chrome (a system one, else Chrome for Testing fetched once into `~/.cache/arena-smoke`) loads https://arena.aiocean.io/
and checks the page and the game's wasm load with no console errors, page errors or failed requests for its own
assets, `live.json` names a relay, the page reaches it over WebTransport and match frames come (a page on other rules than the
server's fails here as stale), the HUD is painted and the match moves, and the page's build (`window.__arena.build`, the
commit `site/build.sh` stamped, "+dirty" for a tree with changes, also in the footer and in `live.json`) against the
server's (the `build` on its newest `match.start` in the server's telemetry files, `~/services/arena/telemetry` or `ARENA_TELEMETRY_DIR`): it fails
only when their rules builds differ, and other commits on one rules build (a docs-only site deploy) are a `WARN` line. PASS/FAIL a check, `screenshot.png` and `summary.json` in `--out` (default `target/arena-smoke`),
exit 0 only when all pass; `--review` adds the run to the review page (last 30 runs). The game gets a fixed 30 s to
start (never stretched for load); if it does not start, and only then, the whole run is tried once more, and it fails
only when both attempts fail (the report and the review say it was retried and why). Each run records the time to the
game starting and to the first live frame and the machine's load averages, and with `--telemetry` writes them to
the telemetry files as `smoke.run` (`<dir>/arena-smoke/<day>.jsonl`; the queries in `ops/telemetry/README.md`):

```
cargo run --release -p arena-smoke -- --review ~/services/arena/site/review/smoke --telemetry
# https://arena.aiocean.io/review/smoke/
```

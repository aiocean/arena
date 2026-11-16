# arena

The arena: the duel game on the pixfx engine, and the bot tournament built on it (arena.aiocean.io). Read `README.md`
for what is here and `docs/` for the game, the bot interface, training and the tournament. The engine is the pixfx
checkout at `../../tools/pixfx-rs` (its `AGENTS.md`: "The boundary", "Layers and where code goes"): the arena's crates
reach its crates by path and are the **Game** layer above it. The engine never depends on the arena; the pixfx site
embeds the arena's duel through `site/embed.sh`.

## Where code goes

| If the code... | It goes to |
|---|---|
| is a rule of the fight: a number, a skill, a status, frame data, a round | `crates/pixfx-duel` (`data.rs` for numbers) |
| is the training env, its reward, an opponent, the ladder | `crates/pixfx-duel` (`train`, `seat`, `scripted`, `ladder`) |
| hands the env to Python | `crates/pixfx-duel-py` (translation only) |
| trains, exports or loads weights in Python | `train/` |
| is how any trained policy sees, acts or runs its network | the engine's `pixfx-mind` (`policy`, in `../../tools/pixfx-rs`): it serves every game |
| is the duel for JS | `crates/arena-wasm` (translation only; the page loads it beside the engine's wasm) |
| draws or plays the duel on the page | `site/assets/duel*.js` (a shell: it decides nothing); `site/embed.sh` lists what the pixfx site embeds |
| is a page of the arena site (arena.aiocean.io): home, watch, the docs pages | this folder's `site/` (`build.sh`; the docs pages are rendered from `docs/*.md`, never edited by hand) |
| explains the game to a player | `docs/` |

## Traps

- The rules are deterministic from a seed; `crates/pixfx-duel/AGENTS.md` lists the goldens a change re-pins.
- A change to the inputs, the acts or the network (`pixfx-mind/src/policy`) changes the weights file and every bot:
  it needs a retrain and a new season (`docs/tournament.md`). Update `docs/bot.md` in the same change.
- A change to a rule changes every bot's world: it waits for a new season once one has started. Update `docs/rules.md`
  in the same change.
- New weights: `train/export.py`, then `cargo test -p pixfx-duel --test policy_export`, and re-pin
  `goldens/policy_fight.json` (delete it, run the tests), which `site/tests/policy.test.js` checks the wasm against.
- Long training runs go on a desktop as launchd jobs, never on a laptop (`train/README.md`, "Training on a
  desktop").
- Rule and balance changes are the owner's call; measure them with the ladder (`--seeds 150 --telemetry`) before and
  after.

## Before you finish

```
cargo test -p pixfx-duel --all-targets
cargo clippy -p pixfx-duel --all-targets -- -D warnings
(cd train && uv run maturin develop --release -m ../crates/pixfx-duel-py/Cargo.toml && uv run pytest)
site/embed.sh /tmp/kit && bun test ./site/tests   # after a change the page sees: rebuilds the arena wasm first
cargo test -p arena-wasm && cargo clippy --workspace --all-targets -- -D warnings
```

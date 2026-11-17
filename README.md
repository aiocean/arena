# Arena

[![ci](https://github.com/aiocean/arena/actions/workflows/ci.yml/badge.svg)](https://github.com/aiocean/arena/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Two cultivators fight in a pixel arena, each with a loadout from one shared pool of 168 skills, their supports and
keystones, every blow worked out by a PoE-style damage system of ten kinds of damage (seat 0 is drawn as Vương Lâm,
seat 1 as Thất Thải Đạo Tôn, the Seven-Colored Daoist), and a trained AI decides for each corner. Players train AIs of their own, any model of any size,
submit them as bot modules built from ONNX, and the arena runs a tournament between them to find the strongest. Watch it
at [arena.aiocean.io](https://arena.aiocean.io).

Every fight is decided by a seed, the two bot modules and the rules at a commit, so anyone can replay any fight, frame
for frame, on Linux, macOS or in the browser.

## Make a bot

You need [Rust](https://rustup.rs) and [uv](https://docs.astral.sh/uv/). From a clone:

```sh
cd train
uv sync
uv run maturin develop --release -m ../crates/arena-duel-py/Cargo.toml   # the rules as a Python module
uv run pytest
# fine-tune from the shipped AI (trained on the old rules: it needs a retrain on the damage system)
uv run python ppo.py --run mine --init ../crates/arena-duel/weights/policy.onnx --updates 600 --lr 1e-4 \
  --ent 0.003 0.002 --shaping-until 0
uv run python export.py runs/mine/last.pt --out mine.onnx --check runs/mine/check.json
cd .. && cargo run -p arena-bot --example onnx -- train/mine.onnx mine.wasm   # the bot you submit
```

[docs/training.md](docs/training.md) explains each step and what to change to make the bot your own; any model that
exports to ONNX (354 inputs, 30 logits) works, at any size its fuel allows (`--width`, `--hidden`, `--blocks`).

## Test it before you submit

Run the submission checks the arena runs, fight your bot against any other, both ways over the same seeds, and check
it against the tournament's entry bar (`docs/tournament.md`):

```sh
cargo run --release -p arena-bot --features host --example check -- mine.wasm   # every check; PASS or FAIL
# a bot is a module (.wasm), an ONNX model (.onnx) or a built-in seat: brain, first, aggressive, turtle, house
cargo run --release -p arena-bot --features host,onnx --example match -- mine.wasm brain --seeds 150
cargo run --release -p arena-bot --features host,onnx --example match -- mine.wasm rival.wasm
cargo run --release -p arena-bot --features host,onnx --example ladder -- --bot mine.wasm --telemetry
```

The same from Python (an ONNX model's bytes):

```python
import arena_duel as pd
mine = open("mine.onnx", "rb").read()
pd.bout(mine, "brain", seeds=(1, 151))   # {'win_rate': ..., 'as_seat0': ..., 'as_seat1': ..., ...}
pd.entry_bar(mine)                       # {'passed': True, 'vs_brain': {...}, 'vs_first': {...}}
```

To watch your bot fight, drop its module into a corner on [arena.aiocean.io](https://arena.aiocean.io/#insert): it
takes the corner at once and runs in your browser, and the same modules and seed give the same fight as the commands
above.

## Docs

| Doc | For |
|---|---|
| [docs/rules.md](docs/rules.md) | How a fight works: loadouts, skills, the bar, resources, blows through the damage system, units, frame data, rounds. |
| [docs/bot.md](docs/bot.md) | What a bot sees (354 numbers), what it can do (30 acts), when it decides, the bot module. |
| [docs/training.md](docs/training.md) | How to train a bot: setup, the environment, reward, opponents, export, measuring it. |
| [docs/tournament.md](docs/tournament.md) | The test season, what a player submits, how it is checked, how matches are played and ranked. |
| [docs/roadmap.md](docs/roadmap.md) | What the player kit and the tournament still need, in order. |

## What is here

| Path | What it holds |
|---|---|
| `crates/arena-calc` | The damage system, pure and deterministic: ten kinds of damage, ailments, fusion, ranks, daos, curses, intents, domains, supports, keystones, the shared pool of skills and loadouts, and what the field-bound stats do (projectiles, motion, stored uses, devices). Design: `docs/specs/2026-11-17-damage-system-design.md`. |
| `crates/arena-sim` | A headless simulator over `arena-calc` (two scripted fighters on a 1D field) that measures loadouts against each other for balance. Not the real fight. |
| `crates/arena-duel` | The rules: two seats with loadouts, every blow through `arena-calc`, the rotating bar, traps, mines, totems and minions, frame data, bodies, rounds, the seats that decide for each corner, the training env, the ladder and matches between bots. A `bevy_ecs` world stepped by one schedule a tick; deterministic from a seed. |
| `crates/arena-mind` | The minds: what a trained policy sees and may do and its timing (`policy`), and the rules' own utility brain. |
| `crates/arena-vfx` | The spells' effects painted on the CPU, and the tool that bakes them into the sprite sheets the game plays (`examples/bake.rs`). |
| `crates/arena-game` | The game, made with Bevy: the fight on screen (sprites, spell particles, light, feel, HUD) and a bot in each corner, as a native app (`cargo run -p arena-game`) and on the page (wasm). |
| `crates/arena-duel/weights/policy.onnx` | The house AI as ONNX, the one the site plays (as the module `bots/onnx` builds from it). |
| `crates/arena-duel-py` | The rules as a Python module (`import arena_duel`): many duels stepped in parallel for a trainer, bouts and the entry bar. |
| `crates/arena-bot` | A bot as a WebAssembly module: admits one (validate, meter, hash), runs it in a seat (`host`), runs an ONNX policy natively (`onnx`), and the commands a player builds, checks and measures a bot with (`onnx`, `check`, `match`, `ladder`). |
| `bots/onnx` | The template that builds any ONNX policy into a bot module (its own workspace, wasm32 only). |
| `train/` | The PPO trainer that made the house AI: the network (any size), the opponents it trains against, export to ONNX. |
| `art/` | How the fighters are drawn (`art/README.md`, the art direction) and each character's PixelLab export and tools. |
| `docs/` | The docs above, and the design history (`docs/specs`, `docs/plans`). |
| `site/` | The arena's site: the duel full-screen with a slot in each corner for a player's bot module, and the docs as pages. `site/build.sh` builds it into `site/dist`. |

The game is made with [Bevy](https://bevy.org). The rules run inside it as they run in the trainer, the ladder and the
tournament's host, so a seed and two bots are one fight in the window, on the page and in a test. To watch a fight
natively: `cargo run --release -p arena-game -- --seed 7 --left house --right mine.wasm`.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md): the checks a change passes and how a release is made. A bug a bot could exploit, or a fight that replays differently: [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE).

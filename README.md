# Arena

[![ci](https://github.com/aiocean/arena/actions/workflows/ci.yml/badge.svg)](https://github.com/aiocean/arena/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Two cultivators fight in a pixel arena, Thunder (drawn as Vương Lâm) against Frost (drawn as Thất Thải Đạo Tôn, the
Seven-Colored Daoist), and a trained AI decides for each corner. Players train AIs of their own, any model of any size,
submit them as bot modules built from ONNX, and the arena runs a tournament between them to find the strongest. Watch it
at [arena.aiocean.io](https://arena.aiocean.io).

Every fight is decided by a seed, the two bot modules and the season's rules, so anyone can replay any fight, frame
for frame, on Linux, macOS or in the browser.

## Make a bot

You need [Rust](https://rustup.rs) and [uv](https://docs.astral.sh/uv/). From a clone:

```sh
cd train
uv sync
uv run maturin develop --release -m ../crates/pixfx-duel-py/Cargo.toml   # the rules as a Python module
uv run pytest
# fine-tune from the shipped AI: 600 updates, about 8 minutes at 40K steps a second
uv run python ppo.py --run mine --init ../crates/pixfx-duel/weights/policy.onnx --updates 600 --lr 1e-4 \
  --ent 0.003 0.002 --shaping-until 0 --thunder 0.9
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
import pixfx_duel as pd
mine = open("mine.onnx", "rb").read()
pd.bout(mine, "brain", seeds=(1, 151))   # {'win_rate': ..., 'as_thunder': ..., 'as_frost': ..., ...}
pd.entry_bar(mine)                       # {'passed': True, 'vs_brain': {...}, 'vs_first': {...}}
```

To watch two bots fight, load their modules on [the watch page](https://arena.aiocean.io/watch.html) with a seed: the
fight runs in your browser, and the same modules and seed give the same fight as the commands above.

## Docs

| Doc | For |
|---|---|
| [docs/rules.md](docs/rules.md) | How a fight works: schools, skills, the bar, qi and rage, statuses, frame data, rounds. |
| [docs/bot.md](docs/bot.md) | What a bot sees (354 numbers), what it can do (30 acts), when it decides, the bot module. |
| [docs/training.md](docs/training.md) | How to train a bot: setup, the environment, reward, opponents, export, measuring it. |
| [docs/tournament.md](docs/tournament.md) | Seasons, what a player submits, how it is checked, how matches are played and ranked. |
| [docs/roadmap.md](docs/roadmap.md) | What the player kit and the tournament still need, in order. |

## What is here

| Path | What it holds |
|---|---|
| `crates/pixfx-duel` | The rules: the two schools, their stats and skills, the rotating bar, statuses, frame data, rounds, the seats that decide for each corner, the training env, the ladder and matches between bots. Deterministic from a seed. |
| `crates/pixfx-duel/weights/policy.onnx` | The house AI as ONNX, the one the site plays (as the module `bots/onnx` builds from it). |
| `crates/arena-wasm` | The rules for the page: `Duel`, a corner's bot module and the tables, as a wasm module the page loads beside the engine's. Translation only. |
| `crates/pixfx-duel-py` | The rules as a Python module (`import pixfx_duel`): many duels stepped in parallel for a trainer, bouts and the entry bar. |
| `crates/arena-bot` | A bot as a WebAssembly module: admits one (validate, meter, hash), runs it in a seat (`host`), runs an ONNX policy natively (`onnx`), and the commands a player builds, checks and measures a bot with (`onnx`, `check`, `match`, `ladder`). |
| `bots/onnx` | The template that builds any ONNX policy into a bot module (its own workspace, wasm32 only). |
| `train/` | The PPO trainer that made the house AI: the network (any size), the opponents it trains against, export to ONNX. |
| `art/` | How the fighters are drawn (`art/README.md`, the art direction) and each character's PixelLab export and tools. |
| `docs/` | The docs above, and the design history (`docs/specs`, `docs/plans`). |
| `site/` | The arena's site: the duel full-screen, a page to watch two bots from their modules and a seed, and the docs as pages. `site/build.sh` builds it into `site/dist`. |

The game runs on the [pixfx engine](https://github.com/aiocean/pixfx-rs), pinned to one commit: the engine draws,
moves the bodies and plays the effects; the arena holds the rules, the training environment and the trainer. The duel
is drawn by `site/assets/duel*.js`, which loads two wasm modules side by side: the engine's (built from the pinned
engine at `.pixfx/`) and the arena's own (`crates/arena-wasm`). The pixfx site embeds the same duel through
`site/embed.sh`.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md): the engine pin, working on the engine beside the arena, the checks a change passes
and how a season is released. A bug a bot could exploit, or a fight that replays differently: [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE).

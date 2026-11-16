# Arena

[![ci](https://github.com/aiocean/arena/actions/workflows/ci.yml/badge.svg)](https://github.com/aiocean/arena/actions/workflows/ci.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Two cultivators fight in a pixel arena, Thunder against Frost, and a trained AI decides for each corner. Players train
AIs of their own, submit them as weights files, and the arena runs a tournament between them to find the strongest.
Watch it at [arena.aiocean.io](https://arena.aiocean.io).

Every fight is decided by a seed, the two weights files and the season's rules, so anyone can replay any fight, frame
for frame, on Linux, macOS or in the browser.

## Make a bot

You need [Rust](https://rustup.rs) and [uv](https://docs.astral.sh/uv/). From a clone:

```sh
cd train
uv sync
uv run maturin develop --release -m ../crates/pixfx-duel-py/Cargo.toml   # the rules as a Python module
uv run pytest
# fine-tune from the shipped AI: 600 updates, about 8 minutes at 40K steps a second
uv run python ppo.py --run mine --init ../crates/pixfx-duel/weights/policy.bin --updates 600 --lr 1e-4 \
  --ent 0.003 0.002 --shaping-until 0 --thunder 0.9
uv run python export.py runs/mine/last.pt   # writes ../crates/pixfx-duel/weights/policy.bin and its check
```

[docs/training.md](docs/training.md) explains each step and what to change to make the bot your own.

## Test it before you submit

Fight your file against any other bot, both ways over the same seeds, and check it against the tournament's entry
bar (`docs/tournament.md`):

```sh
# a bot is a weights file or a built-in seat: brain, first, aggressive, turtle, shipped
cargo run --release -p pixfx-duel --example match -- mine.bin brain --seeds 150
cargo run --release -p pixfx-duel --example match -- mine.bin rival.bin
cargo run --release -p pixfx-duel --example match -- mine.bin --bar   # PASS or FAIL (exit 1)
cargo run --release -p pixfx-duel --example ladder -- --weights mine.bin --telemetry
```

The same from Python:

```python
import pixfx_duel as pd
mine = open("mine.bin", "rb").read()
pd.bout(mine, "brain", seeds=(1, 151))   # {'win_rate': ..., 'as_thunder': ..., 'as_frost': ..., ...}
pd.entry_bar(mine)                       # {'passed': True, 'vs_brain': {...}, 'vs_first': {...}}
```

To watch two files fight, load them on [the watch page](https://arena.aiocean.io/watch.html) with a seed: the fight
runs in your browser, and the same files and seed give the same fight as the commands above.

## Docs

| Doc | For |
|---|---|
| [docs/rules.md](docs/rules.md) | How a fight works: schools, skills, the bar, qi and rage, statuses, frame data, rounds. |
| [docs/bot.md](docs/bot.md) | What a bot sees (354 numbers), what it can do (30 acts), when it decides, the weights file. |
| [docs/training.md](docs/training.md) | How to train a bot: setup, the environment, reward, opponents, export, measuring it. |
| [docs/tournament.md](docs/tournament.md) | Seasons, what a player submits, how it is checked, how matches are played and ranked. |
| [docs/roadmap.md](docs/roadmap.md) | What the player kit and the tournament still need, in order. |

## What is here

| Path | What it holds |
|---|---|
| `crates/pixfx-duel` | The rules: the two schools, their stats and skills, the rotating bar, statuses, frame data, rounds, the seats that decide for each corner, the training env, the ladder and matches between bots. Deterministic from a seed. |
| `crates/pixfx-duel/weights/policy.bin` | The shipped AI, the one the site plays. |
| `crates/arena-wasm` | The rules for the page: `Duel`, `Policy` and the tables, as a wasm module the page loads beside the engine's. Translation only. |
| `crates/pixfx-duel-py` | The rules as a Python module (`import pixfx_duel`): many duels stepped in parallel for a trainer, bouts and the entry bar. |
| `train/` | The PPO trainer that made the shipped AI: the network, the opponents it trains against, export. |
| `docs/` | The docs above, and the design history (`docs/specs`, `docs/plans`). |
| `site/` | The arena's site: the duel full-screen, a page to watch two bots from their weights files and a seed, and the docs as pages. `site/build.sh` builds it into `site/dist`. |

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

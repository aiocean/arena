# Training a bot

Everything here runs from a checkout of the arena, [github.com/aiocean/arena](https://github.com/aiocean/arena)
(`git clone https://github.com/aiocean/arena`); Cargo fetches the pixfx engine at the pinned rev (`CONTRIBUTING.md`). Commands run from the repo root. The trainer is `train/` (Python, PyTorch, `uv`); the rules
it trains on are the same Rust rules the page plays, through `crates/pixfx-duel-py`. What a bot sees and
does is `docs/bot.md`.

## What you need

- Rust (the toolchain in `rust-toolchain.toml`), `uv`, Python 3.12 or 3.13 (`train/.python-version` picks 3.13; torch 2.8 has
  no wheels for 3.14).
- A fast machine for long runs. The env is the cost, and the trainer runs on CPU (CPU beats MPS for this network).
  Measured: about 42K env steps a second on the Mac mini the shipped AI trained on (30K each with two runs side by
  side), 6K-12K on a MacBook Air. A fine-tune of 600 updates is about 20M steps, roughly 8 minutes at 42K; a run from
  scratch (3000 updates, about 98M steps) about 40-45 minutes.

## Setup

```sh
git clone https://github.com/aiocean/arena
cd arena/train
uv sync
uv run maturin develop --release -m ../crates/pixfx-duel-py/Cargo.toml   # again after any change to the Rust rules
uv run pytest
```

## The environment

```python
import pixfx_duel as pd

env = pd.VecEnv(n=512, threads=8)          # 512 duels, stepped on 8 worker threads
env.set_model(0, pd.HOUSE)                 # an opponent to fight, as id 0: an ONNX model's bytes
env.reset(i, seed, learner, style, foe_style, opponent, model, limit)
obs, mask = env.observe()                  # float32[n, 354], bool[n, 30]
reward, done, won, length = env.step(actions, shaping)   # actions int64[n]
```

- `reset` starts env `i` on a fresh round. `learner` is the corner and so the school (0 Thunder, 1 Frost); `style` and
  `foe_style` are 0 aggressive, 1 balanced, 2 defensive; `opponent` is `"brain"`, `"first"`, `"aggressive"`,
  `"turtle"` or `"policy"` (then `model` is a `set_model` id, otherwise -1); `limit` is the round's time limit in
  seconds (the trainer uses 90).
- One step runs the duel to the learner's next decision. `done` marks the round's end; `won` is 1, -1, or 0 for a cut
  round; `length` is the round's seconds.
- A done env ignores its action until it is reset.
- The trainer's fight seeds start at 1,000,000 + 10,000,000 x `--seed` (11,000,000 by default). The ladder plays
  seeds 1-150; never train on them.

## The reward

Today the reward is computed in Rust (`crates/pixfx-duel/src/train.rs`), per decision:

| Term | Value |
|---|---|
| Round won / lost (a cut round is lost) | +1 / -1 |
| Damage | `shaping` x (dealt - taken) / 100, dealt and taken in % of a life (so 1.0 is a whole life), with `shaping` handed to `step` (the trainer anneals it to 0) |
| A fight gone quiet (no damage either way for 6 s) | -0.002 a decision |
| The same act (not wait) 6 times in a row | -0.002 |
| An act already 40% of the last 40 decisions | -0.004 |

To pay your bot differently today, edit those constants or `Env::step` and rebuild the module. A reward written in
Python, from facts `step` hands back, is the first item of `docs/roadmap.md`.

## The trainer

`ppo.py` is PPO in one file (CleanRL-style): 512 envs, 64-step rollouts, masked actions, an HL-Gauss critic.

```sh
uv run python ppo.py --run v1                                    # from scratch: 3000 updates
uv run python ppo.py --run v2 --init runs/v1/last.pt --updates 600 --lr 1e-4 --ent 0.003 0.002 --shaping-until 0
uv run python ppo.py --run v2 --resume runs/v2/last.pt           # continue a run that stopped
```

| Flag | Default | What |
|---|---|---|
| `--updates` | 3000 | PPO updates (each 512 x 64 steps) |
| `--envs`, `--steps`, `--threads` | 512, 64, 8 | rollout size and worker threads |
| `--lr` | 3e-4 | learning rate, falling linearly to 0 |
| `--ent` | 0.01 0.002 | entropy weight, from the first to the second over the run |
| `--shaping-until` | 0.6 | share of the run over which the damage term falls to 0 (0: none at all) |
| `--thunder` | 0.5 | share of fights in which the learner plays Thunder |
| `--snapshot-every` | 20 | how often the latest self joins the league |
| `--gamma`, `--lam` | 0.995, 0.95 | discount and GAE lambda |
| `--clip`, `--epochs`, `--minibatches` | 0.2, 4, 8 | PPO clip, passes over each rollout, minibatches per pass |
| `--device`, `--seed` | cpu, 1 | torch device; the seed for torch, numpy and the fight seeds |
| `--init` | | start from a `.pt` checkpoint or an ONNX model (the house AI's: `../crates/pixfx-duel/weights/policy.onnx`), at its size, with a fresh schedule |
| `--width`, `--hidden`, `--blocks` | 128, 256, 2 | the network's size for a run from scratch (any size a bot's fuel allows) |
| `--resume` | | continue a run, optimiser and schedule included; the league restarts with only the current policy (past selves are not saved) |

It writes `runs/<run>/log.jsonl` every 10 updates (speed, losses, entropy, round length, and the win rate against
each opponent, overall and per school: `win/brain`, `win/latest/thunder`, ...) and `runs/<run>/last.pt` every 25.

**Opponents** (`league.py`): its latest self 35%, a past self 25%, the brain 15%, the first mind 10%, the scripted
pair 15%; schools and styles drawn per fight.

**A lesson from the shipped AI.** Trained with each school half the time, it played Thunder worse: Thunder won 40% of
its own mirror while the rules were even (the brain's mirror: 50.9%). Fine-tuned with `--thunder 0.9`, Thunder won
49.1%. Retrained for the HD rules (`UNIT` 4), 600 updates from it at `--thunder 0.9` overshot (Thunder 60% of the
mirror), and 200 more from there at 0.5 or 0.7 undershot (43-44%); 200 at `--thunder 0.8` shipped (48.9%). Watch the
per-school win rates in the log; a school you lose with needs more of the training.

## Making it your own

| You want | Change |
|---|---|
| A different style of play | the reward (`train.rs` today) |
| Different sparring partners | `MIX` in `league.py`, or other ONNX models through `set_model` |
| Another algorithm or model | anything that drives `VecEnv` and ends in an ONNX model (`net.py`'s `to_onnx`) |
| A bigger network | `--width`, `--hidden`, `--blocks` |
| A head start | `--init ../crates/pixfx-duel/weights/policy.onnx` |
| More of one school | `--thunder` |

The inputs and acts are fixed for a season (`docs/bot.md`); the model is yours.

## Export and check

```sh
uv run python export.py runs/<run>/last.pt --out mine.onnx
```

It writes the policy as an ONNX model and `crates/pixfx-duel/tests/fixtures/policy_check.json`: 32 observations from
real fights and PyTorch's logits for them. Left without `--out`, it replaces the house AI
(`crates/pixfx-duel/weights/policy.onnx`, in your clone the AI the page and the ladder play); then check that Rust
computes what PyTorch did:

```sh
cargo test -p arena-bot --features onnx --test house
```

`--check FILE` writes the check elsewhere: pass it with `--out` when you keep the house AI, or the test compares the
house AI with your model's logits and fails.

## Bring your own model (ONNX)

Any model that maps the 354 inputs to the 30 acts' logits can be a bot, whatever its size or shape, trained with
any library. Export it to ONNX: input float32 `[1, 354]`, output float32 `[1, 30]` (opset 17 with PyTorch's legacy
exporter is what tract reads best). `export.py` does it for the trainer's network; for your own model:

```python
torch.onnx.export(model, (torch.zeros(1, 354),), "mine.onnx", opset_version=17, dynamo=False)
```

Build it into a bot module, then run the submission checks on it:

```sh
cargo run -p arena-bot --example onnx -- mine.onnx mine.wasm
cargo run --release -p arena-bot --features host --example check -- mine.wasm
```

The module (`bots/onnx`) runs the model with tract and samples the act as the shipped bot does (a softmax over the
legal acts with the arena's draw). To pick otherwise, keep memory between decisions or run something that is not
ONNX, change `bots/onnx/src/lib.rs` or write your own module (`docs/bot.md`, "The bot module"). The check prints the
fuel the module uses a decision; the budget is 20M (the house AI: about 3M).

To fight your model in training, hand its ONNX bytes to `set_model`.

## Measure it

```sh
cargo run --release -p arena-bot --features host,onnx --example ladder -- --seeds 150 --telemetry [--onnx FILE | --bot FILE.wasm]
```

The ladder plays seeds 1-150, each both ways (as Thunder, then as Frost) for 3 rounds, rounds cut at 120 s. The
bot (the house AI unless `--onnx` or `--bot` names another) plays the balanced style against the brain, the first mind and the scripts; only the mirror draws styles. A cut
ends that fight and shows in the `cut` column; win rates count finished rounds. It prints:

| Match | The shipped AI | Bar |
|---|---|---|
| vs the brain | 92.0% (Thunder 91.1%, Frost 92.9%) | at least 75%, 65% each school |
| vs the first mind | 90.4% | at least 80% |
| vs the aggressive and turtle scripts | 85.2%, 99.6% | |
| its own mirror, Thunder's share | 48.9% | 45-55% |

`--telemetry` adds what each school's skills did in each mirror: casts, landed, damage, guarded, broken, punished,
final blows.

## Watch it

Drop your module into a corner on [arena.aiocean.io](https://arena.aiocean.io/#insert), or on your own build
(`site/build.sh --serve`, then http://127.0.0.1:8771/): it takes the corner at once, the other corner keeps the house
bot or another module, and the seed box replays a fight. The same two modules and seed play the same fight anywhere.
To make your model the bot the home page plays, export it over the house AI (`crates/pixfx-duel/weights/policy.onnx`)
and build the site: `site/embed.sh` builds the house module from it.
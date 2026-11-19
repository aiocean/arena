# Training a bot

Everything here runs from a checkout of the arena, [github.com/aiocean/arena](https://github.com/aiocean/arena)
(`git clone https://github.com/aiocean/arena`). Commands run from the repo root. The trainer is `train/` (Python, PyTorch, `uv`); the rules
it trains on are the same Rust rules the page plays, through `crates/arena-duel-py`. What a bot sees and
does is `docs/bot.md`.

## What you need

- Rust (the toolchain in `rust-toolchain.toml`), `uv`, Python 3.12 or 3.13 (`train/.python-version` picks 3.13).
- A fast machine for long runs. The env is the cost, and the trainer runs on CPU (CPU beats MPS for this network).
  Measured: about 42K env steps a second on the Mac mini the shipped AI trained on (30K each with two runs side by
  side), 6K-12K on a MacBook Air. A fine-tune of 600 updates is about 20M steps, roughly 8 minutes at 42K; a run from
  scratch (3000 updates, about 98M steps) about 40-45 minutes.

## Setup

```sh
git clone https://github.com/aiocean/arena
cd arena/train
uv sync
uv run maturin develop --release -m ../crates/arena-duel-py/Cargo.toml   # again after any change to the Rust rules
uv run pytest
```

## The environment

```python
import arena_duel as pd

env = pd.VecEnv(n=512, threads=8)          # 512 duels, stepped on 8 worker threads
env.set_model(0, pd.HOUSE)                 # an opponent to fight, as id 0: an ONNX model's bytes
env.reset(i, seed, learner, style, foe_style, opponent, model, limit)
obs, mask = env.observe()                  # float32[n, 1889], bool[n, 27]
reward, done, won, length = env.step(actions, shaping, whiff, whiff_far, variety)   # actions int64[n]; all but shaping default to 0
env.casts()                                # the learners' casts since the last call, by kind (below)
```

- `reset` starts env `i` on a fresh round. `learner` is the seat (0 left, 1 right; each fight draws both seats' loadouts, and the fighters' looks from its seed,
apart from the seats: `ladder::looks_of`); `style` and
  `foe_style` are 0 aggressive, 1 balanced, 2 defensive; `opponent` is `"brain"`, `"first"`, `"aggressive"`,
  `"turtle"` or `"policy"` (then `model` is a `set_model` id, otherwise -1); `limit` is the round's time limit in
  seconds (the trainer uses 90).
- One step runs the duel to the learner's next decision. `done` marks the round's end; `won` is 1, -1, or 0 for a cut
  round; `length` is the round's seconds.
- A done env ignores its action until it is reset.
- `casts()` sums the learners' casts since it was last called, over every env, by kind in `data::Kind` order (blast,
  bolt, orb, zone, void, heal, ward, barrier, blink): `(casts, out_of_reach, whiffs, far_whiffs, damage)` each, and beside that list the damage the learners'
  actives (their loadouts' skills, not the basic blink) dealt with their blows. A cast
  is out of reach when its blast's reach does not get to the foe's hurtboxes as it begins; a whiff
  is a damaging skill cast as its motion that lands nothing (no hit, not guarded, not turned aside) once it and its
  shots are over; a far whiff is one that began out of reach; `damage` is in % of a life.
- The trainer's fight seeds start at 1,000,000 + 10,000,000 x `--seed` (11,000,000 by default). The ladder plays
  seeds 1-150; never train on them.

## The reward

Today the reward is computed in Rust (`crates/arena-duel/src/train.rs`), per decision:

| Term | Value |
|---|---|
| Round won / lost (a cut round is lost) | +1 / -1 |
| Damage | `shaping` x (dealt - taken) / 100, dealt and taken in % of a life (so 1.0 is a whole life), with `shaping` handed to `step` (the trainer anneals it to 0) |
| A fight gone quiet (no damage either way for 6 s) | -0.002 a decision |
| The same act (not wait) 6 times in a row | -0.002 |
| An act already 40% of the last 40 decisions | -0.004 |
| A damaging cast that lands nothing (a whiff, as `casts()` counts it) | `whiff` handed to `step` (0 by default) |
| A whiff begun with the foe out of reach (a far whiff) | `whiff_far` handed to `step`, on top of `whiff` (0 by default) |
| Every decision (about 8 a second) | `-time_cost`, taken in `ppo.py` (`--time-cost`, 0 by default) |
| Damage the learner's actives deal with their blows (not slash, dive or blink) | `variety` x dealt / 100 (0 by default) |
| A decision that could cast (a cast is legal: a skill ready and paid for) and does anything else | `-idle` handed to `step` (0 by default) |
| A damaging cast that lands (hit, guarded or turned aside) | `land` handed to `step` (0 by default) |

To pay your bot differently today, edit those constants or `Env::step` and rebuild the module. A reward written in
Python, from facts `step` hands back, is the first item of `docs/roadmap.md`.

## The trainer

`ppo.py` is PPO in one file (CleanRL-style): 512 envs, 64-step rollouts, masked actions, an HL-Gauss critic.

```sh
uv run python ppo.py --run v1                                    # from scratch: 3000 updates
uv run python ppo.py --run v2 --init runs/v1/last.pt --updates 600 --lr 1e-4 --ent 0.003 0.002 --shaping-until 0 --whiff 0.01
uv run python ppo.py --run v2 --resume runs/v2/last.pt           # continue a run that stopped
```

| Flag | Default | What |
|---|---|---|
| `--updates` | 3000 | PPO updates (each 512 x 64 steps) |
| `--envs`, `--steps`, `--threads` | 512, 64, 8 | rollout size and worker threads |
| `--lr` | 3e-4 | learning rate, falling linearly to 0 |
| `--ent` | 0.01 0.002 | entropy weight, from the first to the second over the run |
| `--shaping-until` | 0.6 | share of the run over which the damage term falls to 0 (0: none at all) |
| `--whiff` | 0 | reward cost of a damaging cast that lands nothing |
| `--variety` | 0 | reward weight of the damage the actives' blows deal (a bonus for using the whole kit) |
| `--whiff-far` | 0 | reward cost of a whiff begun with the foe out of reach (a blow in reach that misses costs only `--whiff`) |
| `--mix` | 0.35 0.25 0.15 0.10 0.15 | the league's opponent shares: latest self, a past self, the brain, the first mind, the scripted pair; or `kind=share` for those and `pool`, `target`, `aggressive`, `turtle` (one script alone) |
| `--pfsp` | 0 | draw a past self or a pool model with weight (1 - p)^k, p the learner's recent win rate against it (0: uniform) |
| `--pool` | | folders whose `*.onnx` are the `pool` opponents (the pool's `runs/pool/models`, another run's `league/`), read again at each snapshot |
| `--target` | | the `target` opponent: a model, or a folder whose newest `*.onnx` is fought (an exploiter aimed at a main run's `league/`) |
| `--stop-at` | | stop and save at this update, the schedule still the `--updates` one |
| `--time-cost` | 0 | reward cost of every decision, so a round that drags on costs more than one closed |
| `--idle` | 0 | reward cost of a decision that could cast and moves, jumps, guards, meditates or waits instead |
| `--land` | 0 | reward for each damaging cast that lands: with `--idle`, holding the skills back costs, against `--whiff` |
| `--seat0` | 0.5 | share of fights in which the learner sits in seat 0 |
| `--keep-every` | 0 | also keep `runs/<run>/u<update>.pt` every so many updates (a multiple of 25), for the ladder |
| `--snapshot-every` | 20 | how often the latest self joins the league |
| `--gamma`, `--lam` | 0.995, 0.95 | discount and GAE lambda |
| `--clip`, `--epochs`, `--minibatches` | 0.2, 4, 8 | PPO clip, passes over each rollout, minibatches per pass |
| `--device`, `--seed` | cpu, 1 | torch device; the seed for torch, numpy and the fight seeds |
| `--init` | | start from a `.pt` checkpoint or an ONNX model (the house AI's: `../crates/arena-duel/weights/policy.onnx`), at its size, with a fresh schedule |
| `--width`, `--hidden`, `--blocks` | 128, 256, 2 | the network's size for a run from scratch (any size a bot's fuel allows) |
| `--resume` | | continue a run, optimiser and schedule included; the league reads its past selves back from `runs/<run>/league/` |

It writes `runs/<run>/log.jsonl` every 10 updates (speed, losses, entropy, round length, and the win rate against
each opponent, overall and per seat: `win/brain`, `win/latest/seat0`, ...; the share of each act, `acts`; and the
learner's casts: `casts`, `whiff`, `out_of_reach`, `far_whiff`, `active_dmg`, `dmg_cast`, `blast_out_of_reach`, `blast_whiff`) and
`runs/<run>/last.pt` every 25.

**Opponents** (`league.py`): its latest self 35%, a past self 25%, the brain 15%, the first mind 10%, the scripted
pair 15%; seats, styles and both loadouts drawn per fight. Every snapshot is kept as `runs/<run>/league/<n>.onnx`.

**A wider observation.** An ABI that appends inputs keeps the old ones at their indices, so a trained policy carries
over: `uv run python widen.py runs/<run>/last.pt runs/<new>/init.pt --old-obs <n>` copies its first layer into the
new network's first columns, the new columns at zero, and checks that with the new inputs at zero both give the same
logits. `ppo.py --init runs/<new>/init.pt` then fine-tunes it to use them. ABI 3's house AI started so, from ABI 2's, and ABI 4's from ABI 3's (`--old-obs 1741`).

**The state of the house AI** (2026-11-19, RULES_VERSION 24: qi regen 8/s, a jump costs 8 qi, flight 8 qi/s). The
shipped AI is update 1200 of run `abi6b`, two fine-tunes from `abi5a/u1200` paid to cast: `abi5c` (800 updates on
RULES 23) then `abi6b` (1200 on RULES 24), each `--lr 1e-4 --ent 0.002 0.0015 --shaping-until 3.0 --mix 0.2 0.15 0.5
0.05 0.1 --time-cost 0.0005 --whiff-far 0.02 --variety 0.5` with `--idle 0.003 --land 0.03 --whiff 0.01`, then
`--idle 0.004 --land 0.04 --whiff 0.01`. On RULES 23 the idle and land terms barely raised the casts (the house AI
spent 93% of the qi it earned, its casts 34 qi each): it ducked instead of jumping. On RULES 24 they doubled them.
A gentler run (`abi6a`, from `abi5b/u800`: idle 0.002, land 0.03, whiff 0.015) came out a little weaker (u1200: 77.2%
against the brain, 10.5 casts a minute in the mirror).

Its ladder (150 seeds, RULES_VERSION 24), `abi5a/u1200`'s on the same rules in brackets where measured:
- vs the brain: 78.5% [71.4] (seats 82.4 / 74.8); vs the first mind: 87.7%; vs the aggressive and turtle scripts:
  62.9% and 99.8%;
- its own mirror: seat 0 48.8%; 10.7 hurting casts a minute a seat [5.4], 5.4 whiffs; off the ground 27% of the
  time [58%], guarding 3%; its heaviest skill 4% of its damage. The bar asked 15 casts a minute: not reached.

**RULES_VERSION 23** (casts 1.6x/1.5x slower, meditation a skill in phases).
The shipped AI was update 1200 of run `abi5a`, `abi4a`'s update 800 fine-tuned 1500 updates on RULES_VERSION 23 with
its recipe (`--init runs/abi4a/u800.pt --lr 1e-4 --ent 0.003 0.0015 --shaping-until 3.0 --mix 0.25 0.15 0.4 0.1 0.1
--time-cost 0.0005 --whiff 0.03 --whiff-far 0.02 --variety 0.5 --keep-every 100`). On RULES 23 `abi4a/u800` won
47.1% against the brain over the ladder; it meditated most, and a meditating fighter is now open. Over the house
test's seeds 1-30 against the brain, `abi5a`'s updates 600-1500 won 56, 66, 78, 74, 73, 74, 86, 70, 71 and 73%: only
800 and 1200 cleared it, and 1200 the ladder too (u1000: 77.4% over the ladder, 73.2% over the house seeds).

Its ladder (150 seeds, RULES_VERSION 23), `abi4a/u800`'s on the same rules in brackets:
- vs the brain: 79.9% [47.1] (seats 82.9 / 76.8; Vương Lâm 85.8 / Thất Thải 73.5), 142 rounds cut of 900 [135];
- vs the first mind: 91.8% [77.6], 180 cut [192]; vs the aggressive and turtle scripts: 78.5% [62.1] and 99.0% [98.5];
- its own mirror: seat 0 55.4%, 152 cut, rounds 61.6 s (tracked, not gated); 8.8 casts a round a seat against 13.0
  for `abi4a` on RULES 22, about 45% fewer a second. Its top act is the jump, not meditating.

**RULES_VERSION 22** (spell knockback, bodies kept apart): update 800 of run `abi4a`, `abi3z`'s update 800 (below)
fine-tuned 800 updates on RULES_VERSION 22 with its recipe
(`--init runs/abi3z/u800.pt --lr 1e-4 --ent 0.003 0.0015 --shaping-until 3.0 --mix 0.25 0.15 0.4 0.1 0.1`, the same
costs). On RULES 22 `abi3z/u800` still won 81.5% against the brain over the ladder but 72.7% over the house test's
seeds 1-30; `abi4a`'s updates 200-600 were no better, 700 and 800 cleared both. A continuation (`abi4b`, brain 50%)
did not improve in training and was stopped.

Its ladder (150 seeds, RULES_VERSION 22), `abi3z/u800`'s on the same rules in brackets:
- vs the brain: 83.5% [81.5] (seats 83.3 / 83.7; Vương Lâm 86.6 / Thất Thải 80.5), 65 rounds cut of 900 [50];
- vs the first mind: 94.3% [94.2], 75 cut [82]; vs the aggressive and turtle scripts: 88.3% [88.4] and 99.0% [99.6];
- its own mirror: seat 0 49.4%, 53 cut; Vương Lâm wins 66.2% of its rounds (tracked, not gated).
- Whiffs: 3.2 a minute, 0.52 begun out of reach.
- Distance: against the brain a median gap of 198 px, within 80 px 1.0% of the time; against the aggressive script
  109 px and 11.5%; in its mirror 135 px and 8.1% (the ladder's `median` and `<80` columns).

**RULES_VERSION 21** (spells only, the blink the one basic): update 800 of run `abi3z`. The blade-era AI (`abi3/u1600`) cast almost nothing on these rules: 9.0% against the
brain on RULES_VERSION 20, every mirror fight cut. The way here, every run with `--whiff 0.03 --whiff-far 0.02
--variety 0.5 --time-cost 0.0005`:

1. `abi3x`, from scratch (3000 updates, the default schedule; RULES_VERSION 20, then 21 from update 710). A warm
   start from the RULES-19 AI (`abi3v`) was tried beside it and dropped: it kept the old AI's habit of casting from
   one bar slot only and stalled near 50% against the brain. `abi3x` peaked near update 1000-1400 (55% against the
   brain, 84% against the first mind) and drifted once its damage term reached 0. Stopped at 2000.
2. `abi3y`: `--init runs/abi3x/u1400.pt --lr 1e-4 --ent 0.004 0.002 --shaping-until 0.5 --mix 0.25 0.15 0.4 0.1 0.1`:
   the brain 40% of the league instead of 15%. Update 400: 74.0% against the brain. It drifted again once the damage
   term reached 0.
3. `abi3z`: `--init runs/abi3y/u400.pt --lr 1e-4 --ent 0.003 0.0015 --shaping-until 3.0`, the same mix. A
   `--shaping-until` over 1 keeps a floor under the damage term (here from 1 to 0.67 over 800 updates), and the drift
   did not come: every update laddered from 200 on (200, 300, 500, 600, 800) clears the win bar.

Every run without a floor drifted after its damage term reached 0. Keep the checkpoints (`--keep-every 100`) and
ladder them; do not trust the last one.

Its ladder (150 seeds, RULES_VERSION 21):
- vs the brain: 82.1% (seats 82.5 / 81.7; Vương Lâm 84.3 / Thất Thải 79.9), 60 rounds cut of 900;
- vs the first mind: 94.5% (seats 92.6 / 96.4), 93 cut;
- vs the aggressive and turtle scripts: 88.7% and 99.3%, 30 cut each;
- its own mirror: seat 0 50.9%, 53 cut; Vương Lâm wins 65.4% of its rounds (tracked, not gated).
- Rounds: 35-49 s. Whiffs: 3.2 a minute, 0.51 begun out of reach (the `whiff` example).
- Damage in its mirror: burns 14%, bleeds 7%, and no skill more than 3.6%: 33 skills and ailments deal at least 1%.

The cut bar (fewer than 5% of rounds) is not met against the brain (6.7%) or the first mind (10.3%), and the rules
themselves do not meet it: on RULES_VERSION 21 the brain against the first mind cuts 151 rounds of 900 (16.8%), the
brain's own mirror 54 (6.0%). A bar skill is ready in only about 4% of decisions each (cooldowns, qi), so rounds are
slow whoever fights them.

Next:
1. Fewer cut rounds: a larger `--time-cost`, or a cut costing more than a loss.
2. Fine-tune from `runs/abi3z/u800.pt`, kept in the trainer's clone (`projects/arena-train`; `runs/` stays out of
   git).
3. Ship only a checkpoint that beats this one on the ladder, on the rules it ships with.

Watch the per-seat and per-look win rates in the log.

## Making it your own

| You want | Change |
|---|---|
| A different style of play | the reward (`train.rs` today) |
| Different sparring partners | `MIX` in `league.py`, or other ONNX models through `set_model` |
| Another algorithm or model | anything that drives `VecEnv` and ends in an ONNX model (`net.py`'s `to_onnx`) |
| A bigger network | `--width`, `--hidden`, `--blocks` |
| A head start | `--init ../crates/arena-duel/weights/policy.onnx` |
| More of one seat | `--seat0` |

The inputs and acts are the arena's (`docs/bot.md`); the model is yours.

## Export and check

```sh
uv run python export.py runs/<run>/last.pt --out mine.onnx
```

It writes the policy as an ONNX model and `crates/arena-duel/tests/fixtures/policy_check.json`: 32 observations from
real fights and PyTorch's logits for them. Left without `--out`, it replaces the house AI
(`crates/arena-duel/weights/policy.onnx`, in your clone the AI the page and the ladder play); then check that Rust
computes what PyTorch did:

```sh
cargo test -p arena-bot --features onnx --test house
```

`--check FILE` writes the check elsewhere: pass it with `--out` when you keep the house AI, or the test compares the
house AI with your model's logits and fails.

## Bring your own model (ONNX)

Any model that maps the 1889 inputs to the 27 acts' logits can be a bot, whatever its size or shape, trained with
any library. Export it to ONNX: input float32 `[1, 1889]`, output float32 `[1, 27]` (opset 17 with PyTorch's legacy
exporter is what tract reads best). `export.py` does it for the trainer's network; for your own model:

```python
torch.onnx.export(model, (torch.zeros(1, 1889),), "mine.onnx", opset_version=17, dynamo=False)
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

The ladder plays seeds 1-150, each both ways (in seat 0, then in seat 1) for 3 rounds, rounds cut at 120 s. The
bot (the house AI unless `--onnx` or `--bot` names another) plays the balanced style against the brain, the first mind and the scripts; only the mirror draws styles. A cut
ends that fight and shows in the `cut` column; win rates count finished rounds. `--rounds` and `--limit` change the
rounds a fight and the seconds a round; `--telemetry-dir DIR` writes each row to the telemetry files (`ops/telemetry`). It
prints:

| Match | Bar |
|---|---|
| vs the brain | at least 75%, 65% in each seat and in each look |
| vs the first mind | at least 80% |
| vs the aggressive and turtle scripts | |
| its own mirror, seat 0's share | 45-55% |

Under the table it prints the bot's share in each look against each opponent, and for each mirror the share of rounds
each look won (each seed is fought in both looks).

**The look gap.** A new house AI ships when it beats the shipped one on the ladder, on the rules it ships with. Its
look gap (Vương Lâm's share against Thất Thải's) is judged against the built-in opponents, with a margin for noise: 150
seeds give each look about 450 rounds, a point or two either way. The house AI's own mirror look share is tracked,
not gating, while Thất Thải's slash is drawn backwards (`art/README.md`): a stronger policy leans on that defect harder
in self-play (Vương Lâm won 67% of the house mirror at the 2026-11-18 ship, against 53% in the brain's mirror, which
shows the rules' own fairness). Since RULES 19 no one slashes (the arena is spells only), so the defect plays no part
any more; the look share is to be measured again on the new rules.
`--telemetry` adds what each seat's skills did in each mirror: casts, landed, damage, guarded, broken, punished,
final blows.

## Getting better over time

The ladder measures a bot against fixed opponents. Whether a model is better than the one before it, what beats it
and which reward terms helped, the trainer answers with no players at all
(`docs/specs/2026-11-19-model-improvement-design.md`):

```sh
cd train
uv run python pool.py init && uv run python pool.py add runs/abi6b/u800.pt runs/abi6b/u1200.pt
uv run python pool.py play && uv run python pool.py table      # ratings, the Nash mixture, the cycles
uv run python exploit.py ../crates/arena-duel/weights/policy.onnx --run x-house --updates 300
uv run python pbt.py --run pbt1 --init ../crates/arena-duel/weights/policy.onnx -- --shaping-until 3.0 --pfsp 2
uv run python cycle.py --run c25 --pbt --exploit 300 --review ~/services/arena/site/review/c25
```

- **The pool** (`pool.py`, `runs/pool/`): the built-in minds, the house AI and any checkpoint (of any ABI: a narrower
  one is widened), every pair fought over seeds 500,000-500,039 both ways (apart from the ladder's), all fought
  again when the rules change. Its table rates each member by a Bradley-Terry fit on the Elo scale (the brain at
  1000, with ±2 standard errors), gives each member's payoff against the meta-game's Nash mixture (0 for the members
  nothing beats on balance), and lists the cycles (A beats B beats C beats A, each by 55%). Pick a checkpoint by its
  rating there; the entry bar stays the gate.
- **Exploitability** (`exploit.py`): a best response fine-tuned from the model against the model alone; its share of
  the rounds against the model over the pool's seeds, less a half. Near 0, no hole found.
- **The league** keeps its past selves on disk, draws by PFSP (`--pfsp 2`), and takes other folders' models
  (`--pool`); a main run and an exploiter run side by side: the exploiter with `--target runs/main/league --mix
  target=1`, the main run with `--pool runs/pool/models runs/exploit/league`.
- **PBT** (`pbt.py`): 4 members by default, each a run with its own `--lr`, `--ent`, `--idle`, `--land`, `--whiff`,
  `--whiff-far`, `--variety`, `--time-cost`; every 100 updates they fight one another, the brain and the house AI,
  fitness is the share of rounds won, the bottom quarter copies the top quarter and perturbs its values. Member 0
  starts on the house AI's recipe. `runs/<run>/pbt.jsonl` is each generation; `runs/<run>/best/` its best members.
- **The cycle** (`cycle.py`): trains (PBT or one run) from the house AI, rates the candidates in the pool, measures
  the best three against the bar, the house AI and (`--exploit`) a best response, which joins the pool for the next
  cycle's league to fight, and writes
  `runs/<run>/report.md` and a review page. It ships nothing; shipping the pick is "Export and check", above.

### Read it (a decision model as a reader)

`train/judge.py` has a policy fight a few dozen rounds and asks TypeSafe's Jev (`typesafe/jev-1.14`, a decision
model on OpenRouter's Decisions API: typed answers with probabilities, no text) how each round reads (deliberate,
risky, dull, which pattern) and what it makes of a few hundred single moves, set beside the critic's advantage. Each
moment is put in words by `arena_duel::words` (`VecEnv.describe`). It writes `rounds.json`, `summary.json` and a
review page; nothing it says feeds a reward.

```sh
uv run python judge.py ../../arena-train/train/runs/abi6b/u1200.pt --rounds 48 --moves 300 \
    --env-file ~/projects/game-factory/ops/.env --out runs/judge-abi6b --review ~/services/arena/site/review/jev-train
```

What it found on abi6b/u1200 (2026-11-19; 348 requests, p50 0.64 s, $0.016): asked twice it answers the same; its
move verdicts have nothing to do with the critic's (Spearman -0.04) nor the policy's own choice (-0.08), because the
words leave out what decides a move here (frame data, who a blow can reach; it calls walking at a foe winding up a
palm blast a sure mistake while flying over it wins); its round scores mostly follow what code counts (risk and dull
with meditations, about 0.6). It called 31 of 48 rounds a turtle's, the duck share the trainer's log shows. Read it
as a second pair of eyes on style, never as a judge of play.

## Watch it

Drop your module into a corner on [arena.aiocean.io](https://arena.aiocean.io/#insert), or on your own build
(`site/build.sh --serve`, then http://127.0.0.1:8771/): it takes the corner at once, the other corner keeps the house
bot or another module, and the seed box replays a fight. The same two modules and seed play the same fight anywhere.
To make your model the bot the home page plays, export it over the house AI (`crates/arena-duel/weights/policy.onnx`)
and build the site: `site/build.sh` builds the house module from it.
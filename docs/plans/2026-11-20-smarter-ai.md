# A much smarter house AI (2026-11-20)

The owner's words: "Đổi ABI luôn cũng được, làm hết mọi bước luôn cũng được, ta cần một thứ thông minh hơn rất nhiều."
Every step below is approved, the ABI bump included. This page is the to-do list and its state; each phase lands
on `main` as it is done.

## Where we stand

- The policy is a small memoryless MLP (`train/net.py`): the 1889 inputs are squeezed into 128 at once, then two
  residual blocks of 128 -> 256 -> 128, a simplicial embedding, and 27 logits. The critic is an HL-Gauss head on
  the same trunk.
- Training is PPO with PBT and a league: a pool, PFSP, best responses (`exploit.py`), and cycles that run without
  end (`cycle.py`, `docs/plans/2026-11-19-model-improvement.md`).
- What is wrong:
  - It does not see its own state. "Me" has no ducking, guarding or meditating flag and no memory of its last
    acts, though the foe's block has them. The house AI ducks ~45 times a minute and flickers ~18 times a minute;
    the `--flicker` and `--duck` costs treat the symptom.
  - A cycle picks on strength alone: c27/g010 and c28/g009 rated +40-50 Elo over the house AI but flew twice as
    often and cast less.

## The phases

### A. A pick must look as good as it plays (no ABI change)

`cycle.py` measures how each top candidate fights (`judge.py`'s counts, no Jev): flicker, ducks and take-offs a
minute, casts a minute. A candidate whose flicker, ducks or take-offs exceed the house AI's by more than 10%, or
whose casts fall more than 10% short, does not clear the bar.

### B. Asymmetric critic (no ABI change)

The critic is used only in training, so it may see what the policy cannot:
- the foe's true current act (no reaction delay) and its next queued act;
- every cooldown and qi;
- the round clock.

The env hands these to Python as a second array, and the value head reads trunk + privileged features. The
exported policy is unchanged.

### C. ABI 5: it knows itself, holds its acts, remembers the foe (RULES bump)

Appended to the observation, so 0..1889 keep their meaning and `widen.py` carries a model over:
- **Me, now:** ducking, guarding, meditating, landing / taking off, how long it has held its current act.
- **My last acts:** the last 3 acts I chose, each as its plain kind (9) or a cast (slot) plus seconds ago.
- **The foe's last acts:** the last 4 things the foe was seen to start (its doing or a skill kind, as the delayed
  view shows them), each with seconds ago.

Acts with a duration: a long duck and a long guard (1 s) beside the short ones (0.4 s, kept as they were so an old
model's acts keep their meaning), which takes ACTIONS from 27 to 29. The new acts are appended so old indices keep their meaning; `widen.py`
also widens the policy head.

The ABI change needs, together:
- `RULES_VERSION` + 1 and `ABI` + 1;
- `docs/bot.md`;
- the templates in `bots/`;
- the house AI retrained;
- the goldens re-pinned;
- native = wasm checked;
- server and page deployed together.

### D. A bigger brain, distilled first (no ABI change beyond C)

- A shared encoder per repeated entity (6 slots, 4 threats, 8 units, 6 foe slots) feeding the trunk, with a
  wider trunk (256, 3-4 blocks).
- It starts by imitating the house AI and the pool's best (KL on their logits, value from returns), then trains
  with PPO.
- tract must run it, and the module must stay within the bot's fuel and time.

### E. Faster env

Profile it, then fix the top items: batched opponent inference, less copying, cheaper observation building. More
samples per hour is the cheapest strength there is.

### F. A standing league of exploiters

Exploiters run without end against the newest house candidate and against the league's mixture, and join the pool,
so holes are found as the main agent trains rather than once a cycle.

### G. Search at training time (expert iteration)

The duel is deterministic from a seed. At chosen decisions, clone the duel and roll each legal act out a few
decisions with the current policy. The best act becomes a target the policy imitates (plus PPO). The bot still
runs one forward pass. It needs a cheap clone of a running duel.

### H. Jev calibrated by the owner

A page of ~50 clips the owner labels (jittery? aimless? skilled?). Jev is scored against them and its prompt
tuned. Once it agrees with the owner, it joins the pick's look gate.

## Order

1. A, B (now)
2. C (with its retrain)
3. D + E
4. F
5. G
6. H as soon as the owner has labelled clips

## Status

| phase | state |
|---|---|
| A | done: `cycle.py` look gate (96 rounds, 10% slack) |
| B | landed: `ppo.py --privileged` (the value head also reads the duel from the foe's side, undelayed; the bot unchanged) |
| C | landed (RULES 27, ABI 5: 2055 inputs, 29 acts; the house AI widened, plays as before); retrain next |
| D | – |
| E | profiled (below) |
| F | – |
| G | the clone is cheap and exact (below) |
| H | – |

## What the study found (2026-11-20)

Measured while four trainers ran, so absolute speeds are pessimistic.

**The clone (G).** Everything a running duel holds can be copied except `Seats` (`Box<dyn Seat>`, `duel.rs`):
- the minds lack only a `derive(Clone)`;
- the ONNX seat keeps its model behind an `Arc`;
- the learner's seat needs a fresh wire.

A prototype adds `Seat::fork`, `Duel::fork_with` / `restore_with` and `Env::fork` / `restore_from`.
- A restore into a spare env costs 20-28 µs, against 150-240 µs a decision.
- Replaying from the seed and the act log costs about 1000 times more.
- A fork stepped with the same acts matched the original bit for bit, 20 of 20 against each opponent.
- Every legal act rolled out 8 decisions costs 8.5 ms a root decision against the brain, 15 ms against ONNX.

A rollout must reseed the rules' dice and the foe's, or the search would know the future crits and samples.

**The env (E).** Decisions a second on one thread: first 10.8k, brain 6.5k, aggressive 6.1k, ONNX 4.2k. Where the
time goes:

| share | what |
|---|---|
| 30% | `sense_of` every tick for each policy seat, though a decision is due every 7-18 ticks; inside it, `KitSkill::costs` allocates a `Vec` per call |
| 22% | the ONNX opponent (tract, 70 µs a call at batch 1) |
| 17.5% | `unit::gather`/`scatter`: a fresh query state each tick, units or none |
| 1% | `observe` |

In `ppo.py`:
- the main thread waits on the workers 61% of the time;
- the workers sit idle 27-38% while Python runs the policy;
- 11% goes to copying observations into numpy.

The fixes, by size:
1. Sense in full only when a decision is due, and cache the kit's costs (+25-35%).
2. Cheap unit gather/scatter (+15-20%).
3. Overlap Python with the env: step half the envs while torch runs the other half, writing into preallocated
   buffers (+30-40% wall).
4. Then batch the ONNX opponents.

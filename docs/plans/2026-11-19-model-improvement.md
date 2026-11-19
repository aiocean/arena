# Making the house AI better over time: what is done, what runs, what is next

**Spec:** `docs/specs/2026-11-19-model-improvement-design.md`. How to run each tool: `docs/training.md`, "Getting
better over time".

**Goal:** the house AI gets better on its own, with nothing but the rules, the trainer and the desktop: there are no
players yet, so nothing here waits for a submitted bot or a human game.

## Where the methods come from

| Method | Who did it | Here |
|---|---|---|
| Rate each checkpoint against a fixed pool | OpenAI Five (TrueSkill), AlphaStar (Elo) | `pool.py`: Bradley-Terry on the Elo scale, all pairs played |
| Non-transitive games: Nash mixture, cycles | Nash averaging, α-Rank, PSRO | `pool.py table`: payoff against the Nash mixture, cyclic triples |
| A league that remembers, PFSP | AlphaStar | `league.py`: snapshots on disk, `--pfsp` |
| Exploiters that find holes, fed back to the league | AlphaStar (main and league exploiters) | `exploit.py`; `cycle.py` adds each best response to the pool |
| Exploitability as a measure | game theory, poker AIs | `exploit.py`: a best response's edge |
| Reward terms and hyperparameters chosen by winning | DeepMind FTW (population-based training) | `pbt.py` |
| Carry weights across a game change | OpenAI Five ("surgery") | `widen.py`, already here |
| Search, then distil into the network | AlphaZero, MuZero | not now |

Two corrections to a first reading: OpenAI Five kept dense shaped rewards to the end, so shaping is not the fault;
choosing its weights by hand each time is, which PBT replaces. And at one desktop's scale a full AlphaStar league is
too much: one pool, one population of four, one exploiter at a time.

## Done

- [x] The pool, its rating, the Nash mixture, the cycles (`train/pool.py`; the pool is `train/runs/pool/` in the
      `projects/arena-league` clone, out of git).
- [x] Exploitability (`train/exploit.py`).
- [x] A league that keeps its past selves, PFSP, other folders' models, a target, a named mix with each script alone
      (`train/league.py`, `train/ppo.py`).
- [x] PBT (`train/pbt.py`).
- [x] The cycle: train, rate, bar, exploit, report and review page; its best responses join the pool
      (`train/cycle.py`).
- [x] Commits f1a8c3e and c4aa065; the trainer's tests (33).

## What the pool said first (RULES 25, seeds 500,000-500,039)

| member | Elo | ±2se |
|---|---|---|
| abi6b/u1200 | 1293 | 27 |
| abi6b/u800 | 1291 | 28 |
| house (abi6b, widened) | 1286 | 27 |
| abi6b/u400 | 1256 | 27 |
| the aggressive script | 1206 | 26 |
| abi5a/u1200 | 1199 | 28 |
| abi4a/u800 | 1160 | 27 |
| brain | 1000 | 0 |
| first mind | 910 | 32 |
| turtle | 458 | 67 |

- The house AI and abi6b/u1200 (the same weights) agree within their error: the measure holds.
- abi6b/u800 is the Nash mixture on its own: nothing beats it on balance.
- The aggressive script rates above abi5a. It is the house AI's weak spot (88.3% against it on RULES 22, 62.9% on
  RULES 24), and c26 leans on it.
- No cycle yet.

## Running

Both run as launchd jobs on the desktop, from the `projects/arena-league` clone. Neither ships anything.

### c25 (`io.aiocean.arena-cycle-c25`, `~/cycle-c25.log`)

PBT, 4 members x 12 generations x 100 updates, from abi7a/u1200 (the house AI since 8217f02);
`--shaping-until 3.0 --pfsp 2 --pool runs/pool/models --mix latest=0.2 past=0.15 brain=0.4 first=0.05 scripted=0.1
pool=0.1`; then the pool, the bar and a 300-update exploit for the best three. Started 19:34, about 16 minutes a
generation; report at https://arena.aiocean.io/review/c25/ around midnight.

The first four generations (fitness: the share of rounds won against the other members, the brain and the house
AI):

| gen | m0 | m1 | m2 | m3 | copied |
|---|---|---|---|---|---|
| 1 | 0.567 | **0.593** | 0.519 | 0.572 | m2 <- m1 |
| 2 | 0.495 | 0.605 | 0.575 | **0.635** | m0 <- m3 |
| 3 | 0.572 | 0.583 | 0.585 | **0.607** | m0 <- m3 |
| 4 | 0.522 | 0.575 | **0.615** | 0.613 | m0 <- m2 |

m0 started on today's recipe (`--idle 0.004 --land 0.04 --whiff 0.01 ...`) and came last three generations of
four. Every generation's best has a lower idle cost (0.0014-0.0018 against 0.004); the land reward goes either way
(0.02 to 0.11). Early, but the first lead: the idle cost the house recipe pays is too high.

c25 measures against the old house AI (abi6b): its clone predates 8217f02. abi7a/u1200 is in the pool, so the table
still says whether a candidate beats the new one.

### c26 (`io.aiocean.arena-cycle-c26`, `~/cycle-c26.log`), after c25

1. Pull main into the clone and rebuild (the house AI is abi7a from here on).
2. Add c25's three best responses to the pool, and a new one trained against abi7a.
3. PBT from c25's pick (the house AI if none clears the bar), with the holes as opponents:
   `--mix latest=0.15 past=0.1 brain=0.25 first=0.05 scripted=0.05 aggressive=0.15 pool=0.25 --pfsp 2`.
4. The pool, the bar, the exploits (which join the pool for c27), the report:
   https://arena.aiocean.io/review/c26/.

## How a pick ships

A candidate is better when it rates above abi7a/u1200 in the pool, clears the bar, beats the house AI head to head,
is less exploitable than it, and wins more against the aggressive script. The owner sees the report first. Then a
session ships it as any house AI ("Export and check" in `docs/training.md`; the goldens and the module's hash
re-pinned, `CLAUDE.md`, "Traps").

## Next

- [ ] c25's report: read it, compare the pick with abi7a/u1200.
- [ ] c26's report: did the exploits' holes close (lower exploitability), did the aggressive rate rise.
- [ ] If PBT keeps preferring a lower idle cost, make it the recipe's default.
- [ ] Each cycle from the last one's pick, with its best responses in the pool (c27, ...), while the Elo rises.
- [ ] Later: search then distil; a bigger network once the pool can say whether it pays; players' bots in the
      league when there are players.

# Making the house AI better over time, with no players

Today a house AI gets better by hand: on each rules change a session fine-tunes the shipped model with a new mix of
reward terms, keeps a checkpoint every 100 updates, and picks one by its ladder against the brain. That ships a model
that beats the brain more, but nobody knows whether it beats the model before it, what it loses to, or which reward
terms helped. Over the house test's seeds 1-30 against the brain, `abi5a`'s updates 600-1500 ranged from 56 to 86%:
a pick by that number is largely luck. And against the aggressive script the house AI fell from 88.3% (RULES 22) to
62.9% (RULES 24) without anything stopping it.

There are no players yet, so everything here runs with nothing but the rules, the trainer and the machine: no
submitted bot, no human game. It is what self-play projects of the kind do (AlphaStar's league, OpenAI Five's
TrueSkill against a fixed pool and its carrying of weights across patches, DeepMind's FTW and its population-based
training), cut to one desktop.

## 1. Measure: a pool and its rating

`train/pool.py` keeps a pool on disk, `train/runs/pool/` (out of git like the rest of `runs/`):

- **Members**: the built-in minds (brain, first mind, the aggressive and turtle scripts), the house AI as shipped,
  and any checkpoint added (`.pt` or `.onnx`; a checkpoint of a narrower observation is widened as `widen.py` does,
  so a model of an older ABI plays as it did). Each member is an ONNX file under its name.
- **Games**: every pair fought over the pool's seeds, both ways, 3 rounds a fight, each pair once. The seeds are
  500,000 and up: apart from the ladder's (1-150), which stay a held-out check, the training seeds (1,000,000 and
  up) and the tournament's.
- **Rules**: the pool records a fingerprint of the rules (the observation's width and two short reference fights).
  When the rules change, every game is fought again; the members stay.
- **Rating**: a Bradley-Terry fit over all the games at once (won and lost rounds; a cut round counts for nobody), on
  the Elo scale with the brain at 1000, each rating with its standard error. All pairs are played, so the fit does
  not depend on the order the games came in, as an incremental TrueSkill would.
- **Cycles**: a fighting game can be rock-paper-scissors (A beats B beats C beats A), where one number misleads. The
  pool reports the cyclic triples among its members and the Nash equilibrium of the meta-game (each member's win
  rate against each other, zero-sum); a member's payoff against that equilibrium mixture is the measure a cycle
  cannot fool.

A checkpoint is picked by its rating against the whole pool, not by its rate against the brain. The entry bar stays
the gate a shipped model must pass.

## 2. Exploitability

How far a model can be beaten is a measure that no fixed opponent gives. `train/exploit.py` trains a best response:
a fine-tune started from the model itself that fights only the model, frozen. The share of rounds that best response
then wins against the model over the pool's seeds, less a half, is the model's exploitability; a model with no
holes stays near 0. What the best response learned to do is a hole to close, and the best response joins the pool
and the league as an opponent.

## 3. A league with a memory

- **Past selves kept.** `league.py` writes each snapshot to `runs/<run>/league/<n>.onnx` and a resumed run reads them
  back (until now a resume restarted the league with the current policy alone).
- **Prioritised fictitious self-play (PFSP).** A past self is drawn with weight (1 - p)^k, p the learner's recent
  win rate against it (`--pfsp k`; 0 keeps the uniform draw). The opponents it beats every time stop taking fights.
- **Other runs as opponents.** `--pool DIR...` adds every model in those folders (the pool's members, another run's
  `league/`), read again at each snapshot, drawn by PFSP too: the learner keeps fighting what beat it in other runs.
- **An exploiter beside the main run.** `--target DIR` makes a run fight the newest model in a folder: an exploiter
  run aims at the main run's `league/`, and the main run takes the exploiter's `league/` into its `--pool`. The two
  run side by side on the desktop (about 30K steps a second each).
- **A named mix.** `--mix latest=0.2 past=0.15 brain=0.3 pool=0.2 target=0 ...`; five numbers keep the old order.

## 4. Population-based training (PBT)

The run's reward terms (`--idle`, `--land`, `--whiff`, `--whiff-far`, `--variety`, `--time-cost`) and its entropy and
learning rate are chosen by hand, again at each rules change. `train/pbt.py` runs a population of learners
instead (4 by default), each a `ppo.py` run with its own values. Every generation (100 updates by default) each
member fights every other and the references over the pool's seeds, and its fitness is its share of rounds won:
winning, not the shaped reward, the terms being only a means. The bottom quarter takes a copy of the top quarter's
network, optimiser and league, and its values are perturbed (each times 0.8 or 1.25, sometimes drawn again). The
reward terms that win are found by the population, and every generation's best is kept for the pool.

## 5. The cycle

`train/cycle.py` runs the whole loop on one command, the way a session would by hand after a rules change:

1. train: a PBT population, or a single run, from the shipped model (widened first if the ABI grew);
2. add every kept checkpoint, and the house AI, to the pool and play the missing games;
3. rank the candidates by their rating, measure the best few against the entry bar on the ladder's seeds, and
   their exploitability when asked (`--exploit`);
4. write a report (`runs/<run>/report.json` and `report.md`, and with `--review DIR` a review page): the pool's
   table, the pick against the house AI head to head, the bar, the cycles.

It ships nothing: exporting the pick, re-pinning the goldens and building the house module stay a session's
steps (`CLAUDE.md`, "Traps"), done once the owner has seen the report. It runs as a launchd job like a long run
(`train/README.md`, "Training on a desktop").

## Not now

- **Search, then distil.** The rules are deterministic, so a trainer could look ahead to find better acts and teach
  them to the network (as AlphaZero does); the bot would still only run the network, within its fuel. The two
  fighters decide at once, which makes the search harder; it waits until the rest stops paying.
- **Players' bots in the league.** When the tournament has entries they are the most varied opponents there are;
  the trainer would need to run modules (the wasm host it left behind to stay light).
- **A bigger network.** The fuel allows much more than 128, 256, 2; the pool says whether a bigger one is stronger.

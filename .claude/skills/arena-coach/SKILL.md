---
name: arena-coach
description: "Use when the arena's trained AI fights in a way that looks wrong to a person (it ducks and stands over and over, flies about for nothing, sits and stands, spams, turtles, wanders) or when asked to judge, critique or improve how a policy plays, not only whether it wins. A loop in which the agent itself is the coach: code counts how a fight looks, a decision model (Jev) reads thousands of clips given the rules, the agent reads the worst ones, ties what it sees to the reward, trains a variant beside a control, measures both, and hands the owner a review page. Not for rule or balance changes (the owner's call) and not for raw strength (the ladder)."
---

# Arena coach

The ladder says whether a policy wins. This loop asks how it **looks**: a spectator's eye on the fight, and a
reward the policy learns it from. Three readers, each where it is strong:

| Reader | Reads | Good at | Cost |
|---|---|---|---|
| Code (`judge.py`, `look`) | every decision | exact counts: flicker, ducks, take-offs, time in the air, casts, act changes | free |
| Jev (`typesafe/jev-1.14`, given `RULES`) | thousands of 3 s clips, whole rounds | a spectator's yes/no and 0-2 scores, stable when asked twice | about $0.00005 and 0.65 s a question |
| You, the agent | the 5-10 worst clips, the reward code, the numbers | why it happens, and what to change | your context |

Jev knows nothing of the game but the words it is given (`arena_duel::words`) and `RULES` in `train/judge.py`. It
cannot judge whether one move was right (its move verdicts are unrelated to the critic's: Spearman about 0 with or
without the rules, 2026-11-19); use it for how a stretch of play **looks**, never as a judge of play.

## The loop

1. **Count and read the house AI.** On the kiosk, key from game-factory `ops/.env` (never print it):

   ```sh
   cd train
   uv run python judge.py ../../arena-train/train/runs/<house run>/u<N>.pt --names house \
       --env-file ~/projects/game-factory/ops/.env --out runs/judge-<date> --review ~/services/arena/site/review/jev-coach
   ```

   It fights 32 rounds (house AI, brain, aggressive, turtle), counts `look`, asks Jev about 80 clips, 32 rounds and 90
   moves, about $0.03. Read `summary.json` and the page. A count far from the others' (ducks a minute, flicker a minute,
   air share) is the symptom; Jev's `jittery_share` and `aimless_flight` say whether a spectator would see it.

2. **Read the worst clips yourself.** The page lists Jev's most jittery clips with the moment and every decision;
   `runs/<out>/<name>.rounds.json` has every move. Read 5-10. Say in one line what the policy is doing and why the
   rules let it (is the duck dodging a bolt, or nothing?). Check a claim about the rules in `docs/rules.md` before you
   rely on it: guessing (flying dodges a blast) is how a coach misleads.

3. **Tie it to the reward.** The pay is `crates/arena-duel/src/train.rs` (`Pay`, `step_paid`, the fixed `REPEAT`,
   `HABIT`, `STALL`): which term pays for the habit, or which missing cost leaves it free? The terms that shape how
   it looks: `--flicker` (a move undone within 1 s), `--fly` (each take-off), `--repeat` and `--habit` (scale the
   fixed penalties for holding one act, which push a policy to switch), `--idle`, `--land`, `--variety`. A new term
   goes in `Pay` with a test in `crates/arena-duel/tests/train.rs`, its default keeping every run as it was, and
   through `crates/arena-duel-py` and `ppo.py`.

4. **Train a variant beside a control.** Always a control: the same init, the same recipe, the same updates, no new
   term. Without it a change is indistinguishable from more training. Launchd jobs on the desktop
   (`~/Library/LaunchAgents/io.aiocean.arena-train-<run>.plist`, as `io.aiocean.arena-train-coach-*`), never in a
   shell that dies with the session. The cycle (`io.aiocean.arena-cycle-*`) shares the machine: `--threads 2` each,
   look at `uptime` first, and say in the report what you slowed. 200 updates from the house AI take about 30 min.

5. **Measure both on the same seeds.** `judge.py <control> <variant>... --names ...` puts them side by side. Then
   strength, which judge's few rounds cannot show: export each (`uv run python export.py runs/<run>/last.pt --out
   /tmp/<run>.onnx --check /tmp/<run>.json`) and run the ladder (`docs/training.md`, "Measure it",
   `--onnx /tmp/<run>.onnx --seeds 150`). A variant that looks better and wins less is a trade the owner decides.

6. **Hand it over.** A `notes.html` in the review folder (what you saw, what you changed, the numbers before and
   after, every doubt) is folded into the page; give the owner https://arena.aiocean.io/review/jev-coach/. Commit the
   tools and terms on `main` (`cargo fmt --all`, the tests, `git pull --rebase`); the runs stay out of git.

7. **Calibrate on the owner.** When the owner marks clips "looks wrong" or "fine" (by the round and time on the
   page), score Jev's `jittery` and `skilled` against those marks and change the questions in `judge.py` until they
   agree; write the agreement down. The owner's eye is the target, Jev only scales it.

## What is not yours

- Shipping a house AI (`docs/training.md`, "Export and check"; the owner says so), any rule, balance or
  `RULES_VERSION` change, and the bot ABI.
- A term that makes it look calm by making it lose is not a fix: report it as a trade.

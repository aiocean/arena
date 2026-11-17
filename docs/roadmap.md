# Roadmap

What the player kit and the tournament still need, most important first. Each item says what exists today.

## The player kit

1. **A reward written in Python.** Today the reward is fixed in Rust (`crates/arena-duel/src/train.rs`) and a player
   changes it only by editing Rust. `step` should also hand back the facts of each decision (damage dealt and taken,
   blows landed, guarded and punished, statuses, the round's end), so a trainer computes any reward in Python; the
   Rust reward stays the default.
2. **Install from PyPI.** A release would carry prebuilt wheels (`CONTRIBUTING.md`, "Releasing"),
   installed with `--find-links`. Publishing them to PyPI makes `pip install arena-duel` enough.
3. **Every submission check from Python.** The checks run today as `arena-bot`'s `check` example (admitted, answers,
   the entry bar) and the bar alone as `arena_duel.entry_bar`; add one `arena_duel.check(module)` that runs them all.
4. **Data to learn from.** A recorder that writes a fight's decisions (observation, legal acts, the act taken, the
   outcome) from a seed and two files, and a published set of the shipped AI's fights, for imitation learning.
   Today `seat::Recorder` and `seat::Replay` (`crates/arena-duel`) record and replay a corner's acts, which the live
   matches use; they keep no observations or outcomes yet.
5. **See why a bot acts.** On the home page, pause on a frame and see each act's probability, which acts are legal and
   what the bot saw.
6. **A local league.** Many files of one's own play each other and are ranked the way a season ranks them, to choose
   which to submit.
7. **A standard environment.** A Gymnasium / PettingZoo wrapper over `VecEnv`, so Stable-Baselines3, CleanRL and RLlib
   work without reading our trainer.
8. **Training without a strong machine.** A notebook that fine-tunes from the shipped AI on a hosted GPU or CPU.

9. **Bot templates beyond ONNX.** `bots/onnx` builds any ONNX model into a module. Templates for a hand-written Rust
   bot (memory and search across a match) and for other runtimes would show what a module can do besides a forward
   pass.

## The tournament

10. **Seasons.** Today there is one test season on `main`. Later: season tags that freeze the rules, and a module's ABI
    version checked against the season's.
11. **Rules of entry.** Who may enter, one account each, how many files are active at once, the season's dates, what
    happens to the files after the season (published, so replays can be checked), and what counts as cheating.
12. **The runner.** Plays every match of a season from the submitted files, both ways, on a pool of machines, and
    writes results, ratings and replays. Deterministic, so a match can be split across machines and checked.
    Today `crates/arena-server` loops the house AI's matches on one machine and publishes them live, and `arena-watch`
    checks one by replaying it; it plays no submitted files and writes no results yet.
13. **Submission and results pages.** Upload, the checks' verdict, the table, each match's replays.

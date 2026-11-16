# Roadmap

What the player kit and the tournament still need, most important first. Each item says what exists today.

## The player kit

1. **A reward written in Python.** Today the reward is fixed in Rust (`crates/pixfx-duel/src/train.rs`) and a player
   changes it only by editing Rust. `step` should also hand back the facts of each decision (damage dealt and taken,
   blows landed, guarded and punished, statuses, the round's end), so a trainer computes any reward in Python; the
   Rust reward stays the default.
2. **Install from PyPI.** Today a season's release carries prebuilt wheels (`CONTRIBUTING.md`, "Releasing a season"),
   installed with `--find-links`. Publishing them to PyPI makes `pip install pixfx-duel` enough.
3. **Every submission check, before submitting.** Today `pixfx_duel.entry_bar` and `examples/match.rs --bar` measure
   the bar. Add the other checks of `docs/tournament.md` (the file loads, every output is finite on the fixed
   observations) as one `pixfx_duel.check(file)`.
4. **Data to learn from.** A recorder that writes a fight's decisions (observation, legal acts, the act taken, the
   outcome) from a seed and two files, and a published set of the shipped AI's fights, for imitation learning.
5. **See why a bot acts.** On the watch page, pause on a frame and see each act's probability, which acts are legal and
   what the bot saw.
6. **A local league.** Many files of one's own play each other and are ranked the way a season ranks them, to choose
   which to submit.
7. **A standard environment.** A Gymnasium / PettingZoo wrapper over `VecEnv`, so Stable-Baselines3, CleanRL and RLlib
   work without reading our trainer.
8. **Training without a strong machine.** A notebook that fine-tunes from the shipped AI on a hosted GPU or CPU.

## The tournament

9. **Seasons.** The first season tag and a check that refuses a weights file built for another season's shapes.
10. **Rules of entry.** Who may enter, one account each, how many files are active at once, the season's dates, what
    happens to the files after the season (published, so replays can be checked), and what counts as cheating.
11. **The runner.** Plays every match of a season from the submitted files, both ways, on a pool of machines, and
    writes results, ratings and replays. Deterministic, so a match can be split across machines and checked.
12. **Submission and results pages.** Upload, the checks' verdict, the table, each match's replays.

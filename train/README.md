# train

PPO for the landing duel's policy, on the Rust rules (`crates/pixfx-duel`) through `crates/pixfx-duel-py`.
Design: `docs/specs/2026-11-15-duel-rules-in-rust-design.md` (Part 2). Scripts, not a CLI.

```
uv sync
uv run maturin develop --release -m ../crates/pixfx-duel-py/Cargo.toml   # after any change to the Rust rules
uv run pytest
uv run python ppo.py --run v1                     # runs/v1/log.jsonl, runs/v1/last.pt
uv run python ppo.py --run v2 --init runs/v1/last.pt --updates 600 --lr 1e-4 --ent 0.003 0.002 --shaping-until 0
                                                  # a fine-tune: a new run from v1's network
uv run python ppo.py --run v3 --init runs/v2/last.pt ... --thunder 0.9
                                                  # the learner plays Thunder in 90% of its fights (0.5 by default)
uv run python export.py runs/v1/last.pt           # crates/pixfx-duel/weights/policy.bin + its Rust check
cargo run --release -p pixfx-duel --example ladder -- --seeds 150
```

- `net.py`: the network, mirroring `crates/pixfx-mind/src/policy/net.rs`, and its weights file; the HL-Gauss critic.
- `league.py`: who the learner fights (latest self 35%, a past self 25%, brain 15%, first mind 10%, scripted 15%),
  and in which school. The shipped policy was fine-tuned with `--thunder 0.9`: at an even share it played Thunder
  worse and won only about 40% of the mirror as Thunder.
- `ppo.py`: the trainer. CPU beats MPS for this network (10.2K vs 8.1K env steps/s on the M4).
- `export.py`: the shipped file and `crates/pixfx-duel/tests/fixtures/policy_check.json`, which
  `cargo test -p pixfx-duel --test policy_export` checks against Rust's own forward pass.

`runs/` stays out of git; the shipped weights go in.

## Training on kiot

A full run trains on macmini-kiot, not the MacBook Air: kiot runs about 42K env steps/s (the Air 6K-12K), so
3000 updates take about 45 minutes, and a crash or sleep on the Air cannot stop it.

- Reach it as `ssh firegroup@100.117.192.75` (the name `macmini-kiot` fails host-key checks).
- The code is already there: Mutagen session `game-factory` keeps `~/projects/game-factory` the same on both
  machines (`mutagen sync flush game-factory` on the Air before starting). A worktree under `.claude/worktrees/`
  syncs too; `.venv` does not, so `long-run.sh` builds it there.
- Start a run as a launchd job, so it belongs to no ssh login and kiot does not sleep (a `nohup ... &` started
  over ssh died silently after 30 minutes):
  ```
  ssh firegroup@100.117.192.75 'cd ~/projects/game-factory/projects/arena/train &&
    launchctl submit -l pixfx.train.frames -o $HOME/train-frames.log -e $HOME/train-frames.log \
      -- /usr/bin/caffeinate -is $PWD/long-run.sh frames'
  ```
  `long-run.sh` builds the rules, runs pytest, trains, and resumes from `runs/<run>/last.pt` (saved every 25
  updates) if the trainer dies. Its log ends in `TRAIN-DONE` or `FAIL`. launchd starts a submitted job again
  whenever it exits, so once the log says `TRAIN-DONE`, run `launchctl remove pixfx.train.frames`.
- Progress is `runs/<run>/log.jsonl`, which syncs back to the Air with the rest of `runs/`; export and the ladder run
  on the Air.
- Check `pgrep -fl "ppo.py"` on kiot before starting, since another session may be training. Two runs side by side
  are fine (about 30K env steps/s each); give each its own `CARGO_BUILD_BUILD_DIR`.

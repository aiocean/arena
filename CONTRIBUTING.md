# Contributing

## The engine the arena runs on

The arena runs on the pixfx engine ([aiocean/pixfx-rs](https://github.com/aiocean/pixfx-rs)), pinned to one commit:

- the Rust rules take it from git at the `rev` in `Cargo.toml` (`[workspace.dependencies]`) and in
  `crates/pixfx-duel-py/Cargo.toml`, which is its own workspace;
- the site and its tests take it from `.pixfx/`, which `site/pixfx.sh` checks out at that same rev (`site/build.sh`
  runs it).

A season's rules are this repo's tag plus that rev, so a fight replays the same anywhere. A fresh clone needs nothing
else: `cargo test --workspace`, then `site/build.sh` and `(cd site && bun test tests)`.

## The checks a change passes

CI (`.github/workflows/ci.yml`) runs them on every push and pull request, the rules on Linux x86_64 and macOS arm64:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets            # the goldens pin seeded fights: green on both platforms = same fights
(cd train && uv sync && uv run maturin develop --release -m ../crates/pixfx-duel-py/Cargo.toml && uv run pytest)
site/build.sh && (cd site && bun test tests)
```

## Releasing a season

1. Set the season's version in `crates/pixfx-duel-py/pyproject.toml`.
2. Tag the commit `arena-s<N>` and push the tag. `.github/workflows/release.yml` builds the wheels (Linux x86_64,
   macOS arm64 and x86_64; Python 3.12 and 3.13) and makes the release: the wheels, `policy.onnx` (the house AI) and `SHA256SUMS`.
3. Players install the env without Rust:
   `pip install pixfx-duel --find-links https://github.com/aiocean/arena/releases/expanded_assets/arena-s<N>`.

Running the release workflow by hand only builds the wheels, as a check.

## Moving to a new engine

1. Put the new pixfx commit's full hash in every `rev = "..."` of `Cargo.toml` and `crates/pixfx-duel-py/Cargo.toml`.
2. `cargo test --workspace --all-targets`, then `site/build.sh` (it checks `.pixfx/` out at the new rev) and
   `(cd site && bun test tests)`.
3. A change in what a bot sees or does is a new season (`AGENTS.md`).

## Working on the engine beside the arena

To change the engine and see it in the arena before it has a commit, point both sides at a pixfx checkout:

1. Rust: create `.cargo/config.toml` (gitignored) with the crates the arena uses, as paths to your checkout:

   ```toml
   [patch."https://github.com/aiocean/pixfx-rs"]
   pixfx-draw = { path = "../pixfx-rs/crates/pixfx-draw" }
   pixfx-effects = { path = "../pixfx-rs/crates/pixfx-effects" }
   pixfx-fighter = { path = "../pixfx-rs/crates/pixfx-fighter" }
   pixfx-mind = { path = "../pixfx-rs/crates/pixfx-mind" }
   ```

   The paths are relative to this repo's root.
2. Site: run the build with `PIXFX=<your checkout>`. `.pixfx/` becomes a link to it (remove a cloned `.pixfx/` first).
3. When the engine change is committed and pushed, delete `.cargo/config.toml`, `rm .pixfx`, and move to its rev as
   above.

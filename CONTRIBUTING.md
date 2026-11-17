# Contributing

## Building

A fresh clone needs Rust (the pinned nightly), and for the page `wasm-bindgen` (0.2.130), `wasm-opt` and `bun` (the
docs pages): `cargo test --workspace`, then `site/build.sh`. A fight's rules are this repo's commit, so a fight
replays the same anywhere.

## The checks a change passes

CI (`.github/workflows/ci.yml`) runs them on every push and pull request, the rules on Linux x86_64 and macOS arm64:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets            # the goldens pin seeded fights: green on both platforms = same fights
(cd train && uv sync && uv run maturin develop --release -m ../crates/arena-duel-py/Cargo.toml && uv run pytest)
cargo clippy -p arena-game --lib --target wasm32-unknown-unknown -- -D warnings
site/build.sh
```

The page replays the matches the server fights natively, so the rules must fight bit for bit the same in the
browser's WebAssembly. Check it before you push a change to the rules (under a minute; not yet in CI):

```sh
cargo build --release -p arena-duel --example wasm_hashes --target wasm32-unknown-unknown
wasm-bindgen --target nodejs --out-dir target/wasm-hashes target/wasm32-unknown-unknown/release/examples/wasm_hashes.wasm
node crates/arena-duel/tests/wasm/native_wasm.mjs target/wasm-hashes   # PASS: the native goldens' hashes in wasm
```

Run `cargo fmt --all` before you push. A clone refuses a push that is not formatted once its hooks are on, which
every clone should do once:

```sh
git config core.hooksPath .githooks              # .githooks/pre-push runs cargo fmt --all --check
```

CI cancels a run when the next push lands, so a red check can hide behind the pushes after it: look at the run of the
last push (`gh run watch`) before calling main green.

## Releasing

While the arena is in testing there is one season on `main` and no release: players build from a clone. When a
release is wanted:

1. Set the version in `crates/arena-duel-py/pyproject.toml`.
2. Tag the commit `arena-s<N>` and push the tag. `.github/workflows/release.yml` builds the wheels (Linux x86_64,
   macOS arm64 and x86_64; Python 3.12 and 3.13) and makes the release: the wheels, `policy.onnx` (the house AI) and `SHA256SUMS`.
3. Players install the env without Rust:
   `pip install arena-duel --find-links https://github.com/aiocean/arena/releases/expanded_assets/arena-s<N>`.

Running the release workflow by hand only builds the wheels, as a check.

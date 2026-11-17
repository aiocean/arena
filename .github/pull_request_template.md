## What and why

## Checks

- [ ] `cargo test --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] the trainer, if the env or `train/` changed: `cd train && uv run maturin develop --release -m ../crates/arena-duel-py/Cargo.toml && uv run pytest`
- [ ] the site, if a page or the wasm changed: `site/build.sh && (cd site && bun test tests)`
- [ ] a change to a rule, to what a bot sees or does, or to the network: `docs/rules.md` / `docs/bot.md` updated, and it waits for the next season (`docs/tournament.md`)

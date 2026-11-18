# arena-calc

Below the fight. The design is `docs/specs/2026-11-17-damage-system-design.md` (section 15 maps each
rule to its module), built on the research in `docs/specs/2026-11-17-skill-system-design.md` and
`docs/specs/2026-11-17-skill-system-rules.md`.

## Job

How damage is worked out, with no fight in it: the ten kinds of damage and the five phases (`element`), tags (`tag`),
ailments (`ailment`), burn and bleed as damage over time (`dot`), a fighter's bars and resources (`cultivator`), ranks
(`grade`), one blow's twelve steps (`hit`), the daos (`dao`), curses (`curse`), intents and their contest (`aura`),
domains (`domain`), one-use cards and bluffs (`card`), supports (`support`), keystones (`keystone`), the shared pool
of skills (`gem`), a skill built into a blow (`build`) and the loadout a player picks (`loadout`); stats and a fighter's
sheet (`stat`, `sheet`), passives and items (`passive`, `item`), and what the field-bound stats do on plain geometry:
projectiles (`projectile`), motion (`motion`), stored uses (`charge`), devices (`device`), radii and durations
(`play`) and the event queue of triggered casts (`event`). No classes, no ice.

The fight (`arena-duel`, which the game `arena-game` and the server run) builds a `Hit` here, calls `hit::resolve` and applies the
`Outcome` it gets back. Positions, bodies, shots and dice live in the fight: every roll comes in as an argument.

## Traps

- Deterministic: no dice inside, `libm` for transcendental maths (`clippy.toml`), the same bits on native and wasm.
- Every hit test (shapes overlapping, a sweep entering one) goes through `collide`, on parry2d-f64 with
  `enhanced-determinism` (its only dependency beyond `libm`); touching is not overlapping.
- The twelve steps of `hit::resolve` run in a fixed order; never swap two.
- Conversion runs only forward in `Element::ALL`'s order. A rule that would need it backward (Âm Hàn) rewrites the
  base split at step 1 instead.
- Every number is a documented `pub const`; a change to one is a rule change: it goes in on `main`, updates
  `docs/rules.md`, re-pins the duel's goldens and bumps `arena_link::RULES_VERSION` (root `AGENTS.md`, "Traps").
- `tests/fixtures/spells.json` is the duel's `data::SPELLS` as the pool was made from it; `tests/gem.rs` checks the
  pool's bases and timings against it.

## Before you finish

```
cargo test -p arena-calc
cargo clippy -p arena-calc --all-targets -- -D warnings
cargo clippy -p arena-calc --all-targets --features serde -- -D warnings
```

# arena-sim

A tool above `arena-calc`. It depends on `arena-calc` and `libm` (and `arena-telemetry` for its examples' telemetry).

**This is not the real fight.** The real fight is `crates/arena-duel` (played by the game, `crates/arena-game`), with
bodies, jumps, guards and its own rules for how a skill travels. This crate is a headless simulator that stands two scripted fighters on a 1D field and lets
them cast their loadouts through `arena-calc`, to measure balance (design `docs/specs/2026-11-17-damage-system-design.md`,
12) beside the real fight, which it does not replace. Its numbers rank skills, supports and keystones under one simple brain; they do not
say how a trained bot or a player would use them.

## Job

- `fight`: the field (`FIELD` T between two walls), the tick in the fixed order of the rules' section 9
  (`docs/specs/2026-11-17-skill-system-rules.md`), rounds (0 life, or 60 s then the higher life share: "cut") and
  best-of-3 matches. `play_match(seed, a, b)` replays bit for bit.
- `fighter`: a loadout made into skills (3 basics, 8 actives) at `SKILL_LEVEL`, the sheet (keystones;
  no gear, no tree), the supports' triggers, and the fighter's place and timers.
- `delivery`: how a skill reaches the foe, from its tags (melee by reach, projectiles, areas, instants, dashes,
  blinks, wards, heals, curses, intents, domains, buffs, devices). Every number is a `pub const` there.
- `units`: traps, mines, totems and minions (`arena_calc::device`) as bodies the foe can strike.
- `brain`: the scripted brain. `report`: what a match measured per skill. `balance`: the balance run.
- Examples: `duel -- <seed> [--loadout-a <json|file>] [--loadout-b ...] [--telemetry-otlp]` logs one match;
  `balance -- --seeds N --pairs M [--seed S] [--threads N] [--out file.json] [--telemetry-otlp]` prints the C/B/A/S
  table and writes JSON (`--telemetry-otlp` sends to the telemetry store, `ops/observe`).

## Calls into arena-calc

`build::build_skill` for every cast (again when the windup ends, at the landing situation), `hit::resolve_with` for
every blow with rolls drawn in the fixed order evade, crit, double, star; `Cultivator::tick_with` every tick; the
event queue (`event::Queue`) after every batch of blows and after damage over time, delivering the casts its
triggers return; curses (`curse`), intents (`aura`), domains (`domain`), cooldown uses (`charge`), run speed,
knockback and dash reach (`motion`), radii and durations (`play`), projectiles (`projectile::Flight`) and devices
(`device`).

## Traps

- Deterministic: one `Rng` (SplitMix64) for the fight's rolls and one per brain, all from the match seed; no
  `HashMap` iteration, no time, no threads inside a match. `balance` plays pairs on threads and gathers them in
  order: the table does not depend on the thread count (a test checks it).
- Seat fairness: both brains decide on the same state, both run from the same positions, and both estimate from
  the first tick. A mirror test holds seat 0 to 50% ± 20 over 50 seeds; the balance run's seat-0 share should sit
  near 50%.
- Never edit `crates/arena-calc` from here: a number that looks wrong there is reported, not patched around.

## Before you finish

```
cargo test -p arena-sim
cargo clippy -p arena-sim --all-targets -- -D warnings
```

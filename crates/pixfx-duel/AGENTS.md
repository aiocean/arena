# pixfx-duel

Read the root `AGENTS.md` ("Where code goes") before changing anything here; this file only says what this crate
holds. The design is
`docs/specs/2026-11-15-duel-rules-in-rust-design.md`.

## Job

The landing duel's rules: two cultivators, Thunder and Frost, their stats and skills (`data`), the seeded dice every
roll comes from (`arena_mind::Rng`), the fighters (`fighter`), statuses (`status`), the rotating bar (`bar`), casts
and blows (`act`), shots (`shot`), the round loop (`duel`), the seats that decide for each corner (`seat`: the brain from
`arena-mind`, a bot (`BotSeat` over a `Decider`: a module or an ONNX policy, run by `crates/arena-bot`), `first_mind`, the yardstick a brain is
measured against, or the scripted fighters in `scripted`), each fighter's style (`style`), what a mind senses
(`sense`), the bodies (`body`, `pose`: gravity, flight, blows, getting up), JS number semantics (`js`), the effect ids
the rules may play (`effects`), and the duel as the game reads it (`view`). The duel is a `bevy_ecs` world (`State` a
resource, the seats a non-send resource) and one tick is a single-threaded schedule of chained systems in the rules'
order (`duel::tick_schedule`). For training and measuring a bot: the trainer's episodes
and rewards (`train`, driven from Python through `crates/pixfx-duel-py`) and the ladder (`ladder`, printed by
`crates/arena-bot/examples/ladder.rs`). `weights/policy.onnx` is the house AI; this crate never reads it (`arena-bot`
does), so it does not depend on an ONNX runtime or a wasm host. A seed replays the same fight on every target. The game (`crates/arena-game`)
plays the effect commands and feels the events the duel returns; it decides nothing.

## Belongs here

- The duel's rules: damage, defences, cooldowns, qi, rage, statuses, wards, barriers, the bar, rounds.
- Frame data (`docs/specs/2026-11-16-duel-frames-design.md`): a blow on a fighter gathering a cast is a
  COUNTER, on one recovering a PUNISH (`Out::Hit.how`); unsafe skills recover longer when guarded; an ultimate's
  opening cannot be hit. Every number is in `data::FRAMES`.
- The duel's tables: spells, schools, stats, tiers, traits, luck.

## Does not belong here

| If the code... | It goes to |
|---|---|
| decides what a fighter does | `crates/arena-mind` (or a trained brain) |
| draws, shakes, plays an effect | `crates/arena-game`, from the commands and events the duel returns |

## Dependencies

- Required: `libm`, `arena-mind` (the minds), `bevy_ecs` (no default features: the world and its schedule).
- Optional, behind `serde`: `serde`.
- Never the renderer or the game (`bevy`, `arena-game`): the trainer builds this crate and stays light.

## Traps

- `tests/goldens/` pins what the rules produce: `event_hashes.json` (every command and event of 120 s of seeds
  1-20), `first_mind.json` (the first mind's first 600 choices of seeds 1-5) and `arenas.json` (each seed's pools and
  opening bars). A change that alters fights fails them; when the change is meant, delete the files it fails and run
  the tests once to rewrite them.
- `tests/fixtures/js_numbers.json` pins the tables, dice and formulas (`tests/data.rs`), one key per number: a
  deliberate change to a number edits its key there too.
- `weights/policy.onnx` is the house AI (`train/export.py` writes it, with `tests/fixtures/policy_check.json`). A new
  one must pass `crates/arena-bot/tests/house.rs` (tract's forward pass against PyTorch's) and re-pins
  `crates/arena-bot/tests/goldens/house_fight.json` (the house module's fights; delete it and run
  `cargo test -p arena-bot --all-features` after `site/build.sh`), which `site/tests/policy.test.js` checks the page
  against. A rule change re-pins it too.
- A school's pool (`data::SchoolData::pool`) is larger than a fight's 8: `bar::fight_pool` draws 8 of it and guarantees a skill of the signature kind, a payoff skill, and a skill that lays the status that payoff spends. A new skill goes in four places: `data::SPELLS` and the school's `pool` (and `evolves`, `PAYOFF`, `FRAMES.unsafe_on_block` where it applies), its key in `tests/fixtures/js_numbers.json`, a tooltip in `site/assets/duel-info.js` and a cell in `site/assets/skill-icons.js`. An effect a spell plays once through `cast` must be a one-shot effect: the duel never stops a looping one.
- Styles are drawn on dice of their own (`style::draw`), never the rules' `Rng`: drawing them must not move a fight.
- One extra or missing roll of the dice changes every fight after it.
- JS maths: `Math.sign(0)` is 0 (never `f64::signum`), `Math.round` rounds half up (`js_round`), `**` is `powi`.

## Before you finish

```
cargo test -p pixfx-duel --all-targets
cargo clippy -p pixfx-duel --all-targets -- -D warnings
```

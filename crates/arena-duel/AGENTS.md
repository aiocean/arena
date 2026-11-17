# arena-duel

Read the root `AGENTS.md` ("Where code goes") before changing anything here; this file only says what this crate
holds. The fight's design is `docs/specs/2026-11-15-duel-rules-in-rust-design.md` (the bodies, the tick, the seats);
how much a blow deals is the damage system, `docs/specs/2026-11-17-damage-system-design.md`, in `crates/arena-calc`.

## Job

The landing duel's rules: two seats with no class, each fighting with a loadout of the shared pool
(`arena_calc::loadout`), drawn at random from the seed on dice of their own or given (`Duel::with_loadouts`). Thunder
and Frost are how a fighter looks (`data::Look`): its sprites and colours, and so its hurtboxes and hitboxes, which are
measured from its frames. Looks are drawn apart from the seats (`Duel::new_with_looks`, `ladder::looks_of`); the rules
decide nothing else by them, and a bot is told its look, never its seat.

- `data`: the motions a skill borrows (`data::motions`, how it moves and looks) and the pool's skills as the duel
  casts them (`data::spell`: a gem on its motion), the looks, the brain's one temperament (`MIND`), the constants.
- `kit`: a loadout made into skills (basics, actives with their supports, linked skills), each with its
  stored uses (`charge::Uses`) and how it works (`Way`: intent, domain, curse, buff, device, or its motion).
- `fighter`: a fighter (its cultivator, sheet, kit, bar, body, intents, curses) and the duel's `State`.
- `act`: casting (`begin`, `release`, `deliver`), each motion's blow (`strike`), guards, barriers, wards, clashes.
- `strike`: blows through the damage system (`build::build_skill`, `hit::resolve_with` with rolls in the order evade,
  crit, double, star), gains, controls on the body, the event queue and its triggered casts, curses, time wearing
  the fighters (`Cultivator::tick_with`), spare lives.
- `shot`: skills in flight; a bolt is `arena_calc::projectile::Flight` (pierce, fork, chain among every live target).
- `unit`: traps, mines, totems and minions, entities of the rules' world (`Unit`) in id order.
- `frames`: where a fighter can be hit and where its blow or bolt comes from, frame by frame (hurtboxes, hitboxes,
  hands, each clip's active frame); `frames_data` is its table, generated from the sprite sheets by
  `cargo run -p arena-game --example frame_boxes` (never edited by hand); `pick`: which frame a pose shows (`CLIPS`).
- `out` (the commands and events a tick returns), `debug` (a fight's state for a person to read).
- `bar` (the rotating bar drawn from the actives; its tiers, traits, luck and evolving pairs are off), `duel` (the
  schedule), `seat`, `first_mind`, `scripted`, `style`, `sense` (what a mind is told), `body`, `pose`, `js`,
  `effects`, `view`, `train`, `ladder`.

The duel is a `bevy_ecs` world (`State` a resource, the seats a non-send resource, the units entities) and one tick is
a single-threaded schedule of chained systems in the rules' order (`duel::tick_schedule`): the units gathered in id
order, the clock, the domains and intents, each seat's turn, the bodies kept apart, the shots, the blows due, the
units, time wearing everyone, the knock-outs, the seats told, the units written back. `weights/policy.onnx` is the
house AI; this crate never reads it (`arena-bot` does). A seed replays the same fight on every target. The game
(`crates/arena-game`) plays the effect commands and feels the events the duel returns; it decides nothing. The
server (`crates/arena-server`) fights the live matches with `seat::Recorder`s, and the live page steps the same duel
with `seat::Replay` seats from the decisions it receives.

Each tick the two fighters take their turns with who goes first alternating (`duel::turns`: seat 0 on even ticks, seat
1 on odd ones), so neither seat strikes first or reacts second every tick; `tests/fair_seats.rs` holds a mirror to
45-55% in either seat and either look. Examples: `seats` (a mind's mirror by seat and by look: `--seeds --rounds
--mind first|brain|aggressive|turtle`) and `speed`.

## Belongs here

- How a skill travels in the real fight (its motion, its repeats, its units), bodies, guards, wards, barriers,
  clashes, the bar, rounds, frame data (`docs/specs/2026-11-16-duel-frames-design.md`; every number in
  `data::FRAMES`; a counter or punish is the damage system's `Hit::frame`).
- Calling `arena-calc` as `arena-sim` does (`crates/arena-sim/src/fight.rs` is the reference).

## Does not belong here

| If the code... | It goes to |
|---|---|
| is how much a blow deals, what it lays, what a support or keystone does, a skill's numbers or tier | `crates/arena-calc` |
| decides what a fighter does | `crates/arena-mind` (or a trained brain) |
| draws, shakes, plays an effect | `crates/arena-game`, from the commands and events the duel returns |

## Dependencies

- Required: `libm`, `arena-calc` (the damage system), `arena-mind` (the minds), `bevy_ecs` (no default features).
- Optional, behind `serde`: `serde` (and `arena-calc/serde`).
- Never the renderer or the game (`bevy`, `arena-game`): the trainer builds this crate and stays light.

## Traps

- `tests/goldens/` pins what the rules produce: `event_hashes.json` (every command and event of 120 s of seeds
  1-20), `first_mind.json` (the first mind's first 600 choices of seeds 1-5) and `arenas.json` (each seed's two
  loadouts and opening bars). A change that alters fights fails them; when the change is meant, delete the files it
  fails and run the tests once to rewrite them. A change in `arena-calc` that alters a blow re-pins them too.
- `tests/fixtures/js_numbers.json` pins the motions, the looks, the brain's temperament and the constants
  (`tests/data.rs`), one key per number: a deliberate change to a number edits its key there too.
- `weights/policy.onnx` is the house AI. A new one must pass `crates/arena-bot/tests/house.rs` and re-pins
  `crates/arena-bot/tests/goldens/house_fight.json` (delete it and run `cargo test -p arena-bot --all-features` after
  `site/build.sh`). A rule change re-pins it too. The shipped one was trained on the old rules: its entry-bar check is
  ignored until it is retrained.
- Loadouts and styles are drawn on dice of their own (`duel::loadouts`, `style::draw`), never the rules' `Rng`:
  drawing them must not move a fight. Units are worked on in id order, never query order.
- Every skill of the pool borrows a motion (`Gem::spell`): a new motion goes in `data`'s motion table and its key in
  `tests/fixtures/js_numbers.json`; an effect a motion plays once through `cast` must be a one-shot effect (the duel
  never stops a looping one), and a new effect id needs a painter in `arena-vfx` and a re-bake, or a drawn sheet
  (`art/vfx/`), and its id in `effects::EFFECTS`.
- One extra or missing roll of the dice changes every fight after it.
- A change to the rules, to a seat's `think` order or to what a bot sees or does bumps `arena_link::RULES_VERSION`
  (root `AGENTS.md`, "Traps"): the live page and the server must run one rules build.
- A redrawn fighter sheet changes the boxes: run `frame_boxes` again, re-pin the goldens and measure with the ladder;
  it is a rule change.
- JS maths: `Math.sign(0)` is 0 (never `f64::signum`), `Math.round` rounds half up (`js_round`), `**` is `powi`.

## Before you finish

```
cargo test -p arena-duel --all-targets
cargo clippy -p arena-duel --all-targets -- -D warnings
```

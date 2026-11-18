# Physics and collision on Bevy's ecosystem: the spike and the decision

## Intent

The owner, 2026-11-18: move the arena's physics and object collision onto Bevy's ecosystem. Today the rules
(`crates/arena-duel`) do it by hand: the hurtboxes of the frame a fighter shows (`frames::hurtboxes`, from
`frames_data`), a blade's hitbox in its active window (`frames::strike_hitbox`, `unit::strike_box`), a point with a
margin against those boxes (`shot::overlaps`), a bolt swept along a ray into the circles laid on the foe's boxes
(`unit::targets_of`, `arena_calc::projectile::Flight::step`), areas as distances along x (`unit::enemies_near`,
`enemies_between`), and the bodies' motion, gravity and walls (`body`). Every number is f64.

Whatever replaces it must keep the fight deterministic from a seed and bit for bit the same natively and in the
page's wasm32 (the page replays the server's matches), and cheap: the trainer and the ladder run millions of ticks.

Bevy has no physics of its own. The candidates: **Avian** (`avian2d`, Bevy-native ECS physics) and **bevy_rapier**
(`bevy_rapier2d`, Bevy's plugin over rapier). Both do their shape queries through **parry** (dimforge), which is
also a candidate on its own: a fighting game's collision is shape queries on boxes and circles moved by frame data,
not rigid bodies.

## The spike

`spike/physics` (its own workspace, not built by CI). It fights seeds 1-20 for 120 s each with two brains (144,000
ticks), and each tick takes the real geometry from the rules (both fighters' hurtboxes, their blade strike boxes,
every bolt and orb in flight, the bolts' target circles) and asks every backend the same queries:

- **strike**: a fighter's strike box against the foe's hurtboxes (`unit::strike_hits`);
- **probe**: a point with a margin against the foe's hurtboxes (`shot::overlaps`), at every bolt and orb, and 16
  synthetic probes a tick on the whole and half pixel near each fighter (the boxes' edges are whole pixels, so
  these land on edges);
- **bolt**: the first target circle a ray enters within a tick's flight, and how far along (`projectile::entry`),
  for every real bolt and 4 synthetic ones a tick.

3,186,345 queries in all. Each backend's answers are hashed (a bolt's distance by its bits), and counted where they
differ from the hand-written ones. The backends:

| backend | how |
|---|---|
| hand | the rules' own tests, copied |
| parry2d-f64 0.31.1 | `intersection_test` on `Cuboid`s, `Ball::cast_ray`; no world, `enhanced-determinism` |
| parry2d-f64 strict | as above, but a pair overlaps only at a contact distance below 0 (the rules' `<`) |
| rapier2d-f64 0.36.1 | a `ColliderSet` and `BroadPhaseBvh`, colliders reshaped and moved each tick, queried through `QueryPipeline`; `enhanced-determinism` |
| parry2d f32 (rapier2d 0.36.1) | parry on f32, which is what bevy_rapier2d 0.37 runs (it has no f64 build) |
| avian2d 0.8.0 f64 | an `App` with `PhysicsPlugins` on a schedule stepped by hand, kinematic bodies reshaped and moved each tick, collision filters off between them, queried through `SpatialQuery`; `parry-f64`, `enhanced-determinism`, no default features |

### Determinism: native = wasm32

Every backend gives the same hash natively (macOS arm64) and in wasm32-unknown-unknown under node, over all
3,186,345 queries:

| backend | hash, native and wasm32 |
|---|---|
| hand | `6e1820335e77ac5d` |
| parry2d-f64 | `a77390391fc120da` |
| rapier2d-f64 | `5445dd8c521c41c3` |
| parry2d f32 | `a36303914bf7fe87` |
| avian2d f64 | `f152dd497a77f068` |

So determinism across the two targets is not what decides: with `enhanced-determinism` (libm for the
transcendentals) all three are reproducible, on f32 as on f64.

### Same answers as the rules?

None is bit for bit the rules. Against the hand-written answers:

| backend | strike | probe | bolt: other target | bolt: same target, other bits |
|---|---|---|---|---|
| parry2d-f64 | 11 | 8,833 | 0 | 195,795 |
| parry2d-f64 strict | 0 | 552 | 0 | 195,795 |
| rapier2d-f64 | 11 | 8,832 | 0 | 195,795 |
| parry2d f32 (bevy_rapier) | 12 | 8,568 | 1 | 289,906 |
| avian2d f64 | 11 | 8,655 | 8 | 289,896 |

- **Touching**: parry counts two boxes that touch as overlapping; the rules do not (`frames::overlap` is strict).
  The hurtboxes are whole pixels and stacked edge to edge (a torso from 109 up on legs up to 109), so it happens:
  8,833 probes of about 2.6 million. Asking parry for a contact below 0 instead brings that to 552, and the rest are
  parry's box being a centre and half extents (`(x0 + x1) / 2` and back is not always `x0` in f64).
- **Bolts**: parry's ray against a ball solves the same quadratic in another order; 6% of the hits land a few ULPs
  away. The bolt is then moved by that distance, so the fight drifts from there.
- **Avian's API is f32** even with `f64`: positions are f64 (`Position(DVec2)`), but shape constructors
  (`Collider::circle(f32)`), ray directions (`Dir2`), max distances and hit distances are f32. Its bolt hits are
  f32 distances (289,896 differ; 8 strike another target). A shape built from parry's f64 `SharedShape` keeps
  the boxes' sizes, which is why its overlaps agree as well as parry's.
- **f32** (bevy_rapier): the same as Avian's bolts and one more wrong target; every golden would move.

Any of them moves the goldens: phase 2 is a rule change whichever is chosen (ladder, re-pins, `RULES_VERSION`).

### Cost per tick

Native, release, Apple M4 under a load of about 16 (a house-AI retrain was running), so the ratios count more than
the nanoseconds; wasm32 under node in brackets. The queries of one tick (about 22) on top of the duel's own tick:

| | ns a tick |
|---|---|
| a duel tick with the geometry gathered | 8,800 (19,800) |
| hand | 1,300 (2,500) |
| parry2d-f64 | 2,000 (4,000) |
| parry2d-f64 strict | 4,700 |
| rapier2d-f64 | 15,700 (31,300) |
| avian2d f64 | 36,700 (79,200) |

Rapier's and Avian's cost is keeping a world of colliders in step with the frame: up to 200 shapes reshaped and
moved and their BVH refitted each tick, for two fighters and a few bolts. Avian also runs its whole physics
schedule (broad phase, narrow phase, solver, writeback) to refresh its query trees; before the filters between its
own bodies were turned off it was 2.1 ms a tick, 240 times the duel. Rapier adds 1.8 times a duel tick, Avian 4
times: the ladder and the trainer would run 3 to 5 times slower for the same answers.

### Size in wasm32

The spike's wasm (the rules and the harness), after `wasm-opt -Oz`:

| | bytes | gzipped | added |
|---|---|---|---|
| rules only | 1,015,606 | 399,449 | |
| + parry2d-f64 | 1,207,247 | 477,859 | +192 KB (+78 KB gz) |
| + rapier2d-f64 (queries only) | 1,235,723 | 489,904 | +220 KB (+90 KB gz) |
| + avian2d | 7,040,254 | 1,999,214 | +6.0 MB (+1.6 MB gz) |

Avian's figure includes `bevy_app`, `bevy_transform`, `bevy_reflect` and the rest of Bevy it pulls in; the page's
game (20.6 MB today) has most of those already, so on the page it adds less, but it is an upper bound worth having.

### Fit

- **Frame-driven shapes**: every fighter's boxes change shape every frame (1 to 3 boxes, the bolt's circles 1 to
  about 90 for a fighter lying down). parry takes a box or a circle where it is, as the rules have it. Rapier and
  Avian keep colliders that must be reshaped, moved and re-indexed first; Avian through component inserts
  (`Collider` replaced, its hooks and mass properties run).
- **Order**: the rules strike in a fixed order (the foe, then units by id; the first target on a tie). Rapier's and
  Avian's queries come in BVH order and must be sorted back; parry answers in the order it is asked.
- **The duel is a bare `bevy_ecs` world**: its tick is one schedule of chained systems, and the crate never
  depends on `bevy` so the trainer stays light. Avian is a set of Bevy plugins on an `App` (task pools,
  `TransformPlugin`, `Time`) and depends on the `bevy` crate; it would make the duel an `App` or keep a second one
  inside it. Rapier and parry are plain libraries.
- **Sweeps**: bolts already sweep (a ray into circles, so a fast bolt cannot pass through). parry's `cast_ray` and
  `cast_shapes` do the same; Avian's `cast_ray` and `cast_shape` only in f32.
- **Movement**: the bodies move by JS-number arithmetic taken from pixfx (`body`, `js`): walls are clamps, gravity
  a velocity, two fighters are parted along x (`body::separate`). Neither engine's character controller (Avian's
  `move_and_slide`, rapier's `KinematicCharacterController`) does less work or gives the same numbers; they are
  for bodies sliding along level geometry, and the arena's is a floor and two walls.
- **Bevy 0.20**: Avian 0.8.0 (2026-10-20) and bevy_rapier2d 0.37.0 (2026-10-10) both build on Bevy 0.20. parry and
  rapier do not depend on Bevy.

## Decision

**parry2d-f64, with `enhanced-determinism`, used directly by the rules.** It is the collision library of Bevy's
physics ecosystem (both Avian and bevy_rapier query through it), it is f64 like the rules, it costs the least (half
again the hand-written tests, a sixth of rapier's, a twentieth of Avian's), it adds 78 KB gzipped to the wasm, it
takes the frame's boxes as they are, and it stays out of the schedule: the duel keeps its bare world and its order.

Not Avian: an f32 API for every sweep, a 4x slower tick, a second Bevy `App` in the rules and `bevy` in the
trainer's dependencies, for queries parry answers itself. Not bevy_rapier: f32 only. Not rapier's world: a
collider set and BVH for at most a few hundred shapes cost more than testing them all, and answer in an order the
rules must undo.

What it means for the phases:

1. **Collision (phase 2)**: one module in `arena-duel` (`collide`) owns the shapes and the queries on parry: the
   frame's hurtboxes and hitbox as `Cuboid`s placed from `frames_data`, a probe with its margin, the bolt's sweep
   (`cast_ray` into the target circles, or `cast_shapes` for a bolt with a radius), areas as `Aabb` overlap. Who
   may hit whom stays the rules' lists (a fighter's foe, a seat's units), not collision groups: with two seats the
   filter is a `1 - me`. `frames::overlap`, `shot::overlaps`, `unit::strike_hits`, `unit::enemies_near`,
   `enemies_between` and the bolt's entry go through it; `debug::shapes` reads the same shapes, so the hitbox
   overlay keeps its data. The rules' strict test is kept (a contact below 0), so the boxes behave as they do now
   except at parry's rounding (552 probes in 2.6 million) and the bolts' last bits: a rule change, measured with
   the ladder (`--seeds 150`) before and after, goldens re-pinned, `RULES_VERSION` bumped, native = wasm checked.
   The bolt's entry lives in `arena-calc` (`projectile::entry`), which is pure: it would take parry too, or the
   duel hands it the entry, decided when that commit is written.
2. **Movement, gravity and walls (phase 3)**: kept as they are. They are a few lines of arithmetic on two bodies
   whose numbers the goldens pin, an engine's character controller would replace them with more work and other
   numbers, and parry has nothing to add to a clamp between two walls.

If the owner wants Avian itself in the game, the place for it is the game (`arena-game`, which is a Bevy `App`):
its effects, debris and anything that only looks, never the rules.

## Doubts

- The timings were taken on a loaded machine; the ratios held across runs, the nanoseconds did not.
- 552 probes still differ under the strict test, from parry's centre-and-half-extents boxes. Holding the boxes as
  parry's `Aabb` (mins and maxs, the rules' corners as they are) and comparing strictly would make the box tests
  exact, but that is the four comparisons the rules already make: for boxes that never turn, parry adds a type, not
  a test. Its worth to the arena is the sweeps and the round or turned shapes (a bolt with a radius, a capsule, a
  slanted blade, should the art ask for them). So phase 2 may well keep the boxes' goldens still and move only the
  bolts; whether that is migration enough is the owner's call.

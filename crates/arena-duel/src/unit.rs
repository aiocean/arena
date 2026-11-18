//! What a cast makes on the field (`arena_calc::device`, design 17.8): traps, mines, totems and minions, as bodies
//! the foe can strike. Each is an entity of the rules' world with a `Unit` component and an id given in the order
//! made, never reused in a fight. The tick works on them in id order (never query order): it gathers them into
//! `State::units` first and writes them back last (`gather`, `scatter`), so a seed is one fight. Each has a
//! cultivator of its own for its life (`device::Spawn::life`; no shield, no sense), and blows resolve on it through
//! `hit::resolve_with` as on a fighter.
//!
//! - **Trap**: thrown where the cast aimed; once armed (`device::TRAP_ARM`) it springs on the first enemy body within
//!   its trigger radius and strikes: an area skill around itself, anything else on the one that sprang it.
//! - **Mine**: thrown where the cast aimed; its owner sets off every armed mine, the oldest first, as soon as the foe
//!   stands within a mine's blast (`DEVICE_RADIUS`). A seat has no act to detonate (the bot inputs and acts are
//!   unchanged), so the rules set them off for it; all go off in the tick it is decided (no stagger).
//! - **A stopped shot** (Lôi Bạo Trễ, `device::STOPPED_MINE`): a fighter's bolt or orb of a skill under the support
//!   that a guard, a ward, a barrier or a unit stops (the shot spent there) stays where it stopped as its caster's
//!   mine, in the skill's group, striking with the stopped blow times `device::STOPPED_SHARE`; it arms and goes off
//!   as a thrown mine does. A bolt a mirror sends back, two shots meeting, and a unit's shot leave none.
//! - **Totem**: stands in front of its owner and casts the skill on the totem's cadence (`Spawn::first_cast`,
//!   `cast_interval`), each cast one blow from where it stands, moving as the skill's motion does.
//! - **Minion**: walks at the foe at a fighter's walking speed until its burst (`MINION_BLOW`) reaches it, and
//!   strikes the nearest enemy body in front within that reach every `MINION_EVERY` seconds; each blow the summoning skill's built blow times
//!   `Spawn::damage`. It dies when its life runs out: its owner's `MinionDied` goes into the event queue, and each
//!   fighter within `RES.souls_reach` gains a soul.
//!
//! A unit's blow is the built blow of the cast that made it, one blow each; its owner gets no gains from it. The
//! rules emit `Out::Spawn`, `Out::UnitAct`, `Out::UnitHurt` and `Out::UnitDie` for the game to draw.

use crate::collide::{self, Shape, overlap};
use crate::data::{Spell, T, body_spec, motion};
use crate::fighter::State;
use crate::js::{js_sign, or_one};
use crate::out::Out;
use crate::strike::{Origin, Payload, Target, hurt, rolls, scale_hit};
use arena_calc::build::Built;
use arena_calc::cultivator::Cultivator;
use arena_calc::device::{self, Kind, Placed};
use arena_calc::event::{Event, EventKind};
use arena_calc::hit::{Defence, Hit, resolve_with};
use arena_calc::play::{Point, Target as Body};
use arena_calc::projectile::Powers;
use arena_calc::support::Behaviour;
use bevy_ecs::prelude::*;

/// `play::Target` ids: a fighter is its seat, a unit `UNIT_ID + id`.
pub const UNIT_ID: u32 = 1000;
/// A minion's or a totem's body: its centre's height and its radius, in T; a trap's or a mine's, on the floor.
pub const UNIT_Y: f64 = 0.4;
pub const UNIT_RADIUS: f64 = 0.4;
pub const FLOOR_Y: f64 = 0.1;
pub const FLOOR_RADIUS: f64 = 0.2;
/// A mine's blast, and a trap's when its skill is not an area, in T.
pub const DEVICE_RADIUS: f64 = 1.0;
/// The seconds between a minion's blows. It strikes as the close burst (`qiburst`'s motion) does, at its reach.
pub const MINION_EVERY: f64 = 1.0;
/// The motion a minion's blows move as: the basic burst of qi (no blade: the arena is spells only).
pub const MINION_BLOW: &str = "qiburst";
/// The effect a unit appears and goes in, when the registry has it.
pub const UNIT_FX: &str = "spawn/poof";
/// The effect a mine goes off in, when the registry has it.
pub const BLAST_FX: &str = "explosions/small-explosion";

/// A trap, mine, totem or minion standing.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Unit {
    /// Its id in the fight, in the order made.
    pub id: u32,
    pub owner: usize,
    /// The owner's skill that made it.
    pub skill: &'static str,
    pub kind: Kind,
    /// Its id in its group's `Placed`.
    pub device: u32,
    /// Where it stands, in the arena's pixels.
    pub x: f64,
    /// Its life, as a cultivator.
    pub c: Cultivator,
    /// The blow it strikes with: the making cast's, times `Spawn::damage`.
    pub hit: Hit,
    pub control: f64,
    pub powers: Powers,
    /// How its blow moves: the skill's motion.
    pub motion: &'static Spell,
    /// A minion's next blow, in round seconds.
    pub next_strike: f64,
}

impl Unit {
    /// Whether it lies on the floor (a trap, a mine): not a body in the way, under a flying shot.
    pub fn on_floor(&self) -> bool {
        matches!(self.kind, Kind::Trap | Kind::Mine)
    }

    /// Its body's radius in the arena's pixels.
    pub fn radius(&self) -> f64 {
        if self.on_floor() {
            FLOOR_RADIUS * T
        } else {
            UNIT_RADIUS * T
        }
    }

    /// Its life as a share of its full life, 0 to 100.
    pub fn hp(&self) -> f64 {
        if self.c.life_max > 0.0 {
            (self.c.life / self.c.life_max * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        }
    }

    /// The blow it strikes with, as a payload.
    pub fn payload(&self) -> Payload {
        Payload {
            by: self.owner,
            skill: self.skill,
            hit: self.hit.clone(),
            control: self.control,
            knockback: 0.0,
            depth: 0,
            unit: Some(self.id),
            power: self.motion.damage.unwrap_or(0.0),
            powers: self.powers,
        }
    }
}

/// One owner's things from one skill.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub owner: usize,
    pub skill: &'static str,
    pub placed: Placed,
}

/// A unit's body: life `life`, nothing else.
fn body(life: f64) -> Cultivator {
    let mut c = Cultivator::new();
    c.life = life;
    c.life_max = life.max(1.0);
    c.shield = 0.0;
    c.shield_max = 0.0;
    c.sense = 0.0;
    c.sense_max = 0.0;
    c.qi = 0.0;
    c
}

/// The units of the rules' world into `State::units`, in id order: the first system of a tick.
pub fn gather(world: &mut World) {
    let mut q = world.query::<&Unit>();
    let mut units: Vec<Unit> = q.iter(world).cloned().collect();
    units.sort_by_key(|u| u.id);
    world.resource_mut::<State>().units = units;
}

/// `State::units` back into the rules' world: changed ones written, new ones spawned in id order, gone ones
/// despawned. The last system of a tick.
pub fn scatter(world: &mut World) {
    let units = world.resource::<State>().units.clone();
    let mut q = world.query::<(Entity, &mut Unit)>();
    let mut gone = Vec::new();
    let mut seen = Vec::new();
    for (e, mut u) in q.iter_mut(world) {
        match units.iter().find(|n| n.id == u.id) {
            Some(n) => {
                if *u != *n {
                    *u = n.clone();
                }
                seen.push(n.id);
            }
            None => gone.push(e),
        }
    }
    for e in gone {
        world.despawn(e);
    }
    for u in units.into_iter().filter(|u| !seen.contains(&u.id)) {
        world.spawn(u);
    }
}

/// A hurtbox `[x0, y0, x1, y1]` (y up from the floor) as round bodies for a projectile: a line of circles as thick as
/// the box's shorter side, laid along its longer one (a column for a fighter standing, a row for one lying or
/// falling), each touching the box's ends, so a bolt strikes only near where the box is: (x, y, radius).
pub fn circles(h: [f64; 4]) -> Vec<(f64, f64, f64)> {
    let (w, tall) = (h[2] - h[0], h[3] - h[1]);
    let r = (w.min(tall) / 2.0).max(1.0);
    // along the longer side: from its start to its end, `r` apart, the last pulled back to touch the end
    let line = |from: f64, to: f64| {
        let mut at = Vec::new();
        let mut c = from + r;
        loop {
            at.push(c.min(to - r).max(from + r.min((to - from) / 2.0)));
            if c + r >= to {
                break;
            }
            c += r;
        }
        at
    };
    if tall >= w {
        let cx = (h[0] + h[2]) / 2.0;
        line(h[1], h[3]).into_iter().map(|cy| (cx, cy, r)).collect()
    } else {
        let cy = (h[1] + h[3]) / 2.0;
        line(h[0], h[2]).into_iter().map(|cx| (cx, cy, r)).collect()
    }
}

/// Seat `me`'s enemies as projectile targets (in T): the foe (dead or not), then the foe's units in id order; with
/// `floor`, traps and mines too.
pub fn targets_of(state: &State, me: usize, floor: bool) -> Vec<Body> {
    let foe = 1 - me;
    let f = &state.fighters[foe];
    let alive = f.alive() && !f.body.untouchable();
    // the foe's hurtboxes as the projectile's round bodies (`circles`), one target (the foe's id) however many, so a
    // bolt strikes it once
    let mut out = Vec::new();
    for h in crate::frames::hurtboxes(state, foe) {
        for (cx, cy, r) in circles(h) {
            let mut t = Body::new(foe as u32, state.to_t(cx), cy / T, r / T);
            t.alive = alive;
            out.push(t);
        }
    }
    for u in state.units.iter().filter(|u| u.owner == foe) {
        if u.on_floor() && !floor {
            continue;
        }
        let (y, r) = if u.on_floor() {
            (FLOOR_Y, FLOOR_RADIUS)
        } else {
            (UNIT_Y, UNIT_RADIUS)
        };
        out.push(Body::new(UNIT_ID + u.id, state.to_t(u.x), y, r));
    }
    out
}

/// The nearest enemy body of seat `me` (the foe, a minion or a totem) a blade swung from `x` reaches: the foe within
/// Does a strike by seat `me` from `x` (its feet at `air`), reaching `reach` ahead the way `face` looks and `level` up
/// from those feet, touch the foe? The strike is a box from `x` to `reach` ahead; a fighter's own blade blow (`blade`)
/// is its clip's hitbox on the active frame (`frames::strike_hitbox`: the drawn hand and blade's height, as long as
/// the drawing or the reach, whichever is farther). It touches the foe when it overlaps one of the hurtboxes of the
/// frame the foe shows (`frames::hurtboxes`). A foe behind the striker is not struck.
#[allow(clippy::too_many_arguments)]
pub fn strike_hits(
    state: &State,
    me: usize,
    x: f64,
    air: f64,
    face: f64,
    reach: f64,
    level: f64,
    blade: bool,
) -> bool {
    let bx = strike_box(state, me, x, air, face, reach, level, blade);
    collide::touches(state, 1 - me, &Shape::rect(bx))
}

/// The box `strike_hits` tests: from `x` to `reach` ahead, `level` up from `air`, or a fighter's blade's hitbox (from
/// the hilt in its hand out to the reach or the drawn tip).
#[allow(clippy::too_many_arguments)]
pub fn strike_box(
    state: &State,
    me: usize,
    x: f64,
    air: f64,
    face: f64,
    reach: f64,
    level: f64,
    blade: bool,
) -> [f64; 4] {
    let ahead = face >= 0.0;
    let mut bx = if ahead {
        [x, air, x + reach, air + level]
    } else {
        [x - reach, air, x, air + level]
    };
    let caster = (state.fighters[me].body.x - x).abs() < 1e-9;
    if blade
        && caster
        && let Some(h) = crate::frames::strike_hitbox(state, me)
    {
        // the blade from its hilt (where the hand holds it) out to the reach or the drawn blade's tip
        bx[1] = h[1];
        bx[3] = h[3];
        if ahead {
            bx[0] = h[0];
            bx[2] = bx[2].max(h[2]);
        } else {
            bx[0] = bx[0].min(h[0]);
            bx[2] = h[2];
        }
    }
    bx
}

/// `reach` of its edge either side and within `level` of `air`, a unit in front within `reach` of its edge.
pub fn in_front(
    state: &State,
    me: usize,
    x: f64,
    air: f64,
    face: f64,
    reach: f64,
    level: f64,
) -> Option<Target> {
    let foe = 1 - me;
    let f = &state.fighters[foe];
    let mut best: Option<(Target, f64)> = None;
    if f.alive() && strike_hits(state, me, x, air, face, reach, level, true) {
        best = Some((Target::Fighter(foe), (f.body.x - x).abs()));
    }
    for u in state
        .units
        .iter()
        .filter(|u| u.owner == foe && !u.on_floor())
    {
        let d = (u.x - x).abs();
        let reaches = Shape::rect(strike_box(state, me, x, air, face, reach, level, false));
        if overlap(&reaches, &collide::unit_body(u)) && best.is_none_or(|b| d < b.1) {
            best = Some((Target::Unit(u.id), d));
        }
    }
    best.map(|b| b.0)
}

/// Every enemy body of seat `me` within `radius` of `x` (to its edge), the foe only within `level` of `air`: the foe
/// first, then the units in id order (traps and mines too).
pub fn enemies_near(
    state: &State,
    me: usize,
    x: f64,
    air: f64,
    radius: f64,
    level: f64,
) -> Vec<Target> {
    let foe = 1 - me;
    let field = collide::area(x - radius, x + radius, air, level);
    let mut out = Vec::new();
    if state.fighters[foe].alive() && collide::touches(state, foe, &field) {
        out.push(Target::Fighter(foe));
    }
    for u in state.units.iter().filter(|u| u.owner == foe) {
        if overlap(&field, &collide::unit_body(u)) {
            out.push(Target::Unit(u.id));
        }
    }
    out
}

/// Every enemy body of seat `me` a dash from `lo` to `hi` passes through: the foe (within `level` of `air`), then
/// its minions and totems in id order.
pub fn enemies_between(
    state: &State,
    me: usize,
    lo: f64,
    hi: f64,
    air: f64,
    level: f64,
) -> Vec<Target> {
    let foe = 1 - me;
    let path = collide::area(lo, hi, air, level);
    let mut out = Vec::new();
    if state.fighters[foe].alive() && collide::touches(state, foe, &path) {
        out.push(Target::Fighter(foe));
    }
    for u in state
        .units
        .iter()
        .filter(|u| u.owner == foe && !u.on_floor())
    {
        if overlap(&path, &collide::unit_body(u)) {
            out.push(Target::Unit(u.id));
        }
    }
    out
}

/// A cast of seat `me`'s kit skill `k` makes its things (`device::spawn` of the built cast): a trap or a mine where
/// the cast aimed (the foe's place when it has no aim), a totem or a minion in front of its owner.
pub fn place(state: &mut State, me: usize, k: usize, built: &Built, aim: Option<f64>) {
    let Some(sp) = device::spawn(built) else {
        return;
    };
    let (skill, m) = {
        let ks = &state.fighters[me].kit.skills[k];
        (ks.gem.id, ks.motion)
    };
    let foe_x = state.fighters[1 - me].body.x;
    let x0 = state.fighters[me].body.x;
    let toward = or_one(js_sign(foe_x - x0));
    let hw = state.fighters[me].body.spec.half_width;
    let x = match sp.kind {
        Kind::Trap | Kind::Mine => aim.unwrap_or(foe_x),
        Kind::Totem | Kind::Minion => x0 + toward * hw * 2.0,
    };
    let x = x.clamp(state.left + hw, state.right - hw);
    let mut hit = built.hit.clone();
    scale_hit(&mut hit, sp.damage);
    put(
        state,
        me,
        skill,
        sp,
        x,
        (hit, built.control, Powers::of_built(built)),
        m,
    );
}

/// Whether a shot carrying `p` stays as a mine where it is stopped (Lôi Bạo Trễ, `Behaviour::MineOnBlock`): a
/// fighter's own shot of a skill under the support. A unit's shot (a totem's cast) does not.
pub fn mines_when_stopped(state: &State, p: &Payload) -> bool {
    let kit = &state.fighters[p.by].kit;
    p.unit.is_none()
        && kit.index(p.skill).is_some_and(|k| {
            kit.skills[k]
                .supports
                .iter()
                .any(|s| s.behaviours.contains(&Behaviour::MineOnBlock))
        })
}

/// Lôi Bạo Trễ: seat `p.by`'s shot of motion `m` carrying `p`, stopped at `x` by a guard, a ward, a barrier or a
/// unit, stays there as its mine (`device::STOPPED_MINE`): its blow the stopped one times `device::STOPPED_SHARE`,
/// armed after `device::MINE_ARM` and set off as any mine (`detonate`). No dice are rolled.
pub fn lay_stopped(state: &mut State, p: &Payload, m: &'static Spell, x: f64) {
    let me = p.by;
    let sp = device::STOPPED_MINE;
    let hw = state.fighters[me].body.spec.half_width;
    let x = x.clamp(state.left + hw, state.right - hw);
    let mut hit = p.hit.clone();
    scale_hit(&mut hit, sp.damage);
    put(state, me, p.skill, sp, x, (hit, p.control, p.powers), m);
}

/// Seat `me`'s skill `skill` makes what `sp` says at `x`, each striking with `blow` (its hit, its control factor,
/// its projectile powers) moving as `m`: into its group (made on the first), the oldest past the cap removed.
fn put(
    state: &mut State,
    me: usize,
    skill: &'static str,
    sp: device::Spawn,
    x: f64,
    blow: (Hit, f64, Powers),
    m: &'static Spell,
) {
    let (hit, control, powers) = blow;
    let gi = match state
        .groups
        .iter()
        .position(|g| g.owner == me && g.skill == skill)
    {
        Some(i) => i,
        None => {
            state.groups.push(Group {
                owner: me,
                skill,
                placed: Placed::new(sp),
            });
            state.groups.len() - 1
        }
    };
    state.groups[gi].placed.spawn = sp;
    let at = Point::new(state.to_t(x), 0.0);
    let placement = state.groups[gi].placed.place(at);
    for dev in placement.removed {
        if let Some(i) = unit_of(state, me, skill, dev) {
            remove(state, i);
        }
    }
    for dev in placement.made {
        state.unit_id += 1;
        let id = state.unit_id;
        state.units.push(Unit {
            id,
            owner: me,
            skill,
            kind: sp.kind,
            device: dev,
            x,
            c: body(sp.life),
            hit: hit.clone(),
            control,
            powers,
            motion: m,
            next_strike: state.time + MINION_EVERY / 2.0,
        });
        state.emit(Out::Spawn {
            id,
            by: me,
            kind: sp.kind,
            skill,
            x,
        });
        poof(state, x);
    }
}

/// The puff a unit appears and goes in.
fn poof(state: &mut State, x: f64) {
    if state.has(UNIT_FX) {
        let g = state.ground;
        crate::act::play(
            state,
            UNIT_FX,
            x,
            g,
            crate::out::PlayOpts {
                seed: Some(7),
                ..crate::out::PlayOpts::default()
            },
        );
    }
}

/// Where unit (`owner`, `skill`, `device`) sits in `State::units`.
fn unit_of(state: &State, owner: usize, skill: &str, device: u32) -> Option<usize> {
    state
        .units
        .iter()
        .position(|u| u.owner == owner && u.skill == skill && u.device == device)
}

/// Unit at `i` is gone (outlived, replaced, set off or sprung): no death, no soul.
fn remove(state: &mut State, i: usize) {
    if i >= state.units.len() {
        return;
    }
    let u = state.units.remove(i);
    state.emit(Out::UnitDie { id: u.id, x: u.x });
    poof(state, u.x);
}

/// Unit `id` is gone, if it still stands.
fn remove_id(state: &mut State, id: u32) {
    if let Some(i) = state.units.iter().position(|u| u.id == id) {
        remove(state, i);
    }
}

/// Blow `p` lands on unit `id`; returns whether it landed. A unit at no life is destroyed: a minion's death goes
/// into its owner's events, and each fighter near it gains a soul.
pub fn hurt_unit(state: &mut State, id: u32, p: &Payload) -> bool {
    let Some(i) = state.units.iter().position(|u| u.id == id) else {
        return false;
    };
    let r = rolls(state);
    let by = p.by;
    let striker = p.unit.is_none() && state.fighters[by].alive();
    let out = {
        let State {
            fighters, units, ..
        } = state;
        let s = striker.then_some(&fighters[by].c);
        resolve_with(&p.hit, s, &mut units[i].c, &Defence::default(), r)
    };
    let x = state.units[i].x;
    if out.evaded {
        state.emit(Out::Dodge { x, y: UNIT_Y * T });
        return false;
    }
    let damage = out.total();
    let share = if state.units[i].c.life_max > 0.0 {
        out.to_life / state.units[i].c.life_max * 100.0
    } else {
        0.0
    };
    state.emit(Out::UnitHurt {
        id,
        by,
        damage,
        share,
        x,
    });
    if striker {
        state.fighters[by].c.gain_souls(out.gains.souls);
        state.queue.push(
            Event::new(EventKind::Hit, by)
                .with_skill(p.skill, p.hit.tags)
                .at_depth(p.depth),
        );
    }
    if state.units[i].c.life <= 0.0 {
        dies(state, i, Some(by));
    } else {
        sync_life(state, i);
    }
    true
}

/// Unit at `i`'s device follows its life.
fn sync_life(state: &mut State, i: usize) {
    let (owner, skill, dev, life) = {
        let u = &state.units[i];
        (u.owner, u.skill, u.device, u.c.life)
    };
    if let Some(g) = state
        .groups
        .iter_mut()
        .find(|g| g.owner == owner && g.skill == skill)
        && let Some(d) = g.placed.standing.iter_mut().find(|d| d.id == dev)
    {
        d.life = life;
    }
}

/// Unit at `i` is destroyed (`by` struck the last blow, if anyone did).
fn dies(state: &mut State, i: usize, by: Option<usize>) {
    let (owner, skill, dev, kind, x) = {
        let u = &state.units[i];
        (u.owner, u.skill, u.device, u.kind, u.x)
    };
    if let Some(g) = state
        .groups
        .iter_mut()
        .find(|g| g.owner == owner && g.skill == skill)
    {
        g.placed.hurt(dev, f64::INFINITY);
    }
    remove(state, i);
    if kind == Kind::Minion {
        let mut e = Event::new(EventKind::MinionDied, owner);
        if let Some(b) = by {
            e = e.against(b);
        }
        state.queue.push(e);
        // souls (rules, 1): every fighter within reach of a minion's death gains one, whoever's it was
        for f in &mut state.fighters {
            let d = (f.body.x - x).abs() / T;
            f.c.minion_died(d);
        }
    }
}

/// The units act, group by group in the order made: they age (and fall apart), totems cast, traps spring, mines go
/// off; then minions walk and strike, in id order.
pub fn act(state: &mut State, dt: f64) {
    for gi in 0..state.groups.len() {
        let (owner, skill, kind) = {
            let g = &state.groups[gi];
            (g.owner, g.skill, g.placed.spawn.kind)
        };
        let ticked = state.groups[gi].placed.tick(dt);
        for (dev, _) in ticked.casts {
            if let Some(i) = unit_of(state, owner, skill, dev) {
                totem_cast(state, i);
            }
        }
        for dev in ticked.expired {
            if let Some(i) = unit_of(state, owner, skill, dev) {
                remove(state, i);
            }
        }
        match kind {
            Kind::Trap => {
                let targets = targets_of(state, owner, false);
                let sprung = state.groups[gi].placed.spring(&targets);
                for (dev, enemy) in sprung {
                    if let Some(i) = unit_of(state, owner, skill, dev) {
                        let on = if enemy >= UNIT_ID {
                            Target::Unit(enemy - UNIT_ID)
                        } else {
                            Target::Fighter(enemy as usize)
                        };
                        spring(state, i, on);
                    }
                }
            }
            Kind::Mine => detonate(state, gi),
            Kind::Totem | Kind::Minion => {}
        }
    }
    let walk = body_spec().walk;
    let Some(blow) = motion(MINION_BLOW) else {
        return;
    };
    let reach = blow.range.unwrap_or(0.0);
    let ids: Vec<u32> = state
        .units
        .iter()
        .filter(|u| u.kind == Kind::Minion)
        .map(|u| u.id)
        .collect();
    for id in ids {
        let Some(i) = state.units.iter().position(|u| u.id == id) else {
            continue;
        };
        let owner = state.units[i].owner;
        let foe = &state.fighters[1 - owner];
        let (tx, hw) = (foe.body.x, foe.body.spec.half_width);
        let x = state.units[i].x;
        let dir = or_one(js_sign(tx - x));
        if (tx - x).abs() > reach + hw * 0.5 {
            let hw = state.fighters[owner].body.spec.half_width;
            let nx = (x + dir * walk * dt).clamp(state.left + hw, state.right - hw);
            state.units[i].x = nx;
            let (skill, dev) = (state.units[i].skill, state.units[i].device);
            let to = state.to_t(nx);
            if let Some(g) = state
                .groups
                .iter_mut()
                .find(|g| g.owner == owner && g.skill == skill)
                && let Some(d) = g.placed.standing.iter_mut().find(|d| d.id == dev)
            {
                d.at.x = to;
            }
        }
        let x = state.units[i].x;
        if state.time >= state.units[i].next_strike
            && in_front(state, owner, x, 0.0, dir, reach, f64::INFINITY).is_some()
        {
            state.units[i].next_strike = state.time + MINION_EVERY;
            let p = state.units[i].payload();
            state.emit(Out::UnitAct { id, x });
            let o = Origin {
                x,
                air: 0.0,
                face: dir,
                unit: Some(id),
            };
            crate::act::strike(state, blow, p, o, None);
        }
    }
}

/// A totem at `i` casts its skill from where it stands, at the foe.
fn totem_cast(state: &mut State, i: usize) {
    let u = &state.units[i];
    let (id, x, owner, m) = (u.id, u.x, u.owner, u.motion);
    let p = u.payload();
    let tx = state.fighters[1 - owner].body.x;
    state.emit(Out::UnitAct { id, x });
    let o = Origin {
        x,
        air: 0.0,
        face: or_one(js_sign(tx - x)),
        unit: Some(id),
    };
    crate::act::strike(state, m, p, o, Some(tx));
}

/// A trap at `i` springs on `on`: an area skill strikes around it, anything else strikes `on`.
fn spring(state: &mut State, i: usize, on: Target) {
    let u = &state.units[i];
    let (id, x, m) = (u.id, u.x, u.motion);
    let p = u.payload();
    state.emit(Out::UnitAct { id, x });
    let area = matches!(
        m.kind,
        crate::data::Kind::Zone | crate::data::Kind::Void | crate::data::Kind::Blast
    );
    if area {
        let radius = m.radius.or(m.range).unwrap_or(DEVICE_RADIUS * T);
        blast(state, p, x, radius);
    } else {
        land(state, on, &p, x);
    }
    // what it struck may have moved it in the list
    remove_id(state, id);
}

/// Seat `owner`'s mines of group `gi` go off when the foe stands within a mine's blast of an armed one: every armed
/// mine, the oldest first.
fn detonate(state: &mut State, gi: usize) {
    let (owner, skill) = (state.groups[gi].owner, state.groups[gi].skill);
    let spawn = state.groups[gi].placed.spawn;
    let r = DEVICE_RADIUS * T;
    // the blast's own field (`blast`), reaching the foe's body
    let near = state.groups[gi].placed.standing.iter().any(|d| {
        let x = state.from_t(d.at.x);
        d.armed(&spawn)
            && collide::touches(
                state,
                1 - owner,
                &collide::area(x - r, x + r, 0.0, f64::INFINITY),
            )
    });
    if !near || !state.fighters[1 - owner].alive() {
        return;
    }
    for (dev, _, _) in state.groups[gi].placed.detonate() {
        if let Some(i) = unit_of(state, owner, skill, dev) {
            let u = &state.units[i];
            let (id, x) = (u.id, u.x);
            let p = u.payload();
            state.emit(Out::UnitAct { id, x });
            if state.has(BLAST_FX) {
                let g = state.ground;
                crate::act::play(
                    state,
                    BLAST_FX,
                    x,
                    g - UNIT_Y * T,
                    crate::out::PlayOpts::default(),
                );
            }
            blast(state, p, x, DEVICE_RADIUS * T);
            remove_id(state, id);
        }
    }
}

/// Blow `p` strikes every enemy body within `radius` of `x`.
fn blast(state: &mut State, p: Payload, x: f64, radius: f64) {
    for t in enemies_near(state, p.by, x, 0.0, radius, f64::INFINITY) {
        land(state, t, &p, x);
    }
}

/// Blow `p` from `x` lands on `t`.
fn land(state: &mut State, t: Target, p: &Payload, x: f64) {
    match t {
        Target::Unit(u) => {
            hurt_unit(state, u, p);
        }
        Target::Fighter(f) => {
            let dir = or_one(js_sign(state.fighters[f].body.x - x));
            hurt(state, f, p, dir);
        }
    }
}

/// Time wears the units: their burns and bleeds; what runs out of life is destroyed.
pub fn wear(state: &mut State, dt: f64) {
    let mut i = 0;
    while i < state.units.len() {
        let moving = state.units[i].kind == Kind::Minion;
        state.units[i].c.tick(dt, moving);
        if state.units[i].c.life <= 0.0 {
            dies(state, i, None);
        } else {
            sync_life(state, i);
            i += 1;
        }
    }
}

/// Every unit and group goes (a new round).
pub fn clear(state: &mut State) {
    state.units.clear();
    state.groups.clear();
}

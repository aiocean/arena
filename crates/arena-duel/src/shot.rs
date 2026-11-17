//! Skills in flight: bolts, orbs, zones (seals, collapses, barrages), sprays and voids.
//!
//! Every shot has an id. A tick runs the shots in flight at its start, by id, so a shot cast during the tick waits for
//! the next, and a shot ended earlier in the tick (two bolts meeting) does not run again. A bolt flies as the damage
//! system's projectile (`arena_calc::projectile::Flight`), in T: it pierces, forks and chains among the foe and the
//! foe's units, and extra projectiles fan out from the cast.

use crate::act::{blocks, burst, cast_once, clash, play, turn_aside, zone_height};
use crate::data::{Spell, T, look, orb_offset, spell};
use crate::fighter::State;
use crate::js::{js_min, js_sign, or_one};
use crate::out::{Note, Out, PlayOpts, Stop};
use crate::strike::{Origin, Payload, Target, hurt};
use crate::unit::{UNIT_ID, enemies_near, hurt_unit, targets_of};
use arena_calc::play::Point;
use arena_calc::projectile::{Flight, Shot as Fired};

/// What kind of shot, and its own state.
#[derive(Clone, Debug, PartialEq)]
pub enum ShotKind {
    /// A bolt: the damage system's projectile, in T from the left wall and the ground.
    Bolt { flight: Flight },
    /// An orb whose origin drifts at `x`; the ball loops round it.
    Orb { x: f64 },
    /// A zone at `x`, `air` up, falling at `t = impact`; a seal shows `lead` seconds before; a barrage strike after
    /// the first `follow`s the foe and is `aimed` once.
    Zone {
        x: f64,
        air: f64,
        lead: f64,
        follow: bool,
        aimed: bool,
        shown: bool,
        fell: bool,
    },
    /// A jet that sprays for the skill's `spray` seconds.
    Spray,
    /// A vortex at `x`, ticking every half second.
    Void { x: f64, tick: f64 },
}

/// A skill in flight, cast by seat `owner`.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub id: u32,
    pub owner: usize,
    pub name: &'static str,
    pub spell: &'static Spell,
    pub t: f64,
    pub key: Option<u32>,
    /// A field's tick as a share of its whole blow (the motion's `tick` over its `damage`).
    pub tick: f64,
    /// The blow it carries.
    pub payload: Payload,
    pub kind: ShotKind,
    /// It holds its caster (`Fighter::bound`) until it is over; a bolt sent back holds no one.
    pub binds: bool,
}

impl Shot {
    /// A bolt's place in the arena's pixels (`y` up from the ground) and its speed along x.
    pub fn bolt_at(&self, state: &State) -> Option<(f64, f64, f64)> {
        match &self.kind {
            ShotKind::Bolt { flight } => Some((
                state.from_t(flight.pos.x),
                flight.pos.y * T,
                flight.dir.x * flight.speed * T,
            )),
            _ => None,
        }
    }
}

/// Launches the bolts of a cast of `sp` carrying `p` from `o`: one, or a fan of them with the cast's extra
/// projectiles, each with its own effect.
pub fn launch_bolt(state: &mut State, sp: &'static Spell, p: Payload, o: Origin) {
    let me = p.by;
    let spec = state.fighters[me].body.spec;
    let x = o.x + o.face * 32.0;
    let y = o.air + spec.chest;
    let fired = Fired {
        origin: Point::new(state.to_t(x), y / T),
        direction: Point::new(o.face, 0.0),
        speed: sp.speed.unwrap_or(0.0) / T,
        range: (state.right - state.left) / T + 1.0,
    };
    for flight in Flight::launch(&fired, &p.powers) {
        let key = bolt_effect(state, sp, me, &flight);
        crate::act::push(state, sp, p.clone(), Some(key), ShotKind::Bolt { flight });
    }
}

/// The effect of a bolt flying as `flight`, in its owner's look, facing its way.
fn bolt_effect(state: &mut State, sp: &'static Spell, owner: usize, flight: &Flight) -> u32 {
    let opts = PlayOpts {
        palette: Some(&look(state.fighters[owner].look).bolt),
        flip: Some(flight.dir.x < 0.0),
        seed: Some(owner as u32 + 7),
        ..PlayOpts::default()
    };
    let (x, y) = (state.from_t(flight.pos.x), state.ground - flight.pos.y * T);
    play(state, sp.fx.unwrap_or(""), x, y, opts)
}

/// Move shot `id` on by `dt`; a shot no longer in flight (ended earlier this tick) does nothing.
pub fn shot(state: &mut State, id: u32, dt: f64) {
    let Some(mut s) = state.shots.iter().find(|s| s.id == id).cloned() else {
        return;
    };
    if run(state, &mut s, dt)
        && let Some(slot) = state.shots.iter_mut().find(|x| x.id == id)
    {
        *slot = s;
    }
}

/// One tick of a bolt; returns whether it is still in flight.
fn run_bolt(state: &mut State, s: &mut Shot, mut flight: Flight, dt: f64) -> bool {
    let g = state.ground;
    let foe = 1 - s.owner;
    let sp = s.spell;
    let x = state.from_t(flight.pos.x);
    let y = flight.pos.y * T;
    // two bolts meet head on, or a bolt meets an orb: both are spent
    let met = state.shots.iter().find_map(|o| {
        if o.id == s.id || o.owner == s.owner {
            return None;
        }
        match &o.kind {
            ShotKind::Bolt { .. } => {
                let (ox, oy, _) = o.bolt_at(state)?;
                ((ox - x).abs() < 24.0 && (oy - y).abs() < 32.0).then(|| (o.clone(), None))
            }
            ShotKind::Orb { .. } => {
                let (bx, by) = orb_at(o);
                (libm::hypot(bx - x, by + y) < 40.0).then(|| (o.clone(), Some((bx, by))))
            }
            _ => None,
        }
    });
    if let Some((o, ball)) = met {
        match (&o.kind, ball) {
            (ShotKind::Bolt { .. }, _) => {
                let ox = o.bolt_at(state).map_or(x, |b| b.0);
                clash(state, (ox + x) / 2.0, g - y, &[]);
            }
            (_, Some((bx, by))) => burst(state, o.spell.hit_fx, bx, g + by),
            _ => {}
        }
        end_shot(state, &o);
        end_shot(state, s);
        return false;
    }
    // a mirror sends the bolt back the way it came, now its own
    let f = &state.fighters[foe];
    let reflects = f.barrier > 0.0 && f.barrier_spell.and_then(spell).is_some_and(|b| b.reflect);
    if reflects && overlaps(state, foe, x, y, 36.0) {
        state.note(Note::Stopped {
            by: s.owner,
            spell: s.name,
            how: Stop::Barrier,
            chip: 0.0,
        });
        s.owner = foe;
        s.payload.by = foe;
        s.binds = false;
        flight.dir = Point::new(-flight.dir.x, flight.dir.y);
        flight.pos = flight.pos.plus(flight.dir, 40.0 / T);
        flight.struck.clear();
        turn_aside(state, foe, x, g - y);
        // the effect cannot turn round in flight: the old bolt goes out and a new one flies back, facing its new way
        // and in the colours of the one who sent it back
        stop_shot(state, s);
        s.key = Some(bolt_effect(state, sp, foe, &flight));
        s.kind = ShotKind::Bolt { flight };
        return true;
    }
    let was = flight.dir.x;
    let targets = targets_of(state, s.owner, false);
    let step = flight.step(dt, &targets);
    let (nx, ny) = (state.from_t(flight.pos.x), flight.pos.y * T);
    // a chain that turns the bolt round: a new effect facing its new way
    if js_sign(flight.dir.x) != js_sign(was) && !flight.done {
        stop_shot(state, s);
        s.key = Some(bolt_effect(state, sp, s.owner, &flight));
    } else if let Some(key) = s.key {
        state.emit(Out::Move {
            key,
            x: nx,
            y: g - ny,
        });
    }
    let mut stopped = false;
    for k in &step.strikes {
        let mut p = s.payload.clone();
        p.hit = k.blow(&p.hit);
        let dir = or_one(js_sign(flight.dir.x));
        if k.target >= UNIT_ID {
            hurt_unit(state, k.target - UNIT_ID, &p);
            continue;
        }
        let f = k.target as usize;
        if f != 1 - s.owner {
            continue;
        }
        let hx = state.from_t(k.at.x);
        if blocks(state, f, hx - dir, &p) {
            clash(state, hx, g - k.at.y * T, &[]);
            stopped = true;
            break;
        }
        hurt(state, f, &p, dir);
    }
    for fork in step.forks {
        let key = bolt_effect(state, sp, s.owner, &fork);
        let p = s.payload.clone();
        crate::act::push(state, sp, p, Some(key), ShotKind::Bolt { flight: fork });
    }
    let off = nx < state.left - 160.0 || nx > state.right + 160.0;
    if stopped || flight.done || off {
        s.kind = ShotKind::Bolt { flight };
        end_shot(state, s);
        return false;
    }
    s.kind = ShotKind::Bolt { flight };
    true
}

/// One tick of shot `s`; returns whether it is still in flight (false: it ended itself).
fn run(state: &mut State, s: &mut Shot, dt: f64) -> bool {
    s.t += dt;
    let foe = 1 - s.owner;
    let g = state.ground;
    let sp = s.spell;
    match s.kind.clone() {
        ShotKind::Bolt { flight } => run_bolt(state, s, flight, dt),
        ShotKind::Orb { mut x } => {
            // the orb's origin drifts toward the foe; the ball itself wanders round it on its own loop
            x += js_sign(state.fighters[foe].body.x - x) * sp.speed.unwrap_or(0.0) * dt;
            s.kind = ShotKind::Orb { x };
            if let Some(key) = s.key {
                state.emit(Out::Move { key, x, y: g });
            }
            let (bx, by) = orb_at(s);
            if overlaps(state, foe, bx, -by, sp.radius.unwrap_or(0.0)) {
                if blocks(state, foe, bx, &s.payload) {
                    clash(state, bx, g + by, &[]);
                } else {
                    let dir = or_one(js_sign(state.fighters[foe].body.x - bx));
                    hurt(state, foe, &s.payload, dir);
                }
                burst(state, sp.hit_fx, bx, g + by);
                end_shot(state, s);
                return false;
            }
            if s.t >= sp.life.unwrap_or(0.0) {
                end_shot(state, s);
                return false;
            }
            true
        }
        ShotKind::Zone {
            mut x,
            mut air,
            lead,
            follow,
            mut aimed,
            mut shown,
            mut fell,
        } => {
            let fspec = state.fighters[foe].body.spec;
            // a strike of a barrage aims where the foe was a moment before it falls
            if follow && !aimed && s.t >= -sp.aim_ahead.unwrap_or(0.0) {
                aimed = true;
                x = crate::act::clamp(
                    state.fighters[foe].body.x,
                    state.left + 24.0,
                    state.right - 24.0,
                );
                air = if sp.grounded {
                    0.0
                } else {
                    state.fighters[foe].body.air
                };
            }
            if !shown && s.t >= 0.0 {
                shown = true;
                if let Some(key) = s.key {
                    state.emit(Out::Stop { key });
                }
                s.key = None;
                let y = g - zone_height(sp, &fspec) - air;
                cast_once(
                    state,
                    s.owner,
                    sp.fx.unwrap_or(""),
                    x,
                    y,
                    PlayOpts::default(),
                );
            }
            s.kind = ShotKind::Zone {
                x,
                air,
                lead,
                follow,
                aimed,
                shown,
                fell,
            };
            if s.t < 0.0 {
                return true;
            }
            // a collapse drags the foe toward its heart while it sucks in
            let reach = sp.reach.unwrap_or(0.0);
            let pull = sp.pull.unwrap_or(0.0);
            let f = &mut state.fighters[foe];
            if pull != 0.0
                && s.t < sp.pull_until.unwrap_or(0.0)
                && (x - f.body.x).abs() < reach
                && f.alive()
            {
                let dx = x - f.body.x;
                f.body.x += js_sign(dx) * js_min(dx.abs(), pull * dt);
            }
            if !fell && s.t >= sp.impact.unwrap_or(0.0) {
                fell = true;
                s.kind = ShotKind::Zone {
                    x,
                    air,
                    lead,
                    follow,
                    aimed,
                    shown,
                    fell,
                };
                let radius = sp.radius.unwrap_or(0.0);
                let level = if sp.grounded { 40.0 } else { 88.0 };
                for t in enemies_near(state, s.owner, x, air, radius, level) {
                    match t {
                        Target::Unit(u) => {
                            hurt_unit(state, u, &s.payload);
                        }
                        Target::Fighter(f) => {
                            let dir = or_one(js_sign(state.fighters[f].body.x - x));
                            hurt(state, f, &s.payload, dir);
                        }
                    }
                }
            }
            if s.t >= sp.impact.unwrap_or(0.0) + 0.2 {
                remove_shot(state, s);
                return false;
            }
            true
        }
        ShotKind::Spray => {
            if s.t >= sp.spray.unwrap_or(0.0) {
                stop_shot(state, s);
                remove_shot(state, s);
                return false;
            }
            true
        }
        ShotKind::Void { x, mut tick } => {
            // the vortex drags the foe in, and strikes whoever stands under it
            let dx = x - state.fighters[foe].body.x;
            let f = &mut state.fighters[foe];
            if dx.abs() < sp.reach.unwrap_or(0.0) && f.alive() {
                f.body.x += js_sign(dx) * js_min(dx.abs(), sp.pull.unwrap_or(0.0) * dt);
            }
            // a tick every half second, for its share of the whole blow
            tick += dt;
            let under = sp.under.unwrap_or(0.0);
            if tick >= 0.5 {
                tick -= 0.5;
                if s.tick > 0.0 {
                    let mut p = s.payload.clone();
                    crate::strike::scale_hit(&mut p.hit, s.tick);
                    for t in enemies_near(state, s.owner, x, 0.0, under, f64::INFINITY) {
                        match t {
                            Target::Unit(u) => {
                                hurt_unit(state, u, &p);
                            }
                            Target::Fighter(f) => {
                                let dir = or_one(-js_sign(x - state.fighters[f].body.x));
                                hurt(state, f, &p, dir);
                            }
                        }
                    }
                }
            }
            s.kind = ShotKind::Void { x, tick };
            if s.t >= sp.life.unwrap_or(0.0) {
                burst(state, sp.end_fx, x, g - sp.height.unwrap_or(0.0));
                for t in enemies_near(state, s.owner, x, 0.0, under, f64::INFINITY) {
                    match t {
                        Target::Unit(u) => {
                            hurt_unit(state, u, &s.payload);
                        }
                        Target::Fighter(f) => {
                            let dir = or_one(-js_sign(x - state.fighters[f].body.x));
                            hurt(state, f, &s.payload, dir);
                        }
                    }
                }
                end_shot(state, s);
                return false;
            }
            true
        }
    }
}

/// An orb's ball relative to the ground: its origin plus its loop at its age.
pub fn orb_at(s: &Shot) -> (f64, f64) {
    let x = match s.kind {
        ShotKind::Orb { x } => x,
        _ => 0.0,
    };
    let (ox, oy) = orb_offset(s.t);
    (x + ox, oy)
}

/// Does a point (`x`, `y` up from the ground) touch fighter `f`, give or take `r`?
pub fn overlaps(state: &State, f: usize, x: f64, y: f64, r: f64) -> bool {
    let b = &state.fighters[f].body;
    if b.untouchable() {
        return false;
    }
    let top = if b.duck > 0.0 {
        b.spec.duck
    } else {
        b.spec.body
    };
    (x - b.x).abs() < b.spec.half_width + r && y > b.air - r && y < b.air + top + r
}

/// Shot `s` ends: its effect stops and it leaves the list.
pub fn end_shot(state: &mut State, s: &Shot) {
    stop_shot(state, s);
    remove_shot(state, s);
}

/// Shot `s`'s effect stops.
pub fn stop_shot(state: &mut State, s: &Shot) {
    if let Some(key) = s.key {
        state.emit(Out::Stop { key });
    }
}

/// Shot `s` leaves the list.
fn remove_shot(state: &mut State, s: &Shot) {
    state.shots.retain(|x| x.id != s.id);
}

//! What a cast makes on the field: traps, mines, totems and minions (`arena_calc::device`), as bodies the foe can
//! strike. Each has a cultivator of its own for its life (`device::Spawn::life`; no shield, no sense, the base
//! armour and no resistances), so the foe's blows resolve on it through `hit::resolve_with` like on a fighter.
//!
//! - **Trap** (`device::TRAP_ARM`, `TRAP_RADIUS`): thrown where the foe stood when the cast began; once armed it
//!   springs on the first enemy body within its trigger radius and strikes with the skill's blow (an area skill
//!   strikes every enemy in its radius).
//! - **Mine** (`MINE_ARM`): thrown where the foe stood; its owner's brain sets off every armed mine as soon as the foe
//!   stands within a mine's blast (`DEVICE_RADIUS`), oldest first, `MINE_STAGGER` apart.
//! - **Totem**: stands just in front of its owner and casts the skill on the totem's cadence
//!   (`Spawn::first_cast`, `cast_interval`), each cast one blow from where it stands, travelling as the skill does
//!   without its totem tag.
//! - **Minion**: runs at the foe at `MINION_SPEED` and strikes the nearest enemy body in front within
//!   `MINION_REACH` every `MINION_EVERY`, each blow the summoning skill's built blow times `Spawn::damage`.
//!   It dies when its life runs out: its owner's `MinionDied` goes into the event queue. The rules give no souls for
//!   a minion's death, so none are given.
//!
//! A device's or a minion's blow is the built blow of the cast that made it, kept at the making: one blow each (a
//! skill's `Hits` is spread over the totem's casts and the minion's blows, not multiplied by them). Its owner gets
//! no gains, no rage and no events of its own from it.
//!
//! Targeting: a melee or targeted blow (a fighter's or a minion's) strikes the nearest enemy body in front within
//! reach, the foe or a minion or totem of the foe's in the way. That is the brain's whole choice of target: it
//! never aims past a body in front of it. Areas strike every enemy body inside; projectiles fly among the foe and
//! its units (`projectile::Flight`), so pierce, fork and chain find them. Traps and mines lie on the floor, under a
//! flying shot.

use arena_calc::build::Built;
use arena_calc::cultivator::Cultivator;
use arena_calc::device::{self, Kind, Placed};
use arena_calc::event::{Event, EventKind};
use arena_calc::hit::{Defence, Hit, Rolls, resolve_with};
use arena_calc::play::{self, Point, Target};
use arena_calc::projectile::Powers;

use crate::delivery::{
    AREA_RADIUS, DEVICE_RADIUS, Delivery, FIGHTER_RADIUS, FIGHTER_Y, FLOOR_RADIUS, FLOOR_Y,
    MINION_EVERY, MINION_REACH, MINION_SPEED, NOVA_RADIUS, PROJECTILE_SPEED, UNIT_RADIUS, UNIT_Y,
};
use crate::fight::{Emission, Fight, Payload, SEAT, Tgt, scale_hit};
use crate::{BODY, DT, FIELD, WALL};

/// `play::Target` ids: a fighter is its seat, a unit `UNIT_ID + uid`.
pub const UNIT_ID: u32 = 1000;

/// A trap, mine, totem or minion standing.
#[derive(Clone, Debug)]
pub struct Unit {
    /// Its id in the fight, never reused in a round.
    pub uid: u32,
    pub owner: usize,
    /// The owner's skill that made it.
    pub slot: usize,
    pub kind: Kind,
    /// Its id in its `Group`'s `Placed`.
    pub device: u32,
    pub x: f64,
    /// Its life, as a cultivator (no shield, no sense).
    pub c: Cultivator,
    /// The blow it strikes with: the making cast's, times `Spawn::damage`.
    pub hit: Hit,
    pub control: f64,
    /// How its blow travels, its reach, its radius (a mine's blast, an area's), a projectile's speed and powers.
    pub inner: Delivery,
    pub reach: f64,
    pub radius: f64,
    pub powers: Powers,
    /// A minion's next blow, in round seconds.
    pub next_strike: f64,
}

/// One owner's things from one skill.
#[derive(Clone, Debug)]
pub struct Group {
    pub owner: usize,
    pub slot: usize,
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

/// Whether a unit lies on the floor (a trap, a mine): not a body in the way, under a flying shot.
fn on_floor(k: Kind) -> bool {
    matches!(k, Kind::Trap | Kind::Mine)
}

impl Fight {
    /// Seat `me`'s enemies as targets: the foe (dead or not), then the foe's units in the order made; with
    /// `floor`, traps and mines too.
    pub(crate) fn enemies(&self, me: usize, floor: bool) -> Vec<(Tgt, Target)> {
        let foe = 1 - me;
        let mut t = Target::new(foe as u32, self.f[foe].x, FIGHTER_Y, FIGHTER_RADIUS);
        t.alive = self.c[foe].life > 0.0;
        let mut out = vec![(Tgt::Fighter(foe), t)];
        for u in self.units.iter().filter(|u| u.owner == foe) {
            let (y, r) = if on_floor(u.kind) {
                if !floor {
                    continue;
                }
                (FLOOR_Y, FLOOR_RADIUS)
            } else {
                (UNIT_Y, UNIT_RADIUS)
            };
            out.push((Tgt::Unit(u.uid), Target::new(UNIT_ID + u.uid, u.x, y, r)));
        }
        out
    }

    /// The nearest enemy body (the foe, a minion, a totem) in front of `x`, facing the foe, within `reach` T.
    pub(crate) fn in_front(&self, me: usize, x: f64, reach: f64) -> Option<Tgt> {
        let tx = self.f[1 - me].x;
        let dir = if tx >= x { 1.0 } else { -1.0 };
        self.enemies(me, false)
            .into_iter()
            .filter(|(_, t)| t.alive)
            .map(|(g, t)| (g, (t.x - x) * dir))
            .filter(|&(_, ahead)| ahead >= -BODY / 2.0 && ahead <= reach + 1e-9)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(g, _)| g)
    }

    /// The units of a group, by device id.
    fn unit_of(&self, owner: usize, slot: usize, device: u32) -> Option<usize> {
        self.units
            .iter()
            .position(|u| u.owner == owner && u.slot == slot && u.device == device)
    }

    /// Drops units whose device no longer stands in its group.
    pub(crate) fn sync_units(&mut self) {
        let groups = &self.groups;
        self.units.retain(|u| {
            groups.iter().any(|g| {
                g.owner == u.owner && g.slot == u.slot && g.placed.standing.iter().any(|d| d.id == u.device)
            })
        });
    }

    /// A cast of seat `me`'s `slot` makes its things (`device::spawn` of the built cast), at `aim` for a trap or a
    /// mine, in front of the caster for a totem or a minion.
    pub(crate) fn place(&mut self, me: usize, slot: usize, built: &Built, aim: f64) {
        let Some(sp) = device::spawn(built) else {
            return;
        };
        let foe_x = self.f[1 - me].x;
        let x0 = self.f[me].x;
        let toward = if foe_x >= x0 { 1.0 } else { -1.0 };
        let x = match sp.kind {
            Kind::Trap | Kind::Mine => aim,
            Kind::Totem | Kind::Minion => (x0 + toward * BODY).clamp(WALL, FIELD - WALL),
        };
        let s = &self.f[me].slots[slot];
        let inner = s.inner;
        let reach = crate::delivery::reach(crate::delivery::without_device(s.tags));
        let radius = match (sp.kind, inner) {
            (Kind::Mine, _) => play::radius(DEVICE_RADIUS, &built.table),
            (_, Delivery::Nova) => play::radius(NOVA_RADIUS, &built.table),
            (_, Delivery::Area { .. }) => play::radius(AREA_RADIUS, &built.table),
            _ => play::radius(DEVICE_RADIUS, &built.table),
        };
        let gi = match self
            .groups
            .iter()
            .position(|g| g.owner == me && g.slot == slot)
        {
            Some(i) => i,
            None => {
                self.groups.push(Group {
                    owner: me,
                    slot,
                    placed: Placed::new(sp),
                });
                self.groups.len() - 1
            }
        };
        self.groups[gi].placed.spawn = sp;
        let made = self.groups[gi].placed.place(Point::new(x, 0.0)).made;
        let mut hit = built.hit.clone();
        scale_hit(&mut hit, sp.damage);
        for id in made {
            let uid = self.next_uid;
            self.next_uid += 1;
            self.units.push(Unit {
                uid,
                owner: me,
                slot,
                kind: sp.kind,
                device: id,
                x,
                c: body(sp.life),
                hit: hit.clone(),
                control: built.control,
                inner,
                reach,
                radius,
                powers: Powers::of_built(built),
                next_strike: self.time + MINION_EVERY / 2.0,
            });
        }
        self.sync_units();
        let g = self.f[me].slots[slot].gem;
        self.note(|_| format!("{} makes {:?} ({}) at {:.1}", SEAT[me], sp.kind, g.id, x));
    }

    /// A blow of unit `u`, due now, from where it stands: `target` when it must land there.
    fn unit_blow(&mut self, u: usize, delay: f64, delivery: Delivery, aim: f64, target: Option<Tgt>) {
        let unit = &self.units[u];
        let place = self.f[unit.owner].slots[unit.slot].place;
        // an area lands no farther than the unit reaches
        let aim = unit.x + (aim - unit.x).clamp(-unit.reach, unit.reach);
        self.pending.push(Emission {
            at: self.time + delay,
            delivery,
            origin: Some(unit.x),
            target,
            aim,
            reach: unit.reach,
            radius: unit.radius,
            speed: PROJECTILE_SPEED,
            powers: unit.powers,
            first: false,
            payload: Payload {
                by: unit.owner,
                slot: unit.slot,
                hit: unit.hit.clone(),
                control: unit.control,
                knockback: 0.0,
                depth: 0,
                ultimate: place == crate::report::Place::Ultimate,
                rage: false,
                from_unit: Some(unit.uid),
            },
        });
    }

    /// Seat `me`'s brain sets off its armed mines when the foe stands within a mine's blast.
    pub(crate) fn detonate(&mut self, me: usize) {
        let foe_x = self.f[1 - me].x;
        let mut blows = Vec::new();
        for gi in 0..self.groups.len() {
            let g = &self.groups[gi];
            if g.owner != me || g.placed.spawn.kind != Kind::Mine {
                continue;
            }
            let (owner, slot) = (g.owner, g.slot);
            let near = g.placed.standing.iter().any(|d| {
                d.armed(&g.placed.spawn)
                    && self
                        .unit_of(owner, slot, d.id)
                        .is_some_and(|u| (foe_x - d.at.x).abs() <= self.units[u].radius + FIGHTER_RADIUS)
            });
            if !near {
                continue;
            }
            // the units' numbers before the mines go
            let units: Vec<(u32, Option<usize>)> = self.groups[gi]
                .placed
                .standing
                .iter()
                .map(|d| (d.id, self.unit_of(owner, slot, d.id)))
                .collect();
            for (id, delay, at) in self.groups[gi].placed.detonate() {
                if let Some(u) = units.iter().find(|x| x.0 == id).and_then(|x| x.1) {
                    blows.push((u, delay, at.x));
                }
            }
        }
        if blows.is_empty() {
            return;
        }
        for &(u, delay, x) in &blows {
            self.unit_blow(u, delay, Delivery::Area { delayed: false }, x, None);
        }
        self.note(|_| format!("{} sets off {} mine(s)", SEAT[me], blows.len()));
        self.sync_units();
    }

    /// Step 4 for the units: ages, totem casts, traps springing, minions running and striking, what fell apart.
    pub(crate) fn tick_units(&mut self) {
        for gi in 0..self.groups.len() {
            let (owner, slot) = (self.groups[gi].owner, self.groups[gi].slot);
            let ticked = self.groups[gi].placed.tick(DT);
            for (id, at) in ticked.casts {
                if let Some(u) = self.unit_of(owner, slot, id) {
                    let aim = self.f[1 - owner].x;
                    let inner = self.units[u].inner;
                    self.unit_blow(u, at.max(0.0), inner, aim, None);
                }
            }
            if self.groups[gi].placed.spawn.kind == Kind::Trap {
                let ens = self.enemies(owner, false);
                let targets: Vec<Target> = ens.iter().map(|e| e.1).collect();
                let units: Vec<(u32, Option<usize>)> = self.groups[gi]
                    .placed
                    .standing
                    .iter()
                    .map(|d| (d.id, self.unit_of(owner, slot, d.id)))
                    .collect();
                for (trap, enemy) in self.groups[gi].placed.spring(&targets) {
                    let Some(u) = units.iter().find(|x| x.0 == trap).and_then(|x| x.1) else {
                        continue;
                    };
                    let springer = ens.iter().find(|e| e.1.id == enemy).map(|e| e.0);
                    let x = self.units[u].x;
                    match self.units[u].inner {
                        Delivery::Area { .. } | Delivery::Nova => {
                            self.unit_blow(u, 0.0, Delivery::Area { delayed: false }, x, None)
                        }
                        d => self.unit_blow(u, 0.0, d, x, springer),
                    }
                    self.note(|_| format!("{}'s trap springs", SEAT[owner]));
                }
            }
        }
        // minions
        for u in 0..self.units.len() {
            if self.units[u].kind != Kind::Minion {
                continue;
            }
            let owner = self.units[u].owner;
            let tx = self.f[1 - owner].x;
            let x = self.units[u].x;
            if (tx - x).abs() > MINION_REACH * 0.8 {
                let dir = if tx >= x { 1.0 } else { -1.0 };
                let nx = (x + dir * MINION_SPEED * DT).clamp(WALL, FIELD - WALL);
                self.units[u].x = nx;
                let (slot, id) = (self.units[u].slot, self.units[u].device);
                if let Some(g) = self
                    .groups
                    .iter_mut()
                    .find(|g| g.owner == owner && g.slot == slot)
                    && let Some(d) = g.placed.standing.iter_mut().find(|d| d.id == id)
                {
                    d.at.x = nx;
                }
            }
            if self.time >= self.units[u].next_strike
                && let Some(t) = self.in_front(owner, self.units[u].x, MINION_REACH)
            {
                self.units[u].next_strike = self.time + MINION_EVERY;
                self.unit_blow(u, 0.0, Delivery::Melee, self.units[u].x, Some(t));
            }
        }
        self.sync_units();
    }

    /// A blow lands on unit `uid`.
    pub(crate) fn strike_unit(&mut self, p: Payload, uid: u32) {
        let Some(i) = self.units.iter().position(|u| u.uid == uid) else {
            return;
        };
        let rolls = Rolls {
            evade: self.rng.next_f64(),
            crit: self.rng.next_f64(),
            double: self.rng.next_f64(),
            star: self.rng.next_f64(),
        };
        let by = p.by;
        let out = {
            let striker = if p.from_unit.is_some() {
                None
            } else {
                Some(&self.c[by])
            };
            resolve_with(&p.hit, striker, &mut self.units[i].c, &Defence::default(), rolls)
        };
        let line = &mut self.f[by].report.skills[p.slot];
        if out.evaded {
            line.missed += 1;
            return;
        }
        let dealt = out.to_shield + out.to_life + out.to_sense;
        line.landed += 1;
        line.unit_damage += dealt;
        line.device_blows += u32::from(p.from_unit.is_some());
        if p.from_unit.is_none() {
            let gem = self.f[by].slots[p.slot].gem;
            self.queue
                .push(Event::new(EventKind::Hit, by).with_skill(gem.id, p.hit.tags).at_depth(p.depth));
        }
        let kind = self.units[i].kind;
        self.note(|f| {
            format!(
                "{} hits {}'s {:?} for {:.0} (life {:.0})",
                SEAT[by],
                SEAT[1 - by],
                kind,
                dealt,
                f.units.get(i).map_or(0.0, |u| u.c.life)
            )
        });
        self.after_unit_hurt(i, Some(by));
    }

    /// Unit `i` took damage: its device's life follows; at none it is destroyed.
    pub(crate) fn after_unit_hurt(&mut self, i: usize, by: Option<usize>) {
        let (owner, slot, id, life) = {
            let u = &self.units[i];
            (u.owner, u.slot, u.device, u.c.life)
        };
        let Some(g) = self
            .groups
            .iter_mut()
            .find(|g| g.owner == owner && g.slot == slot)
        else {
            return;
        };
        if life > 0.0 {
            if let Some(d) = g.placed.standing.iter_mut().find(|d| d.id == id) {
                d.life = life;
            }
            return;
        }
        g.placed.hurt(id, f64::INFINITY);
        let kind = self.units[i].kind;
        self.units.remove(i);
        if kind == Kind::Minion {
            let mut e = Event::new(EventKind::MinionDied, owner);
            if let Some(b) = by {
                e = e.against(b);
            }
            self.queue.push(e);
        }
        self.note(|_| format!("{}'s {:?} is destroyed", SEAT[owner], kind));
    }

    /// Steps 7 to 9 for the units: their burns and bleeds.
    pub(crate) fn wear_units(&mut self) {
        let mut i = 0;
        while i < self.units.len() {
            let moving = self.units[i].kind == Kind::Minion;
            self.units[i].c.tick(DT, moving);
            let before = self.units.len();
            self.after_unit_hurt(i, None);
            if self.units.len() == before {
                i += 1;
            }
        }
    }

    /// Seat `me`'s units of `kind` standing.
    pub fn units_of(&self, me: usize, kind: Kind) -> impl Iterator<Item = &Unit> {
        self.units
            .iter()
            .filter(move |u| u.owner == me && u.kind == kind)
    }
}

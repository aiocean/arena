//! The fight: two fighters on a 1D field, 60 ticks a second, in the fixed order of the rules' tick
//! (`docs/specs/2026-11-17-skill-system-rules.md`, 9) as far as it applies here:
//!
//! 1. each seat's brain decides, seat 0 then seat 1; a cast starts at once (its costs paid, its cooldown use spent);
//! 2. domains and intents: the field's domains run down and put their rules on the fighters, each fighter's intents
//!    age and press;
//! 3. movement: each fighter runs as its brain asked, at `RUN_SPEED` through its `MoveSpeed`;
//! 4. the clocks: stuns, roots, wards, cooldown uses; a windup that ends lets its blows out (or its movement, ward,
//!    heal, intent, domain, curse, buff, or its traps, mines, totems and minions), and the fighter recovers; the
//!    units age, totems cast, traps spring, minions run and strike (each brain set off its mines in step 1);
//! 5. projectiles fly; blows whose time has come are checked against where the foe stands;
//! 6. the blows that land are resolved, highest rank first, then instants, then the seat `(seed + tick) mod 2`
//!    picks first; each takes four rolls (evade, crit, double, star) in that order; the event queue runs and the
//!    casts its triggers return are delivered (their blows land in the same tick, up to the chain cap);
//! 7. to 9. curses beat, then `Cultivator::tick_with` (damage over time, ailments running out, grit, regen), seat 0
//!    then seat 1; the queue runs again on the damage over time;
//! 10. deaths (spare lives first) and the round's end.
//!
//! Not the real fight: no jumps, no guard, no bodies but a width, no clash, no ranks of shields (step 9), no fusion's
//! after-effects (Lôi hỏa's second blow, Dung nham's zone); a channel is a plain cast. Traps, mines, totems and
//! minions are `units`; projectiles fly among the foe and its units (`projectile::Flight`).

use arena_calc::ailment::{AIL, Ailment, Control};
use arena_calc::aura;
use arena_calc::build::{Built, Situation, Skill, build_skill};
use arena_calc::cultivator::{Active, Cultivator, RES};
use arena_calc::curse::{self, Curse, Lay};
use arena_calc::domain;
use arena_calc::element::Element;
use arena_calc::event::{self, Blow, Cast, Dice, Event, EventKind, Queue, Seat};
use arena_calc::gem::{self, Effect, Resource};
use arena_calc::grade::Grade;
use arena_calc::hit::{self, Defence, Frame, Hit, Rolls, resolve, resolve_with};
use arena_calc::keystone::Keystone;
use arena_calc::loadout::Loadout;
use arena_calc::motion;
use arena_calc::play::{self, Point, Shape, Target};
use arena_calc::projectile::{Flight, Powers, Shot as ProjectileShot};
use arena_calc::stat::{Counts, Happened, Mod, RECENT, Source, Stat};
use arena_calc::support::Behaviour;
use arena_calc::tag::Tag;

use crate::delivery::{
    AREA_RADIUS, BLESSING_TAKEN, BLINK_LENGTH, BUFF_SECONDS, DASH_LENGTH, DELAYED, Delivery,
    FIGHTER_Y, NOVA_RADIUS, PROJECTILE_SPEED, PULSE, REACH_NEAR, STANCE_MORE, WARD_CHARGES,
    WARD_SECONDS,
};
use crate::fighter::{Act, Fighter};
use crate::report::SideReport;
use crate::rng::Rng;
use crate::units::{Group, Unit};
use crate::{
    BODY, COUNTER_STAGGER, DT, FIELD, HITSTUN, MAX_ROUNDS, PUNISH_STAGGER, ROUND_TIME,
    ROUNDS_TO_WIN, RUN_SPEED, WALL, brain,
};

/// Rolls handed to the event queue each time it runs (its chance checks take them in order).
const QUEUE_ROLLS: usize = 16;
/// Times the blows of one tick and the casts their triggers return are worked through, at most.
const LAND_PASSES: usize = 8;
/// Seconds between two refreshes of a brain's damage estimates.
const ESTIMATE_EVERY: f64 = 1.0;
/// Seconds after a guard skill during which the foe "just guarded" (Nhử Đòn).
const JUST_GUARDED: f64 = 1.0;

/// What a brain asks for on a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Order {
    /// Nothing (or keep doing what it does).
    Wait,
    /// Run toward `+x` (1) or `-x` (-1).
    Move(f64),
    /// Stand and meditate (qi back faster).
    Meditate,
    /// Start a cast of `slot`; a movement skill goes toward the foe (`dir` 1) or away (-1).
    Cast { slot: usize, dir: f64 },
}

/// A blow on its way: who, from which skill, the blow itself and what the fight does around it.
#[derive(Clone, Debug)]
pub(crate) struct Payload {
    pub(crate) by: usize,
    pub(crate) slot: usize,
    pub(crate) hit: Hit,
    /// `Built::control`: the skill's own controls last this many times as long.
    pub(crate) control: f64,
    /// T the blow pushes the foe.
    pub(crate) knockback: f64,
    pub(crate) depth: u8,
    /// Struck by a unit (a totem's cast, a trap, a mine, a minion's blow): no striker's cultivator, no gains, no
    /// no striker events.
    pub(crate) from_unit: Option<u32>,
}

/// What a blow lands on: a fighter, or a unit by its `Unit::uid`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tgt {
    Fighter(usize),
    Unit(u32),
}

/// A blow due at a time, checked by its delivery when it comes.
#[derive(Clone, Debug)]
pub(crate) struct Emission {
    pub(crate) at: f64,
    pub(crate) delivery: Delivery,
    /// Where it comes from (`None`: where its caster stands when it comes).
    pub(crate) origin: Option<f64>,
    /// What it must land on, if anything (a sprung trap's springer, a minion's mark).
    pub(crate) target: Option<Tgt>,
    /// Where an area lands; the reach of a blow, the radius of an area or a nova; a projectile's speed and powers.
    pub(crate) aim: f64,
    pub(crate) reach: f64,
    pub(crate) radius: f64,
    pub(crate) speed: f64,
    pub(crate) powers: Powers,
    /// A dash's first blow carries the caster.
    pub(crate) first: bool,
    pub(crate) payload: Payload,
}

/// A projectile in flight (`arena_calc::projectile::Flight`), and the blow it carries.
#[derive(Clone, Debug)]
pub(crate) struct Shot {
    pub(crate) flight: Flight,
    pub(crate) payload: Payload,
}

/// How a round ended.
#[derive(Clone, Debug, PartialEq)]
pub struct RoundResult {
    /// The seat that won, `None` for a draw (both down in one tick, or equal shares at the time limit).
    pub winner: Option<usize>,
    /// Ended by the time limit (the higher share of life won).
    pub cut: bool,
    pub seconds: f64,
    /// Each seat's share of full life at the end.
    pub life: [f64; 2],
}

/// How a match ended, and what it measured.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchResult {
    /// The seat that took `ROUNDS_TO_WIN` rounds, `None` for a drawn match.
    pub winner: Option<usize>,
    pub rounds: Vec<RoundResult>,
    pub sides: [SideReport; 2],
    /// The fight's log, when it was asked for.
    pub log: Vec<String>,
}

/// Plays a best-of-3 match of `a` (seat 0, on the left) against `b` (seat 1) from `seed`. The same three give the
/// same match, bit for bit.
pub fn play_match(seed: u64, a: &Loadout, b: &Loadout) -> MatchResult {
    Fight::new(seed, a, b, false).play()
}

/// A match being played.
#[derive(Clone, Debug)]
pub struct Fight {
    /// Each seat's damage-system state, as one slice for the event queue.
    pub c: [Cultivator; 2],
    pub f: [Fighter; 2],
    pub field: domain::Field,
    /// Traps, mines, totems and minions standing, both sides', in the order made.
    pub units: Vec<Unit>,
    /// Each (owner, skill)'s placed things (`device::Placed`: caps, ages, arming, springing, detonating).
    pub(crate) groups: Vec<Group>,
    pub(crate) next_uid: u32,
    pub(crate) queue: Queue,
    pub(crate) pending: Vec<Emission>,
    pub(crate) shots: Vec<Shot>,
    pub(crate) rng: Rng,
    brains: [Rng; 2],
    seed: u64,
    pub tick: u64,
    /// Seconds into the round.
    pub time: f64,
    next_estimate: [f64; 2],
    /// The log, when asked for.
    pub log: Option<Vec<String>>,
}

/// Both of a pair, `me`'s first.
pub(crate) fn two<T>(a: &mut [T; 2], me: usize) -> (&mut T, &mut T) {
    let (l, r) = a.split_at_mut(1);
    if me == 0 {
        (&mut l[0], &mut r[0])
    } else {
        (&mut r[0], &mut l[0])
    }
}

/// `amount` of a factor on a blow: a more on its hit and its damage over time.
pub(crate) fn scale_hit(hit: &mut Hit, factor: f64) {
    if (factor - 1.0).abs() > 1e-12 {
        hit.more.push(factor - 1.0);
        for d in &mut hit.dot {
            d.more *= factor;
        }
    }
}

/// The order blows of one tick resolve in: higher rank first (a restriction above every rank).
fn rank_key(g: Grade) -> u8 {
    match g {
        Grade::Ranked(r) => r as u8,
        Grade::Restriction => 6,
    }
}

pub(crate) const SEAT: [&str; 2] = ["A", "B"];

impl Fight {
    /// A match of `a` against `b` from `seed`, logging when `log`.
    pub fn new(seed: u64, a: &Loadout, b: &Loadout, log: bool) -> Fight {
        let (fa, ca) = Fighter::new(a, 0);
        let (fb, cb) = Fighter::new(b, 1);
        Fight {
            c: [ca, cb],
            f: [fa, fb],
            field: domain::Field::default(),
            units: Vec::new(),
            groups: Vec::new(),
            next_uid: 0,
            queue: Queue::new(),
            pending: Vec::new(),
            shots: Vec::new(),
            rng: Rng::salted(seed, 1),
            brains: [Rng::salted(seed, 2), Rng::salted(seed, 3)],
            seed,
            tick: 0,
            time: 0.0,
            next_estimate: [0.0, 0.0],
            log: log.then(Vec::new),
        }
    }

    pub(crate) fn note(&mut self, line: impl FnOnce(&Fight) -> String) {
        if self.log.is_some() {
            let text = format!("{:6.2} {}", self.time, line(self));
            if let Some(l) = &mut self.log {
                l.push(text);
            }
        }
    }

    /// The distance between the fighters, in T.
    pub fn distance(&self) -> f64 {
        (self.f[0].x - self.f[1].x).abs()
    }

    /// Plays rounds until a seat takes `ROUNDS_TO_WIN` (or `MAX_ROUNDS` pass).
    pub fn play(mut self) -> MatchResult {
        let mut rounds = Vec::new();
        let mut wins = [0u8; 2];
        while wins[0] < ROUNDS_TO_WIN
            && wins[1] < ROUNDS_TO_WIN
            && rounds.len() < MAX_ROUNDS as usize
        {
            let r = self.round();
            if let Some(w) = r.winner {
                wins[w] += 1;
            }
            rounds.push(r);
        }
        let winner = if wins[0] >= ROUNDS_TO_WIN {
            Some(0)
        } else if wins[1] >= ROUNDS_TO_WIN {
            Some(1)
        } else {
            None
        };
        let [a, b] = self.f;
        MatchResult {
            winner,
            rounds,
            sides: [a.report, b.report],
            log: self.log.unwrap_or_default(),
        }
    }

    /// Plays one round from fresh bars.
    pub fn round(&mut self) -> RoundResult {
        for seat in 0..2 {
            let (f, c) = Fighter::new(&self.f[seat].loadout, seat);
            let mut kept = std::mem::replace(&mut self.f[seat], f);
            kept.new_round(seat);
            self.f[seat] = kept;
            self.c[seat] = c;
        }
        self.field.new_round();
        self.queue = Queue::new();
        self.pending.clear();
        self.shots.clear();
        self.units.clear();
        self.groups.clear();
        self.time = 0.0;
        self.next_estimate = [0.0, 0.0];
        self.note(|_| "round begins".to_string());
        loop {
            self.step();
            let dead = [self.c[0].life <= 0.0, self.c[1].life <= 0.0];
            let share = |c: &Cultivator| (c.life / c.life_max.max(1.0)).max(0.0);
            let life = [share(&self.c[0]), share(&self.c[1])];
            let end = |winner, cut, time| RoundResult {
                winner,
                cut,
                seconds: time,
                life,
            };
            let r = match dead {
                [true, true] => Some(end(None, false, self.time)),
                [true, false] => Some(end(Some(1), false, self.time)),
                [false, true] => Some(end(Some(0), false, self.time)),
                [false, false] if self.time >= ROUND_TIME => {
                    let w = if life[0] > life[1] {
                        Some(0)
                    } else if life[1] > life[0] {
                        Some(1)
                    } else {
                        None
                    };
                    Some(end(w, true, self.time))
                }
                _ => None,
            };
            if let Some(r) = r {
                self.note(|_| {
                    format!(
                        "round ends: {} {} (life A {:.0}%, B {:.0}%)",
                        r.winner.map_or("draw", |w| SEAT[w]),
                        if r.cut { "on time" } else { "by knockout" },
                        r.life[0] * 100.0,
                        r.life[1] * 100.0
                    )
                });
                return r;
            }
        }
    }

    /// One tick, in the order of the module's doc.
    pub fn step(&mut self) {
        self.tick += 1;
        self.time += DT;
        // 1
        let mut orders = [Order::Wait; 2];
        for (me, o) in orders.iter_mut().enumerate() {
            if self.time >= self.next_estimate[me] {
                self.estimate_all(me);
                self.next_estimate[me] = self.time + ESTIMATE_EVERY;
            }
            let mut r = std::mem::replace(&mut self.brains[me], Rng::new(0));
            *o = brain::decide(self, me, &mut r);
            self.brains[me] = r;
        }
        for (me, o) in orders.iter().enumerate() {
            match *o {
                Order::Cast { slot, dir } => {
                    self.begin_cast(me, slot, dir);
                }
                Order::Meditate => self.c[me].meditating = self.f[me].free(),
                _ => self.c[me].meditating = false,
            }
        }
        for me in 0..2 {
            self.detonate(me);
        }
        // 2
        let pos = [self.f[0].x, self.f[1].x];
        self.field.tick(DT, pos);
        let moving = [self.f[0].moving, self.f[1].moving];
        {
            let [c0, c1] = &mut self.c;
            self.field.sync([c0, c1], pos, moving);
        }
        for me in 0..2 {
            let calm = self.f[me].has(Keystone::GreyHeart);
            let (a, b) = two(&mut self.c, me);
            let fm = &mut self.f[me];
            fm.auras.tick(DT, a);
            aura::sync(&fm.auras, &fm.sheet, a, pos[me], b, pos[1 - me], calm);
        }
        // 3
        let before = [self.f[0].x, self.f[1].x];
        for (me, o) in orders.iter().enumerate() {
            self.walk(me, *o, before[1 - me]);
        }
        // both ran at once: two bodies that met share the gap
        let gap = self.f[1].x - self.f[0].x;
        if gap.abs() < BODY && (before[1] - before[0]).abs() >= BODY {
            let mid = (self.f[0].x + self.f[1].x) / 2.0;
            let side = if before[1] >= before[0] { 1.0 } else { -1.0 };
            self.f[0].x = (mid - side * BODY / 2.0).clamp(WALL, FIELD - WALL);
            self.f[1].x = (mid + side * BODY / 2.0).clamp(WALL, FIELD - WALL);
        }
        // 4
        for me in 0..2 {
            self.clock(me);
        }
        self.tick_units();
        // 5, 6
        self.land_all();
        // 7-9
        self.wear();
        self.run_queue();
        // 10
        for me in 0..2 {
            self.spare(me);
        }
    }

    /// What a seat's lately counts are (`stat::Counts`, over `RECENT`).
    fn counts(&self, me: usize) -> Counts {
        let mut c = Counts::default();
        for &(t, k, h, q, kinds) in &self.f[me].history {
            if self.time - t <= RECENT {
                c.kills += k;
                c.hits += h;
                c.qi_spent += q;
                c.kinds |= kinds;
            }
        }
        c
    }

    /// The cast's situation for seat `me` against the other.
    fn situation(&self, me: usize, use_count: u32, travelled: f64) -> Situation<'_> {
        let foe = 1 - me;
        let curses = self.f[foe].curses.on.len().min(u8::MAX as usize) as u8;
        Situation {
            target: Some(&self.c[foe]),
            target_cursed: curses > 0,
            target_curses: curses,
            target_moving: self.f[foe].moving,
            use_count,
            counts: self.counts(me),
            distance: self.distance(),
            moving: self.f[me].moving,
            travelled,
            target_guarded: self.c[foe].recent.since(Happened::UsedGuardSkill) <= JUST_GUARDED,
            fury: self.f[me].fury,
            lives_used: self.f[me].lives_used,
            ..Situation::default()
        }
    }

    /// Seat `me`'s skill in `slot` built now (at `level` for a triggered cast).
    fn build(&self, me: usize, slot: usize, level: Option<u8>) -> Option<Built> {
        let f = &self.f[me];
        let s = f.slots.get(slot)?;
        let sit = self.situation(me, s.uses + 1, 0.0);
        let skill = match level {
            Some(l) => Skill::of(s.gem, l, self.c[me].might),
            None => s.skill.clone(),
        };
        Some(build_skill(
            &skill,
            &s.supports,
            &self.c[me],
            &f.sheet,
            &sit,
        ))
    }

    /// The damage a blow of `built` is expected to deal the foe now: its parts after steps 1 to 4, times its
    /// blows, crit and double damage on average, after the foe's armour and resistances, plus half its burn and
    /// bleed (only the strongest burn burns).
    pub fn expected(&self, me: usize, g: &gem::Gem, built: &Built, delivery: Delivery) -> f64 {
        let foe = &self.c[1 - me];
        let h = &built.hit;
        let parts = hit::scale(h, self.c[me].power());
        let mut total = 0.0;
        for e in Element::ALL {
            let p = parts[e.index()];
            if p <= 0.0 {
                continue;
            }
            let cut = if e == Element::Force {
                let a = foe.armour * (1.0 - h.armour_pen);
                1.0 - (a / (a + RES_K * p)).min(0.9)
            } else {
                1.0 - (foe.resist_of(e) - h.pen[e.index()]).max(-1.0)
            };
            total += p * cut;
        }
        let dot =
            parts[Element::Fire.index()] * arena_calc::dot::BURN_RATE * arena_calc::dot::BURN_TIME
                + if h.force_ailment == arena_calc::ailment::ForceAilment::Bleed {
                    parts[Element::Force.index()]
                        * arena_calc::dot::BLEED_RATE
                        * arena_calc::dot::BLEED_TIME
                } else {
                    0.0
                };
        total += 0.5 * dot;
        let crit = if h.instant {
            1.0
        } else {
            1.0 + h.crit.clamp(0.0, 1.0) * (h.crit_multi - 1.0).max(0.0)
        };
        let shots = if delivery == Delivery::Projectile {
            1.0 + f64::from(built.projectiles)
        } else {
            1.0
        };
        total * crit * (1.0 + h.double) * g.hits() * f64::from(built.strikes) * shots
    }

    /// Refreshes seat `me`'s estimates of every skill against the foe now.
    fn estimate_all(&mut self, me: usize) {
        for i in 0..self.f[me].slots.len() {
            let s = &self.f[me].slots[i];
            let (g, d) = (s.gem, s.delivery);
            let est = if d.strikes() {
                self.build(me, i, None)
                    .map_or(0.0, |b| self.expected(me, g, &b, d))
            } else {
                0.0
            };
            self.f[me].slots[i].estimate = est;
        }
    }

    /// Whether seat `me` can pay `costs` now.
    fn can_pay(c: &Cultivator, costs: &[(Resource, f64)]) -> bool {
        costs.iter().all(|&(r, v)| match r {
            Resource::Qi => c.qi >= v,
            Resource::Life => true,
            Resource::Lifespan => c.lifespan >= v,
            Resource::Intent => c.intent >= v,
            Resource::Souls => c.souls >= v,
            Resource::Sense => c.sense >= v,
            Resource::DeathQi => c.death_qi >= v,
            Resource::Star(s) => c.stars[s as usize] >= v,
        })
    }

    /// Whether seat `me` may start `slot` now (the brain reads this): free, a use stored, a card left, its costs
    /// payable with what it built last, a spell not while silenced, nothing barring it, an intent not already on, never a triggered skill, a skill that binds not while its last blow plays out.
    pub fn can_cast(&self, me: usize, slot: usize, costs: &[(Resource, f64)]) -> bool {
        let f = &self.f[me];
        let c = &self.c[me];
        let Some(s) = f.slots.get(slot) else {
            return false;
        };
        f.free()
            && !s.triggered
            && s.charges.ready()
            && s.card != Some(0)
            && !(Fighter::is_spell(s.tags) && c.soul_lost > 0.0)
            && !f.curses.barred(s.tags)
            && !(s.delivery == Delivery::Aura
                && gem::aura_of(s.gem).is_some_and(|k| f.auras.on.iter().any(|a| a.kind == k)))
            && Self::can_pay(c, costs)
            && !(s.gem.binds() && self.bound(me))
    }

    /// Whether a blow of `p` holds its caster until it plays out: cast by hand (not triggered, not a unit's) from a
    /// skill that binds (`Gem::binds`).
    fn binds(&self, p: &Payload) -> bool {
        p.depth == 0
            && p.from_unit.is_none()
            && self.f[p.by].slots.get(p.slot).is_some_and(|s| s.gem.binds())
    }

    /// Whether a blow seat `me` cast still plays out: a projectile in flight, a blow still due.
    pub fn bound(&self, me: usize) -> bool {
        self.shots.iter().any(|s| s.payload.by == me && self.binds(&s.payload))
            || self.pending.iter().any(|e| e.payload.by == me && self.binds(&e.payload))
    }

    /// The costs of `slot` as built now.
    pub fn costs(&self, me: usize, slot: usize) -> Vec<(Resource, f64)> {
        self.f[me]
            .slots
            .get(slot)
            .map(|s| {
                s.skill
                    .costs
                    .iter()
                    .map(|&(r, v)| (r, v * self.cost_factor(me, slot)))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn cost_factor(&self, me: usize, slot: usize) -> f64 {
        self.f[me].slots.get(slot).map_or(1.0, |s| {
            s.supports
                .iter()
                .filter(|x| x.fits(s.skill.all_tags()))
                .map(|x| x.cost)
                .product()
        })
    }

    /// Starts a cast: builds it, pays, spends a use, and gathers. Returns whether it started.
    /// Starts a cast of seat `me`'s `slot` (a movement skill toward the foe for `dir` 1): built, paid, a use spent,
    /// gathering. Returns whether it started (what a brain's `Order::Cast` does; a test may call it).
    pub fn begin_cast(&mut self, me: usize, slot: usize, dir: f64) -> bool {
        let Some(built) = self.build(me, slot, None) else {
            return false;
        };
        if !self.can_cast(me, slot, &built.costs) {
            return false;
        }
        let foe = 1 - me;
        let mut qi_paid = 0.0;
        {
            let c = &mut self.c[me];
            for &(r, v) in &built.costs {
                match r {
                    Resource::Qi => {
                        c.qi -= v;
                        qi_paid += v;
                    }
                    Resource::Life => {}
                    Resource::Lifespan => c.lifespan -= v,
                    Resource::Intent => c.intent -= v,
                    Resource::Souls => c.souls -= v,
                    Resource::Sense => c.sense -= v,
                    Resource::DeathQi => c.death_qi -= v,
                    Resource::Star(s) => c.stars[s as usize] = (c.stars[s as usize] - v).max(0.0),
                }
            }
            c.pay_life(built.life);
            c.on_cast();
            c.meditating = false;
        }
        let tags = built.hit.tags;
        let d = self.distance();
        let now = self.time;
        let aim = self.f[foe].x;
        let f = &mut self.f[me];
        let s = &mut f.slots[slot];
        let fresh = charge_of(&built);
        s.charges.max = fresh.max;
        s.charges.cooldown = fresh.cooldown;
        s.charges.stored = s.charges.stored.min(s.charges.max);
        s.charges.try_use();
        s.uses += 1;
        if let Some(n) = &mut s.card {
            *n = n.saturating_sub(1);
        }
        let gem = s.gem;
        let rejection = s.supports.iter().find_map(|x| {
            x.behaviours.iter().find_map(|b| match b {
                Behaviour::Rejection { chance, share } => Some((*chance, *share)),
                _ => None,
            })
        });
        let estimate = s.estimate;
        f.history.push((now, 0, 0, qi_paid, 0));
        f.report.skills[slot].casts += 1;
        f.report.skills[slot].dist_cast += d;
        let stall: f64 = f
            .curses
            .on_cast(tags, false)
            .iter()
            .map(|e| match e {
                curse::Event::Stall(s) => *s,
                _ => 0.0,
            })
            .sum();
        f.act = Act::Windup {
            slot,
            left: built.windup + stall,
            gone: 0.0,
            aim,
            dir,
        };
        if let Some((chance, share)) = rejection
            && self.rng.next_f64() < chance
        {
            self.c[me].hurt(share * estimate);
        }
        self.queue
            .push(Event::new(EventKind::Cast, me).with_skill(gem.id, tags));
        self.note(|_| format!("{} casts {} at {:.1} T", SEAT[me], gem.id, d));
        true
    }

    /// Step 3: runs as ordered, held by the walls and the foe's body.
    fn walk(&mut self, me: usize, o: Order, foe_x: f64) {
        let f = &self.f[me];
        let Order::Move(dir) = o else {
            self.f[me].moving = false;
            return;
        };
        if !f.free() || f.root > 0.0 || dir == 0.0 {
            self.f[me].moving = false;
            return;
        }
        let speed = motion::run_speed(RUN_SPEED, &self.c[me].bearer_table(&f.sheet, true), false);
        let mut x = f.x + dir.signum() * speed * DT;
        if (foe_x - f.x).signum() == dir.signum() {
            x = if dir > 0.0 {
                x.min(foe_x - BODY)
            } else {
                x.max(foe_x + BODY)
            };
        }
        let x = x.clamp(WALL, FIELD - WALL);
        self.f[me].moving = (x - f.x).abs() > 1e-9;
        self.f[me].x = x;
    }

    /// Step 4: timers, and a windup that ends.
    fn clock(&mut self, me: usize) {
        let choke = if self.c[me].choke > 0.0 {
            AIL.choke_cooldown
        } else {
            1.0
        };
        let f = &mut self.f[me];
        f.stun = (f.stun - DT).max(0.0);
        f.root = (f.root - DT).max(0.0);
        f.ward_left = (f.ward_left - DT).max(0.0);
        if f.ward_left <= 0.0 {
            f.ward = 0;
        }
        for s in &mut f.slots {
            s.charges.tick(DT * choke);
        }
        let now = self.time;
        f.history.retain(|h| now - h.0 <= RECENT);
        match f.act {
            Act::Windup {
                slot,
                left,
                gone,
                aim,
                dir,
            } => {
                if left - DT <= 0.0 {
                    self.release(me, slot, aim, dir);
                } else {
                    f.act = Act::Windup {
                        slot,
                        left: left - DT,
                        gone: gone + DT,
                        aim,
                        dir,
                    };
                }
            }
            Act::Recover { left } => {
                f.act = if left - DT <= 0.0 {
                    Act::Idle
                } else {
                    Act::Recover { left: left - DT }
                };
            }
            Act::Idle => {}
        }
    }

    /// A windup ended: the skill, built again now, does what it does; the fighter recovers.
    fn release(&mut self, me: usize, slot: usize, aim: f64, dir: f64) {
        let Some(built) = self.build(me, slot, None) else {
            self.f[me].act = Act::Idle;
            return;
        };
        self.f[me].act = Act::Recover {
            left: built.recover,
        };
        self.deliver(me, slot, built, aim, dir, 0);
    }

    /// What a cast does when it comes out (a hand cast after its windup, a triggered one at once).
    fn deliver(&mut self, me: usize, slot: usize, built: Built, aim: f64, dir: f64, depth: u8) {
        let foe = 1 - me;
        let s = &self.f[me].slots[slot];
        let (g, delivery) = (s.gem, s.delivery);
        let d = self.distance();
        let toward = (self.f[foe].x - self.f[me].x).signum();
        let toward = if toward == 0.0 { 1.0 } else { toward };
        let lasting = |base: f64| play::duration(base, &built.table);
        match delivery {
            Delivery::Movement => {
                let len = motion::dash_reach_of(&self.c[me], BLINK_LENGTH);
                let x = self.f[me].x;
                let to = if dir >= 0.0 {
                    x + toward * len.min((d - BODY).max(0.0))
                } else {
                    x - toward * len
                };
                let to = to.clamp(WALL, FIELD - WALL);
                self.f[me].x = to;
                self.queue.push(Event::new(EventKind::Blinked, me));
                self.note(|_| format!("{} moves to {:.1}", SEAT[me], to));
            }
            Delivery::Guard => {
                self.f[me].ward = WARD_CHARGES;
                self.f[me].ward_left = lasting(WARD_SECONDS);
            }
            Delivery::Heal => {
                let share: f64 = g
                    .effects
                    .iter()
                    .map(|e| match e {
                        Effect::Heal(h) => *h,
                        _ => 0.0,
                    })
                    .sum();
                let c = &mut self.c[me];
                let healed = c.heal(share * c.life_max);
                self.note(|_| format!("{} heals {:.0}", SEAT[me], healed));
            }
            Delivery::Aura => {
                if let Some(k) = gem::aura_of(g) {
                    let (f, c) = (&mut self.f[me], &mut self.c[me]);
                    let _ = f
                        .auras
                        .light_with(k, built.rank, true, c, built.reservation);
                }
            }
            Delivery::Domain { damaging } => {
                if let Some(k) = gem::domain_of(g) {
                    let pos = [self.f[0].x, self.f[1].x];
                    let centre = if domain::can_raise(k, pos[me], FIELD / 2.0) {
                        pos[me]
                    } else {
                        FIELD / 2.0
                    };
                    let _ = self
                        .field
                        .raise(me, k, built.rank, centre, pos, FIELD / 2.0);
                }
                if damaging {
                    self.emit(me, slot, &built, delivery, self.f[foe].x, depth);
                }
            }
            Delivery::Device(_) => {
                self.place(me, slot, &built, aim);
            }
            Delivery::Curse => {
                if d <= s.range {
                    self.lay_curse(me, slot, &built, depth);
                }
            }
            Delivery::Buff => {
                let src = Source::Skill(g.id);
                let m = if built.hit.tags.has(Tag::Stance) {
                    Mod::more(Stat::Damage, STANCE_MORE, src)
                } else {
                    Mod::more(Stat::Taken, BLESSING_TAKEN, src)
                };
                self.c[me].put_on(Active {
                    source: src,
                    mods: vec![m],
                    left: Some(lasting(BUFF_SECONDS)),
                });
            }
            _ => {
                if gem::curse_of(g).is_some() && d <= s.range {
                    self.lay_curse(me, slot, &built, depth);
                }
                self.emit(me, slot, &built, delivery, aim, depth);
            }
        }
    }

    /// Schedules a striking cast's blows: its `Hits`, times its repeats, times its projectiles, `PULSE` apart; a
    /// fraction of a blow lands last at that fraction.
    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        me: usize,
        slot: usize,
        built: &Built,
        delivery: Delivery,
        aim: f64,
        depth: u8,
    ) {
        let g = self.f[me].slots[slot].gem;
        // extra projectiles fan out in `projectile::Flight::launch`
        let total = g.hits() * f64::from(built.strikes);
        let count = libm::ceil(total - 1e-9).max(1.0) as usize;
        let frac = total - (count as f64 - 1.0);
        let wait = match delivery {
            Delivery::Area { delayed: true } => DELAYED,
            _ => 0.0,
        };
        let (reach, radius) = match delivery {
            Delivery::Nova => (0.0, play::radius(NOVA_RADIUS, &built.table)),
            Delivery::Area { .. } => (0.0, play::radius(AREA_RADIUS, &built.table)),
            Delivery::Domain { .. } => (0.0, FIELD),
            _ => (self.f[me].slots[slot].range, 0.0),
        };
        // `Powers::speed` carries the caster's ProjectileSpeed
        let speed = PROJECTILE_SPEED;
        let knockback = motion::knockback_distance(0.0, &built.table);
        for i in 0..count {
            let mut hit = built.hit.clone();
            if i + 1 == count && frac < 1.0 - 1e-9 {
                scale_hit(&mut hit, frac.max(0.0));
            }
            self.pending.push(Emission {
                at: self.time + wait + i as f64 * PULSE,
                delivery,
                origin: None,
                target: None,
                aim,
                reach,
                radius,
                speed,
                powers: Powers::of_built(built),
                first: i == 0,
                payload: Payload {
                    by: me,
                    slot,
                    hit,
                    control: built.control,
                    knockback,
                    depth,
                    from_unit: None,
                },
            });
        }
    }

    /// Lays the curse of `slot`'s skill on the foe.
    fn lay_curse(&mut self, me: usize, slot: usize, built: &Built, depth: u8) {
        let g = self.f[me].slots[slot].gem;
        let Some(kind) = gem::curse_of(g) else {
            return;
        };
        let foe = 1 - me;
        let witch = self.f[me].has(Keystone::Witch);
        let cap = curse::curse_limit(&self.c[me], &self.f[me].sheet, witch);
        let effect = curse::curse_effect(&self.c[me], &self.f[me].sheet);
        let curse = Curse::new(kind, built.rank, me as u32, foe as u32);
        let (mine, theirs) = two(&mut self.f, me);
        theirs.curses.cap = cap;
        let lay = curse::cast_with(curse, &mut theirs.curses, &mut mine.curses, effect);
        if lay != Lay::Failed {
            theirs.curses.sync(&mut self.c[foe], false);
            mine.curse_slots.retain(|k| k.0 != kind.index());
            mine.curse_slots.push((kind.index(), slot));
            self.queue
                .extend(event::curse_events(me, foe, g.id, built.hit.tags, depth));
            self.note(|_| format!("{} curses {} with {}", SEAT[me], SEAT[foe], g.id));
        }
    }

    /// Steps 5 and 6: projectiles fly, due blows find what they strike, landing blows resolve, triggers cast.
    fn land_all(&mut self) {
        for pass in 0..LAND_PASSES {
            let mut landing: Vec<(Payload, Tgt)> = Vec::new();
            let mut missed: Vec<(usize, usize)> = Vec::new();
            if pass == 0 {
                let shots = std::mem::take(&mut self.shots);
                for mut s in shots {
                    let ens = self.enemies(s.payload.by, true);
                    let targets: Vec<Target> = ens.iter().map(|e| e.1).collect();
                    let st = s.flight.step(DT, &targets);
                    for k in &st.strikes {
                        if let Some(&(t, _)) = ens.iter().find(|e| e.1.id == k.target) {
                            let mut p = s.payload.clone();
                            p.hit = k.blow(&p.hit);
                            landing.push((p, t));
                        }
                    }
                    for fk in st.forks {
                        self.shots.push(Shot {
                            flight: fk,
                            payload: s.payload.clone(),
                        });
                    }
                    if !s.flight.done {
                        self.shots.push(s);
                    } else if s.flight.struck.is_empty() {
                        missed.push((s.payload.by, s.payload.slot));
                    }
                }
            }
            let now = self.time + 1e-9;
            let (due, wait): (Vec<Emission>, Vec<Emission>) = std::mem::take(&mut self.pending)
                .into_iter()
                .partition(|e| e.at <= now);
            self.pending = wait;
            for e in due {
                let me = e.payload.by;
                let foe = 1 - me;
                let x = e.origin.unwrap_or(self.f[me].x);
                let tx = self.f[foe].x;
                let dir = if tx >= x { 1.0 } else { -1.0 };
                let ens = self.enemies(me, true);
                let before = landing.len();
                if let Some(t) = e.target {
                    if ens.iter().any(|en| en.0 == t && en.1.alive) {
                        landing.push((e.payload.clone(), t));
                    }
                } else {
                    match e.delivery {
                        Delivery::Melee | Delivery::Targeted => {
                            if let Some(t) = self.in_front(me, x, e.reach) {
                                landing.push((e.payload.clone(), t));
                            }
                        }
                        Delivery::Nova
                        | Delivery::Area { .. }
                        | Delivery::Domain { .. }
                        | Delivery::Device(_) => {
                            let centre = if e.delivery == Delivery::Nova {
                                x
                            } else {
                                e.aim
                            };
                            let shape = Shape::Circle {
                                centre: Point::new(centre, FIGHTER_Y),
                                radius: e.radius,
                            };
                            for en in ens.iter().filter(|en| shape.holds(&en.1)) {
                                landing.push((e.payload.clone(), en.0));
                            }
                        }
                        Delivery::Instant => landing.push((e.payload.clone(), Tgt::Fighter(foe))),
                        Delivery::Dash if e.first => {
                            let d = (x - tx).abs();
                            let len = motion::dash_reach_of(&self.c[me], DASH_LENGTH.max(e.reach));
                            if d <= len {
                                let through = tx + dir * BODY * 1.5;
                                self.f[me].x = if (WALL..=FIELD - WALL).contains(&through) {
                                    through
                                } else {
                                    tx - dir * BODY
                                };
                                landing.push((e.payload.clone(), Tgt::Fighter(foe)));
                            } else {
                                self.f[me].x = (x + dir * len).clamp(WALL, FIELD - WALL);
                            }
                        }
                        Delivery::Dash => {
                            if let Some(t) = self.in_front(me, self.f[me].x, REACH_NEAR * 1.5) {
                                landing.push((e.payload.clone(), t));
                            }
                        }
                        Delivery::Projectile => {
                            let shot = ProjectileShot {
                                origin: Point::new(x, FIGHTER_Y),
                                direction: Point::new(dir, 0.0),
                                speed: e.speed,
                                range: e.reach.max(2.0),
                            };
                            for flight in Flight::launch(&shot, &e.powers) {
                                self.shots.push(Shot {
                                    flight,
                                    payload: e.payload.clone(),
                                });
                            }
                            continue;
                        }
                        _ => {}
                    }
                }
                if landing.len() == before {
                    missed.push((me, e.payload.slot));
                }
            }
            for (me, slot) in missed {
                if let Some(l) = self.f[me].report.skills.get_mut(slot) {
                    l.missed += 1;
                }
            }
            if landing.is_empty() && self.queue.pending().is_empty() {
                break;
            }
            let first = ((self.seed.wrapping_add(self.tick)) % 2) as usize;
            landing.sort_by_key(|(p, _)| {
                (
                    std::cmp::Reverse(rank_key(p.hit.grade)),
                    !p.hit.instant,
                    usize::from(p.by != first),
                )
            });
            for (p, t) in landing {
                match t {
                    Tgt::Fighter(_) => self.strike(p),
                    Tgt::Unit(uid) => self.strike_unit(p, uid),
                }
            }
            self.run_queue();
        }
    }

    /// Step 6 for one blow.
    fn strike(&mut self, p: Payload) {
        let by = p.by;
        let on = 1 - by;
        let gem = self.f[by].slots[p.slot].gem;
        let d = self.distance();
        let unit = p.from_unit.is_some();
        if !unit && self.c[by].life <= 0.0 {
            self.f[by].report.skills[p.slot].missed += 1;
            return;
        }
        if self.f[on].ward > 0 {
            self.f[on].ward -= 1;
            self.f[by].report.skills[p.slot].missed += 1;
            self.queue
                .push(Event::new(EventKind::Blocked, on).against(by));
            self.note(|_| format!("{}'s ward stops {}", SEAT[on], gem.id));
            return;
        }
        let mut hit = p.hit;
        hit.frame = match self.f[on].act {
            Act::Windup { .. } => Frame::Counter,
            Act::Recover { .. } => Frame::Punish,
            Act::Idle => Frame::Plain,
        };
        let mut qi = 0.0;
        for k in &self.f[on].loadout.keystones {
            qi += k.on_taken(&mut hit);
        }
        {
            let c = &mut self.c[on];
            c.qi = (c.qi + qi).min(c.qi_max);
        }
        let blow = Blow {
            by: (!unit).then_some(by),
            on,
            skill: Some(gem.id),
            tags: hit.tags,
            depth: p.depth,
            life_before: self.c[on].life,
            shield_before: self.c[on].shield,
        };
        let rolls = Rolls {
            evade: self.rng.next_f64(),
            crit: self.rng.next_f64(),
            double: self.rng.next_f64(),
            star: self.rng.next_f64(),
        };
        let out = {
            let def = Defence {
                sheet: &self.f[on].sheet,
                distance: d,
                moving: self.f[on].moving,
                striker_cursed: !self.f[by].curses.on.is_empty(),
            };
            let (striker, target) = two(&mut self.c, by);
            resolve_with(&hit, (!unit).then_some(&*striker), target, &def, rolls)
        };
        self.queue
            .extend(event::events_of(&out, &blow, &self.c[on]));
        if out.evaded {
            self.f[by].report.skills[p.slot].missed += 1;
            self.note(|_| format!("{} dodges {}", SEAT[on], gem.id));
            return;
        }
        let line = &mut self.f[by].report.skills[p.slot];
        line.device_blows += u32::from(unit);
        // what the striker gets back (a unit's blow gives its owner nothing)
        if !unit {
            let c = &mut self.c[by];
            c.heal(out.gains.life + out.leech);
            c.qi = (c.qi + out.gains.qi).min(c.qi_max);
            c.sense = (c.sense + out.gains.sense).min(c.sense_max);
            c.gain_intent(out.gains.intent);
            c.gain_souls(out.gains.souls);
            for &s in &out.stars {
                c.add_star(s, RES.stars_max);
            }
            if out.recoil > 0.0 {
                c.hurt(out.recoil);
            }
        }
        // Nạp Hồn: soul damage dealt earns souls again (`RES.souls_per_damage` a soul), on top of the income every
        // fighter has (`Gains::souls`)
        let soul = out.dealt[Element::Soul.index()];
        if !unit
            && soul > 0.0
            && self.f[by].slots[p.slot]
                .supports
                .iter()
                .any(|x| x.behaviours.contains(&Behaviour::SoulsFromSoul))
        {
            self.c[by].gain_souls(soul / RES.souls_per_damage);
        }
        let f = &mut self.f[by];
        if !unit && f.has(Keystone::Fury) {
            f.fury = (f.fury + 1).min(arena_calc::keystone::FURY_STACKS);
        }
        let killed = blow.life_before > 0.0 && self.c[on].life <= 0.0;
        let mut kinds = 0u16;
        for e in Element::ALL {
            if out.dealt[e.index()] > 0.0 {
                kinds |= 1 << e.index();
            }
        }
        if !unit {
            f.history
                .push((self.time, u32::from(killed), 1, 0.0, kinds));
        }
        let dealt = out.to_shield + out.to_life + out.to_sense;
        let line = &mut f.report.skills[p.slot];
        line.landed += 1;
        line.damage += dealt;
        line.dist_hit += d;
        f.report.damage += dealt;
        for dt in &out.dots {
            match dt.ailment {
                Ailment::Burn => f.last_burn = Some(p.slot),
                Ailment::Bleed => f.last_bleed = Some(p.slot),
                _ => {}
            }
        }
        // controls: the skill's own, then a full ailment's
        for e in gem.effects {
            if let Effect::Control(ctl, secs) = e {
                let t = self.c[on].control(secs * p.control, false);
                self.control(on, *ctl, t);
                self.queue
                    .push(Event::new(EventKind::Controlled, by).against(on));
            }
        }
        for &(ctl, t) in &out.controls {
            self.control(on, ctl, t);
        }
        if out.total() > 0.0 {
            let fo = &mut self.f[on];
            if fo.in_windup() {
                fo.act = Act::Idle;
            }
            let extra = match hit.frame {
                Frame::Counter => COUNTER_STAGGER,
                Frame::Punish => PUNISH_STAGGER,
                Frame::Plain => 0.0,
            };
            fo.stun = fo.stun.max(HITSTUN + extra);
        }
        if p.knockback > 0.0 {
            let field = play::Field { width: FIELD };
            let x = motion::knockback(self.f[on].x, WALL, self.f[by].x, p.knockback, &field);
            self.f[on].x = x;
        }
        self.note(|fight| {
            let kinds: Vec<String> = Element::ALL
                .iter()
                .filter(|e| out.dealt[e.index()] > 0.0)
                .map(|e| format!("{} {:.0}", e.name(), out.dealt[e.index()]))
                .collect();
            let ails: Vec<String> = Ailment::ALL
                .iter()
                .filter(|a| out.stacks[a.index()] > 0.0)
                .map(|a| format!("{}+{:.1}", a.name(), out.stacks[a.index()]))
                .collect();
            format!(
                "{} hits {} with {}: {:.0} ({}){}{}{} | {} life {:.0}",
                SEAT[by],
                SEAT[on],
                gem.id,
                dealt,
                kinds.join(", "),
                if out.crit { " crit" } else { "" },
                match hit.frame {
                    Frame::Counter => " counter",
                    Frame::Punish => " punish",
                    Frame::Plain => "",
                },
                if ails.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", ails.join(" "))
                },
                SEAT[on],
                fight.c[on].life
            )
        });
    }

    /// A hard control lands on seat `on` for `t` seconds.
    fn control(&mut self, on: usize, ctl: Control, t: f64) {
        let f = &mut self.f[on];
        match ctl {
            Control::Stun | Control::Knockdown | Control::Hold | Control::Jolt => {
                f.stun = f.stun.max(t);
                if f.in_windup() {
                    f.act = Act::Idle;
                }
            }
            Control::Root => f.root = f.root.max(t),
            Control::Soulless => {
                let c = &mut self.c[on];
                c.soul_lost = c.soul_lost.max(t);
            }
        }
        self.note(|_| format!("{} {:?} {:.2}s", SEAT[on], ctl, t));
    }

    /// Runs the event queue and delivers the casts its triggers return.
    fn run_queue(&mut self) {
        if self.queue.pending().is_empty() {
            return;
        }
        let rolls: Vec<f64> = (0..QUEUE_ROLLS).map(|_| self.rng.next_f64()).collect();
        let d = self.distance();
        let out = {
            let seats = [0, 1].map(|i| Seat {
                triggers: &self.f[i].triggers,
                cursed: !self.f[i].curses.on.is_empty(),
                moving: self.f[i].moving,
                distance: d,
            });
            self.queue.run(&mut self.c, &seats, &mut Dice::new(&rolls))
        };
        for cast in out.casts {
            self.trigger_cast(cast);
        }
    }

    /// A cast a trigger returned: built at its level and factor, delivered at once at the foe.
    fn trigger_cast(&mut self, cast: Cast) {
        let me = cast.by.min(1);
        // a linked attack the fighter does not carry (Liên Hoàn Suy Yếu's) joins its skills, unsupported, unused
        // by hand
        let slot = match self.f[me].slots.iter().position(|s| s.gem.id == cast.skill) {
            Some(i) => i,
            None => match self.f[me].link(cast.skill, self.c[me].might) {
                Some(i) => i,
                None => return,
            },
        };
        let Some(mut built) = self.build(me, slot, Some(cast.level)) else {
            return;
        };
        if let Some(base) = cast.base {
            let split = &self.f[me].slots[slot].skill.split;
            built.hit.base = Hit::split(base, split).base;
        }
        scale_hit(&mut built.hit, cast.factor);
        built.hit.tags = built.hit.tags.with(Tag::Trigger);
        let g = self.f[me].slots[slot].gem;
        self.f[me].report.skills[slot].triggered += 1;
        self.queue.push(
            Event::new(EventKind::Cast, me)
                .with_skill(g.id, built.hit.tags)
                .at_depth(cast.depth),
        );
        self.note(|_| format!("{}'s {} triggers {}", SEAT[me], cast.trigger, g.id));
        let aim = self.f[1 - me].x;
        self.deliver(me, slot, built, aim, 1.0, cast.depth);
    }

    /// Steps 7 to 9: curses beat, then time passes for each cultivator (damage over time, regen, timers).
    fn wear(&mut self) {
        self.queue.tick(DT);
        self.wear_units();
        for me in 0..2 {
            let foe = 1 - me;
            let evs = self.f[me].curses.tick(DT, &mut self.c[me], false);
            for e in evs {
                self.curse_event(me, e);
            }
            let ticked = self.c[me].tick_with(DT, self.f[me].moving, &self.f[me].sheet);
            let events = event::dot_events(&ticked, me, &self.c[me]);
            for e in events {
                self.queue.push(e.against(foe));
            }
            for t in &ticked.dots {
                let src = match t.ailment {
                    Ailment::Burn => self.f[foe].last_burn,
                    _ => self.f[foe].last_bleed,
                };
                let r = &mut self.f[foe].report;
                r.damage += t.total();
                r.dot += t.total();
                if let Some(l) = src.and_then(|s| r.skills.get_mut(s)) {
                    l.damage += t.total();
                }
            }
        }
    }

    /// A curse on seat `me` did something.
    fn curse_event(&mut self, me: usize, e: curse::Event) {
        match e {
            curse::Event::Strike {
                kind,
                element,
                share,
            } => {
                let h = curse::strike_hit(element, share, self.c[me].life_max);
                let out = resolve(&h, None, &mut self.c[me], Rolls::NONE);
                let dealt = out.to_shield + out.to_life + out.to_sense;
                let foe = 1 - me;
                let src = self.f[foe]
                    .curse_slots
                    .iter()
                    .find(|k| k.0 == kind.index())
                    .map(|k| k.1);
                let r = &mut self.f[foe].report;
                r.damage += dealt;
                if let Some(l) = src.and_then(|s| r.skills.get_mut(s)) {
                    l.damage += dealt;
                }
            }
            curse::Event::Backfire {
                by, element, share, ..
            } => {
                let by = (by as usize).min(1);
                let h = curse::strike_hit(element, share, self.c[by].life_max);
                resolve(&h, None, &mut self.c[by], Rolls::NONE);
            }
            curse::Event::Control {
                control, seconds, ..
            } => {
                let t = self.c[me].control(seconds, false);
                self.control(me, control, t);
            }
            curse::Event::Stall(s) => {
                if let Act::Windup { left, .. } = &mut self.f[me].act {
                    *left += s;
                }
            }
            _ => {}
        }
    }

    /// Step 10: a fighter at 0 life uses a spare life if its keystones give one (Bất Khuất once a round, Tam Mệnh
    /// and Ngọn Đèn Hồn their lives a match).
    fn spare(&mut self, me: usize) {
        if self.c[me].life > 0.0 {
            return;
        }
        let ks = self.f[me].loadout.keystones.clone();
        for k in ks {
            let used = match k {
                Keystone::Unyielding => u8::from(self.f[me].unyielding_used),
                Keystone::ThreeLives | Keystone::SoulLamps => self.f[me].lives_used,
                _ => continue,
            };
            if k.on_death(&mut self.c[me], used) {
                if k == Keystone::Unyielding {
                    self.f[me].unyielding_used = true;
                } else {
                    self.f[me].lives_used += 1;
                }
                self.note(|_| format!("{} rises ({:?})", SEAT[me], k));
                return;
            }
        }
        self.note(|_| format!("{} falls", SEAT[me]));
    }

    /// Whether seat `me` stands in its windup's last stretch, so a blow now would counter it (the brain reads it).
    pub fn foe_winding(&self, me: usize) -> bool {
        self.f[1 - me].in_windup()
    }
}

/// Armour's constant (`cultivator::BASE.armour_k`), for the brain's estimate.
const RES_K: f64 = arena_calc::cultivator::BASE.armour_k;

/// A built cast's cooldown uses.
fn charge_of(b: &Built) -> arena_calc::charge::Uses {
    arena_calc::charge::Uses::of(b)
}

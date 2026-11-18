//! Blows through the damage system (`arena-calc`, design 5 and 17), as `arena-sim` calls it: a skill is built for
//! the situation (`build::build_skill`), each blow that lands resolves (`hit::resolve_with`) with rolls from the
//! rules' dice in the fixed order evade, crit, double, star, and the striker takes what it gains. The blow's events
//! go into the queue (`event::Queue`), whose triggers cast skills the duel delivers at once. Hard controls land on
//! the body; time wears the fighters each tick (`Cultivator::tick_with`: burns, bleeds, regen, timers).

use crate::act::{break_act, clash, set_meditating};
use crate::body::Blow as BodyBlow;
use crate::data::{FRAMES, Kind, POWER, Spell, T, body_spec};
use crate::fighter::{Phase, State, Told};
use crate::out::{How, Note, Out, Stop};
use arena_calc::ailment::{Ailment, Control};
use arena_calc::build::{Built, Situation, build_skill};
use arena_calc::cultivator::{Cultivator, RES};
use arena_calc::curse::{self, Curse, Lay};
use arena_calc::element::Element;
use arena_calc::event::{self, Blow, Cast, Dice, Event, EventKind, Seat};
use arena_calc::gem::{self, Effect, Resource};
use arena_calc::hit::{Defence, Frame, Hit, Rolls, resolve, resolve_with};
use arena_calc::keystone::{self, Keystone};
use arena_calc::motion;
use arena_calc::play;
use arena_calc::projectile::Powers;
use arena_calc::stat::{Happened, Mod, Source, Stat};
use arena_calc::support::Behaviour;
use arena_calc::tag::Tag;
use arena_mind::Outcome;

/// Rolls handed to the event queue each time it runs (its chance checks take them in order).
pub const QUEUE_ROLLS: usize = 16;
/// Times the queue runs and the casts its triggers return are delivered, at most, in one go.
pub const QUEUE_PASSES: usize = 8;
/// Seconds after a guard skill during which the foe "just guarded" (Nhử Đòn).
pub const JUST_GUARDED: f64 = 1.0;
/// Seconds between the blows of a skill that lands more than once (`gem::Effect::Hits`, supports' repeats).
pub const PULSE: f64 = 0.15;
/// A stance's damage more and a blessing's damage taken less, and how long either lasts before `SkillDuration`
/// (a body skill with no data of its own in the pool), as `arena-sim` has them.
pub const STANCE_MORE: f64 = 0.25;
pub const BLESSING_TAKEN: f64 = -0.2;
pub const BUFF_SECONDS: f64 = 8.0;
/// Damage over time is shown (`Out::Tick`) this often, what it took since; it is dealt every tick.
pub const DOT_SHOWN: f64 = 0.5;

/// A blow on its way: who, from which skill, the blow itself and what the fight does around it.
#[derive(Clone, Debug, PartialEq)]
pub struct Payload {
    /// The seat whose blow it is.
    pub by: usize,
    pub skill: &'static str,
    pub hit: Hit,
    /// `Built::control`: the skill's own controls last this many times as long.
    pub control: f64,
    /// T the blow pushes its target (`motion::knockback_distance`).
    pub knockback: f64,
    /// The trigger chain's depth: 0 for a cast by hand.
    pub depth: u8,
    /// Struck by a unit (a totem's cast, a trap, a mine, a minion's blow): no striker's cultivator, no gains.
    pub unit: Option<u32>,
    /// Its size in the old power (`data::POWER`s): what a guard's chance and the minds read.
    pub power: f64,
    /// A projectile's pierce, fork, chain, extra projectiles and speed (`projectile::Powers::of_built`).
    pub powers: Powers,
}

impl Payload {
    /// Whether it holds its caster until it plays out (`Fighter::bound`): a blow of a skill cast by hand that binds
    /// (`Gem::binds`); a unit's blow and a triggered one never do.
    pub fn binds(&self) -> bool {
        self.depth == 0
            && self.unit.is_none()
            && arena_calc::gem::gem(self.skill).is_some_and(|g| g.binds())
    }
}

/// What a blow lands on: a fighter, or a unit by its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Fighter(usize),
    Unit(u32),
}

/// Where a blow comes from: a fighter's body, or a unit standing at `x`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Origin {
    pub x: f64,
    pub air: f64,
    pub face: f64,
    pub unit: Option<u32>,
}

impl Origin {
    /// Fighter `me`'s body.
    pub fn of(state: &State, me: usize) -> Origin {
        let b = &state.fighters[me].body;
        Origin {
            x: b.x,
            air: b.air,
            face: b.face,
            unit: None,
        }
    }
}

/// A blow due later: one of a skill's repeats (its motion struck again from `origin`), or a stored force (Lưu Lực)
/// on `target`.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    /// Round time it is due.
    pub at: f64,
    pub payload: Payload,
    pub motion: &'static Spell,
    pub origin: Origin,
    /// Where an area aims.
    pub aim: Option<f64>,
    /// Lands on this, if it still stands (a stored force); `None`: the motion strikes again.
    pub target: Option<Target>,
    /// A fighter's missed blade trying again next tick: it strikes only if the blade's clip is still in its active
    /// window then (`frames::in_window`), and a broken act takes it away (`act::break_act`).
    pub retry: bool,
}

/// Both fighters' cultivators, `me`'s first.
pub fn two(state: &mut State, me: usize) -> (&mut Cultivator, &mut Cultivator) {
    let [a, b] = &mut state.fighters;
    if me == 0 {
        (&mut a.c, &mut b.c)
    } else {
        (&mut b.c, &mut a.c)
    }
}

/// `factor` on a blow: a more on its hit and on its damage over time.
pub fn scale_hit(hit: &mut Hit, factor: f64) {
    if (factor - 1.0).abs() > 1e-12 {
        hit.more.push(factor - 1.0);
        for d in &mut hit.dot {
            d.more *= factor;
        }
    }
}

/// The situation of a cast by seat `me` against the other.
pub fn situation(state: &State, me: usize, use_count: u32, travelled: f64) -> Situation<'_> {
    let f = &state.fighters[me];
    let foe = &state.fighters[1 - me];
    let curses = foe.curses.on.len().min(u8::MAX as usize) as u8;
    Situation {
        target: Some(&foe.c),
        target_cursed: curses > 0,
        target_curses: curses,
        target_moving: foe.moving,
        use_count,
        counts: f.counts(state.time),
        distance: state.distance_t(),
        moving: f.moving,
        travelled,
        target_guarded: foe.c.recent.since(Happened::UsedGuardSkill) <= JUST_GUARDED,
        fury: f.fury,
        lives_used: f.lives_used,
        ..Situation::default()
    }
}

/// Seat `me`'s kit skill `k` built now (at `level` for a triggered cast).
pub fn build(state: &State, me: usize, k: usize, level: Option<u8>) -> Option<Built> {
    let f = &state.fighters[me];
    let ks = f.kit.skills.get(k)?;
    let sit = situation(state, me, ks.count + 1, 0.0);
    let skill = match level {
        Some(l) => {
            let mut s = arena_calc::build::Skill::of(ks.gem, l, f.c.might);
            s.split.clone_from(&ks.skill.split);
            s
        }
        None => ks.skill.clone(),
    };
    Some(build_skill(&skill, &ks.supports, &f.c, &f.sheet, &sit))
}

/// Seat `me`'s every kit skill built as it would be cast now, kept for its mind (`KitSkill::seen`): at a round's
/// start, once both fighters are fresh. A cast keeps its own build (`act::begin`). Building reads the state and
/// changes nothing in the fight.
pub fn see(state: &mut State, me: usize) {
    for k in 0..state.fighters[me].kit.skills.len() {
        if let Some(b) = build(state, me, k, None) {
            state.fighters[me].kit.skills[k].see(&b);
        }
    }
}

/// The payload of seat `me`'s kit skill `k` as `built`, at chain `depth`.
pub fn payload(state: &State, me: usize, k: usize, built: &Built, depth: u8) -> Payload {
    let ks = &state.fighters[me].kit.skills[k];
    Payload {
        by: me,
        skill: ks.gem.id,
        hit: built.hit.clone(),
        control: built.control,
        knockback: motion::knockback_distance(0.0, &built.table),
        depth,
        unit: None,
        power: ks.motion.damage.unwrap_or(0.0),
        powers: Powers::of_built(built),
    }
}

/// Whether a cultivator can pay `costs` now.
pub fn can_pay(c: &Cultivator, costs: &[(Resource, f64)]) -> bool {
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

/// Pays `built`'s costs from `c`; returns the qi paid.
pub fn pay(c: &mut Cultivator, built: &Built) -> f64 {
    let mut qi = 0.0;
    for &(r, v) in &built.costs {
        match r {
            Resource::Qi => {
                c.qi -= v;
                qi += v;
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
    qi
}

/// Four rolls of the rules' dice, in the order a blow reads them: evade, crit, double, star.
pub fn rolls(state: &mut State) -> Rolls {
    Rolls {
        evade: state.rng.next(),
        crit: state.rng.next(),
        double: state.rng.next(),
        star: state.rng.next(),
    }
}

/// The share of full life `amount` is.
fn life_share(c: &Cultivator, amount: f64) -> f64 {
    if c.life_max > 0.0 {
        amount / c.life_max * 100.0
    } else {
        0.0
    }
}

/// Fighter `f` takes blow `p`, pushed toward `dir`: through a barrier, a ward's charge, a
/// dodge, then the damage system. Caught gathering a cast it is a COUNTER, caught recovering a PUNISH (never from a
/// field's strike): the blow says so (`Hit::frame`), and the body is staggered longer. Returns whether it landed.
pub fn hurt(state: &mut State, f: usize, p: &Payload, dir: f64) -> bool {
    if !state.fighters[f].alive() || state.fighters[f].body.untouchable() {
        return false;
    }
    let by = p.by;
    if crate::act::turns_aside(state, f, dir, p) {
        return false;
    }
    // a ward's charge erases the whole blow
    if state.fighters[f].charges > 0.0 {
        state.note(Note::Stopped {
            by,
            spell: p.skill,
            how: Stop::Ward,
            chip: 0.0,
        });
        state.fighters[f].charges -= 1.0;
        state
            .queue
            .push(Event::new(EventKind::Blocked, f).against(by));
        let me = &state.fighters[f];
        let y = state.ground - me.body.spec.chest - me.body.air;
        let x = me.body.x;
        clash(state, x, y, &[f]);
        return false;
    }
    let field_strike = crate::data::spell(p.skill).is_some_and(|s| s.kind == Kind::Void);
    // caught at a weak moment, decided from what it was doing before the blow breaks it
    let how = match state.fighters[f].act.map(|a| a.phase) {
        _ if field_strike => How::Plain,
        Some(Phase::Windup) => How::Counter,
        Some(Phase::Recover) => How::Punish,
        None => How::Plain,
    };
    let mut hit = p.hit.clone();
    hit.frame = match how {
        How::Plain => Frame::Plain,
        How::Counter => Frame::Counter,
        How::Punish => Frame::Punish,
    };
    let mut qi = 0.0;
    for k in state.fighters[f].loadout.keystones.clone() {
        qi += k.on_taken(&mut hit);
    }
    {
        let c = &mut state.fighters[f].c;
        c.qi = (c.qi + qi).min(c.qi_max);
    }
    let unit = p.unit.is_some();
    let striker_alive = state.fighters[by].alive();
    let blow = Blow {
        by: (!unit).then_some(by),
        on: f,
        skill: Some(p.skill),
        tags: hit.tags,
        depth: p.depth,
        life_before: state.fighters[f].c.life,
        shield_before: state.fighters[f].c.shield,
    };
    let r = rolls(state);
    let distance = state.distance_t();
    let out = {
        let [a, b] = &mut state.fighters;
        let (striker, target) = if by == 0 { (&*a, b) } else { (&*b, a) };
        let def = Defence {
            sheet: &target.sheet,
            distance,
            moving: target.moving,
            striker_cursed: !striker.curses.on.is_empty(),
        };
        let s = (!unit && striker_alive).then_some(&striker.c);
        resolve_with(&hit, s, &mut target.c, &def, r)
    };
    let events = event::events_of(&out, &blow, &state.fighters[f].c);
    state.queue.extend(events);
    if out.evaded {
        let me = &mut state.fighters[f];
        me.dodged = 0.3;
        me.body.vx -= dir * 160.0;
        let (x, y) = (me.body.x, me.chest());
        state.emit(Out::Dodge { x, y });
        state.note(Note::Stopped {
            by,
            spell: p.skill,
            how: Stop::Dodge,
            chip: 0.0,
        });
        return false;
    }
    let dealt = out.total();
    let killed = blow.life_before > 0.0 && state.fighters[f].c.life <= 0.0;
    // what the striker gets back (a unit's blow gives its owner nothing)
    if !unit {
        let c = &mut state.fighters[by].c;
        let gained = c.heal(out.gains.life + out.leech);
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
        note_healed(state, by, gained);
        on_hit_behaviours(state, by, f, p, &out.dealt, killed);
    }
    let attacker = &mut state.fighters[by];
    if !unit && attacker.has(Keystone::Fury) {
        attacker.fury = (attacker.fury + 1).min(keystone::FURY_STACKS);
    }
    let mut kinds = 0u16;
    for e in Element::ALL {
        if out.dealt[e.index()] > 0.0 {
            kinds |= 1 << e.index();
        }
    }
    if !unit {
        let now = state.time;
        state.fighters[by]
            .history
            .push((now, u32::from(killed), 1, 0.0, kinds));
    }
    let bonus = match how {
        How::Plain => None,
        How::Counter => Some(FRAMES.counter),
        How::Punish => Some(FRAMES.punish),
    };
    let power = dealt / POWER;
    let launch = BodyBlow {
        power,
        dir: 1.0,
        flying: false,
    }
    .heavy(&body_spec());
    let ranged =
        crate::data::spell(p.skill).is_some_and(|s| matches!(s.kind, Kind::Bolt | Kind::Orb));
    let share = life_share(&state.fighters[f].c, out.to_life);
    let (x, y) = (state.fighters[f].body.x, state.fighters[f].chest());
    state.emit(Out::Hit {
        by,
        spell: p.skill,
        power,
        launch,
        ranged,
        damage: dealt,
        crit: out.crit,
        how,
        share,
        x,
        y,
    });
    state.tell(
        by,
        Told::Outcome(Outcome::Landed {
            skill: p.skill.to_string(),
        }),
    );
    // a heavy blow throws it up, where it can be hit again before it lands; a lighter one staggers it where it stands
    let me = &mut state.fighters[f];
    let flying = me.fly;
    if dealt > 0.0 {
        me.body.hit(BodyBlow { power, dir, flying });
        // a blow that staggers rather than throws holds a fighter caught at a weak moment longer
        if let Some(b) = bonus
            && me.body.down.is_none()
        {
            me.body.stun += b.stagger;
        }
        me.fly = false;
        // struck while recovering from a move: the move was punished, and its mind learns so
        if let Some(a) = me.act
            && a.phase == Phase::Recover
        {
            state.tell(
                f,
                Told::Outcome(Outcome::Punished {
                    skill: a.spell.to_string(),
                }),
            );
        }
        // a hit breaks a cast: what was gathering is lost
        if let Some(a) = state.fighters[f].act
            && a.phase == Phase::Windup
        {
            state.note(Note::Broken {
                who: f,
                spell: a.spell,
            });
        }
        break_act(state, f);
        state.fighters[f].guard = 0.0;
        set_meditating(state, f, false);
    }
    // controls: the skill's own, then a full ailment's
    if let Some(g) = gem::gem(p.skill) {
        for e in g.effects {
            if let Effect::Control(ctl, secs) = e {
                let t = state.fighters[f].c.control(secs * p.control, false);
                control(state, f, *ctl, t);
                state
                    .queue
                    .push(Event::new(EventKind::Controlled, by).against(f));
            }
        }
    }
    for &(ctl, t) in &out.controls {
        control(state, f, ctl, t);
    }
    if p.knockback > 0.0 {
        let field = play::Field {
            width: state.width_t(),
        };
        let x = state.to_t(state.fighters[f].body.x);
        let from = match p.unit.and_then(|u| state.units.iter().find(|x| x.id == u)) {
            Some(u) => state.to_t(u.x),
            None => state.to_t(state.fighters[by].body.x),
        };
        let r = state.fighters[f].body.spec.half_width / T;
        let to = motion::knockback(x, r, from, p.knockback, &field);
        state.fighters[f].body.x = state.from_t(to);
    }
    true
}

/// What a landing blow's supports do that only the fight can: Không Khóa and Chặn Lui lock blinking, Mê Dục locks
/// guard skills, Lưu Lực strikes again with stored force, Sát Sinh Bổ Huyết heals on a kill, Nạp Hồn earns souls
/// again.
fn on_hit_behaviours(
    state: &mut State,
    by: usize,
    on: usize,
    p: &Payload,
    dealt: &[f64; arena_calc::element::N],
    killed: bool,
) {
    let Some(k) = state.fighters[by].kit.index(p.skill) else {
        return;
    };
    let behaviours: Vec<Behaviour> = state.fighters[by].kit.skills[k]
        .supports
        .iter()
        .flat_map(|s| s.behaviours.iter().copied())
        .collect();
    for b in behaviours {
        match b {
            Behaviour::NoBlink { seconds } | Behaviour::NoEscape { seconds } => {
                let f = &mut state.fighters[on];
                f.sealed = f.sealed.max(seconds);
            }
            Behaviour::NoGuard { seconds } => {
                let f = &mut state.fighters[on];
                f.no_guard = f.no_guard.max(seconds);
            }
            Behaviour::LifeOnKill { share } if killed => {
                let c = &mut state.fighters[by].c;
                let life = c.life_max * share;
                let gained = c.heal(life);
                note_healed(state, by, gained);
            }
            Behaviour::SoulsFromSoul => {
                let soul = dealt[Element::Soul.index()];
                state.fighters[by].c.gain_souls(soul / RES.souls_per_damage);
            }
            Behaviour::StoredForce { share, delay } if p.depth == 0 => {
                let force = dealt[Element::Force.index()] * share;
                if force > 0.0 {
                    let mut hit = Hit::split(force, &[(Element::Force, 1.0)]);
                    hit.tags = p.hit.tags;
                    hit.grade = p.hit.grade;
                    let at = state.time + delay;
                    let origin = Origin::of(state, by);
                    if let Some(motion) = crate::data::spell(p.skill) {
                        state.pending.push(Pending {
                            at,
                            payload: Payload {
                                hit,
                                depth: p.depth + 1,
                                ..p.clone()
                            },
                            motion,
                            origin,
                            retry: false,
                            aim: None,
                            target: Some(Target::Fighter(on)),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// A hard control lands on fighter `on` for `t` seconds (after grit): a stun, a hold, a jolt or a knockdown holds
/// the body where it stands and breaks a cast gathering; a root stops walking and blinking; losing the soul
/// silences (only attacks answer).
pub fn control(state: &mut State, on: usize, ctl: Control, t: f64) {
    if t <= 0.0 || !state.fighters[on].alive() {
        return;
    }
    match ctl {
        Control::Stun | Control::Knockdown => {
            let b = &mut state.fighters[on].body;
            b.stun = b.stun.max(t);
            b.shocked = b.shocked.max(t);
            if state.fighters[on]
                .act
                .is_some_and(|a| a.phase == Phase::Windup)
            {
                break_act(state, on);
            }
        }
        Control::Hold | Control::Jolt => {
            let b = &mut state.fighters[on].body;
            b.shocked = b.shocked.max(t);
            if state.fighters[on]
                .act
                .is_some_and(|a| a.phase == Phase::Windup)
            {
                break_act(state, on);
            }
        }
        Control::Root => {
            let f = &mut state.fighters[on];
            f.root = f.root.max(t);
        }
        Control::Soulless => {
            let c = &mut state.fighters[on].c;
            c.soul_lost = c.soul_lost.max(t);
        }
    }
}

/// Runs the event queue and delivers the casts its triggers return, until it is empty or `QUEUE_PASSES` have run.
pub fn run_queue(state: &mut State) {
    for _ in 0..QUEUE_PASSES {
        if state.queue.pending().is_empty() {
            return;
        }
        let rolls: Vec<f64> = (0..QUEUE_ROLLS).map(|_| state.rng.next()).collect();
        let d = state.distance_t();
        let out = {
            let State {
                fighters, queue, ..
            } = state;
            let triggers = [
                fighters[0].kit.triggers.clone(),
                fighters[1].kit.triggers.clone(),
            ];
            let seats = [0, 1].map(|i| Seat {
                triggers: &triggers[i],
                cursed: !fighters[i].curses.on.is_empty(),
                moving: fighters[i].moving,
                distance: d,
            });
            let [a, b] = fighters;
            let mut cs = [a.c.clone(), b.c.clone()];
            let out = queue.run(&mut cs, &seats, &mut Dice::new(&rolls));
            let [ca, cb] = cs;
            a.c = ca;
            b.c = cb;
            out
        };
        for cast in out.casts {
            triggered(state, cast);
        }
    }
}

/// A cast a trigger returned: built at its level and factor, delivered at once at the foe.
fn triggered(state: &mut State, cast: Cast) {
    let me = cast.by.min(1);
    if !state.fighters[me].alive() || state.winner.is_some() {
        return;
    }
    let k = {
        let f = &mut state.fighters[me];
        let (c, sheet) = (f.c.clone(), f.sheet.clone());
        f.kit.link(cast.skill, &c, &sheet)
    };
    let Some(k) = k else { return };
    // a skill whose effect is not in the registry is never cast
    let fx = state.fighters[me].kit.skills[k].motion.fx;
    if !fx.is_none_or(|fx| state.has(fx)) {
        return;
    }
    let Some(mut built) = build(state, me, k, Some(cast.level)) else {
        return;
    };
    if let Some(base) = cast.base {
        let split = &state.fighters[me].kit.skills[k].skill.split;
        built.hit.base = Hit::split(base, split).base;
    }
    scale_hit(&mut built.hit, cast.factor);
    built.hit.tags = built.hit.tags.with(Tag::Trigger);
    let id = state.fighters[me].kit.skills[k].gem.id;
    state.queue.push(
        Event::new(EventKind::Cast, me)
            .with_skill(id, built.hit.tags)
            .at_depth(cast.depth),
    );
    let aim = state.fighters[1 - me].body.x;
    crate::act::deliver(state, me, k, built, Some(aim), cast.depth);
}

/// Seat `me` lays the curse of its kit skill `k` (built as `built`) on the foe.
pub fn lay_curse(state: &mut State, me: usize, k: usize, built: &Built, depth: u8) {
    let g = state.fighters[me].kit.skills[k].gem;
    let Some(kind) = gem::curse_of(g) else {
        return;
    };
    let foe = 1 - me;
    let witch = state.fighters[me].has(Keystone::Witch);
    let cap = curse::curse_limit(&state.fighters[me].c, &state.fighters[me].sheet, witch);
    let effect = curse::curse_effect(&state.fighters[me].c, &state.fighters[me].sheet);
    let curse = Curse::new(kind, built.rank, me as u32, foe as u32);
    let [a, b] = &mut state.fighters;
    let (mine, theirs) = if me == 0 { (a, b) } else { (b, a) };
    theirs.curses.cap = cap;
    let lay = curse::cast_with(curse, &mut theirs.curses, &mut mine.curses, effect);
    if lay != Lay::Failed {
        theirs.curses.sync(&mut theirs.c, false);
        state
            .queue
            .extend(event::curse_events(me, foe, g.id, built.hit.tags, depth));
    }
}

/// A timed buff of seat `me`'s body skill `id` (a stance: damage more; anything else: damage taken less).
pub fn buff(state: &mut State, me: usize, id: &'static str, built: &Built) {
    let src = Source::Skill(id);
    let m = if built.hit.tags.has(Tag::Stance) {
        Mod::more(Stat::Damage, STANCE_MORE, src)
    } else {
        Mod::more(Stat::Taken, BLESSING_TAKEN, src)
    };
    state.fighters[me].c.put_on(arena_calc::cultivator::Active {
        source: src,
        mods: vec![m],
        left: Some(play::duration(BUFF_SECONDS, &built.table)),
    });
}

/// Time wears both fighters, seat 0 then seat 1: their curses beat, then their cultivators tick (burns and bleeds,
/// regen, timers), each damage-over-time tick shown and queued.
pub fn wear(state: &mut State, dt: f64) {
    state.queue.tick(dt);
    for me in 0..2 {
        let foe = 1 - me;
        let evs = {
            let f = &mut state.fighters[me];
            f.curses.tick(dt, &mut f.c, false)
        };
        for e in evs {
            curse_event(state, me, e);
        }
        let ticked = {
            let f = &mut state.fighters[me];
            f.c.meditating = f.medit;
            f.c.tick_with(dt, f.moving, &f.sheet)
        };
        let events = event::dot_events(&ticked, me, &state.fighters[me].c);
        for e in events {
            state.queue.push(e.against(foe));
        }
        for t in &ticked.dots {
            let amount = t.total();
            if amount <= 0.0 {
                continue;
            }
            let share = life_share(&state.fighters[me].c, t.to_life);
            state.note(Note::Ticked {
                by: foe,
                cause: if t.ailment == Ailment::Burn {
                    "burn"
                } else {
                    "bleed"
                },
                share,
            });
            let shown = &mut state.fighters[me].dots;
            shown.0 += amount;
            shown.1 += share;
        }
        // damage over time shows every `DOT_SHOWN` seconds, what it took since
        let f = &mut state.fighters[me];
        f.dots.2 += dt;
        if f.dots.2 >= DOT_SHOWN {
            let (damage, share, _) = core::mem::take(&mut f.dots);
            if damage > 0.0 {
                let (x, y) = (f.body.x, f.chest());
                state.emit(Out::Tick {
                    by: foe,
                    damage,
                    share,
                    x,
                    y,
                });
            }
        }
    }
}

/// A curse on seat `me` did something.
fn curse_event(state: &mut State, me: usize, e: curse::Event) {
    match e {
        curse::Event::Strike { element, share, .. } => {
            let h = curse::strike_hit(element, share, state.fighters[me].c.life_max);
            let out = resolve(&h, None, &mut state.fighters[me].c, Rolls::NONE);
            let share = life_share(&state.fighters[me].c, out.to_life);
            let (x, y) = (state.fighters[me].body.x, state.fighters[me].chest());
            state.emit(Out::Tick {
                by: 1 - me,
                damage: out.total(),
                share,
                x,
                y,
            });
        }
        curse::Event::Backfire {
            by, element, share, ..
        } => {
            let by = (by as usize).min(1);
            let h = curse::strike_hit(element, share, state.fighters[by].c.life_max);
            resolve(&h, None, &mut state.fighters[by].c, Rolls::NONE);
        }
        curse::Event::Control {
            control: ctl,
            seconds,
            ..
        } => {
            let t = state.fighters[me].c.control(seconds, false);
            control(state, me, ctl, t);
        }
        curse::Event::Stall(s) => {
            if let Some(a) = state.fighters[me].act.as_mut()
                && a.phase == Phase::Windup
            {
                a.windup += s;
            }
        }
        _ => {}
    }
}

/// Fighter `me` got `gained` life back (a leech, a kill's heal, a spare life): noted for the telemetry, which reads
/// the life a fighter lost as what it lost less what it got back.
fn note_healed(state: &mut State, me: usize, gained: f64) {
    if gained > 0.0 {
        let max = state.fighters[me].c.life_max.max(1.0);
        state.note(Note::Healed {
            by: me,
            share: gained / max * 100.0,
        });
    }
}

/// Fighter `me` at no life uses a spare life if its keystones give one (Bất Khuất once a round, Tam Mệnh and Ngọn Đèn
/// Hồn their lives a match). Returns whether it rose.
pub fn spare(state: &mut State, me: usize) -> bool {
    if state.fighters[me].alive() {
        return false;
    }
    let ks = state.fighters[me].loadout.keystones.clone();
    for k in ks {
        let used = match k {
            Keystone::Unyielding => u8::from(state.fighters[me].unyielding_used),
            Keystone::ThreeLives | Keystone::SoulLamps => state.fighters[me].lives_used,
            _ => continue,
        };
        if k.on_death(&mut state.fighters[me].c, used) {
            let risen = state.fighters[me].c.life;
            note_healed(state, me, risen);
            let f = &mut state.fighters[me];
            if k == Keystone::Unyielding {
                f.unyielding_used = true;
            } else {
                f.lives_used += 1;
            }
            let (x, y) = (f.body.x, f.chest());
            state.emit(Out::Unyielding { by: me, x, y });
            return true;
        }
    }
    false
}

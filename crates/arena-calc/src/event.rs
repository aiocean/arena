//! Events and triggers (Kích hoạt), PoE's "trigger" mechanics as data: what happened in a fight ([`Event`]), the
//! on-event rules a build carries ([`Trigger`]), and a deterministic [`Queue`] that matches the one to the other.
//!
//! The fight reports what happened (a blow's [`Outcome`] through [`events_of`], the rest with [`Event::new`]),
//! pushes it on the queue and calls [`Queue::run`] with the fighters, their triggers and the dice for the chance
//! checks. The queue marks each fighter's [`Recent`](crate::stat::Recent), puts on the buffs and resource gains the
//! triggers give, and returns the triggered casts ([`Cast`]). It never resolves a cast itself: that needs the
//! fight's positions. The fight builds and resolves each cast, reports its events with the cast's
//! [`Cast::depth`], and runs the queue again; the chain cap ([`CHAIN_DEPTH`]) and the per-tick cap
//! ([`TRIGGERS_PER_TICK`]) stop every loop.

use crate::ailment::{AILMENTS, Ailment};
use crate::cultivator::{Active, Cultivator, RES};
use crate::dot::Ticked;
use crate::element::{Element, N, Parts};
use crate::gem::Resource;
use crate::hit::Outcome;
use crate::stat::{Condition, Ctx, Happened, Mod, ModKind, Per, Source, Stat};
use crate::tag::{Tag, Tags};

/// A fighter, by its index in the fight's list (the slices given to [`Queue::run`]).
pub type Fighter = usize;

/// How deep a chain of triggered casts may go: an event of depth `CHAIN_DEPTH` or more triggers nothing. Events of
/// a skill a player used are depth 0; a cast triggered by an event of depth `d` is depth `d + 1`.
pub const CHAIN_DEPTH: u8 = 3;

/// How many triggers one fighter may fire in one tick (between two [`Queue::tick`]s); the rest are dropped.
pub const TRIGGERS_PER_TICK: u32 = 8;

/// The first low-life line (Nửa máu): life crossing down through this share of full life.
pub const LOW_LIFE_HALF: f64 = 0.5;

/// The second low-life line (Nguy kịch), PoE's "low life".
pub const LOW_LIFE: f64 = 0.35;

/// A low-life line crossed going down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum LifeMark {
    /// Nửa máu: [`LOW_LIFE_HALF`].
    Half,
    /// Nguy kịch: [`LOW_LIFE`].
    Low,
}

impl LifeMark {
    /// Both lines, highest first (the order they are crossed going down).
    pub const ALL: [LifeMark; 2] = [LifeMark::Half, LifeMark::Low];

    /// The line as a share of full life.
    pub fn share(self) -> f64 {
        match self {
            LifeMark::Half => LOW_LIFE_HALF,
            LifeMark::Low => LOW_LIFE,
        }
    }
}

/// What happened. Each is told from the side of the fighter it happened to ([`Event::who`]); the other side, if
/// any, is [`Event::other`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", tag = "type", content = "of")
)]
pub enum EventKind {
    /// Trúng: this fighter's blow landed (not dodged).
    Hit,
    /// Bạo kích: this fighter's blow landed a critical strike.
    Crit,
    /// Sát: this fighter's blow killed.
    Kill,
    /// Bị trúng: a blow landed on this fighter.
    BeenHit,
    /// Bị bạo kích: a critical strike landed on this fighter.
    BeenCrit,
    /// Đỡ: this fighter's guard stopped a blow.
    Guarded,
    /// Chặn: this fighter's shield (a barrier, a treasure, a formation) stopped a blow whole.
    Blocked,
    /// Khiên vỡ: this fighter's shield or guard broke; [`Event::damage`] is what it absorbed.
    ShieldBroke,
    /// Né: this fighter dodged a blow.
    Evaded,
    /// This fighter laid stacks of an ailment ([`Event::stacks`]).
    AilmentLaid(Ailment),
    /// This fighter filled an ailment on its target (the full effect fired).
    AilmentFull(Ailment),
    /// This fighter's blow laid a hard control (a full ailment's stun, root, knockdown...).
    Controlled,
    /// This fighter laid a curse.
    CursedSomeone,
    /// A curse was laid on this fighter.
    Cursed,
    /// This fighter used a skill ([`Event::skill`]); triggered casts report it too, at their depth.
    Cast,
    /// Thuấn di: this fighter blinked.
    Blinked,
    /// This fighter used a guard skill (Hộ thể).
    UsedGuardSkill,
    /// One of this fighter's minions died.
    MinionDied,
    /// This fighter's life crossed a low-life line going down.
    LowLife(LifeMark),
    /// Hồi tố: this fighter was rewound in time (Tuế nguyệt).
    Rewound,
    /// This fighter's domain (Lĩnh vực) collapsed.
    DomainCollapsed,
    /// A burn or a bleed on this fighter dealt damage this tick ([`dot_events`]); [`Event::damage`] is what it took.
    Dotted(Ailment),
    /// For a trigger's `on` only: any [`EventKind::AilmentLaid`], whichever ailment. No event carries it.
    AnyAilmentLaid,
    /// For a trigger's `on` only: any [`EventKind::AilmentFull`].
    AnyAilmentFull,
    /// For a trigger's `on` only: any [`EventKind::Dotted`].
    AnyDotted,
}

impl EventKind {
    /// The [`Happened`] it marks on the fighter it happened to, if any. A cast marks `UsedGuardSkill` or
    /// `UsedMovementSkill` by its tags (see [`Event::happened`]).
    pub fn happened(self) -> Option<Happened> {
        match self {
            EventKind::Hit => Some(Happened::Hit),
            EventKind::Crit => Some(Happened::Crit),
            EventKind::Kill => Some(Happened::Kill),
            EventKind::BeenHit => Some(Happened::BeenHit),
            EventKind::BeenCrit => Some(Happened::BeenCrit),
            EventKind::Guarded | EventKind::Blocked => Some(Happened::Guarded),
            EventKind::ShieldBroke => Some(Happened::ShieldBroke),
            EventKind::AilmentLaid(_) | EventKind::AilmentFull(_) => Some(Happened::LaidAilment),
            EventKind::CursedSomeone => Some(Happened::CursedSomeone),
            EventKind::Blinked => Some(Happened::Blinked),
            EventKind::UsedGuardSkill => Some(Happened::UsedGuardSkill),
            EventKind::Evaded => Some(Happened::Evaded),
            EventKind::Cursed => Some(Happened::Cursed),
            EventKind::MinionDied => Some(Happened::MinionDied),
            EventKind::LowLife(_) => Some(Happened::LowLife),
            EventKind::Rewound => Some(Happened::Rewound),
            EventKind::DomainCollapsed => Some(Happened::DomainCollapsed),
            EventKind::Dotted(_) => Some(Happened::BeenDotted),
            EventKind::Controlled => Some(Happened::LaidControl),
            EventKind::Cast
            | EventKind::AnyAilmentLaid
            | EventKind::AnyAilmentFull
            | EventKind::AnyDotted => None,
        }
    }

    /// Whether a trigger listening for `self` takes an event of kind `e`: the same kind, or a wildcard
    /// ([`EventKind::AnyAilmentLaid`], [`EventKind::AnyAilmentFull`], [`EventKind::AnyDotted`]) and an event of its
    /// family.
    pub fn takes(self, e: EventKind) -> bool {
        match self {
            EventKind::AnyAilmentLaid => matches!(e, EventKind::AilmentLaid(_)),
            EventKind::AnyAilmentFull => matches!(e, EventKind::AilmentFull(_)),
            EventKind::AnyDotted => matches!(e, EventKind::Dotted(_)),
            k => k == e,
        }
    }
}

/// One thing that happened: what, to whom, against whom, with which skill and blow.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Event {
    pub kind: EventKind,
    /// The fighter it happened to: its triggers are the ones that listen.
    pub who: Fighter,
    /// The other side: the target of a hit, the striker of a blow taken, the one cursed.
    pub other: Option<Fighter>,
    /// The skill involved, by its pool id (`gem::Gem::id`).
    pub skill: Option<&'static str>,
    /// The blow's or the skill's tags, kinds of damage included. A triggered cast's tags carry [`Tag::Trigger`].
    #[cfg_attr(feature = "serde", serde(skip))]
    pub tags: Tags,
    /// The damage it carried: a blow's total dealt, a broken shield's absorbed.
    pub damage: f64,
    /// The damage by kind, after mitigation.
    pub kinds: Parts,
    /// Whether the blow was a critical strike.
    pub crit: bool,
    /// Stacks laid (an ailment event).
    pub stacks: f64,
    /// The stacks of each ailment on the target after the blow (what a herald reads on a kill).
    pub ailments: [f64; AILMENTS],
    /// The chain depth: 0 for a skill a player used, [`Cast::depth`] for a triggered one.
    pub depth: u8,
}

impl Event {
    /// An event with nothing but its kind and the fighter it happened to.
    pub fn new(kind: EventKind, who: Fighter) -> Event {
        Event {
            kind,
            who,
            other: None,
            skill: None,
            tags: Tags::NONE,
            damage: 0.0,
            kinds: [0.0; N],
            crit: false,
            stacks: 0.0,
            ailments: [0.0; AILMENTS],
            depth: 0,
        }
    }

    /// With the other side.
    pub fn against(mut self, other: Fighter) -> Event {
        self.other = Some(other);
        self
    }

    /// With the skill and its tags.
    pub fn with_skill(mut self, skill: &'static str, tags: Tags) -> Event {
        self.skill = Some(skill);
        self.tags = tags;
        self
    }

    /// With a damage amount and its split by kind.
    pub fn with_damage(mut self, damage: f64, kinds: Parts) -> Event {
        self.damage = damage;
        self.kinds = kinds;
        self
    }

    /// At a chain depth.
    pub fn at_depth(mut self, depth: u8) -> Event {
        self.depth = depth;
        self
    }

    /// The [`Happened`] it marks on [`Event::who`]: its kind's, and for a cast, `UsedGuardSkill` for a guard skill
    /// or `UsedMovementSkill` for a movement skill.
    pub fn happened(&self) -> Option<Happened> {
        match self.kind {
            EventKind::Cast if self.tags.has(Tag::Guard) => Some(Happened::UsedGuardSkill),
            EventKind::Cast if self.tags.has(Tag::Movement) => Some(Happened::UsedMovementSkill),
            k => k.happened(),
        }
    }
}

/// The blow an [`Outcome`] came from, for [`events_of`]: who struck whom with what, and the target's bars just
/// before the blow (the fight reads them before `hit::resolve`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    /// The striker, `None` for damage with no striker (a lava zone).
    pub by: Option<Fighter>,
    /// The target.
    pub on: Fighter,
    pub skill: Option<&'static str>,
    pub tags: Tags,
    /// [`Cast::depth`] for a triggered blow, else 0.
    pub depth: u8,
    pub life_before: f64,
    pub shield_before: f64,
}

/// A blow's events, in this fixed order: a dodge gives `Evaded` alone; else `Hit`, `BeenHit`, `Crit`, `BeenCrit`,
/// `ShieldBroke`, `AilmentLaid` (in [`Ailment::ALL`]'s order), `AilmentFull` (in the outcome's order), `Controlled`
/// (when the blow laid a hard control), `LowLife`
/// (half, then low) and `Kill`. Events of the striker are left out when there is none. `target` is the target after
/// the blow.
pub fn events_of(out: &Outcome, blow: &Blow, target: &Cultivator) -> Vec<Event> {
    let mut ailments = [0.0; AILMENTS];
    for a in Ailment::ALL {
        ailments[a.index()] = target.ailments.stacks(a);
    }
    let base = |kind: EventKind, who: Fighter, other: Option<Fighter>| Event {
        kind,
        who,
        other,
        skill: blow.skill,
        tags: blow.tags,
        damage: out.total(),
        kinds: out.dealt,
        crit: out.crit,
        stacks: 0.0,
        ailments,
        depth: blow.depth,
    };
    let on = blow.on;
    let by = blow.by;
    let mut ev = Vec::new();
    if out.evaded {
        let mut e = base(EventKind::Evaded, on, by);
        e.damage = 0.0;
        e.kinds = [0.0; N];
        ev.push(e);
        return ev;
    }
    if let Some(s) = by {
        ev.push(base(EventKind::Hit, s, Some(on)));
    }
    ev.push(base(EventKind::BeenHit, on, by));
    if out.crit {
        if let Some(s) = by {
            ev.push(base(EventKind::Crit, s, Some(on)));
        }
        ev.push(base(EventKind::BeenCrit, on, by));
    }
    if blow.shield_before > 0.0 && target.shield <= 0.0 {
        let mut e = base(EventKind::ShieldBroke, on, by);
        e.damage = out.to_shield;
        ev.push(e);
    }
    if let Some(s) = by {
        for a in Ailment::ALL {
            let n = out.stacks[a.index()];
            if n > 0.0 {
                let mut e = base(EventKind::AilmentLaid(a), s, Some(on));
                e.stacks = n;
                ev.push(e);
            }
        }
        for &a in &out.full {
            ev.push(base(EventKind::AilmentFull(a), s, Some(on)));
        }
        if !out.controls.is_empty() {
            ev.push(base(EventKind::Controlled, s, Some(on)));
        }
    }
    let max = target.life_max.max(1.0);
    for m in LifeMark::ALL {
        let line = m.share() * max;
        if blow.life_before > line && target.life <= line {
            ev.push(base(EventKind::LowLife(m), on, by));
        }
    }
    if let Some(s) = by
        && blow.life_before > 0.0
        && target.life <= 0.0
    {
        ev.push(base(EventKind::Kill, s, Some(on)));
    }
    ev
}

/// A fighter's tick of damage over time (`Cultivator::tick_with`) as events: one `Dotted` per ailment that dealt
/// something, in the tick's order, with what it took off the bars, of its kind, at depth 0. The striker is not
/// known here: the fight adds it with [`Event::against`] if it keeps it.
pub fn dot_events(ticked: &Ticked, who: Fighter, target: &Cultivator) -> Vec<Event> {
    let mut ailments = [0.0; AILMENTS];
    for a in Ailment::ALL {
        ailments[a.index()] = target.ailments.stacks(a);
    }
    ticked
        .dots
        .iter()
        .filter(|d| d.total() > 0.0)
        .map(|d| {
            let mut kinds = [0.0; N];
            kinds[d.kind.index()] = d.total();
            let mut e = Event::new(EventKind::Dotted(d.ailment), who).with_damage(d.total(), kinds);
            e.ailments = ailments;
            e
        })
        .collect()
}

/// A curse laid: `CursedSomeone` for the one who laid it, then `Cursed` for the one it lies on.
pub fn curse_events(
    by: Fighter,
    on: Fighter,
    skill: &'static str,
    tags: Tags,
    depth: u8,
) -> [Event; 2] {
    [
        Event::new(EventKind::CursedSomeone, by)
            .against(on)
            .with_skill(skill, tags)
            .at_depth(depth),
        Event::new(EventKind::Cursed, on)
            .against(by)
            .with_skill(skill, tags)
            .at_depth(depth),
    ]
}

/// Marks each event's [`Happened`] on the fighter it happened to. Events of fighters outside `fighters` (a minion
/// the fight keeps elsewhere) are skipped.
pub fn mark(events: &[Event], fighters: &mut [Cultivator]) {
    for e in events {
        if let (Some(h), Some(f)) = (e.happened(), fighters.get_mut(e.who)) {
            f.recent.mark(h);
        }
    }
}

/// Which events a trigger takes, beyond its kind and condition: the event's tags, kinds and skill, and what the
/// target carried. Empty lists and `None` mean any.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Filter {
    /// The event's tags carry every one of these, at least one of `any_of` (when not empty), and none of `none_of`.
    pub all_of: &'static [Tag],
    pub any_of: &'static [Tag],
    pub none_of: &'static [Tag],
    /// The event dealt some damage of at least one of these kinds.
    pub kinds: &'static [Element],
    /// The event's skill is this one.
    pub skill: Option<&'static str>,
    /// The target carried stacks of this ailment ([`Event::ailments`]).
    pub ailing: Option<Ailment>,
    /// The event carried at least this much damage.
    pub min_damage: f64,
}

impl Filter {
    /// Takes every event.
    pub const ANY: Filter = Filter {
        all_of: &[],
        any_of: &[],
        none_of: &[],
        kinds: &[],
        skill: None,
        ailing: None,
        min_damage: 0.0,
    };

    /// Whether it takes `e`.
    pub fn takes(&self, e: &Event) -> bool {
        e.tags.all(self.all_of)
            && (self.any_of.is_empty() || e.tags.any(self.any_of))
            && !e.tags.any(self.none_of)
            && (self.kinds.is_empty() || self.kinds.iter().any(|k| e.kinds[k.index()] > 0.0))
            && self.skill.is_none_or(|s| e.skill == Some(s))
            && self.ailing.is_none_or(|a| e.ailments[a.index()] > 0.0)
            && e.damage >= self.min_damage
    }
}

/// How a triggered cast's damage is set.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", tag = "type"))]
pub enum Scale {
    /// The skill's own damage at its level, times the factor.
    Skill,
    /// The skill's base becomes this share of the event's damage (Liên Hoàn Suy Yếu: what the guard absorbed),
    /// split by the skill's own kinds; the factor still applies.
    Event { share: f64 },
    /// A herald's blast: the factor grows by `per_stack` for each stack of the ailment the target carried.
    Stacks { ailment: Ailment, per_stack: f64 },
}

/// A skill a trigger casts: its pool id, the level (hỏa hầu) it is cast at, and a damage factor (PoE's "less
/// damage" on triggered skills is a factor below 1).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TriggeredCast {
    pub skill: &'static str,
    pub level: u8,
    pub factor: f64,
    pub scale: Scale,
}

/// A timed buff a trigger puts on the fighter it belongs to (refreshed, not stacked: same source replaces).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Buff {
    pub source: Source,
    pub mods: &'static [Mod],
    pub seconds: f64,
}

impl Buff {
    /// The effect to put on a fighter.
    pub fn active(&self) -> Active {
        Active {
            source: self.source,
            mods: self.mods.to_vec(),
            left: Some(self.seconds),
        }
    }
}

/// A resource a trigger gives the fighter it belongs to, capped at its bar.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Gain {
    pub resource: Resource,
    pub amount: f64,
}

impl Gain {
    /// Gives it to `c`: qi, sense, souls, death qi and stars up to their caps; life as healing (wither cuts it);
    /// intent through `gain_intent`; lifespan as is.
    pub fn apply(&self, c: &mut Cultivator) {
        let n = self.amount.max(0.0);
        match self.resource {
            Resource::Qi => c.qi = (c.qi + n).min(c.qi_max),
            Resource::Life => {
                c.heal(n);
            }
            Resource::Lifespan => c.lifespan += n,
            Resource::Intent => c.gain_intent(n),
            Resource::Souls => c.souls = (c.souls + n).min(RES.souls_max),
            Resource::Sense => c.sense = (c.sense + n).min(c.sense_max),
            Resource::DeathQi => c.death_qi = (c.death_qi + n).min(RES.death_max),
            Resource::Star(s) => {
                let i = s as usize;
                c.stars[i] = (c.stars[i] + n).min(RES.stars_max);
                c.star_time[i] = RES.star_time;
            }
        }
    }
}

/// What a trigger does.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", tag = "type"))]
pub enum Effect {
    /// Casts a skill: returned to the fight as a [`Cast`].
    Cast(TriggeredCast),
    /// Puts on a buff (applied by the queue).
    Buff(Buff),
    /// Gives a resource (applied by the queue).
    Gain(Gain),
}

/// An on-event rule. It fires on an event of kind `on` that happened to its fighter, at a depth under
/// [`CHAIN_DEPTH`], that its `filter` takes and while `when` holds; with a `threshold`, only once the damage of such
/// events adds up to it (then the sum starts over); only off its cooldown and under the per-tick cap; and only if
/// the roll beats `chance` (a roll is drawn only when `chance < 1`, after every other check passes).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Trigger {
    /// Its code name: cooldowns and sums are kept by it.
    pub id: &'static str,
    /// Its Vietnamese name.
    pub name: &'static str,
    pub on: EventKind,
    /// Read with the fighter as `me`, the event's other side as `other` and the event's tags as the skill.
    pub when: Condition,
    pub filter: Filter,
    /// The chance to fire, 0 to 1.
    pub chance: f64,
    /// Seconds before it can fire again (an internal cooldown).
    pub cooldown: f64,
    /// Damage to add up before it fires (0: every event).
    pub threshold: f64,
    pub does: Effect,
}

// ---- the PoE cases as data ----

/// Bạo Kích Dẫn Lôi's internal cooldown (PoE's Cast On Critical Strike).
pub const CRIT_CAST_COOLDOWN: f64 = 0.25;
/// Bạo Kích Dẫn Lôi's damage factor.
pub const CRIT_CAST_FACTOR: f64 = 0.8;
/// Bạo Kích Dẫn Lôi's level.
pub const CRIT_CAST_LEVEL: u8 = 10;

/// Bạo Kích Dẫn Lôi (cast on crit): an attack's critical strike casts Cực Lôi. Triggered blows do not trigger it.
pub const CAST_ON_CRIT: Trigger = Trigger {
    id: "castoncrit",
    name: "Bạo Kích Dẫn Lôi",
    on: EventKind::Crit,
    when: Condition::Always,
    filter: Filter {
        all_of: &[Tag::Attack],
        none_of: &[Tag::Trigger],
        ..Filter::ANY
    },
    chance: 1.0,
    cooldown: CRIT_CAST_COOLDOWN,
    threshold: 0.0,
    does: Effect::Cast(TriggeredCast {
        skill: "jithunder",
        level: CRIT_CAST_LEVEL,
        factor: CRIT_CAST_FACTOR,
        scale: Scale::Skill,
    }),
};

/// Thụ Thương Phản Pháp's damage taken to add up before it fires (PoE's Cast when Damage Taken).
pub const HURT_CAST_THRESHOLD: f64 = 300.0;
/// Thụ Thương Phản Pháp's internal cooldown.
pub const HURT_CAST_COOLDOWN: f64 = 0.25;
/// Thụ Thương Phản Pháp's level.
pub const HURT_CAST_LEVEL: u8 = 5;

/// Thụ Thương Phản Pháp (cast when damage taken): every [`HURT_CAST_THRESHOLD`] of damage taken casts Chấn Địa.
pub const CAST_WHEN_HURT: Trigger = Trigger {
    id: "castwhenhurt",
    name: "Thụ Thương Phản Pháp",
    on: EventKind::BeenHit,
    when: Condition::Always,
    filter: Filter::ANY,
    chance: 1.0,
    cooldown: HURT_CAST_COOLDOWN,
    threshold: HURT_CAST_THRESHOLD,
    does: Effect::Cast(TriggeredCast {
        skill: "quake",
        level: HURT_CAST_LEVEL,
        factor: 1.0,
        scale: Scale::Skill,
    }),
};

/// Sát Sinh Dẫn Hỏa's level (PoE's Cast on Death of an enemy, the kill side).
pub const KILL_CAST_LEVEL: u8 = 10;

/// Sát Sinh Dẫn Hỏa (cast on kill): a kill casts Hỏa Cầu. Triggered blows do not trigger it.
pub const CAST_ON_KILL: Trigger = Trigger {
    id: "castonkill",
    name: "Sát Sinh Dẫn Hỏa",
    on: EventKind::Kill,
    when: Condition::Always,
    filter: Filter {
        none_of: &[Tag::Trigger],
        ..Filter::ANY
    },
    chance: 1.0,
    cooldown: 0.0,
    threshold: 0.0,
    does: Effect::Cast(TriggeredCast {
        skill: "fireball",
        level: KILL_CAST_LEVEL,
        factor: 1.0,
        scale: Scale::Skill,
    }),
};

/// Liên Hoàn Suy Yếu's share of what the guard absorbed (research: "sát thương bằng phần đã hấp thụ").
pub const GUARD_BREAK_SHARE: f64 = 1.0;
/// Liên Hoàn Suy Yếu's internal cooldown.
pub const GUARD_BREAK_COOLDOWN: f64 = 1.0;

/// Liên Hoàn Suy Yếu (cast when the shield breaks, CM6): the shield or guard breaking casts Chưởng with a base of
/// what it absorbed.
pub const CAST_ON_SHIELD_BREAK: Trigger = Trigger {
    id: "castonshieldbreak",
    name: "Liên Hoàn Suy Yếu",
    on: EventKind::ShieldBroke,
    when: Condition::Always,
    filter: Filter::ANY,
    chance: 1.0,
    cooldown: GUARD_BREAK_COOLDOWN,
    threshold: 0.0,
    does: Effect::Cast(TriggeredCast {
        skill: "palm",
        level: 1,
        factor: 1.0,
        scale: Scale::Event {
            share: GUARD_BREAK_SHARE,
        },
    }),
};

/// Nguyền Ấn Truy Kích's chance on each hit.
pub const CURSED_HUNT_CHANCE: f64 = 0.5;
/// Nguyền Ấn Truy Kích's damage increase.
pub const CURSED_HUNT_DAMAGE: f64 = 0.2;
/// Nguyền Ấn Truy Kích's buff, in seconds.
pub const CURSED_HUNT_SECONDS: f64 = 4.0;

const CURSED_HUNT_MODS: &[Mod] = &[Mod {
    stat: Stat::Damage,
    kind: ModKind::Increased,
    value: CURSED_HUNT_DAMAGE,
    need: &[],
    when: Condition::Always,
    source: Source::Other("cursedhunt"),
    per: Per::One,
    max: None,
}];

/// Nguyền Ấn Truy Kích (on hit against a cursed foe): a hit on a cursed target has [`CURSED_HUNT_CHANCE`] to give
/// [`CURSED_HUNT_DAMAGE`] increased damage for [`CURSED_HUNT_SECONDS`].
pub const ON_HIT_VS_CURSED: Trigger = Trigger {
    id: "cursedhunt",
    name: "Nguyền Ấn Truy Kích",
    on: EventKind::Hit,
    when: Condition::TargetCursed,
    filter: Filter::ANY,
    chance: CURSED_HUNT_CHANCE,
    cooldown: 0.0,
    threshold: 0.0,
    does: Effect::Buff(Buff {
        source: Source::Other("cursedhunt"),
        mods: CURSED_HUNT_MODS,
        seconds: CURSED_HUNT_SECONDS,
    }),
};

/// Thiêu Tận Hồi Khí's qi.
pub const BURN_FULL_QI: f64 = 20.0;
/// Thiêu Tận Hồi Khí's internal cooldown.
pub const BURN_FULL_COOLDOWN: f64 = 2.0;

/// Thiêu Tận Hồi Khí (on ailment full): filling a burn gives back [`BURN_FULL_QI`] qi.
pub const ON_BURN_FULL: Trigger = Trigger {
    id: "burnfull",
    name: "Thiêu Tận Hồi Khí",
    on: EventKind::AilmentFull(Ailment::Burn),
    when: Condition::Always,
    filter: Filter::ANY,
    chance: 1.0,
    cooldown: BURN_FULL_COOLDOWN,
    threshold: 0.0,
    does: Effect::Gain(Gain {
        resource: Resource::Qi,
        amount: BURN_FULL_QI,
    }),
};

/// Hiệu Lệnh Thiêu Thi's growth per burn stack on the slain.
pub const HERALD_PER_STACK: f64 = 0.1;
/// Hiệu Lệnh Thiêu Thi's damage factor before the stacks.
pub const HERALD_FACTOR: f64 = 0.5;
/// Hiệu Lệnh Thiêu Thi's level.
pub const HERALD_LEVEL: u8 = 10;

/// Hiệu Lệnh Thiêu Thi (a herald, PoE's Herald of Ash): killing a burning foe sets off Tam Giới Hỏa Nguyên where it
/// fell, [`HERALD_PER_STACK`] more per burn stack it carried.
pub const HERALD_OF_ASH: Trigger = Trigger {
    id: "heraldofash",
    name: "Hiệu Lệnh Thiêu Thi",
    on: EventKind::Kill,
    when: Condition::Always,
    filter: Filter {
        ailing: Some(Ailment::Burn),
        ..Filter::ANY
    },
    chance: 1.0,
    cooldown: 0.0,
    threshold: 0.0,
    does: Effect::Cast(TriggeredCast {
        skill: "flameorigin",
        level: HERALD_LEVEL,
        factor: HERALD_FACTOR,
        scale: Scale::Stacks {
            ailment: Ailment::Burn,
            per_stack: HERALD_PER_STACK,
        },
    }),
};

/// Lâm Nguy Phát Pháp's internal cooldown (PoE's Cast when Damage Taken at low life, the arena's Cast on Death).
pub const LOW_LIFE_CAST_COOLDOWN: f64 = 4.0;
/// Lâm Nguy Phát Pháp's level.
pub const LOW_LIFE_CAST_LEVEL: u8 = 10;

/// Lâm Nguy Phát Pháp (cast at death's door): life crossing [`LOW_LIFE`] going down casts Chấn Địa. The support of
/// that name (`support`) casts the skill it supports instead ([`Trigger::casting`]).
pub const CAST_ON_LOW_LIFE: Trigger = Trigger {
    id: "castonlowlife",
    name: "Lâm Nguy Phát Pháp",
    on: EventKind::LowLife(LifeMark::Low),
    when: Condition::Always,
    filter: Filter::ANY,
    chance: 1.0,
    cooldown: LOW_LIFE_CAST_COOLDOWN,
    threshold: 0.0,
    does: Effect::Cast(TriggeredCast {
        skill: "quake",
        level: LOW_LIFE_CAST_LEVEL,
        factor: 1.0,
        scale: Scale::Skill,
    }),
};

/// Every trigger of the crate, for a bot, a tooltip or a test.
pub const ALL: &[Trigger] = &[
    CAST_ON_CRIT,
    CAST_WHEN_HURT,
    CAST_ON_KILL,
    CAST_ON_SHIELD_BREAK,
    ON_HIT_VS_CURSED,
    ON_BURN_FULL,
    HERALD_OF_ASH,
    CAST_ON_LOW_LIFE,
];

impl Trigger {
    /// The same trigger casting `skill` at `level` (a trigger support casts the skill it is hung on). A trigger that
    /// does not cast is returned as it is.
    pub fn casting(mut self, skill: &'static str, level: u8) -> Trigger {
        if let Effect::Cast(c) = &mut self.does {
            c.skill = skill;
            c.level = level;
        }
        self
    }
}

// ---- the queue ----

/// A triggered cast for the fight to build (`build`, with `level`), scale and resolve.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Cast {
    /// The caster: the fighter whose trigger fired.
    pub by: Fighter,
    /// The event's other side: what to aim at (or where a slain foe fell).
    pub at: Option<Fighter>,
    /// The trigger that fired.
    pub trigger: &'static str,
    pub skill: &'static str,
    pub level: u8,
    /// Multiplies the built blow's damage (a "more").
    pub factor: f64,
    /// When set, the skill's base is this amount, split by its own kinds ([`Scale::Event`]).
    pub base: Option<f64>,
    /// The chain depth its events carry: the fight passes it to [`Blow::depth`] and [`Event::at_depth`], and adds
    /// [`Tag::Trigger`] to the blow's tags.
    pub depth: u8,
}

/// One trigger that fired, for a log or a test.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Fired {
    pub by: Fighter,
    pub trigger: &'static str,
    pub on: EventKind,
    pub does: Effect,
}

/// What one [`Queue::run`] did.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Output {
    /// The casts for the fight, in firing order.
    pub casts: Vec<Cast>,
    /// Every trigger that fired, casts included, in firing order.
    pub fired: Vec<Fired>,
    /// Fires the per-tick cap stopped.
    pub capped: u32,
    /// Events too deep to trigger anything (at [`CHAIN_DEPTH`] or more).
    pub too_deep: u32,
}

/// One fighter's side for a run: its triggers, and what its conditions read that a [`Cultivator`] does not hold.
#[derive(Clone, Copy, Debug, Default)]
pub struct Seat<'a> {
    pub triggers: &'a [Trigger],
    /// It carries a curse (curses live outside the cultivator).
    pub cursed: bool,
    pub moving: bool,
    /// How far it stands from the other side of its events.
    pub distance: f64,
}

/// The rolls for the chance checks, drawn by the fight from its dice. Used in order; past the end a roll is 1
/// (nothing fires on it).
#[derive(Clone, Copy, Debug)]
pub struct Dice<'a> {
    rolls: &'a [f64],
    next: usize,
}

impl<'a> Dice<'a> {
    pub fn new(rolls: &'a [f64]) -> Dice<'a> {
        Dice { rolls, next: 0 }
    }

    /// How many rolls were drawn.
    pub fn used(&self) -> usize {
        self.next
    }

    fn roll(&mut self) -> f64 {
        let r = self.rolls.get(self.next).copied().unwrap_or(1.0);
        self.next += 1;
        r
    }
}

/// The event queue: events waiting, triggers' cooldowns and damage sums, and the per-tick fire counts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Queue {
    events: Vec<Event>,
    /// `(fighter, trigger id, seconds left)`.
    cooldowns: Vec<(Fighter, &'static str, f64)>,
    /// `(fighter, trigger id, damage summed)`.
    sums: Vec<(Fighter, &'static str, f64)>,
    /// Fires this tick, by fighter.
    fired: Vec<u32>,
}

impl Queue {
    pub fn new() -> Queue {
        Queue::default()
    }

    /// Adds an event; it is matched at the next [`Queue::run`].
    pub fn push(&mut self, e: Event) {
        self.events.push(e);
    }

    /// Adds events in order.
    pub fn extend(&mut self, es: impl IntoIterator<Item = Event>) {
        self.events.extend(es);
    }

    /// The events waiting.
    pub fn pending(&self) -> &[Event] {
        &self.events
    }

    /// Seconds before a fighter's trigger can fire again (0 when ready).
    pub fn cooldown(&self, who: Fighter, id: &str) -> f64 {
        self.cooldowns
            .iter()
            .find(|c| c.0 == who && c.1 == id)
            .map_or(0.0, |c| c.2)
    }

    /// The damage a fighter's threshold trigger has summed so far.
    pub fn summed(&self, who: Fighter, id: &str) -> f64 {
        self.sums
            .iter()
            .find(|c| c.0 == who && c.1 == id)
            .map_or(0.0, |c| c.2)
    }

    /// A new tick: cooldowns run down by `dt`, and the per-tick counts start over.
    pub fn tick(&mut self, dt: f64) {
        for c in &mut self.cooldowns {
            c.2 -= dt;
        }
        self.cooldowns.retain(|c| c.2 > 0.0);
        self.fired.clear();
    }

    /// Matches every waiting event against the triggers of the fighter it happened to, in a stable order: by
    /// fighter index, then in the order pushed; each fighter's triggers in their order. First marks every event's
    /// [`Happened`] on its fighter. Buffs and gains go on at once; casts are returned. `fighters` and `seats` are
    /// indexed by [`Fighter`]; an event of a fighter outside either is only dropped.
    pub fn run(&mut self, fighters: &mut [Cultivator], seats: &[Seat], dice: &mut Dice) -> Output {
        let mut events = std::mem::take(&mut self.events);
        events.sort_by_key(|e| e.who);
        mark(&events, fighters);
        let mut out = Output::default();
        for e in &events {
            let Some(seat) = seats.get(e.who) else {
                continue;
            };
            if e.depth >= CHAIN_DEPTH {
                out.too_deep += 1;
                continue;
            }
            for t in seat.triggers {
                if !t.on.takes(e.kind) || !t.filter.takes(e) {
                    continue;
                }
                let holds = {
                    let Some(me) = fighters.get(e.who) else {
                        break;
                    };
                    let other = e.other.and_then(|o| fighters.get(o));
                    let ctx = Ctx {
                        other,
                        other_cursed: e.other.and_then(|o| seats.get(o)).is_some_and(|s| s.cursed),
                        distance: seat.distance,
                        moving: seat.moving,
                        skill: e.tags,
                        ..Ctx::alone(me)
                    };
                    t.when.holds(&ctx)
                };
                if !holds {
                    continue;
                }
                if t.threshold > 0.0 && self.add_sum(e.who, t.id, e.damage) < t.threshold {
                    continue;
                }
                if self.cooldown(e.who, t.id) > 0.0 {
                    continue;
                }
                if self.fired_by(e.who) >= TRIGGERS_PER_TICK {
                    out.capped += 1;
                    continue;
                }
                if t.chance < 1.0 && dice.roll() >= t.chance {
                    continue;
                }
                self.fire(e, t, fighters, &mut out);
            }
        }
        out
    }

    fn add_sum(&mut self, who: Fighter, id: &'static str, d: f64) -> f64 {
        match self.sums.iter_mut().find(|c| c.0 == who && c.1 == id) {
            Some(c) => {
                c.2 += d;
                c.2
            }
            None => {
                self.sums.push((who, id, d));
                d
            }
        }
    }

    fn fired_by(&self, who: Fighter) -> u32 {
        self.fired.get(who).copied().unwrap_or(0)
    }

    fn fire(&mut self, e: &Event, t: &Trigger, fighters: &mut [Cultivator], out: &mut Output) {
        if self.fired.len() <= e.who {
            self.fired.resize(e.who + 1, 0);
        }
        self.fired[e.who] += 1;
        if t.cooldown > 0.0 {
            self.cooldowns.retain(|c| !(c.0 == e.who && c.1 == t.id));
            self.cooldowns.push((e.who, t.id, t.cooldown));
        }
        if t.threshold > 0.0 {
            self.sums.retain(|c| !(c.0 == e.who && c.1 == t.id));
        }
        match t.does {
            Effect::Cast(c) => {
                let (factor, base) = match c.scale {
                    Scale::Skill => (c.factor, None),
                    Scale::Event { share } => (c.factor, Some(e.damage * share)),
                    Scale::Stacks { ailment, per_stack } => (
                        c.factor * (1.0 + per_stack * e.ailments[ailment.index()]),
                        None,
                    ),
                };
                out.casts.push(Cast {
                    by: e.who,
                    at: e.other,
                    trigger: t.id,
                    skill: c.skill,
                    level: c.level,
                    factor,
                    base,
                    depth: e.depth + 1,
                });
            }
            Effect::Buff(b) => {
                if let Some(f) = fighters.get_mut(e.who) {
                    f.put_on(b.active());
                }
            }
            Effect::Gain(g) => {
                if let Some(f) = fighters.get_mut(e.who) {
                    g.apply(f);
                }
            }
        }
        out.fired.push(Fired {
            by: e.who,
            trigger: t.id,
            on: e.kind,
            does: t.does,
        });
    }
}

//! The modifier engine, the backbone every source feeds (design, section 17). A support, a keystone, an item worn, a
//! passive node, an intent, a blessing, a curse, an ailment, a star or a domain gives a list of [`Mod`]s; [`Table::gather`] keeps those whose
//! tags and [`Condition`] hold in the moment ([`Ctx`]) and sums them per [`Stat`]; `build`, `resolve` and the
//! damage over time read only the table. Nothing has to "remember" to add a source by hand.
//!
//! The arithmetic is PoE's: `value = (base + Σ flat) × (1 + Σ increased) × Π (1 + more)`, and an override, if any,
//! wins (the last one gathered, so a source's order is part of its meaning).
//!
//! A modifier can count more than once ([`Per`]: per stack on the target, per curse, per star, per T of distance,
//! per 1% of life missing...) up to a cap ([`Mod::max`]). The stats the calculator gathers but leaves to the fight
//! are [`FIGHT_READS`].

use crate::ailment::{AILMENTS, Ailment};
use crate::cultivator::{Cultivator, Star};
use crate::element::{Element, N};
use crate::tag::{Tag, Tags};

/// A number a modifier can change. Damage stats split into what hits ([`Stat::HitDamage`]) and what burns, bleeds
/// and withers over time ([`Stat::DotDamage`], [`Stat::DotMulti`]): a support can feed one and not the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Stat {
    // ---- the striker's damage
    /// Every damage this fighter deals, hit or over time.
    Damage,
    /// Damage of a kind, for every part that is or has been that kind.
    DamageOf(Element),
    /// Hits only.
    HitDamage,
    /// Damage over time only (every ailment that deals damage).
    DotDamage,
    /// One ailment's damage over time.
    AilmentDamage(Ailment),
    /// The damage-over-time multiplier: `+x` adds `x` to a factor that starts at 1 (PoE's DoT multi).
    DotMulti,
    /// The damage-over-time multiplier of a kind.
    DotMultiOf(Element),
    /// Flat damage of a kind added to hits (scaled by the skill's effectiveness).
    Added(Element),
    /// A share of damage of the first kind converted to the second (forward only; see `hit::scale`).
    Convert(Element, Element),
    /// Gain as extra: a share of each part that is the first kind after conversion is gained as an extra part of
    /// the second, which has been both kinds (any direction: nothing converts after it; see `hit::scale`).
    Extra(Element, Element),
    CritChance,
    CritMulti,
    /// The chance a blow deals double damage (step 5, its own roll: `hit::Rolls::double`).
    ChanceDoubleDamage,
    Penetration(Element),
    ArmourPen,
    /// A share of every part that would meet the shield first (all but yin and soul) passes it to life.
    ShieldBypass,
    /// Culling: a target left at or under this share of its full life by the blow dies.
    Execute,
    /// Life drunk, a share of damage dealt.
    Leech,
    /// Qi drunk, a share of damage dealt.
    QiLeech,
    /// Flat life, qi, sense and killing intent the striker gains for each blow that lands.
    LifeOnHit,
    QiOnHit,
    SenseOnHit,
    IntentOnHit,
    /// Stacks of an ailment every landing blow lays besides what its parts lay (Tịch Thủ's wither).
    Inflict(Ailment),
    /// The chance (0..1) to gain a star of a kind when the thing happens. `hit::resolve` rolls the striker's on
    /// `Hit`, `Crit`, `Kill` and `LaidControl`, and the target's on `BeenHit` and `BeenCrit`; the fight the rest.
    StarOn(Happened, Star),
    // ---- the striker's ailments
    /// The stacks a blow lays of every ailment (PoE's "effect of ailments" for stacking ones).
    AilmentEffect,
    AilmentEffectOf(Ailment),
    /// The effect of the ailments that do not deal damage (every one but burn and bleed): the stacks they lay toward
    /// their full, PoE's "effect of non-damaging ailments".
    ControlEffect,
    /// How long the hard controls this fighter lays last (a full ailment's, a skill's own).
    ControlDuration,
    /// How long the ailments it lays last.
    AilmentDuration,
    AilmentDurationOf(Ailment),
    // ---- the skill
    Projectiles,
    /// Times a projectile chains to another target, pierces one, forks in two; its speed (a factor from 1). The
    /// field behaviours read these (`projectile`).
    Chain,
    Pierce,
    Fork,
    ProjectileSpeed,
    /// T the target is pushed back (`motion::knockback_distance`).
    Knockback,
    /// Extra repeats of a strike or a cast.
    Repeats,
    /// Cast and attack speed: windup and recover are divided by this.
    Speed,
    /// The windup alone, as a factor that starts at 1 (a "more" lengthens it): Cửu Biến, Chắc Trúng, Trọng, Đạo Dung
    /// Thân's basics, Nhiếp's stacks. Speed divides windup and recover alike; these touch the windup only.
    Windup,
    /// Cooldown recovery: cooldowns are divided by this.
    CooldownRecovery,
    /// Cost of a skill.
    Cost,
    /// The share of a skill's qi cost paid in life instead (`build::LIFE_PER_QI` of full life a point of qi).
    CostAsLife,
    /// The sense a skill holds while it stands (an intent's, a curse aura's), as a factor from 1.
    Reservation,
    /// Flat sense a skill holds while it stands, besides its own (Chú Vực's curse aura).
    Reserved,
    /// Uses a skill stores before its cooldown starts (`charge::Uses`).
    ExtraCooldownUse,
    /// How long a skill's own effects last (zones, buffs, curses it lays).
    SkillDuration,
    /// The size of areas.
    Area,
    /// The effect of curses this fighter lays.
    CurseEffect,
    /// The effect of intents (auras) and blessings this fighter holds.
    AuraEffect,
    /// Curses this fighter can keep laid at once, on top of the rule's (`curse::curse_limit`).
    CurseCount,
    // ---- minions and devices (`device::spawn`)
    MinionDamage,
    MinionLife,
    MinionCount,
    TrapCount,
    MineCount,
    TotemCount,
    /// The life of what a skill makes (a trap, a mine, a totem).
    DeviceLife,
    // ---- the bearer
    Life,
    Shield,
    Sense,
    /// Full qi.
    QiMax,
    /// Stars of each kind this fighter can hold.
    StarsMax,
    Might,
    Armour,
    Evasion,
    Resist(Element),
    /// The resistance cap of a kind.
    ResistCap(Element),
    AilmentResist(Ailment),
    /// Every damage this fighter takes.
    Taken,
    TakenOf(Element),
    /// Damage over time this fighter takes.
    DotTaken,
    /// Damage from blows this fighter takes (PoE's Fortify lowers it; damage over time is [`Stat::DotTaken`]).
    HitTaken,
    /// Hard controls on this fighter last this much longer (Kiên and its kin lower it).
    ControlTaken,
    LifeRegen,
    ShieldRegen,
    QiRegen,
    /// Healing received.
    Healing,
    MoveSpeed,
}

/// The stats the calculator gathers but never applies: none now. The stats whose effect needs positions and
/// bodies (run speed, areas, skill durations, projectiles, knockback, minions and devices, stored cooldown uses) are
/// read by the field behaviours (`play`, `projectile`, `motion`, `device`, `charge`) as pure functions over plain
/// geometry; the fight moves bodies and calls them. A new stat the calculator cannot read goes here, and the
/// coverage test (`tests/stat.rs`) holds every stat to one side or the other.
pub const FIGHT_READS: &[Stat] = &[];

/// How a modifier changes its stat.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ModKind {
    /// Added to the base.
    Flat,
    /// Summed with every other increase (a reduction is negative).
    Increased,
    /// Multiplied on its own (`1 + x`; less is negative).
    More,
    /// Sets the stat outright.
    Override,
}

/// Something that happened lately, for "recently" conditions. A fighter keeps the seconds since each
/// ([`Recent`]); the fight reports them with [`Recent::mark`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Happened {
    /// Trúng: landed a blow.
    Hit,
    /// Bạo kích: landed a critical strike.
    Crit,
    /// Sát: killed.
    Kill,
    /// Bị trúng: a blow landed on this fighter.
    BeenHit,
    /// Bị bạo kích.
    BeenCrit,
    /// Đỡ: a guard or a shield stopped a blow.
    Guarded,
    /// Thuấn di.
    Blinked,
    /// Used a guard skill (Hộ thể).
    UsedGuardSkill,
    /// Used a movement skill.
    UsedMovementSkill,
    /// Laid a curse.
    CursedSomeone,
    /// Khiên vỡ: its shield broke.
    ShieldBroke,
    /// Laid an ailment (stacks or an instance) or filled one.
    LaidAilment,
    /// Né: dodged a blow.
    Evaded,
    /// A curse was laid on this fighter.
    Cursed,
    /// One of this fighter's minions died.
    MinionDied,
    /// Life crossed a low-life line going down (either line).
    LowLife,
    /// Hồi tố: rewound in time.
    Rewound,
    /// This fighter's domain collapsed.
    DomainCollapsed,
    /// Took damage over time (a burn or a bleed tick).
    BeenDotted,
    /// Laid a hard control (a full ailment's stun, root, knockdown...).
    LaidControl,
}

impl Happened {
    /// How many there are.
    pub const COUNT: usize = 20;

    /// Every one, in [`Recent`]'s order.
    pub const ALL: [Happened; Happened::COUNT] = [
        Happened::Hit,
        Happened::Crit,
        Happened::Kill,
        Happened::BeenHit,
        Happened::BeenCrit,
        Happened::Guarded,
        Happened::Blinked,
        Happened::UsedGuardSkill,
        Happened::UsedMovementSkill,
        Happened::CursedSomeone,
        Happened::ShieldBroke,
        Happened::LaidAilment,
        Happened::Evaded,
        Happened::Cursed,
        Happened::MinionDied,
        Happened::LowLife,
        Happened::Rewound,
        Happened::DomainCollapsed,
        Happened::BeenDotted,
        Happened::LaidControl,
    ];
}

/// Seconds since each [`Happened`], `INFINITY` for never.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Recent(pub [f64; Happened::COUNT]);

impl Default for Recent {
    fn default() -> Self {
        Recent([f64::INFINITY; Happened::COUNT])
    }
}

impl Recent {
    pub fn mark(&mut self, h: Happened) {
        self.0[h as usize] = 0.0;
    }

    pub fn since(&self, h: Happened) -> f64 {
        self.0[h as usize]
    }

    pub fn tick(&mut self, dt: f64) {
        for t in &mut self.0 {
            *t += dt;
        }
    }
}

/// When a modifier holds. Data, so a source carries its conditions with it and a bot or a test can read them.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Condition {
    Always,
    /// The target carries at least one curse.
    TargetCursed,
    /// The target carries at least this many curses ([`Ctx::curses`]).
    TargetCurses(u8),
    /// The target carries at least `n` stacks of the ailment (instances, for burn and bleed).
    TargetHas(Ailment, f64),
    /// The target carries any damaging ailment (bleed, burn, wither).
    TargetAiling,
    /// The target carries at least this many different ailments.
    TargetDistinctAilments(u8),
    /// The target's life is at or below this share.
    TargetLowLife(f64),
    /// The target's life is full.
    TargetFullLife,
    /// The target's root is this element.
    TargetRoot(Element),
    /// The target's shield is gone.
    TargetShieldDown,
    /// The target moves ([`Ctx::other_moving`]).
    TargetMoving,
    /// The target has a soul: not a soulless body (a puppet, a fire spirit), not thất hồn now.
    TargetHasSoul,
    /// The target carries a timed effect from this source (`cultivator::Active`).
    TargetHasEffect(Source),
    /// This fighter's life is at or below this share.
    SelfLowLife(f64),
    /// This fighter's life is full.
    SelfFullLife,
    /// This fighter's shield is up (above zero).
    SelfShieldUp,
    /// This fighter carries at least `n` stacks of the ailment (instances, for burn and bleed).
    SelfHas(Ailment, f64),
    /// This fighter carries a timed effect from this source.
    SelfHasEffect(Source),
    /// This fighter's qi is at or above this share of its full qi.
    SelfQiAbove(f64),
    /// At least `n` stars of a kind.
    Stars(Star, f64),
    /// Killing intent at least this.
    Intent(f64),
    /// The thing happened within this many seconds.
    Recently(Happened, f64),
    /// At least this much qi spent recently ([`Counts::qi_spent`]).
    QiSpent(f64),
    /// This use of the skill is an `n`-th one: the 3rd, 6th, 9th... for 3 ([`Ctx::use_count`]).
    EveryNth(u32),
    /// Within this many T of the target, or beyond it.
    Near(f64),
    Far(f64),
    Moving,
    Still,
    /// The skill carries this tag (a mod limited to one skill type, on a source that is not a support).
    SkillHas(Tag),
    Not(&'static Condition),
    All(&'static [Condition]),
    Any(&'static [Condition]),
}

/// What happened lately, counted by the fight over its last [`RECENT`] seconds (the "recently" of a [`Per`] or a
/// [`Condition`] that counts rather than times).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Counts {
    /// Kills.
    pub kills: u32,
    /// Blows landed.
    pub hits: u32,
    /// Qi spent.
    pub qi_spent: f64,
    /// The kinds of damage dealt, a bit per `Element::index`.
    pub kinds: u16,
}

/// The window [`Counts`] covers, in seconds (PoE's "recently").
pub const RECENT: f64 = 4.0;

/// What a modifier's value counts: it applies once for each (PoE's "x per y"), then [`Mod::max`] caps the sum.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Per {
    /// Once: a plain modifier.
    #[default]
    One,
    /// Each stack of the ailment on the target (each instance, for burn and bleed).
    Stacks(Ailment),
    /// Each different ailment on the target.
    DistinctAilments,
    /// Each curse on the target ([`Ctx::curses`]).
    Curses,
    /// Each star of a kind this fighter holds.
    Stars(Star),
    /// Every this much killing intent (whole steps).
    Intent(f64),
    /// Each whole 1% of its own life missing.
    MissingLife,
    /// Each whole 1% of the target's life missing.
    TargetMissingLife,
    /// Each soul this fighter holds.
    Souls,
    /// Every this many T between this fighter and the target (whole steps; none when the distance is not known).
    Distance(f64),
    /// Each kill, and each blow landed, recently ([`Counts`]).
    Kills,
    Hits,
    /// Every this much qi spent recently (whole steps).
    QiSpent(f64),
    /// Each kind of damage dealt recently.
    KindsDealt,
}

/// Whole steps of `n` in `x`: 0 for a step that is not above zero or an amount that is not finite.
fn steps(x: f64, n: f64) -> f64 {
    if n > 0.0 && x.is_finite() {
        libm::floor(x.max(0.0) / n)
    } else {
        0.0
    }
}

/// Whole percent of life missing.
fn missing(c: &Cultivator) -> f64 {
    libm::floor((1.0 - c.life / c.life_max.max(1.0)).clamp(0.0, 1.0) * 100.0 + 1e-9)
}

/// How many ailments `c` carries (stacks or instances above zero).
pub fn distinct_ailments(c: &Cultivator) -> u8 {
    Ailment::ALL
        .iter()
        .filter(|&&a| c.ailments.stacks(a) > 0.0)
        .count() as u8
}

impl Per {
    /// How many times a modifier counts in `c`.
    pub fn count(self, c: &Ctx) -> f64 {
        match self {
            Per::One => 1.0,
            Per::Stacks(a) => c.other.map_or(0.0, |o| o.ailments.stacks(a)),
            Per::DistinctAilments => c.other.map_or(0.0, |o| f64::from(distinct_ailments(o))),
            Per::Curses => f64::from(c.curses),
            Per::Stars(s) => c.me.stars[s as usize],
            Per::Intent(n) => steps(c.me.intent, n),
            Per::MissingLife => missing(c.me),
            Per::TargetMissingLife => c.other.map_or(0.0, missing),
            Per::Souls => c.me.souls,
            Per::Distance(n) => steps(c.distance, n),
            Per::Kills => f64::from(c.counts.kills),
            Per::Hits => f64::from(c.counts.hits),
            Per::QiSpent(n) => steps(c.counts.qi_spent, n),
            Per::KindsDealt => f64::from(c.counts.kinds.count_ones()),
        }
    }
}

/// What a condition is read against: the fighter whose stats are gathered, the other side if there is one, how far
/// apart they are, whether each moves, the skill's tags, which use of the skill this is and what happened lately.
#[derive(Clone, Copy, Debug)]
pub struct Ctx<'a> {
    pub me: &'a Cultivator,
    pub other: Option<&'a Cultivator>,
    /// Whether the other side carries a curse (curses live in `curse::Curses`, outside the cultivator).
    pub other_cursed: bool,
    /// How many curses the other side carries (at least 1 when `other_cursed`, for a caller that only knows that).
    pub curses: u8,
    pub distance: f64,
    pub moving: bool,
    /// Whether the other side moves.
    pub other_moving: bool,
    pub skill: Tags,
    /// Which use of the skill this is, counted by the fight from 1 (0 when not known).
    pub use_count: u32,
    /// What happened lately ([`Counts`]).
    pub counts: Counts,
}

impl<'a> Ctx<'a> {
    /// A fighter on its own: no target, no skill.
    pub fn alone(me: &'a Cultivator) -> Ctx<'a> {
        Ctx {
            me,
            other: None,
            other_cursed: false,
            curses: 0,
            distance: f64::INFINITY,
            moving: false,
            other_moving: false,
            skill: Tags::NONE,
            use_count: 0,
            counts: Counts::default(),
        }
    }
}

const DAMAGING: [Ailment; 3] = [Ailment::Bleed, Ailment::Burn, Ailment::Wither];

impl Condition {
    pub fn holds(&self, c: &Ctx) -> bool {
        let life = |x: &Cultivator| x.life / x.life_max.max(1.0);
        let has = |x: &Cultivator, s: Source| x.effects.iter().any(|e| e.source == s);
        match *self {
            Condition::Always => true,
            Condition::TargetCursed => c.other_cursed || c.curses > 0,
            Condition::TargetCurses(n) => c.curses.max(u8::from(c.other_cursed)) >= n,
            Condition::TargetHas(a, n) => c.other.is_some_and(|o| o.ailments.stacks(a) >= n),
            Condition::TargetAiling => c
                .other
                .is_some_and(|o| DAMAGING.iter().any(|&a| o.ailments.stacks(a) > 0.0)),
            Condition::TargetDistinctAilments(n) => {
                c.other.is_some_and(|o| distinct_ailments(o) >= n)
            }
            Condition::TargetLowLife(s) => c.other.is_some_and(|o| life(o) <= s),
            Condition::TargetFullLife => c.other.is_some_and(|o| o.life >= o.life_max),
            Condition::TargetRoot(e) => c.other.is_some_and(|o| o.root == Some(e)),
            Condition::TargetShieldDown => c.other.is_some_and(|o| o.shield <= 0.0),
            Condition::TargetMoving => c.other_moving,
            Condition::TargetHasSoul => c
                .other
                .is_some_and(|o| !o.soulless_body && o.soul_lost <= 0.0),
            Condition::TargetHasEffect(s) => c.other.is_some_and(|o| has(o, s)),
            Condition::SelfLowLife(s) => life(c.me) <= s,
            Condition::SelfFullLife => c.me.life >= c.me.life_max,
            Condition::SelfShieldUp => c.me.shield > 0.0,
            Condition::SelfHas(a, n) => c.me.ailments.stacks(a) >= n,
            Condition::SelfHasEffect(s) => has(c.me, s),
            Condition::SelfQiAbove(s) => c.me.qi >= s * c.me.qi_max,
            Condition::Stars(s, n) => c.me.stars[s as usize] >= n,
            Condition::Intent(n) => c.me.intent >= n,
            Condition::Recently(h, secs) => c.me.recent.since(h) <= secs,
            Condition::QiSpent(n) => c.counts.qi_spent >= n,
            Condition::EveryNth(n) => n > 0 && c.use_count > 0 && c.use_count.is_multiple_of(n),
            Condition::Near(t) => c.distance <= t,
            Condition::Far(t) => c.distance > t,
            Condition::Moving => c.moving,
            Condition::Still => !c.moving,
            Condition::SkillHas(t) => c.skill.has(t),
            Condition::Not(inner) => !inner.holds(c),
            Condition::All(cs) => cs.iter().all(|x| x.holds(c)),
            Condition::Any(cs) => cs.iter().any(|x| x.holds(c)),
        }
    }
}

/// Where a modifier comes from, for a bot, a tooltip or a breakdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Source {
    Base,
    Support(&'static str),
    Keystone(&'static str),
    Aura(&'static str),
    Blessing(&'static str),
    Curse(&'static str),
    Ailment(Ailment),
    Star(Star),
    Domain(&'static str),
    Skill(&'static str),
    /// An item worn in a slot (`item`).
    Item(crate::item::ItemSlot),
    /// A node of the passive tree (`passive`).
    Passive(u32),
    Root,
    Other(&'static str),
}

/// One modifier: `value` to `stat` by `kind`, for skills carrying every tag in `need` (none for all), while
/// `when` holds; `value` counts once for each `per` (PoE's "x per y"), the sum at most `max` in size.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Mod {
    pub stat: Stat,
    pub kind: ModKind,
    pub value: f64,
    pub need: &'static [Tag],
    pub when: Condition,
    pub source: Source,
    pub per: Per,
    /// The scaled value at most this in size (its sign kept); `None` for no cap.
    pub max: Option<f64>,
}

impl Mod {
    /// A modifier with no tag and no condition.
    pub const fn new(stat: Stat, kind: ModKind, value: f64, source: Source) -> Mod {
        Mod {
            stat,
            kind,
            value,
            need: &[],
            when: Condition::Always,
            source,
            per: Per::One,
            max: None,
        }
    }

    pub const fn inc(stat: Stat, value: f64, source: Source) -> Mod {
        Mod::new(stat, ModKind::Increased, value, source)
    }

    pub const fn more(stat: Stat, value: f64, source: Source) -> Mod {
        Mod::new(stat, ModKind::More, value, source)
    }

    pub const fn flat(stat: Stat, value: f64, source: Source) -> Mod {
        Mod::new(stat, ModKind::Flat, value, source)
    }

    pub const fn needs(mut self, need: &'static [Tag]) -> Mod {
        self.need = need;
        self
    }

    pub const fn when(mut self, when: Condition) -> Mod {
        self.when = when;
        self
    }

    /// Counts once for each `per`.
    pub const fn per(mut self, per: Per) -> Mod {
        self.per = per;
        self
    }

    /// Its scaled value at most `cap` in size.
    pub const fn max(mut self, cap: f64) -> Mod {
        self.max = Some(cap);
        self
    }

    /// Its value in `c`: `value × per.count(c)`, held to `max` in size. An override is never scaled.
    pub fn value_in(&self, c: &Ctx) -> f64 {
        if self.kind == ModKind::Override {
            return self.value;
        }
        let v = self.value * self.per.count(c);
        match self.max {
            Some(cap) => v.clamp(-cap.abs(), cap.abs()),
            None => v,
        }
    }

    /// Whether it applies: its tags on the skill, and its condition.
    pub fn applies(&self, c: &Ctx) -> bool {
        c.skill.all(self.need) && self.when.holds(c)
    }

    /// The same modifier scaled (a curse spread at half, an aura's effect raised).
    pub fn scaled(mut self, k: f64) -> Mod {
        if self.kind != ModKind::Override {
            self.value *= k;
        }
        self
    }
}

/// One stat's sums.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Acc {
    pub flat: f64,
    pub increased: f64,
    pub more: f64,
    pub over: Option<f64>,
}

impl Default for Acc {
    fn default() -> Self {
        Acc {
            flat: 0.0,
            increased: 0.0,
            more: 1.0,
            over: None,
        }
    }
}

impl Acc {
    fn add(&mut self, m: &Mod) {
        match m.kind {
            ModKind::Flat => self.flat += m.value,
            ModKind::Increased => self.increased += m.value,
            ModKind::More => self.more *= 1.0 + m.value,
            ModKind::Override => self.over = Some(m.value),
        }
    }

    /// `(base + flat) × (1 + increased) × more`, never below zero from the increases; an override wins.
    pub fn apply(&self, base: f64) -> f64 {
        if let Some(v) = self.over {
            return v;
        }
        (base + self.flat) * (1.0 + self.increased).max(0.0) * self.more
    }

    /// The factor alone (`(1 + increased) × more`), for a base that is not known here.
    pub fn factor(&self) -> f64 {
        if let Some(v) = self.over {
            return v;
        }
        (1.0 + self.increased).max(0.0) * self.more
    }
}

/// Every stat's sums after gathering, and which modifiers made them.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Table {
    accs: Vec<(Stat, Acc)>,
    pub used: Vec<Mod>,
}

impl Table {
    /// Gathers the modifiers that apply in `ctx`, in the order given, each at its value in `ctx`
    /// ([`Mod::value_in`]); `used` keeps them so scaled.
    pub fn gather<'m>(mods: impl IntoIterator<Item = &'m Mod>, ctx: &Ctx) -> Table {
        let mut t = Table::default();
        for m in mods {
            if m.applies(ctx) {
                let mut m = *m;
                m.value = m.value_in(ctx);
                t.acc_mut(m.stat).add(&m);
                t.used.push(m);
            }
        }
        t
    }

    fn acc_mut(&mut self, s: Stat) -> &mut Acc {
        let i = match self.accs.iter().position(|(k, _)| *k == s) {
            Some(i) => i,
            None => {
                self.accs.push((s, Acc::default()));
                self.accs.len() - 1
            }
        };
        &mut self.accs[i].1
    }

    /// The sums of a stat (empty sums if nothing touched it).
    pub fn acc(&self, s: Stat) -> Acc {
        self.accs
            .iter()
            .find(|(k, _)| *k == s)
            .map_or(Acc::default(), |(_, a)| *a)
    }

    /// A stat from its base.
    pub fn get(&self, s: Stat, base: f64) -> f64 {
        self.acc(s).apply(base)
    }

    /// The flat sum of a stat (counts, added damage, shares).
    pub fn flat(&self, s: Stat) -> f64 {
        self.acc(s).flat
    }

    pub fn increased(&self, s: Stat) -> f64 {
        self.acc(s).increased
    }

    pub fn more(&self, s: Stat) -> f64 {
        self.acc(s).more
    }

    /// The increase and more of several stats together (`Damage` + `HitDamage` + `DamageOf(e)`...): increases sum
    /// across them, mores multiply.
    pub fn combined(&self, stats: &[Stat]) -> Acc {
        let mut a = Acc::default();
        for &s in stats {
            let b = self.acc(s);
            a.flat += b.flat;
            a.increased += b.increased;
            a.more *= b.more;
            if b.over.is_some() {
                a.over = b.over;
            }
        }
        a
    }

    /// The per-kind increases and mores a hit carries (step 4 reads them for every kind a part has been).
    pub fn by_kind(&self) -> ([f64; N], [f64; N]) {
        let mut inc = [0.0; N];
        let mut more = [0.0; N];
        for e in Element::ALL {
            let a = self.acc(Stat::DamageOf(e));
            inc[e.index()] = a.increased;
            more[e.index()] = a.more - 1.0;
        }
        (inc, more)
    }

    /// Ailment effect and duration factors, per ailment. The ailments that deal no damage (all but burn and bleed)
    /// take [`Stat::ControlEffect`] in their effect too.
    pub fn ailment_factors(&self) -> ([f64; AILMENTS], [f64; AILMENTS]) {
        let mut effect = [1.0; AILMENTS];
        let mut duration = [1.0; AILMENTS];
        for a in Ailment::ALL {
            effect[a.index()] = if crate::dot::is_dot(a) {
                self.combined(&[Stat::AilmentEffect, Stat::AilmentEffectOf(a)])
            } else {
                self.combined(&[
                    Stat::AilmentEffect,
                    Stat::AilmentEffectOf(a),
                    Stat::ControlEffect,
                ])
            }
            .factor();
            duration[a.index()] = self
                .combined(&[Stat::AilmentDuration, Stat::AilmentDurationOf(a)])
                .factor();
        }
        (effect, duration)
    }
}

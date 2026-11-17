//! The shared pool: every skill (thần thông) any fighter may take, with its tags, kinds of damage, rank, base, costs
//! and timings (design, sections 3 and 11). No class owns a skill. `spell` names the `data::SPELLS` entry of the duel (`arena-duel`) a skill
//! borrows its motion and effect from until it has its own.

use crate::ailment::{Control, ForceAilment};
use crate::aura::AuraKind;
use crate::cultivator::Star;
use crate::curse;
use crate::domain::DomainKind;
use crate::element::Element;
use crate::grade::Rank;
use crate::tag::{Tag, Tags};

/// A resource a skill spends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Resource {
    Qi,
    /// Máu: paid at the cast, never the last point; each 1% of full life paid adds `BLOOD_POWER`.
    Life,
    /// Thọ nguyên: never regrows in a fight.
    Lifespan,
    /// Sát khí.
    Intent,
    /// Hồn.
    Souls,
    /// Thần thức.
    Sense,
    /// Tử khí: what a life-death cultivator pays in place of life below 10%.
    DeathQi,
    /// Tinh điểm of a kind.
    Star(Star),
}

/// A skill's tier, from measurement (design, 12.3); it sets the loadout points the skill costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tier {
    C,
    B,
    A,
    S,
}

impl Tier {
    /// Loadout points (design, 2).
    pub fn points(self) -> u32 {
        match self {
            Tier::C => 3,
            Tier::B => 4,
            Tier::A => 5,
            Tier::S => 6,
        }
    }
}

/// What a skill does besides its damage. The fight reads these; the calculator reads only what changes a blow.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Effect {
    /// A hard control of this many seconds (through grit).
    Control(Control, f64),
    /// Extra stacks of the ailment its kind lays.
    Ailment(f64),
    /// The base lands this many times a cast: a chain's blows, a volley's arrows, a field's pulses (a field's
    /// first blow and its ticks, from the `data::SPELLS` entry it borrows, counted as so many full blows). The
    /// starting budget (`power_score`) multiplies by it.
    Hits(f64),
    /// Heals this share of full life (a `data::SPELLS` heal's `amount`, which is in percent of life).
    Heal(f64),
    /// Lowers the target's resistance to one kind of damage by this share (Khắc Ấn, as PoE's Exposure).
    Expose(f64),
}

/// A skill in the shared pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gem {
    /// Its code name (a `data::SPELLS` name where the skill already exists).
    pub id: &'static str,
    /// Its name in the novel.
    pub name: &'static str,
    pub tags: &'static [Tag],
    /// Its damage by kind, shares summing to 1 (empty for a skill that deals none).
    pub split: &'static [(Element, f64)],
    /// The rank it starts at; `restriction` puts it outside the ladder.
    pub rank: Rank,
    pub restriction: bool,
    /// A spell's base at level 1, or an attack's effectiveness (a share of might).
    pub base: f64,
    pub costs: &'static [(Resource, f64)],
    pub cooldown: f64,
    pub windup: f64,
    pub recover: f64,
    /// Uses a match (lá bài một lần), and whether it can be shown without casting (Lộ bài).
    pub uses: Option<u8>,
    pub bluff: bool,
    pub force: ForceAilment,
    pub tier: Tier,
    /// The `data::SPELLS` entry it borrows its motion and effect from.
    pub spell: &'static str,
    /// Where it comes from: `TN<chapter>` or `CM<chapter>`, or "arena" for the arena's own.
    pub source: &'static str,
    pub effects: &'static [Effect],
}

/// The roles of a skill that tends its caster or works on its foe rather than strikes, as PoE's: it never waits for
/// a blow to play out, nor holds its caster (`Gem::binds`).
pub const UNBOUND: &[Tag] = &[
    Tag::Curse,
    Tag::Aura,
    Tag::Blessing,
    Tag::Movement,
    Tag::Guard,
    Tag::Recovery,
    Tag::Stance,
];

impl Gem {
    pub fn tags(&self) -> Tags {
        Tags::of(self.tags)
    }

    pub fn is_attack(&self) -> bool {
        self.tags().has(Tag::Attack)
    }

    /// Whether a cast of it holds its caster until it plays out: it deals damage and is no curse, aura, blessing,
    /// movement, guard, recovery or stance (`UNBOUND`). The duel casts no other such skill meanwhile.
    pub fn binds(&self) -> bool {
        let tags = self.tags();
        !self.split.is_empty() && !UNBOUND.iter().any(|&t| tags.has(t))
    }

    /// The tags including those of its kinds of damage.
    pub fn all_tags(&self) -> Tags {
        self.split
            .iter()
            .fold(self.tags(), |t, &(e, _)| t.with(Tag::of(e)))
    }
}

impl Gem {
    /// How many times its base lands a cast (`Effect::Hits`, else 1).
    pub fn hits(&self) -> f64 {
        self.effects
            .iter()
            .find_map(|e| match e {
                Effect::Hits(n) => Some(*n),
                _ => None,
            })
            .unwrap_or(1.0)
    }

    /// Whether it deals damage at all (a split; non-damaging Body skills and pure curses have none).
    pub fn deals_damage(&self) -> bool {
        !self.split.is_empty() && self.base > 0.0
    }

    /// Whether it lays a hard control of its own.
    pub fn controls(&self) -> bool {
        self.effects
            .iter()
            .any(|e| matches!(e, Effect::Control(..)))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The base: from today's damage power to a level-1 base.
//
// Today (`status::damage`) a blow deals `atk × power / 10 × DEFENCE / (DEFENCE + def)`. Against today's numbers
// (the two schools' mean attack, (345 + 520) / 2 = 432.5, and mean magic defence, (110 + 165) / 2 = 137.5, so
// 500 / 637.5 ≈ 0.784) one point of power is ≈ 33.9 damage. A spell's base grows 1.07 a level, so its level-10 base
// is `base(1) × 1.07⁹ ≈ base(1) × 1.838`; a spell's level-1 base is therefore `power × 33.9 / 1.838 ≈ power × 18.45`
// (`spell_base_of`), and a level-10 cast deals today's damage before resistances. An attack's effectiveness is
// `power / 10` (`effectiveness_of`): might 400 × power / 10 is today's `atk × power / 10` at a might close to today's
// mean attack (before armour, which the new system applies to Lực alone).
// ---------------------------------------------------------------------------------------------------------------

/// Today's mean attack, the two schools' `atk` (345 and 520).
pub const TODAY_ATK: f64 = 432.5;
/// Today's mean magic defence factor: the old `DEFENCE / (DEFENCE + mdef)` (`DEFENCE` 500) at the schools' mean
/// `mdef` (110 and 165).
pub const TODAY_DEFENCE: f64 = 500.0 / (500.0 + 137.5);
/// The level (hỏa hầu) at which a spell's base deals today's damage.
pub const BASE_LEVEL: u8 = 10;

/// `1.07^(level − 1)`, as `hit::spell_base` grows a base (by repeated products, so it can be a constant).
pub const fn growth(level: u8) -> f64 {
    let mut g = 1.0;
    let mut l = 1;
    while l < level {
        g *= 1.07;
        l += 1;
    }
    g
}

/// A spell's level-1 base from today's damage power: a level-`BASE_LEVEL` cast deals today's damage.
pub const fn spell_base_of(power: f64) -> f64 {
    power / 10.0 * TODAY_ATK * TODAY_DEFENCE / growth(BASE_LEVEL)
}

/// An attack's effectiveness (its share of might) from today's damage power.
pub const fn effectiveness_of(power: f64) -> f64 {
    power / 10.0
}

// ---------------------------------------------------------------------------------------------------------------
// The starting tier (design, 12.1), before measurement replaces it (12.3).
// ---------------------------------------------------------------------------------------------------------------

/// The reach band's damage budget factor (design, 7): near 1.30, mid 1.00, far 0.80, whole arena 0.65.
pub fn reach_factor(tags: Tags) -> f64 {
    if tags.has(Tag::Near) {
        1.3
    } else if tags.has(Tag::Far) {
        0.8
    } else if tags.has(Tag::Whole) {
        0.65
    } else {
        1.0
    }
}

/// The skills that recover longer when their blow is guarded (the duel's `FRAMES.unsafe_on_block`): the dive, the
/// dashes and the close blasts.
pub const UNSAFE_ON_BLOCK: &[&str] = &[
    "dive",
    "step",
    "palm",
    "burst",
    "lotus",
    "ancientgod",
    "slashluo",
    "onestep",
    "nineturn",
    "sevencolor",
    "heavenlychop",
    "fireescape",
    "warprint",
    "daofusion",
];

/// The safety factor (design, 12.1): how much safer than a plain blow a skill is to throw. Instant 1.25 (no dodge),
/// area 1.10 (no guard), unsafe on block 0.85 (`UNSAFE_ON_BLOCK`); the factors that apply multiply.
/// A safer skill scores higher for the same damage.
pub fn safety_factor(g: &Gem) -> f64 {
    let tags = g.tags();
    let mut s = 1.0;
    if tags.has(Tag::Instant) {
        s *= 1.25;
    }
    if tags.has(Tag::Area) {
        s *= 1.1;
    }
    if UNSAFE_ON_BLOCK.contains(&g.id) {
        s *= 0.85;
    }
    s
}

/// The damage one use is expected to deal against a fighter with no defences: a spell at `BASE_LEVEL`, an attack at
/// the base might (`cultivator::BASE.might`), times its hits. Ailments are not priced in (every damaging skill lays
/// them in proportion to its damage). Zero for a skill that deals none.
pub fn expected_damage(g: &Gem) -> f64 {
    if !g.deals_damage() {
        return 0.0;
    }
    let one = if g.is_attack() {
        crate::hit::attack_base(crate::cultivator::BASE.might, g.base)
    } else {
        crate::hit::spell_base(g.base, BASE_LEVEL)
    };
    one * g.hits()
}

/// The power score of design 12.1:
/// `expected damage / (windup + recover + 0.25 × cooldown) / reach factor × safety factor`. The safety factor
/// multiplies (an unavoidable or unguardable skill is worth more, an unsafe one less); the design first divided by
/// it, which rated them the wrong way round.
pub fn power_score(g: &Gem) -> f64 {
    let time = g.windup + g.recover + 0.25 * g.cooldown;
    if time <= 0.0 {
        return 0.0;
    }
    expected_damage(g) / time / reach_factor(g.tags()) * safety_factor(g)
}

/// The score bands: below `C_BELOW` is C, below `B_BELOW` B, below `A_BELOW` A, and S above. Chosen on the pool's
/// scores with the safety factor multiplying (C 18, B 42, A 39, S 18 of the 117 damaging skills: about 15/35/35/15, as
/// design 12.3 splits the measured tiers).
pub const C_BELOW: f64 = 295.0;
pub const B_BELOW: f64 = 500.0;
pub const A_BELOW: f64 = 700.0;

/// The tier of a power score.
pub fn band(score: f64) -> Tier {
    if score < C_BELOW {
        Tier::C
    } else if score < B_BELOW {
        Tier::B
    } else if score < A_BELOW {
        Tier::A
    } else {
        Tier::S
    }
}

/// One tier up, S staying S.
pub fn up(t: Tier) -> Tier {
    match t {
        Tier::C => Tier::B,
        Tier::B => Tier::A,
        Tier::A | Tier::S => Tier::S,
    }
}

/// A skill's starting tier (design, 12.1). A damaging skill takes its score's `band`, one step up if it lays a hard
/// control of its own or is a one-use card (what it does beyond damage). A skill that deals none is placed by role:
/// aura, field, stance, trigger, blessing and one-use cards A; guard and recovery B; movement C; anything else B.
pub fn starting_tier(g: &Gem) -> Tier {
    let tags = g.tags();
    if g.deals_damage() {
        let t = band(power_score(g));
        return if g.controls() || g.uses.is_some() {
            up(t)
        } else {
            t
        };
    }
    if tags.any(&[
        Tag::Aura,
        Tag::Field,
        Tag::Stance,
        Tag::Trigger,
        Tag::Blessing,
        Tag::Vaal,
    ]) {
        Tier::A
    } else if tags.has(Tag::Movement) {
        Tier::C
    } else {
        Tier::B
    }
}

/// Every skill in the pool, basics and the top-tier (đỉnh cấp) ones included (design, 11, with the research's techniques that section
/// leaves out). The removed ice skills (`icicle`, `shard`, `shatter`, `frostfire`, `flurry`, `prison`) are not here,
/// and nothing chills or freezes: what chilled deals Water (Soak); a freeze became nothing, or a Hold only where the
/// novel holds the body (Định Thân, Đạo Cổ Tứ Thức's stone).
///
/// - `base`: a spell's `spell_base_of(power)`, an attack's `effectiveness_of(power)`, from its `data::SPELLS`
///   damage (a new skill from the skill it borrows, scaled by what the research says it does).
/// - Timings and qi from `data::SPELLS` (a new skill: those of the spell it borrows, unless the research says
///   otherwise). Life is in percent of full life, lifespan, sense, intent and souls in their own points.
/// - `tier` is `starting_tier` (design 12.1) until measurement (12.3) replaces it.
/// - `rank` is the research's "phẩm khởi", Art (1) where it gives none.
pub const GEMS: &[Gem] = pool::ALL;

/// The pool's rows, with tags, kinds and resources in scope.
mod pool {
    use super::{Effect, Gem, Resource as R, Tier, effectiveness_of, spell_base_of};
    use crate::ailment::Control as Ctl;
    use crate::ailment::ForceAilment;
    use crate::cultivator::Star;
    use crate::element::Element as E;
    use crate::grade::Rank;
    use crate::tag::Tag::{self, *};

    /// Fist, palm and slam shake (Chấn); everything else cuts (Thương).
    const fn force_of(tags: &[Tag]) -> ForceAilment {
        let mut i = 0;
        while i < tags.len() {
            if matches!(tags[i], Fist | Palm | Slam) {
                return ForceAilment::Daze;
            }
            i += 1;
        }
        ForceAilment::Bleed
    }

    /// Timings as `data::SPELLS` lists them: windup, recover, cooldown.
    type Times = [f64; 3];

    #[allow(clippy::too_many_arguments)]
    const fn row(
        id: &'static str,
        name: &'static str,
        tags: &'static [Tag],
        split: &'static [(E, f64)],
        base: f64,
        costs: &'static [(R, f64)],
        t: Times,
        spell: &'static str,
        source: &'static str,
        tier: Tier,
    ) -> Gem {
        Gem {
            id,
            name,
            tags,
            split,
            rank: Rank::Art,
            restriction: false,
            base,
            costs,
            cooldown: t[2],
            windup: t[0],
            recover: t[1],
            uses: None,
            bluff: false,
            force: force_of(tags),
            tier,
            spell,
            source,
            effects: &[],
        }
    }

    /// A spell (Pháp) from today's damage power.
    #[allow(clippy::too_many_arguments)]
    const fn magic(
        id: &'static str,
        name: &'static str,
        tags: &'static [Tag],
        split: &'static [(E, f64)],
        power: f64,
        costs: &'static [(R, f64)],
        t: Times,
        spell: &'static str,
        source: &'static str,
        tier: Tier,
    ) -> Gem {
        row(
            id,
            name,
            tags,
            split,
            spell_base_of(power),
            costs,
            t,
            spell,
            source,
            tier,
        )
    }

    /// An attack (Đòn) from today's damage power.
    #[allow(clippy::too_many_arguments)]
    const fn attack(
        id: &'static str,
        name: &'static str,
        tags: &'static [Tag],
        split: &'static [(E, f64)],
        power: f64,
        costs: &'static [(R, f64)],
        t: Times,
        spell: &'static str,
        source: &'static str,
        tier: Tier,
    ) -> Gem {
        row(
            id,
            name,
            tags,
            split,
            effectiveness_of(power),
            costs,
            t,
            spell,
            source,
            tier,
        )
    }

    /// A body skill (Thân): no damage.
    #[allow(clippy::too_many_arguments)]
    const fn body(
        id: &'static str,
        name: &'static str,
        tags: &'static [Tag],
        costs: &'static [(R, f64)],
        t: Times,
        spell: &'static str,
        source: &'static str,
        tier: Tier,
    ) -> Gem {
        row(id, name, tags, &[], 0.0, costs, t, spell, source, tier)
    }

    const fn rank(g: Gem, r: Rank) -> Gem {
        Gem { rank: r, ..g }
    }

    const fn with(g: Gem, effects: &'static [Effect]) -> Gem {
        Gem { effects, ..g }
    }

    /// A cấm chế: outside the rank ladder.
    const fn ban(g: Gem) -> Gem {
        Gem {
            restriction: true,
            ..g
        }
    }

    /// A one-use card (lá bài): `n` uses a match, and it can be shown without casting (Lộ bài).
    const fn card(g: Gem, n: u8) -> Gem {
        Gem {
            uses: Some(n),
            bluff: true,
            ..g
        }
    }

    use Tier::{A, B, C, S};

    const DIVINE: Rank = Rank::Divine;
    const RULE: Rank = Rank::Rule;
    const ORIGIN: Rank = Rank::Origin;
    const FAITH: Rank = Rank::Faith;

    /// One row a skill, kept on one line each so the pool reads as a table.
    #[rustfmt::skip]
    pub const ALL: &[Gem] = &[
        // ---- 11.1 basics: always held, no points ----
        attack("slash", "Chém", &[Attack, Sword, Near, Melee, Strike], &[(E::Force, 1.0)], 5.0, &[], [0.2, 0.32, 0.7], "slash", "arena", C),
        attack("dive", "Lao Kiếm", &[Attack, Sword, Near, Melee, Dash], &[(E::Force, 1.0)], 11.0, &[(R::Qi, 12.0)], [0.22, 0.5, 1.6], "dive", "arena", C),
        body("blink", "Độn", &[Body, Escape, Mid, Movement, Space], &[(R::Qi, 20.0)], [0.06, 0.12, 1.8], "blink", "arena", C),
        // ---- 11.2 close attacks: strike, slam, dash ----
        attack("step", "Bộ", &[Attack, Escape, Mid, Dash], &[(E::Force, 1.0)], 15.0, &[(R::Qi, 24.0)], [0.26, 0.45, 3.4], "step", "arena", B),
        attack("palm", "Chưởng", &[Attack, Palm, Near, Melee, Slam], &[(E::Force, 1.0)], 20.0, &[(R::Qi, 27.0)], [0.32, 0.5, 2.8], "palm", "arena", B),
        attack("burst", "Bạo", &[Attack, Palm, Near, Nova], &[(E::Thunder, 1.0)], 24.0, &[(R::Qi, 30.0)], [0.36, 0.5, 3.0], "burst", "arena", B),
        attack("lotus", "Liên Hoa", &[Attack, Palm, Near, Melee], &[(E::Light, 1.0)], 18.0, &[(R::Qi, 24.0)], [0.3, 0.45, 2.2], "lotus", "arena", B),
        attack("ancientgod", "Cổ Thần Huyết", &[Attack, Fist, Transform, Near, Melee, Slam, Ancient], &[(E::Force, 1.0)], 28.0, &[(R::Qi, 36.0)], [0.5, 0.55, 5.0], "ancientgod", "TN190", B),
        attack("slashluo", "Trảm La", &[Attack, Sword, Mid, Dash, Line], &[(E::Metal, 1.0)], 22.0, &[(R::Qi, 30.0)], [0.3, 0.5, 3.8], "slashluo", "TN684", B),
        attack("onestep", "Nhất Bộ Đăng Thiên", &[Attack, Escape, Mid, Dash], &[(E::Force, 1.0)], 14.0, &[(R::Qi, 30.0)], [0.2, 0.4, 4.0], "onestep", "TN", B),
        attack("nineturn", "Cửu Chuyển Luyện Tiên", &[Attack, Palm, Near, Slam, Charged, Immortal], &[(E::Force, 1.0)], 34.0, &[(R::Qi, 40.0)], [0.85, 0.5, 6.5], "nineturn", "TN493", B),
        attack("vermilion", "Chu Tước Biến", &[Attack, Transform, Palm, Near, Melee], &[(E::Fire, 1.0)], 18.0, &[(R::Qi, 30.0)], [0.35, 0.4, 3.0], "vermilion", "TN1085", B),
        attack("heavenlychop", "Thiên Trảm", &[Attack, Sword, Near, Melee, Strike], &[(E::Metal, 1.0)], 26.0, &[(R::Qi, 34.0)], [0.45, 0.5, 4.5], "heavenlychop", "TN", B),
        attack("fireescape", "Hỏa Độn", &[Attack, Escape, Mid, Dash], &[(E::Fire, 1.0)], 10.0, &[(R::Qi, 28.0)], [0.22, 0.4, 3.6], "fireescape", "TN1127", C),
        attack("warprint", "Chiến Hồn Ấn", &[Attack, Seal, Near, Melee, Slam], &[(E::Force, 0.5), (E::Metal, 0.5)], 22.0, &[(R::Qi, 32.0)], [0.4, 0.5, 4.0], "warprint", "TN", B),
        attack("sevencolor", "Thất Thải Đạo Thương", &[Attack, Sword, Mid, Dash, Line], &[(E::Light, 1.0)], 17.0, &[(R::Qi, 30.0)], [0.28, 0.45, 3.6], "sevencolor", "TN1669", B),
        attack("devour", "Phệ Linh", &[Attack, Fist, Near, Melee, Death], &[(E::Yin, 1.0)], 16.0, &[(R::Qi, 26.0)], [0.35, 0.4, 3.2], "devour", "TN885", B),
        // Each punch pays life, not qi; under 10% life it pays death qi (Sinh tử).
        rank(with(attack("hundredfist", "Trăm Quyền Sinh Cơ", &[Attack, Fist, Near, Melee, Strike, Chain, Ancient, Life, Blood], &[(E::Force, 1.0)], 9.0, &[(R::Life, 3.0)], [0.3, 0.45, 3.0], "slash", "TN1277", A), &[Effect::Hits(3.0)]), DIVINE),
        // Ten blows that stack; the Belief intent: it cannot be broken off.
        rank(with(attack("tenfist", "Thập Băng Quyền Ý", &[Attack, Fist, Intent, Near, Melee, Strike, Chain, Ancient], &[(E::Force, 1.0)], 4.0, &[(R::Qi, 34.0)], [0.4, 0.6, 6.5], "slash", "TN542", B), &[Effect::Hits(10.0)]), DIVINE),
        // Grows huge; punches on the same target build up to seven.
        rank(with(attack("manshen", "Man Thần Biến", &[Attack, Fist, Transform, Near, Melee, Strike, Chain], &[(E::Force, 1.0)], 5.0, &[(R::Qi, 40.0)], [0.4, 0.6, 7.0], "slash", "CM1203", B), &[Effect::Hits(7.0)]), RULE),
        // Rushes in and drinks deep of the foe's life.
        rank(attack("devourbody", "Thôn Linh Phệ Thể", &[Attack, Fist, Near, Dash, Single, Death], &[(E::Yin, 1.0)], 20.0, &[(R::Qi, 30.0)], [0.25, 0.45, 4.0], "onestep", "TN885", B), DIVINE),
        // A cone that cuts by the foe's remaining lifespan and gives half back.
        rank(attack("autumn", "Thu Ý", &[Attack, Sword, Near, Melee, Cone, Area, Fate], &[(E::Metal, 0.5), (E::Yin, 0.5)], 22.0, &[(R::Qi, 34.0)], [0.45, 0.5, 4.5], "heavenlychop", "CM746", B), RULE),
        // One cut that strikes the unseen too and breaks shots and zones in its way.
        rank(attack("manspear", "Man Thương", &[Attack, Sword, Mid, Dash, Line], &[(E::Force, 1.0)], 22.0, &[(R::Qi, 30.0)], [0.3, 0.5, 3.8], "slashluo", "CM238", A), DIVINE),
        // Drags the foe into a row before the palm and breaks shield after shield.
        rank(attack("godquake", "Thần Chấn", &[Attack, Palm, Mid, Slam, Cone, Ancient], &[(E::Earth, 1.0)], 22.0, &[(R::Qi, 30.0)], [0.35, 0.5, 3.2], "palm", "TN1468", A), DIVINE),
        // ---- 11.3 projectiles and instants ----
        magic("bolt", "Lôi", &[Spell, Finger, Far, Projectile], &[(E::Thunder, 1.0)], 13.0, &[(R::Qi, 18.0)], [0.28, 0.22, 1.3], "bolt", "arena", A),
        magic("orb", "Lôi Cầu", &[Spell, Talisman, Far, Projectile], &[(E::Thunder, 1.0)], 14.0, &[(R::Qi, 45.0)], [0.6, 0.3, 4.5], "orb", "arena", C),
        magic("spark", "Hoa", &[Spell, Finger, Far, Projectile], &[(E::Fire, 1.0)], 8.0, &[(R::Qi, 10.0)], [0.18, 0.18, 0.9], "spark", "arena", A),
        magic("fireball", "Hỏa Cầu", &[Spell, Talisman, Far, Projectile], &[(E::Fire, 1.0)], 12.0, &[(R::Qi, 14.0)], [0.26, 0.24, 1.5], "fireball", "TN", A),
        magic("finger", "Diệt Chỉ", &[Spell, Finger, Far, Projectile], &[(E::Metal, 1.0)], 21.0, &[(R::Qi, 28.0)], [0.4, 0.3, 3.5], "finger", "TN", A),
        magic("jithunder", "Cực Lôi", &[Spell, Far, Projectile], &[(E::Thunder, 1.0)], 15.0, &[(R::Qi, 24.0)], [0.3, 0.25, 2.0], "jithunder", "TN", A),
        // Turns the foe's immortal qi to demon qi; silences.
        rank(with(magic("demonfinger", "Hóa Ma Chỉ", &[Spell, Finger, Far, Projectile, Death], &[(E::Yin, 1.0)], 12.0, &[(R::Qi, 22.0)], [0.3, 0.25, 2.8], "demonfinger", "TN483", A), &[Effect::Control(Ctl::Soulless, 1.0)]), DIVINE),
        magic("tigerflag", "Thiên Hổ Phiên", &[Spell, Banner, Far, Projectile, Line], &[(E::Metal, 1.0)], 17.0, &[(R::Qi, 26.0)], [0.34, 0.3, 3.0], "tigerflag", "TN", A),
        magic("celestialslaughter", "Thiên Lục", &[Spell, Intent, Far, Projectile, Charged, Slaughter], &[(E::Thunder, 0.5), (E::Force, 0.5)], 26.0, &[(R::Qi, 36.0)], [0.5, 0.35, 4.5], "celestialslaughter", "TN", A),
        with(magic("spiritchange", "Hóa Linh", &[Spell, Spirit, Far, Projectile], &[(E::Soul, 1.0)], 6.0, &[(R::Qi, 20.0)], [0.28, 0.25, 3.0], "spiritchange", "TN", B), &[Effect::Control(Ctl::Soulless, 1.4)]),
        magic("thirdeye", "Thiên Nhãn", &[Spell, Far, Instant], &[(E::Light, 1.0)], 12.0, &[(R::Qi, 20.0)], [0.3, 0.25, 2.4], "thirdeye", "TN756", A),
        magic("seize", "Đoạt Cơ", &[Spell, Finger, Far, Projectile], &[(E::Yin, 1.0)], 6.0, &[(R::Qi, 18.0)], [0.25, 0.25, 3.2], "seize", "TN72", C),
        magic("voidnail", "Thất Thải Hư Không Đinh", &[Spell, Treasure, Far, Projectile], &[(E::Metal, 1.0)], 20.0, &[(R::Qi, 34.0)], [0.4, 0.3, 4.5], "voidnail", "TN1196", B),
        magic("springsoul", "Ngưng Tuyền Hồn", &[Spell, Spirit, Far, Projectile], &[(E::Soul, 1.0)], 14.0, &[(R::Qi, 26.0)], [0.4, 0.3, 3.0], "springsoul", "TN607", B),
        magic("jet", "Thủy Tiễn", &[Spell, Near, Line], &[(E::Water, 1.0)], 14.0, &[(R::Qi, 22.0)], [0.3, 0.35, 2.4], "jet", "arena", C),
        // A red beam from the eyes that strikes the sense; it costs the caster's own sense. Kills an equal rank
        // under a threshold, only stuns a higher one.
        rank(with(magic("extremesense", "Cực Cảnh Thần Thức", &[Spell, Finger, Far, Instant], &[(E::Soul, 1.0)], 16.0, &[(R::Qi, 20.0), (R::Sense, 10.0)], [0.3, 0.25, 3.0], "thirdeye", "TN125", S), &[Effect::Control(Ctl::Stun, 0.5)]), DIVINE),
        // Holds the body a moment and breaks the cast; pays life by the gap in strength (a placeholder 2%).
        rank(with(magic("bodyfix", "Định Thân Thuật", &[Spell, Finger, Mid, Instant, Tag::Control], &[(E::Soul, 1.0)], 10.0, &[(R::Qi, 36.0), (R::Life, 2.0)], [0.4, 0.3, 5.5], "bodyfix", "TN625", B), &[Effect::Control(Ctl::Hold, 0.4)]), RULE),
        // Drinks life along its flight; every shot or shield it passes adds to it; a hit locks blinking 2 s.
        rank(magic("silentfinger", "Tịch Diệt Chỉ", &[Spell, Finger, Far, Projectile, Death], &[(E::Yin, 1.0)], 15.0, &[(R::Qi, 24.0)], [0.32, 0.25, 3.0], "demonfinger", "TN483", B), DIVINE),
        // Five arrows in a row, each cutting (Thương): Huyết Tán's opener.
        with(attack("fivearrows", "Ngũ Tiễn Liên Châu", &[Attack, Far, Projectile, Chain, Blood], &[(E::Force, 1.0)], 4.0, &[(R::Qi, 20.0)], [0.3, 0.3, 3.0], "tigerflag", "CM13", S), &[Effect::Hits(5.0)]),
        // Pierces every shield, each one shrinking the arrow; three a match.
        rank(card(attack("libow", "Cung Lý Quảng", &[Attack, Treasure, Far, Projectile, Line, Vaal], &[(E::Metal, 1.0)], 40.0, &[(R::Qi, 30.0)], [0.5, 0.35, 2.0], "tigerflag", "TN1468", S), 3), RULE),
        // Grows with the damage taken.
        rank(attack("godfinger", "Cổ Thần Chỉ", &[Attack, Finger, Mid, Projectile, Ancient], &[(E::Force, 1.0)], 21.0, &[(R::Qi, 28.0)], [0.4, 0.3, 3.5], "finger", "TN716", A), RULE),
        // Strong but builds instability; spends slaughter intent.
        rank(magic("slaughterbolt", "Sát Lục Lôi", &[Spell, Finger, Far, Instant, Slaughter], &[(E::Thunder, 1.0)], 24.0, &[(R::Qi, 24.0), (R::Intent, 20.0)], [0.4, 0.3, 3.0], "jithunder", "TN1900", S), ORIGIN),
        // Black dragons breathe yin wind: it puts out the fire of life (Tịch).
        rank(magic("summonwind", "Hô Phong", &[Spell, Spirit, Far, Projectile], &[(E::Water, 0.5), (E::Yin, 0.5)], 12.0, &[(R::Qi, 26.0)], [0.3, 0.4, 3.0], "summonwind", "TN783", B), DIVINE),
        // ---- 11.4 areas, seals, sky strikes, traps, restrictions ----
        magic("seal", "Ấn", &[Spell, Seal, Mid, Delayed, Area], &[(E::Thunder, 1.0)], 22.0, &[(R::Qi, 30.0)], [0.3, 0.3, 2.4], "seal", "arena", A),
        with(magic("collapse", "Sụp", &[Spell, Mid, Area], &[(E::Earth, 1.0)], 25.0, &[(R::Qi, 39.0)], [0.5, 0.3, 2.4], "collapse", "arena", S), &[Effect::Control(Ctl::Soulless, 1.5)]),
        magic("quake", "Chấn Địa", &[Spell, Mid, Area], &[(E::Earth, 1.0)], 18.0, &[(R::Qi, 26.0)], [0.3, 0.3, 2.6], "quake", "arena", A),
        magic("gravity", "Trọng Lực Thuật", &[Spell, Mid, Area], &[(E::Earth, 1.0)], 20.0, &[(R::Qi, 22.0)], [0.3, 0.3, 2.6], "gravity", "TN", A),
        magic("earthsplit", "Địa Liệt", &[Spell, Mid, Area], &[(E::Earth, 1.0)], 26.0, &[(R::Qi, 28.0)], [0.32, 0.3, 3.2], "earthsplit", "TN25", A),
        magic("gather", "Tụ Lôi Ấn", &[Spell, Seal, Mid, Delayed, Area], &[(E::Thunder, 1.0)], 34.0, &[(R::Qi, 36.0)], [0.4, 0.35, 4.2], "gather", "TN1334", S),
        // Raises a mountain, then bursts it into stone and fire.
        rank(magic("mountain", "Sơn Băng", &[Spell, Tribulation, Far, Area, Delayed], &[(E::Earth, 0.6), (E::Fire, 0.4)], 34.0, &[(R::Qi, 44.0)], [0.6, 0.45, 5.5], "mountain", "TN857", A), DIVINE),
        with(magic("ancientthunder", "Thái Cổ Lôi Vực", &[Spell, Tribulation, Far, Area], &[(E::Thunder, 1.0)], 16.0, &[(R::Qi, 38.0)], [0.4, 0.35, 5.0], "ancientthunder", "TN649", S), &[Effect::Hits(2.0)]),
        with(magic("fingerdeath", "Tử Chỉ", &[Spell, Finger, Tribulation, Far, Area, Death], &[(E::Yin, 1.0)], 9.0, &[(R::Qi, 34.0)], [0.35, 0.3, 4.0], "fingerdeath", "TN", S), &[Effect::Hits(3.0)]),
        magic("flameorigin", "Tam Giới Hỏa Nguyên", &[Spell, Mid, Area], &[(E::Fire, 1.0)], 19.0, &[(R::Qi, 34.0)], [0.4, 0.3, 3.8], "flameorigin", "TN1091", B),
        with(magic("eightextremes", "Bát Cực Đạo", &[Spell, Tribulation, Far, Area, Chain], &[(E::Fire, 1.0)], 11.0, &[(R::Qi, 40.0)], [0.45, 0.35, 5.5], "eightextremes", "TN1958", S), &[Effect::Hits(4.0)]),
        magic("yinmoon", "Âm Nguyệt", &[Spell, Seal, Mid, Delayed, Area], &[(E::Yin, 1.0)], 20.0, &[(R::Qi, 32.0)], [0.35, 0.3, 3.4], "yinmoon", "TN1563", B),
        ban(magic("restriction", "Diệt Cấm", &[Spell, Restriction, Mid, Trap, Ban], &[(E::Metal, 1.0)], 26.0, &[(R::Qi, 40.0)], [0.5, 0.35, 5.0], "restriction", "TN753", B)),
        ban(magic("plumblossom", "Thập Bát Mai Hoa Cấm", &[Spell, Restriction, Mid, Trap, Ban], &[(E::Wood, 1.0)], 24.0, &[(R::Qi, 34.0)], [0.35, 0.3, 3.6], "plumblossom", "TN559", A)),
        // The left palm sows Life, the right sets off Death.
        rank(magic("lifedeath", "Sinh Tử Ấn", &[Spell, Seal, Palm, Near, Delayed, LifeDeath, Life, Death], &[(E::Yin, 0.5), (E::Wood, 0.5)], 28.0, &[(R::Qi, 36.0)], [0.35, 0.3, 3.6], "lifedeath", "TN1616", B), RULE),
        with(magic("remnantnight", "Tàn Dạ Thiên Liệt", &[Spell, Tribulation, Far, Area], &[(E::Yin, 0.5), (E::Light, 0.5)], 30.0, &[(R::Qi, 46.0)], [0.5, 0.4, 6.0], "remnantnight", "TN987", S), &[Effect::Control(Ctl::Soulless, 1.0)]),
        rank(magic("heavenflip", "Phiên Thiên Ấn", &[Spell, Seal, Tribulation, Far, Area], &[(E::Earth, 1.0)], 32.0, &[(R::Qi, 50.0)], [0.6, 0.4, 7.5], "heavenflip", "TN1319", A), DIVINE),
        magic("umbrella", "Phần Thế Tán", &[Spell, Treasure, Mid, Area], &[(E::Fire, 1.0)], 22.0, &[(R::Qi, 34.0)], [0.4, 0.3, 3.8], "umbrella", "TN1323", B),
        with(magic("starfall", "Lạc Tinh", &[Spell, Tribulation, Far, Area], &[(E::Light, 1.0)], 15.0, &[(R::Qi, 40.0)], [0.4, 0.35, 5.0], "starfall", "TN", A), &[Effect::Hits(2.0)]),
        // Locks blinking while it runs; its old freeze is gone (no hold: the novel's time restriction ages, it
        // does not hold the body).
        ban(magic("timerestrict", "Thời Cấm", &[Spell, Restriction, Mid, Trap, Time, Ban], &[(E::Soul, 1.0)], 20.0, &[(R::Qi, 38.0)], [0.45, 0.35, 5.5], "timerestrict", "TN1189", B)),
        with(magic("servitude", "Linh Nô Ấn", &[Spell, Seal, Mid, Delayed, Area], &[(E::Soul, 1.0)], 18.0, &[(R::Qi, 34.0)], [0.4, 0.3, 4.5], "servitude", "TN", A), &[Effect::Control(Ctl::Soulless, 1.0)]),
        magic("soulvortex", "Hồn Xoáy", &[Spell, Spirit, Mid, Area], &[(E::Soul, 1.0)], 16.0, &[(R::Qi, 30.0)], [0.35, 0.3, 3.6], "soulvortex", "TN", B),
        with(magic("starrotation", "Tinh Chuyển", &[Spell, Tribulation, Far, Area, Chain], &[(E::Light, 1.0)], 11.0, &[(R::Qi, 38.0)], [0.4, 0.35, 4.5], "starrotation", "TN475", S), &[Effect::Hits(3.0)]),
        with(magic("truefalse", "Chân Giả Ấn", &[Spell, Seal, Illusion, Mid, Delayed, Area, TrueFalse], &[(E::Soul, 0.5), (E::Light, 0.5)], 20.0, &[(R::Qi, 36.0)], [0.35, 0.3, 4.0], "truefalse", "TN1220", A), &[Effect::Control(Ctl::Soulless, 0.8)]),
        // Spends 20% of the caster's life (never the last point), each 1% adding BLOOD_POWER; once a match.
        card(with(magic("spiritsummon", "Diệt Sinh Triệu Linh", &[Spell, Tribulation, Far, Area, Vaal, Death, Blood], &[(E::Yin, 0.5), (E::Fire, 0.5)], 14.0, &[(R::Qi, 36.0), (R::Life, 20.0)], [0.4, 0.3, 4.5], "spiritsummon", "TN", S), &[Effect::Hits(2.0)]), 1),
        // Three layers break one by one: movement, body skills, soul skills. Nothing undoes it.
        ban(with(magic("threerestrict", "Tam Đại Cấm Chế", &[Spell, Restriction, Mid, Delayed, Area, Ban], &[(E::Force, 1.0)], 24.0, &[(R::Qi, 40.0)], [0.5, 0.35, 6.0], "restriction", "TN1271", A), &[Effect::Control(Ctl::Root, 0.8)])),
        // Calls mountains from memory.
        rank(magic("movemountain", "Di Sơn", &[Spell, Tribulation, Far, Area], &[(E::Earth, 1.0)], 32.0, &[(R::Qi, 44.0)], [0.6, 0.45, 6.0], "mountain", "CM1007", A), RULE),
        // Tears out one of the foe's origins (a buff or a kind of damage for a while) and heals the caster with it.
        rank(magic("karmaseal", "Nhân Quả Ấn", &[Spell, Seal, Palm, Near, Channel, Karma], &[(E::Soul, 1.0)], 24.0, &[(R::Qi, 36.0)], [0.4, 0.35, 5.0], "lifedeath", "TN1614", B), RULE),
        // ---- 11.5 channels, totems, minions, fields ----
        with(magic("void", "Hư Không", &[Spell, Mid, Channel, Area], &[(E::Yin, 1.0)], 14.0, &[(R::Qi, 60.0)], [0.9, 0.45, 11.0], "void", "arena", B), &[Effect::Hits(2.8)]),
        with(magic("arsenal", "Vạn Kiếm Trận", &[Spell, Sword, Formation, Mid, Totem, Area], &[(E::Metal, 1.0)], 12.0, &[(R::Qi, 48.0)], [0.6, 0.4, 9.0], "arsenal", "TN", S), &[Effect::Hits(5.5)]),
        // Rain round the caster, every drop a small shot; holds sense as an aura.
        rank(with(magic("summonrain", "Hoán Vũ", &[Spell, Mid, Area, Aura], &[(E::Water, 1.0)], 6.0, &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.4, 0.3, 5.0], "summonrain", "TN794", B), &[Effect::Hits(3.0)]), DIVINE),
        // As many soldiers as souls held; they rush in and blow up.
        rank(with(magic("beans", "Tát Đậu Thành Binh", &[Spell, Spirit, Mid, Minion], &[(E::Force, 1.0)], 8.0, &[(R::Qi, 36.0), (R::Souls, 2.0)], [0.5, 0.3, 7.0], "beans", "TN856", A), &[Effect::Hits(5.8)]), DIVINE),
        ban(with(magic("lifedeathrestrict", "Sinh Tử Cấm", &[Spell, Restriction, Mid, Area, LifeDeath, Death, Ban], &[(E::Yin, 1.0)], 7.0, &[(R::Qi, 40.0)], [0.5, 0.3, 7.5], "lifedeathrestrict", "TN990", A), &[Effect::Hits(5.3)])),
        rank(with(magic("soulbanner", "Hồn Phiên", &[Spell, Banner, Mid, Totem, Area], &[(E::Soul, 1.0)], 8.0, &[(R::Qi, 44.0)], [0.55, 0.3, 8.0], "soulbanner", "TN174", B), &[Effect::Hits(4.75)]), DIVINE),
        with(magic("ghostsummon", "Triệu Quỷ", &[Spell, Spirit, Mid, Minion, Death], &[(E::Yin, 1.0)], 7.0, &[(R::Qi, 34.0)], [0.45, 0.3, 6.0], "ghostsummon", "TN", B), &[Effect::Hits(4.7)]),
        with(magic("daoyellow", "Hoàng Tuyền Đạo Hóa", &[Spell, Domain, Whole, Area, Field, Death], &[(E::Yin, 1.0)], 6.0, &[(R::Qi, 42.0)], [0.5, 0.3, 7.5], "daoyellow", "TN604", S), &[Effect::Hits(6.0)]),
        // A swarm of souls: they drain, bait, and peel a fifth off a big shot per layer.
        rank(with(magic("wanderingsouls", "Du Hồn", &[Spell, Spirit, Far, Minion], &[(E::Soul, 1.0)], 7.0, &[(R::Qi, 34.0), (R::Souls, 1.0)], [0.45, 0.3, 6.0], "ghostsummon", "TN295", A), &[Effect::Hits(4.7)]), DIVINE),
        // A black curtain that cages, nine restriction dragons fired from it.
        ban(with(magic("restrictbanner", "Cấm Phiên", &[Spell, Banner, Restriction, Mid, Totem, Area, Ban], &[(E::Yin, 0.5), (E::Force, 0.5)], 9.0, &[(R::Qi, 44.0)], [0.55, 0.3, 8.0], "soulbanner", "TN226", A), &[Effect::Hits(4.75)])),
        // The finishing finger: the channeller heals nothing while it runs; more for everything dead in the zone.
        rank(with(magic("yellowfinger", "Hoàng Tuyền Chỉ", &[Spell, Finger, Far, Channel, Death], &[(E::Yin, 1.0)], 16.0, &[(R::Qi, 50.0)], [0.6, 0.45, 9.0], "void", "TN500", A), &[Effect::Hits(2.8)]), RULE),
        // Moon souls that merge into the Remnant God.
        rank(with(magic("moonwing", "Nguyệt Dực", &[Spell, Spirit, Mid, Minion], &[(E::Fire, 1.0)], 7.0, &[(R::Qi, 34.0)], [0.45, 0.3, 6.0], "ghostsummon", "CM25", B), &[Effect::Hits(4.7)]), DIVINE),
        // ---- 11.6 curses, warcries, soul ----
        magic("heartbeat", "Tâm Động Lôi Âm", &[Spell, Sound, Near, Nova, Warcry], &[(E::Thunder, 0.5), (E::Soul, 0.5)], 12.0, &[(R::Qi, 20.0)], [0.24, 0.35, 2.0], "heartbeat", "TN1777", C),
        with(magic("kunjiwhip", "Côn Cực Tiên", &[Spell, Treasure, Near, Line, Karma], &[(E::Soul, 1.0)], 13.0, &[(R::Qi, 26.0)], [0.32, 0.4, 3.0], "kunjiwhip", "TN363", B), &[Effect::Control(Ctl::Soulless, 0.8)]),
        with(magic("soulsearch", "Sưu Hồn", &[Spell, Spirit, Near, Single], &[(E::Soul, 1.0)], 8.0, &[(R::Qi, 28.0)], [0.4, 0.4, 4.0], "soulsearch", "TN19", B), &[Effect::Control(Ctl::Soulless, 1.2)]),
        with(magic("dreamdao", "Mộng Đạo", &[Spell, Illusion, Mid, Single, Curse, Hex], &[(E::Soul, 1.0)], 10.0, &[(R::Qi, 26.0)], [0.4, 0.4, 3.6], "dreamdao", "TN1294", B), &[Effect::Control(Ctl::Soulless, 1.6)]),
        magic("yellowspring", "Hoàng Tuyền Chỉ (cận)", &[Spell, Finger, Near, Single, Death], &[(E::Yin, 1.0)], 14.0, &[(R::Qi, 24.0)], [0.3, 0.4, 2.4], "yellowspring", "TN500", C),
        magic("starsoul", "Tinh Hồn Trích", &[Spell, Spirit, Near, Single], &[(E::Soul, 1.0)], 12.0, &[(R::Qi, 30.0)], [0.45, 0.4, 4.5], "starsoul", "TN", C),
        magic("hex", "Lục Giác Nguyền", &[Spell, Seal, Mid, Single, Curse, Hex], &[(E::Yin, 1.0)], 6.0, &[(R::Qi, 40.0)], [0.45, 0.25, 8.0], "hex", "arena", C),
        // The straw doll: each knot (a short channel) a stack; from the seventh knot each one costs the caster life.
        rank(magic("strawknot", "Thảo Kết Ký Sự", &[Spell, Talisman, Far, Channel, Curse, Hex], &[(E::Soul, 1.0)], 10.0, &[(R::Qi, 30.0)], [0.5, 0.3, 6.0], "soulvortex", "CM600", C), DIVINE),
        // As Doom: every blow the target throws makes it take more.
        magic("ancestorcurse", "Trớ Thuật Tiên Tổ", &[Spell, Seal, Far, Single, Curse, Hex], &[(E::Yin, 1.0)], 10.0, &[(R::Qi, 30.0)], [0.4, 0.3, 6.0], "yinmoon", "TN343", C),
        // Shows the target, draws shots to it, and lowers one resistance (as Exposure); no damage.
        with(magic("exposemark", "Khắc Ấn", &[Spell, Seal, Far, Projectile, Curse, Hex], &[], 0.0, &[(R::Qi, 18.0)], [0.25, 0.25, 3.2], "seize", "CM126", B), &[Effect::Expose(0.2)]),
        // ---- 11.7 guard, recovery, stance, movement, aura ----
        body("ward", "Hộ Phù", &[Body, Talisman, Near, Guard], &[(R::Qi, 45.0)], [0.5, 0.25, 8.0], "ward", "arena", B),
        body("bell", "Chuông", &[Body, Treasure, Near, Guard], &[(R::Qi, 18.0)], [0.06, 0.1, 6.5], "bell", "arena", B),
        body("mirror", "Gương", &[Body, Treasure, Near, Guard], &[(R::Qi, 18.0)], [0.06, 0.1, 6.5], "mirror", "arena", B),
        body("lightshield", "Quang Ảnh Thuẫn", &[Body, Near, Guard], &[(R::Qi, 20.0)], [0.06, 0.1, 7.0], "lightshield", "TN1319", B),
        body("ancientprotect", "Cổ Hộ", &[Body, Spirit, Near, Guard, Ancient], &[(R::Qi, 42.0)], [0.5, 0.25, 9.0], "ancientprotect", "TN", B),
        body("karma", "Nhân Quả Thuẫn", &[Body, Near, Guard, Karma], &[(R::Qi, 22.0)], [0.06, 0.1, 7.0], "karma", "TN670", B),
        body("dreamback", "Mộng Hồi Viễn Cổ", &[Body, Illusion, Near, Guard, Ancient], &[(R::Qi, 44.0)], [0.5, 0.25, 8.5], "dreamback", "TN1081", B),
        body("meritspirit", "Công Đức Linh", &[Body, Spirit, Near, Guard, Ancient], &[(R::Qi, 42.0)], [0.5, 0.25, 8.5], "meritspirit", "TN", B),
        with(body("heal", "Hồi", &[Body, Near, Recovery], &[(R::Qi, 35.0)], [0.85, 0.45, 9.0], "heal", "arena", B), &[Effect::Heal(0.2)]),
        with(body("indestructible", "Bất Diệt Thân", &[Body, Near, Recovery, Ancient], &[(R::Qi, 40.0)], [0.7, 0.4, 11.0], "indestructible", "TN1639", B), &[Effect::Heal(0.26)]),
        with(body("flowingmoon", "Lưu Nguyệt", &[Body, Near, Recovery, Time], &[(R::Qi, 38.0), (R::Lifespan, 5.0)], [0.8, 0.4, 10.0], "flowingmoon", "TN1243", B), &[Effect::Heal(0.24)]),
        with(body("soulrefine", "Luyện Hồn", &[Body, Spirit, Near, Recovery], &[(R::Qi, 28.0)], [0.7, 0.4, 8.0], "soulrefine", "TN385", B), &[Effect::Heal(0.16)]),
        with(body("bodyformation", "Tố Thân", &[Body, Near, Recovery, Immortal], &[(R::Qi, 34.0)], [0.75, 0.4, 9.5], "bodyformation", "TN", B), &[Effect::Heal(0.2)]),
        // Grows huge: more life, heavier fists; each body star lengthens it.
        rank(body("godform", "Cổ Thần Hóa", &[Body, Transform, Near, Stance, Ancient], &[(R::Qi, 40.0)], [0.7, 0.4, 11.0], "indestructible", "TN284", A), DIVINE),
        // 15 s in which time skills have no cooldown; spent afterwards.
        rank(body("fate", "Túc Mệnh", &[Body, Near, Stance, Time], &[(R::Qi, 38.0), (R::Lifespan, 20.0)], [0.8, 0.4, 14.0], "flowingmoon", "CM477", A), ORIGIN),
        body("inchearth", "Súc Địa", &[Body, Escape, Far, Movement, Space], &[(R::Qi, 26.0)], [0.06, 0.12, 3.0], "inchearth", "TN770", C),
        body("earthescape", "Thổ Độn", &[Body, Escape, Mid, Movement], &[(R::Qi, 14.0)], [0.06, 0.1, 1.4], "earthescape", "TN131", C),
        body("ancientcauldron", "Cổ Thần Đỉnh", &[Body, Treasure, Whole, Movement, Ancient, Space], &[(R::Qi, 30.0)], [0.2, 0.3, 4.5], "ancientcauldron", "TN713", C),
        body("starcompass", "Tinh Bàn", &[Body, Treasure, Whole, Movement, Space], &[(R::Qi, 18.0)], [0.1, 0.2, 2.5], "starcompass", "TN", C),
        body("teleport", "Thuấn Di", &[Body, Escape, Far, Movement, Space], &[(R::Qi, 26.0)], [0.06, 0.12, 3.0], "inchearth", "TN201", C),
        // Auras hold sense until put out or soulless.
        body("slaughteraura", "Sát Lục Ý Cảnh", &[Body, Intent, Near, Aura, Slaughter], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN505", A),
        body("timeaura", "Tuế Nguyệt Ý Cảnh", &[Body, Intent, Mid, Aura, Time], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "flowingmoon", "TN252", A),
        // ---- 11.8 đỉnh cấp: the former ultimates, skills like any other at the top tier and the top rank, paid in
        // qi and cooled like the tier's other areas ----
        rank(with(magic("tribulation", "Thiên Kiếp", &[Spell, Tribulation, Far, Area], &[(E::Thunder, 1.0)], 14.0, &[(R::Qi, 44.0)], [0.7, 0.8, 6.0], "tribulation", "arena", S), &[Effect::Hits(3.0)]), FAITH),
        // Night over the arena; the sunrise strikes light no one dodges.
        rank(with(magic("remnantdusk", "Tàn Dạ", &[Spell, Domain, Whole, Area], &[(E::Fire, 0.5), (E::Light, 0.5)], 16.0, &[(R::Qi, 48.0)], [0.7, 0.8, 6.5], "tribulation", "TN987", S), &[Effect::Hits(3.0)]), FAITH),
        // A ring drawn, tribulations 1 to 8 called, each a different kind.
        rank(with(magic("sukiep", "Tô Kiếp", &[Spell, Tribulation, Far, Area], &[(E::Metal, 0.125), (E::Wood, 0.125), (E::Water, 0.125), (E::Fire, 0.125), (E::Earth, 0.125), (E::Thunder, 0.125), (E::Light, 0.125), (E::Yin, 0.125)], 6.0, &[(R::Qi, 48.0)], [0.7, 0.8, 6.5], "tribulation", "CM1303", S), &[Effect::Hits(8.0)]), FAITH),
        rank(magic("daofusion", "Đạo Pháp Dung Hợp", &[Spell, Near, Nova, Charged], &[(E::Force, 0.34), (E::Thunder, 0.33), (E::Fire, 0.33)], 60.0, &[(R::Qi, 40.0)], [0.8, 0.5, 4.0], "daofusion", "TN", S), FAITH),
        // ---- research techniques the design's section 11 leaves out ----
        // Tens of restriction qi made one spear; it grows as it gathers the restriction qi on the ground.
        attack("banspear", "Cấm Khí Trường Thương", &[Attack, Sword, Mid, Line, Strike, Ban], &[(E::Force, 1.0)], 24.0, &[(R::Qi, 34.0)], [0.45, 0.5, 4.5], "heavenlychop", "TN309", B),
        // "I say it is thunder": every effect on the foe becomes Lôi damage (a herald on its ailments).
        rank(magic("thunderchart", "Lôi Đồ", &[Spell, Seal, Whole, Area, Herald], &[(E::Thunder, 1.0)], 20.0, &[(R::Qi, 36.0)], [0.4, 0.35, 6.0], "gather", "TN1269", A), ORIGIN),
        // Smashes every body star: a blast (or a full heal, a later rule).
        rank(attack("starbreak", "Toái Tinh", &[Attack, Fist, Near, Nova, Area, Ancient], &[(E::Force, 1.0)], 30.0, &[(R::Qi, 20.0), (R::Star(Star::Body), 1.0)], [0.36, 0.5, 6.0], "burst", "TN842", B), RULE),
        // Blows on the caster feed back a growing recoil (dropping a shield first turns it into a buff).
        rank(body("defiance", "Nghịch Ý", &[Body, Near, Trigger, Ancient], &[(R::Qi, 22.0)], [0.06, 0.1, 7.0], "lightshield", "TN588", A), DIVINE),
        // Both sealed: the fight locks into bodies (divine skills −50%, body blows up, no one leaves).
        rank(body("warseal", "Cổ Tộc Chiến Ấn", &[Body, Seal, Whole, Field, Ancient], &[(R::Qi, 40.0)], [0.45, 0.25, 12.0], "hex", "TN1672", A), RULE),
        // Four presses: pull, whirl, the foe's healing turned to damage, stone if all four land; stopping loses all.
        rank(with(magic("fourforms", "Đạo Cổ Tứ Thức", &[Spell, Seal, Mid, Chain, Delayed, Ancient], &[(E::Force, 0.5), (E::Yin, 0.5)], 9.0, &[(R::Qi, 40.0)], [0.4, 0.35, 6.0], "gather", "TN1778", S), &[Effect::Hits(4.0), Effect::Control(Ctl::Hold, 0.5)]), RULE),
        // Hồn Phiên, "Dung!": N souls merged into one master soul.
        rank(with(magic("soulmerge", "Hồn Phiên · Dung", &[Spell, Banner, Spirit, Mid, Minion], &[(E::Soul, 1.0)], 8.0, &[(R::Qi, 44.0), (R::Souls, 3.0)], [0.55, 0.3, 8.0], "soulbanner", "TN390", B), &[Effect::Hits(4.75)]), DIVINE),
        // Hồn Phiên, "Vỡ!": the swarm blows itself up on command.
        rank(magic("soulburst", "Hồn Phiên · Vỡ", &[Spell, Banner, Mid, Mine, Area], &[(E::Soul, 1.0)], 30.0, &[(R::Qi, 30.0), (R::Souls, 2.0)], [0.35, 0.3, 6.0], "soulvortex", "TN390", A), DIVINE),
        // Huge damage, but only on the bleeding; a caster who bleeds catches it too.
        attack("bloodpowder", "Huyết Tán", &[Attack, Talisman, Near, Area, Blood], &[(E::Force, 1.0)], 30.0, &[(R::Qi, 27.0)], [0.32, 0.5, 4.0], "palm", "CM10", A),
        // Three cuts toward the Tam Sát quarter, each paying lifespan: the environment's buffs, space, time.
        rank(with(attack("threeslay", "Trảm Tam Sát", &[Attack, Sword, Mid, Line, Chain, Bond, Fate], &[(E::Force, 1.0)], 8.0, &[(R::Qi, 30.0), (R::Lifespan, 15.0)], [0.45, 0.5, 6.0], "heavenlychop", "CM81", B), &[Effect::Hits(3.0)]), RULE),
        // Khai Dương, Táng Âm, Ly Phong: fire and water in one chain, so the second blow fuses into wind.
        rank(with(attack("liphong", "Ly Phong Tam Thức", &[Attack, Fist, Mid, Chain, Strike], &[(E::Fire, 0.5), (E::Water, 0.5)], 9.0, &[(R::Qi, 34.0)], [0.4, 0.5, 5.0], "palm", "CM358", A), &[Effect::Hits(3.0)]), RULE),
        // Drags the foe back 0.5 s and undoes its cast.
        rank(with(magic("timereverse", "Đảo Ngược Thời Gian", &[Spell, Palm, Mid, Instant, Time], &[(E::Force, 1.0)], 8.0, &[(R::Qi, 30.0), (R::Lifespan, 10.0)], [0.3, 0.25, 6.0], "thirdeye", "CM498", B), &[Effect::Control(Ctl::Jolt, 0.3)]), RULE),
        // Eight sentences, one a second; costs all life but 1, only when the target has no shield.
        rank(with(magic("cursewords", "Thuật Nguyền Rủa", &[Spell, Sound, Far, Chain, Curse, Hex, Blood], &[(E::Yin, 0.5), (E::Soul, 0.5)], 5.0, &[(R::Qi, 30.0), (R::Life, 100.0)], [0.4, 0.3, 8.0], "fingerdeath", "CM429", A), &[Effect::Hits(8.0)]), ORIGIN),
        // Winter (slows: its ice is gone), autumn (drains), summer (fire for life), spring (heals).
        rank(body("fourseasons", "Mệnh Cách Bốn Mùa", &[Body, Intent, Mid, Aura, Fate], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "flowingmoon", "CM692", A), RULE),
        // Eyes shut 0.5 s: no spell or shot lands; eyes open: the foe's illusions, minions and zones in reach are gone.
        rank(body("realfake", "Chân Giả", &[Body, Illusion, Near, Channel, Guard, TrueFalse], &[(R::Qi, 30.0)], [0.06, 0.5, 8.0], "mirror", "TN1617", B), RULE),
        // One-use cards: each can be shown without casting (Lộ bài).
        card(magic("heaventhread", "Sợi Tơ Thiên Kiếp", &[Spell, Tribulation, Far, Area, Vaal], &[(E::Thunder, 1.0)], 40.0, &[(R::Qi, 20.0)], [0.5, 0.4, 2.0], "ancientthunder", "TN226", S), 1),
        card(magic("bannertribulation", "Cấm Phiên Dẫn Kiếp", &[Spell, Banner, Tribulation, Far, Area, Vaal, Ban], &[(E::Thunder, 1.0)], 45.0, &[(R::Qi, 30.0)], [0.6, 0.45, 2.0], "ancientthunder", "TN248", S), 1),
        card(attack("manhair", "Sợi Tóc Man Thần", &[Attack, Finger, Mid, Projectile, Line, Vaal], &[(E::Force, 1.0)], 36.0, &[(R::Qi, 20.0)], [0.4, 0.35, 2.0], "tigerflag", "CM499", S), 2),
        card(attack("skyqueen", "Kiếm Khí Lăng Thiên Hậu", &[Attack, Sword, Far, Projectile, Line, Vaal], &[(E::Metal, 1.0)], 45.0, &[(R::Qi, 30.0)], [0.5, 0.35, 2.0], "tigerflag", "TN512", S), 1),
        // Blessings: buffs for a time that hold no sense.
        body("soularmour", "Hồn Khải", &[Body, Spirit, Near, Blessing], &[(R::Qi, 35.0)], [0.6, 0.4, 12.0], "heal", "TN1870", A),
        body("deathlessbreath", "Hơi Thở Bất Diệt", &[Body, Near, Blessing, Life], &[(R::Qi, 30.0), (R::Lifespan, 20.0)], [0.6, 0.4, 14.0], "indestructible", "CM1213", A),
        // ---- the curses of research 6.1 (curse.rs `Kind`) the rows above leave out ----
        // Nine beats of yin, heavier for every guard the target raises; only another curse or nine beats lift it.
        rank(with(magic("ninedeaths", "Cửu Tử", &[Spell, Seal, Far, Single, Chain, Curse, Hex], &[(E::Yin, 1.0)], 4.0, &[(R::Qi, 30.0)], [0.4, 0.3, 7.0], "fingerdeath", "CM732", A), &[Effect::Hits(9.0)]), DIVINE),
        // A tether: the target cannot go further than so far from the curser; the thread can be cut.
        rank(magic("deathseal", "Tử Ấn Tổ Quyết", &[Spell, Seal, Far, Single, Curse, Hex], &[], 0.0, &[(R::Qi, 30.0)], [0.4, 0.3, 8.0], "seize", "TN2029", B), DIVINE),
        // Five beats on the zither, each breaking an organ; the fifth is a heavy blow.
        rank(with(magic("fiveorgans", "Ngũ Tạng", &[Spell, Sound, Far, Chain, Curse, Hex], &[(E::Yin, 1.0)], 6.0, &[(R::Qi, 34.0)], [0.45, 0.3, 7.0], "starrotation", "TN1658", A), &[Effect::Hits(6.0)]), DIVINE),
        // A three-second countdown to heavy soul damage; broken, it turns on the diviner.
        rank(magic("threebreaths", "Bói Ba Nhịp Thở", &[Spell, Seal, Far, Delayed, Curse, Hex], &[(E::Soul, 1.0)], 30.0, &[(R::Qi, 34.0)], [0.4, 0.3, 7.0], "servitude", "TN2048", A), DIVINE),
        // A spoken bond: using a skill of the named kind while it lasts cracks the heart (Tâm Nứt).
        rank(magic("bindingword", "Lời Giam Cầm", &[Spell, Sound, Far, Single, Curse, Hex], &[], 0.0, &[(R::Qi, 26.0)], [0.35, 0.3, 6.0], "dreamdao", "CM1015", B), DIVINE),
        // Seals one kind of the target's skills; paid in the curser's full life (a placeholder 10%).
        rank(magic("sealgod", "Phong Thần", &[Spell, Seal, Far, Single, Curse, Hex], &[], 0.0, &[(R::Qi, 30.0), (R::Life, 10.0)], [0.4, 0.3, 8.0], "hex", "TN1313", B), DIVINE),
        // Ages the target: slows it and drains its lifespan; a still tree watcher is untouched.
        rank(magic("agingcurse", "Tuế Nguyệt Nguyền", &[Spell, Seal, Far, Single, Curse, Hex, Time], &[], 0.0, &[(R::Qi, 30.0)], [0.4, 0.3, 7.0], "yinmoon", "TN260", B), DIVINE),
        // ---- the domains of research 6.4 (domain.rs `DomainKind`) with no gem above; one each a side ----
        // Windless: no one regrows qi or casts a qi skill; shots and wind are gone.
        rank(body("stillwind", "Ly Phong Lĩnh Vực", &[Body, Domain, Whole, Field], &[(R::Qi, 50.0)], [0.6, 0.4, 14.0], "daoyellow", "CM358", A), DIVINE),
        // Whoever moves inside is bound: no escape, no flight; standing still is free.
        rank(body("fixedrule", "Định Quy Tắc", &[Body, Domain, Whole, Field, Space], &[(R::Qi, 50.0)], [0.6, 0.4, 14.0], "daoyellow", "TN1753", A), DIVINE),
        // Foes inside use 40% of their power; raised only at the floor's centre.
        rank(body("truepressure", "Uy Áp Chân Giới", &[Body, Domain, Whole, Field], &[(R::Qi, 50.0)], [0.6, 0.4, 14.0], "daoyellow", "CM6", A), DIVINE),
        // Foes draw no qi from the world; a minion dying inside gives the owner incense.
        rank(body("windrain", "Hô Phong Hoán Vũ", &[Body, Domain, Whole, Field], &[(R::Qi, 50.0)], [0.6, 0.4, 14.0], "daoyellow", "TN782", A), DIVINE),
        // ---- the auras of research 6.2 and 6.3 (aura.rs `AuraKind`) with no gem above; each holds sense ----
        body("belief", "Niềm Tin Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN542", A),
        body("waraura", "Chiến Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN1213", A),
        body("ease", "An Nhàn Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN260", A),
        body("dispassion", "Vô Tình Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN299", A),
        body("frenzy", "Điên Cuồng Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN258", A),
        body("moods", "Hỉ Nộ Ai Oán", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "CM1030", A),
        body("rebirth", "Luân Hồi Ý Cảnh", &[Body, Intent, Mid, Aura, LifeDeath], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN265", A),
        body("desire", "Dục Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN403", A),
        body("soullock", "Tỏa Thần Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN404", A),
        body("memory", "Hồi Ức Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN268", A),
        body("infatuation", "Si Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN308", A),
        body("severance", "Tuyệt Tình Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN365", A),
        body("charm", "Mê Hoặc Ý Cảnh", &[Body, Intent, Mid, Aura], &[(R::Qi, 30.0), (R::Sense, 20.0)], [0.5, 0.3, 8.0], "heal", "TN365", A),
    ];
}

/// The curse a skill lays (curse.rs `Kind`), if it is one. A kind that carries a choice (the tag a binding word
/// names, the kind Phong Thần seals, the kind Khắc Ấn exposes) gets a placeholder here until the fight lets the caster
/// choose: Guard, Thunder and Force.
pub fn curse_of(g: &Gem) -> Option<curse::Kind> {
    use curse::Kind;
    Some(match g.id {
        "strawknot" => Kind::StrawKnot,
        "ninedeaths" => Kind::NineDeaths,
        "ancestorcurse" => Kind::AncestorDoom,
        "deathseal" => Kind::DeathSeal,
        "fiveorgans" => Kind::FiveOrgans,
        "threebreaths" => Kind::ThreeBreaths,
        "bindingword" => Kind::BindingWord(Tag::Guard),
        "dreamdao" => Kind::DreamDao,
        "sealgod" => Kind::SealGod(Tag::Thunder),
        // Hóa Ma Chỉ is the Hóa Ma curse: it turns the foe's immortal buffs to demon qi.
        "demonfinger" => Kind::Demonize,
        "agingcurse" => Kind::Aging,
        "exposemark" => Kind::ExposeMark(Element::Force),
        _ => return None,
    })
}

/// The domain a skill raises (domain.rs `DomainKind`), if it is one.
pub fn domain_of(g: &Gem) -> Option<DomainKind> {
    Some(match g.id {
        "daoyellow" => DomainKind::YellowSprings,
        "remnantdusk" => DomainKind::RemnantNight,
        "stillwind" => DomainKind::StillWind,
        "fixedrule" => DomainKind::FixedRule,
        "truepressure" => DomainKind::TruePressure,
        "windrain" => DomainKind::WindRain,
        _ => return None,
    })
}

/// The aura a skill lights (aura.rs `AuraKind`), if it is one.
pub fn aura_of(g: &Gem) -> Option<AuraKind> {
    Some(match g.id {
        "slaughteraura" => AuraKind::Slaughter,
        "timeaura" => AuraKind::Years,
        "belief" => AuraKind::Belief,
        "waraura" => AuraKind::War,
        "ease" => AuraKind::Ease,
        "dispassion" => AuraKind::Dispassion,
        "frenzy" => AuraKind::Frenzy,
        "moods" => AuraKind::Moods,
        "rebirth" => AuraKind::Rebirth,
        "desire" => AuraKind::Lust,
        "soullock" => AuraKind::SoulLock,
        "memory" => AuraKind::Memory,
        "infatuation" => AuraKind::Infatuation,
        "severance" => AuraKind::Severance,
        "charm" => AuraKind::Charm,
        _ => return None,
    })
}

/// The skill named `id`.
pub fn gem(id: &str) -> Option<&'static Gem> {
    GEMS.iter().find(|g| g.id == id)
}

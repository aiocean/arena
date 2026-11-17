//! A skill of the pool, with its supports and the fighter's sheet (items, tree, keystones), made into a [`Hit`] and
//! the timings and costs it is cast with (design, 2, 5, 8, 9 and 10). An attack's base is its weapon's damage (bare
//! hands: the striker's might) times the skill's effectiveness; a spell's is its base at its level. Every number
//! after the base comes from one gathered table (`stat::Table`): nothing adds a source by hand. Supports whose tags
//! do not fit sit inert (rules, 2.2.1); the cost multipliers multiply; each 1% of full life paid is
//! [`BLOOD_POWER`] more.

use crate::ailment::{Ailment, ForceAilment};
use crate::cultivator::Cultivator;
use crate::cultivator::Star;
use crate::dot::DotScale;
use crate::element::{Element, N};
use crate::gem::{Effect, Gem, Resource};
use crate::grade::{Grade, Rank};
use crate::hit::{Frame, Gains, Hit, attack_base, spell_base};
use crate::item::ItemSlot;
use crate::sheet::Sheet;
use crate::stat::{Counts, Ctx, Happened, Mod, ModKind, Source, Stat, Table};
use crate::support::{Behaviour, Support};
use crate::tag::{Tag, Tags};

/// More a 1% of full life paid for a cast (design, 10).
pub const BLOOD_POWER: f64 = 0.03;
/// The levels (hỏa hầu) at which fire burns blue and purple (rules, 4.3).
pub const FIRE_TIER_LEVELS: [u8; 2] = [8, 14];
/// Thunder more on a minion tagged yin or insect (design, 4.1).
pub const THUNDER_VS_MINION: f64 = 0.5;
/// Qi paid in life ([`Stat::CostAsLife`]): this share of full life a point of qi.
pub const LIFE_PER_QI: f64 = 0.002;

/// A skill as one fighter casts it: a [`Gem`] at a level and the striker's might, or two fused by Dung
/// (`support::fuse`).
#[derive(Clone, Debug, PartialEq)]
pub struct Skill {
    /// The gem's code name (the first one's, when fused).
    pub id: &'static str,
    /// Its tags, without its kinds of damage.
    pub tags: Tags,
    /// Its damage by kind, shares summing to 1.
    pub split: Vec<(Element, f64)>,
    /// Its base damage as cast (an attack's from might, a spell's from its level).
    pub base: f64,
    /// What added damage is scaled by: an attack's share of might, 1 for a spell.
    pub effectiveness: f64,
    pub level: u8,
    pub rank: Rank,
    pub restriction: bool,
    /// Its costs. A [`Resource::Life`] cost is a share of full life.
    pub costs: Vec<(Resource, f64)>,
    pub cooldown: f64,
    pub windup: f64,
    pub recover: f64,
    pub force: ForceAilment,
    pub effects: &'static [Effect],
    /// One of the three basics (`loadout::BASICS`).
    pub basic: bool,
    /// The second skill, when fused.
    pub fused: Option<&'static str>,
}

impl Skill {
    /// `gem` at `level` (hỏa hầu 1..20) for a striker of `might`.
    pub fn of(gem: &Gem, level: u8, might: f64) -> Skill {
        let attack = gem.is_attack();
        Skill {
            id: gem.id,
            tags: gem.tags(),
            split: gem.split.to_vec(),
            base: if gem.split.is_empty() {
                0.0
            } else if attack {
                attack_base(might, gem.base)
            } else {
                spell_base(gem.base, level)
            },
            effectiveness: if attack { gem.base } else { 1.0 },
            level: level.clamp(1, 20),
            rank: gem.rank,
            restriction: gem.restriction,
            // a gem lists life in percent of full life; a skill's costs keep it as a share
            costs: gem
                .costs
                .iter()
                .map(|&(r, v)| (r, if r == Resource::Life { v / 100.0 } else { v }))
                .collect(),
            cooldown: gem.cooldown,
            windup: gem.windup,
            recover: gem.recover,
            force: gem.force,
            effects: gem.effects,
            basic: crate::loadout::BASICS.contains(&gem.id),
            fused: None,
        }
    }

    /// Its tags with its kinds of damage, what supports are matched against.
    pub fn all_tags(&self) -> Tags {
        self.split
            .iter()
            .fold(self.tags, |t, &(e, _)| t.with(Tag::of(e)))
    }
}

/// What the fight knows at the cast that changes the blow.
#[derive(Clone, Copy, Debug)]
pub struct Situation<'a> {
    /// The target, when there is one (keystones that pass resistance and conditions on the target read it).
    pub target: Option<&'a Cultivator>,
    /// The target's tags (a minion tagged yin or insect takes more thunder).
    pub target_tags: Tags,
    /// The target carries a curse (`curse::Curses` lives outside the cultivator), and how many.
    pub target_cursed: bool,
    pub target_curses: u8,
    /// The target is moving.
    pub target_moving: bool,
    /// Which use of this skill the cast is, counted by the fight from 1 (Tàn Nhẫn's every third).
    pub use_count: u32,
    /// What the striker did lately (kills, blows, qi spent, kinds dealt).
    pub counts: Counts,
    /// T between the striker and the target (Near and Far conditions); infinite when unknown.
    pub distance: f64,
    /// The striker is moving.
    pub moving: bool,
    /// Counter or punish.
    pub frame: Frame,
    /// Turns a charged skill was held (Cửu Chuyển).
    pub turns: u8,
    /// T a projectile has flown (Dịch Linh).
    pub travelled: f64,
    /// The target just used a guard skill (Nhử Đòn).
    pub target_guarded: bool,
    /// Sát Ý stacks, Đạo Dung Thân vortices, Tam Mệnh lives used, the season's win streak (Niềm Tin Tất Thắng).
    pub fury: u8,
    pub vortices: u8,
    pub lives_used: u8,
    pub streak: u8,
    /// A soul lamp was snuffed for this blow (Ngọn Đèn Hồn).
    pub snuffed: bool,
}

impl Default for Situation<'_> {
    fn default() -> Self {
        Situation {
            target: None,
            target_tags: Tags::NONE,
            target_cursed: false,
            target_curses: 0,
            target_moving: false,
            use_count: 0,
            counts: Counts::default(),
            distance: f64::INFINITY,
            moving: false,
            frame: Frame::Plain,
            turns: 0,
            travelled: 0.0,
            target_guarded: false,
            fury: 0,
            vortices: 0,
            lives_used: 0,
            streak: 0,
            snuffed: false,
        }
    }
}

impl<'a> Situation<'a> {
    /// The context `striker`'s modifiers are gathered in for a skill of `skill` tags.
    pub fn ctx(&self, striker: &'a Cultivator, skill: Tags) -> Ctx<'a> {
        Ctx {
            me: striker,
            other: self.target,
            other_cursed: self.target_cursed || self.target_curses > 0,
            curses: self.target_curses.max(u8::from(self.target_cursed)),
            distance: self.distance,
            moving: self.moving,
            other_moving: self.target_moving,
            skill,
            use_count: self.use_count,
            counts: self.counts,
        }
    }
}

/// A skill ready to cast: its blow, and its timings and costs after supports and keystones.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    pub hit: Hit,
    /// Its rank after supports.
    pub rank: Rank,
    pub windup: f64,
    pub recover: f64,
    pub cooldown: f64,
    /// Every cost times the cost modifiers; [`Resource::Life`] as a share of full life.
    pub costs: Vec<(Resource, f64)>,
    /// The life the striker pays now (never the last point).
    pub life: f64,
    /// Times the blow lands, and extra projectiles.
    pub strikes: u8,
    pub projectiles: u8,
    /// Controls it lays last this many times as long (`Stat::ControlDuration`, and Nhử Đòn on a target that just
    /// guarded).
    pub control: f64,
    /// The sense the skill holds while it stands is this many times its own (`Stat::Reservation`): an intent's
    /// `aura::AuraData::reserve`, given to `aura::Auras::light_with`.
    pub reservation: f64,
    /// Sense its supports hold on top while it stands (`Stat::Reserved` after `reservation`: Chú Vực).
    pub reserve: f64,
    /// What the fight must do besides the blow.
    pub behaviours: Vec<Behaviour>,
    /// Supports whose tags do not fit: they do nothing (the bot sees an "inert" flag).
    pub inert: Vec<&'static str>,
    /// The striker's table for this cast, everything gathered: what damage over time and a breakdown read.
    pub table: Table,
}

/// Moves `share` of `from` to `to` in the blow's base and added damage, at step 1: the only way a kind goes back
/// in the order (Âm Hàn).
pub fn rewrite(hit: &mut Hit, from: Element, to: Element, share: f64) {
    if from == to {
        return;
    }
    let share = share.clamp(0.0, 1.0);
    let (f, t) = (from.index(), to.index());
    let b = hit.base[f] * share;
    hit.base[f] -= b;
    hit.base[t] += b;
    let a = hit.added[f] * share;
    hit.added[f] -= a;
    hit.added[t] += a;
}

/// `gem` at `level` with `supports`, struck by `striker` wearing `sheet` (its items, tree and keystones) in `sit`.
/// An attack starts from the weapon (bare hands: the striker's might); a spell from its base at its level.
pub fn build(
    gem: &Gem,
    level: u8,
    supports: &[&Support],
    striker: &Cultivator,
    sheet: &Sheet,
    sit: &Situation,
) -> Built {
    let might = match sheet.gear.in_slot(ItemSlot::Weapon) {
        Some(_) => sheet.gear.weapon().damage[Element::Force.index()],
        None => striker.might,
    };
    build_skill(&Skill::of(gem, level, might), supports, striker, sheet, sit)
}

/// [`build`] for a skill already made (a fused one). Every number of the blow, its timings and its costs comes from
/// one table: the sheet's modifiers, the striker's timed effects and state (stars, ailments), the fitting
/// supports' and the keystones' situational ones, gathered in the cast's context ([`Situation::ctx`]).
pub fn build_skill(
    skill: &Skill,
    supports: &[&Support],
    striker: &Cultivator,
    sheet: &Sheet,
    sit: &Situation,
) -> Built {
    let own = skill.all_tags();
    let (on, off): (Vec<&Support>, Vec<&Support>) = supports.iter().partition(|s| s.fits(own));
    // a support can make the skill something more (a trap, a mine, a totem): its modifiers and the fight see it
    let tags = on
        .iter()
        .flat_map(|s| s.mods.becomes.iter().copied())
        .fold(own, Tags::with);
    let rank = Rank::of_skill(skill.rank, on.iter().any(|s| s.mods.rank_step), false);
    let grade = if skill.restriction {
        Grade::Restriction
    } else {
        Grade::Ranked(rank)
    };
    let keystones = sheet.keystones();

    // every modifier of the cast that is not on the sheet
    let mut cast: Vec<Mod> = Vec::new();
    for s in &on {
        cast.extend(s.mods(skill, sit));
    }
    if sit.target_tags.has(Tag::Minion) && sit.target_tags.any(&[Tag::Yin, Tag::Insect]) {
        cast.push(Mod::more(
            Stat::DamageOf(Element::Thunder),
            THUNDER_VS_MINION,
            Source::Other("thunder on a yin or insect minion"),
        ));
    }
    for k in &keystones {
        cast.extend(k.strike_mods(skill, grade, sit));
    }
    let ctx = sit.ctx(striker, tags);
    let t = sheet.table(&ctx, cast.iter());

    let mut hit = Hit::split(skill.base, &skill.split);
    hit.tags = tags;
    hit.effectiveness = skill.effectiveness;
    // step 1: an attack's weapon brings its other kinds (its force is the skill's base already)
    if tags.has(Tag::Attack) && sheet.gear.in_slot(ItemSlot::Weapon).is_some() {
        let w = sheet.gear.weapon();
        for e in Element::ALL.into_iter().skip(1) {
            hit.base[e.index()] += w.damage[e.index()] * skill.effectiveness;
        }
    }
    // step 2: added, scaled by effectiveness in `scale`
    for e in Element::ALL {
        hit.added[e.index()] = t.flat(Stat::Added(e));
    }
    // step 1: rewrites (after added, so they move it too)
    for s in &on {
        for &(from, to, share) in s.mods.rewrite {
            rewrite(&mut hit, from, to, share);
        }
    }
    // step 3: forward only; then gain as extra, any direction
    for from in Element::ALL {
        for to in Element::ALL.into_iter().filter(|&to| to > from) {
            hit.convert[from.index()][to.index()] = t.flat(Stat::Convert(from, to));
        }
        for to in Element::ALL.into_iter().filter(|&to| to != from) {
            hit.extra[from.index()][to.index()] = t.flat(Stat::Extra(from, to)).max(0.0);
        }
    }
    // step 4: increases sum, each more stands alone
    hit.increased = t.combined(&[Stat::Damage, Stat::HitDamage]).increased;
    hit.more = t
        .used
        .iter()
        .filter(|m| m.kind == ModKind::More && matches!(m.stat, Stat::Damage | Stat::HitDamage))
        .map(|m| m.value)
        .collect();
    (hit.increased_by, hit.more_by) = t.by_kind();
    // step 5: an attack's weapon's crit; bare hands and spells, the striker's own
    let crit = match sheet.gear.in_slot(ItemSlot::Weapon) {
        Some(_) if tags.has(Tag::Attack) => sheet.gear.weapon().crit,
        _ => striker.crit,
    };
    hit.crit = t.get(Stat::CritChance, crit);
    hit.crit_multi = t.get(Stat::CritMulti, striker.crit_multi);
    hit.double = t.get(Stat::ChanceDoubleDamage, 0.0).clamp(0.0, 1.0);
    hit.instant = tags.has(Tag::Instant);
    hit.evadable = tags.evadable() && !on.iter().any(|s| s.mods.unevadable);
    // step 7 and 12: fire
    let by_level = FIRE_TIER_LEVELS
        .iter()
        .filter(|&&l| skill.level >= l)
        .count() as u8;
    hit.fire_tier = on
        .iter()
        .filter_map(|s| s.mods.fire_tier)
        .fold(by_level, u8::max);
    hit.hollow_fire = on.iter().any(|s| s.mods.hollow_fire);
    // step 8
    hit.armour_pen = t.get(Stat::ArmourPen, 0.0).clamp(0.0, 1.0);
    for e in Element::ALL {
        hit.pen[e.index()] = t.get(Stat::Penetration(e), 0.0);
    }
    hit.grade = grade;
    hit.frame = sit.frame;
    hit.force_ailment = skill.force;
    // step 12: ailment effect and duration, and the damage-over-time modifiers in place of step 4's
    (hit.ailment_effect, hit.ailment_duration) = t.ailment_factors();
    for a in Ailment::ALL {
        hit.dot[a.index()] = DotScale::of(&t, a);
    }
    hit.leech = t.get(Stat::Leech, 0.0);
    hit.qi_leech = t.get(Stat::QiLeech, 0.0);
    hit.on_hit = Gains {
        life: t.get(Stat::LifeOnHit, 0.0),
        qi: t.get(Stat::QiOnHit, 0.0),
        sense: t.get(Stat::SenseOnHit, 0.0),
        intent: t.get(Stat::IntentOnHit, 0.0),
    };
    hit.shield_bypass = t.get(Stat::ShieldBypass, 0.0).clamp(0.0, 1.0);
    hit.execute = t.get(Stat::Execute, 0.0).clamp(0.0, 1.0);
    for a in Ailment::ALL {
        hit.inflict[a.index()] = t.get(Stat::Inflict(a), 0.0).max(0.0);
    }
    hit.debuffs = on.iter().filter_map(|s| s.mods.debuff).collect();
    hit.cruelty = on.iter().map(|s| s.mods.cruelty).sum();
    for h in Happened::ALL {
        for s in [Star::Body, Star::Speed, Star::Mind] {
            let chance = t.get(Stat::StarOn(h, s), 0.0);
            if chance > 0.0 {
                hit.star_on.push((h, s, chance));
            }
        }
    }
    hit.control = t.get(Stat::ControlDuration, 1.0).max(0.0);

    // costs: the cost modifiers; a share of the qi paid in life (Huyết Luyện, Huyết Tế); Phun Máu pays life on top
    let factor = t.get(Stat::Cost, 1.0).max(0.0);
    let mut costs: Vec<(Resource, f64)> =
        skill.costs.iter().map(|&(r, v)| (r, v * factor)).collect();
    let as_life = t.get(Stat::CostAsLife, 0.0).clamp(0.0, 1.0);
    if as_life > 0.0 {
        let mut qi = 0.0;
        for c in costs.iter_mut().filter(|c| c.0 == Resource::Qi) {
            qi += c.1 * as_life;
            c.1 *= 1.0 - as_life;
        }
        add_cost(&mut costs, Resource::Life, qi * LIFE_PER_QI);
    }
    add_cost(
        &mut costs,
        Resource::Life,
        on.iter().map(|s| s.mods.blood).sum(),
    );
    costs.retain(|c| c.1 > 0.0);
    let share: f64 = costs
        .iter()
        .filter(|c| c.0 == Resource::Life)
        .map(|c| c.1)
        .sum();
    let life = (share * striker.life_max)
        .min((striker.life - 1.0).max(0.0))
        .max(0.0);
    // the blood paid is a rule of the cast, read after the costs it comes from
    if life > 0.0 && striker.life_max > 0.0 {
        let blood = BLOOD_POWER * life / striker.life_max * 100.0;
        hit.more.push(blood);
        // a more to all the cast's damage: its burns and bleeds too
        for d in &mut hit.dot {
            d.more *= 1.0 + blood;
        }
    }

    for k in &keystones {
        k.on_strike(&mut hit, sit);
    }

    let speed = t.get(Stat::Speed, 1.0);
    let speed = if speed > 0.0 { speed } else { 1.0 };
    let recovery = t.get(Stat::CooldownRecovery, 1.0);
    let recovery = if recovery > 0.0 { recovery } else { 1.0 };
    let control = hit.control
        * if sit.target_guarded {
            on.iter().map(|s| s.mods.bait_control).product()
        } else {
            1.0
        };
    let reservation = t.get(Stat::Reservation, 1.0).max(0.0);
    let count = |s: Stat| libm::round(t.flat(s).clamp(0.0, f64::from(u8::MAX - 1))) as u8;
    Built {
        hit,
        rank,
        windup: skill.windup * t.get(Stat::Windup, 1.0).max(0.0) / speed,
        recover: skill.recover / speed,
        cooldown: skill.cooldown / recovery,
        costs,
        life,
        strikes: 1 + count(Stat::Repeats),
        projectiles: count(Stat::Projectiles),
        control,
        reservation,
        reserve: t.get(Stat::Reserved, 0.0).max(0.0) * reservation,
        behaviours: on
            .iter()
            .flat_map(|s| s.behaviours.iter().copied())
            .collect(),
        inert: off.iter().map(|s| s.id).collect(),
        table: t,
    }
}

fn add_cost(costs: &mut Vec<(Resource, f64)>, r: Resource, v: f64) {
    if v <= 0.0 {
        return;
    }
    match costs.iter_mut().find(|c| c.0 == r) {
        Some(c) => c.1 += v,
        None => costs.push((r, v)),
    }
}

/// The kinds of damage a skill deals with its fitting supports, as a row by [`Element::index`].
pub fn kinds(skill: &Skill, supports: &[&Support]) -> [bool; N] {
    let tags = skill.all_tags();
    let mut out = [false; N];
    for &(e, s) in &skill.split {
        if s > 0.0 {
            out[e.index()] = true;
        }
    }
    for s in supports.iter().filter(|s| s.fits(tags)) {
        for e in s.kinds() {
            out[e.index()] = true;
        }
    }
    out
}

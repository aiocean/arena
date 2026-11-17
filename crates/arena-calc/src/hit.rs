//! One blow through the twelve steps (design, section 5). The order is the system's depth: never swap two steps.
//! What the target brings to a blow (resistances, armour, damage taken, evasion) is read from its gathered table
//! ([`defence_table`]): its sheet, its timed effects and its state, so a curse, an intent or an ailment on it counts
//! without the caller adding it.
//! Step 9 (rank against shields) needs the field's things and lives in `act`; a caller that lets a shield stop the
//! blow never calls [`resolve`].

use crate::ailment::{AIL, AILMENTS, Ailment, Control, ForceAilment};
use crate::cultivator::{Active, BASE, Cultivator, RES, Star};
use crate::dot::{self, Dot, DotScale};
use crate::element::{Element, FUSE, Fusion, N, Parts, ROOT, root_factor};
use crate::grade::{Grade, Rank};
use crate::sheet::{EMPTY, Sheet};
use crate::stat::{Ctx, Happened, Mod, Source, Stat, Table};
use crate::tag::Tags;

/// Whether the blow caught the target in its windup (counter) or its recover (punish).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Frame {
    #[default]
    Plain,
    Counter,
    Punish,
}

/// Counter and punish multiply the blow by this.
pub const FRAME_MORE: f64 = 1.2;

/// The fire tier that is dao fire (black, the fourth): it overcomes a water root at step 7 and draws sense when its
/// burn fills.
pub const DAO_FIRE_TIER: u8 = 3;

/// A blow, built from a skill, its supports, the keystones and the striker (`gem::build`).
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// The skill's tags with its kinds of damage: what a target's keystones, guards and curses read.
    pub tags: Tags,
    /// Step 1: the base, split by kind.
    pub base: Parts,
    /// Step 2: flat added damage, scaled by `effectiveness`.
    pub added: Parts,
    pub effectiveness: f64,
    /// Step 3: `convert[from][to]`, only to a later kind.
    pub convert: [[f64; N]; N],
    /// Step 3, after conversion: `extra[from][to]`, a share of each part that is `from` gained as an extra part of
    /// `to` (gain as extra; any direction).
    pub extra: [[f64; N]; N],
    /// Step 4: increased from tags that touch every part, and by kind (a part counts every kind it has been).
    pub increased: f64,
    pub increased_by: Parts,
    /// Step 4: each "more" on its own, and more by kind.
    pub more: Vec<f64>,
    pub more_by: Parts,
    /// Step 5.
    pub crit: f64,
    pub crit_multi: f64,
    /// Step 5: the chance the blow deals double ([`Rolls::double`]).
    pub double: f64,
    /// Thuấn: never dodged, never a crit.
    pub instant: bool,
    /// Evasion can dodge it (an attack or a projectile, not instant, not Chắc Trúng).
    pub evadable: bool,
    /// Step 7: fire of the fourth tier (dao fire) overcomes a water root. A `fire_tier` of [`DAO_FIRE_TIER`] or more
    /// is dao fire too, with or without this flag.
    pub dao_fire: bool,
    /// The burn tier it lays (0 red .. 3 black), and whether it is hollow fire.
    pub fire_tier: u8,
    pub hollow_fire: bool,
    /// Step 8: armour ignored (a share), and resistance penetration by kind.
    pub armour_pen: f64,
    pub pen: Parts,
    /// Step 9's input, and what grades the effects it lays.
    pub grade: Grade,
    /// Step 10.
    pub frame: Frame,
    /// Step 12.
    pub force_ailment: ForceAilment,
    /// The striker's ailment effect and duration factors, by ailment (`stat::Table::ailment_factors`): the stacks a
    /// stacking ailment lays and each stack's time are scaled by them, and so are a burn's or a bleed's damage a
    /// second and its time.
    pub ailment_effect: [f64; AILMENTS],
    pub ailment_duration: [f64; AILMENTS],
    /// The striker's damage-over-time modifiers, by ailment ([`DotScale::of`]): what step 12 reads for a burn or a
    /// bleed in place of step 4's hit modifiers (with the per-kind `increased_by` and `more_by`).
    pub dot: [DotScale; AILMENTS],
    /// Chân giả: an illusory blow deals `ILLUSION` of its damage and lays nothing.
    pub illusory: bool,
    /// Life the striker drinks, as a share of what lands.
    pub leech: f64,
    /// Qi the striker drinks, as a share of what lands.
    pub qi_leech: f64,
    /// What the striker gains for a blow that lands (life, qi, sense, intent).
    pub on_hit: Gains,
    /// Step 11: the share of each part that meets the shield first passing it to life.
    pub shield_bypass: f64,
    /// Step 11, culling: a target left at or under this share of full life dies.
    pub execute: f64,
    /// Step 12: stacks of each ailment every landing blow lays besides its parts' (`Stat::Inflict`).
    pub inflict: [f64; AILMENTS],
    /// Step 12: timed effects the blow puts on the target ([`Debuff`]).
    pub debuffs: Vec<Debuff>,
    /// Step 12: damage over time this much more at most, by the hit's size ([`CRUELTY_LIFE`]).
    pub cruelty: f64,
    /// The striker's chances of a star on each thing that happens (`Stat::StarOn`).
    pub star_on: Vec<(Happened, Star, f64)>,
    /// Hard controls the blow lays last this many times as long (`Stat::ControlDuration`).
    pub control: f64,
    /// The striker's intent rank is higher than the target's (controls halve).
    pub higher_intent: bool,
}

/// What a striker gains from a blow that lands: life (as healing), qi, sense and killing intent. The fight gives
/// them to the striker (`hit::resolve` holds the striker by reference).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Gains {
    pub life: f64,
    pub qi: f64,
    pub sense: f64,
    pub intent: f64,
}

/// A timed effect a blow puts on its target: on every landing blow, or only when it lays stacks of `on` (Nhiên
/// Thiêu: a burn lowers fire resistance). Put on as a `cultivator::Active` of `source`, replacing one from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Debuff {
    pub source: Source,
    pub on: Option<Ailment>,
    pub mods: &'static [Mod],
    pub seconds: f64,
}

/// Cruelty: a blow's parts after step 7 worth this share of the target's full life give its damage over time the
/// whole of [`Hit::cruelty`]; a smaller blow a part of it.
pub const CRUELTY_LIFE: f64 = 0.1;

/// An illusion's share of a true blow; a believed one turns true and passes this much armour.
pub const ILLUSION: f64 = 0.5;
pub const ILLUSION_TRUE_PEN: f64 = 0.3;

impl Default for Hit {
    fn default() -> Self {
        Hit {
            tags: Tags::NONE,
            base: [0.0; N],
            added: [0.0; N],
            effectiveness: 1.0,
            convert: [[0.0; N]; N],
            extra: [[0.0; N]; N],
            increased: 0.0,
            increased_by: [0.0; N],
            more: Vec::new(),
            more_by: [0.0; N],
            crit: BASE.crit,
            crit_multi: BASE.crit_multi,
            double: 0.0,
            instant: false,
            evadable: false,
            dao_fire: false,
            fire_tier: 0,
            hollow_fire: false,
            armour_pen: 0.0,
            pen: [0.0; N],
            grade: Grade::Ranked(Rank::Art),
            frame: Frame::Plain,
            force_ailment: ForceAilment::Bleed,
            ailment_effect: [1.0; AILMENTS],
            ailment_duration: [1.0; AILMENTS],
            dot: [DotScale::default(); AILMENTS],
            illusory: false,
            leech: 0.0,
            qi_leech: 0.0,
            on_hit: Gains::default(),
            shield_bypass: 0.0,
            execute: 0.0,
            inflict: [0.0; AILMENTS],
            debuffs: Vec::new(),
            cruelty: 0.0,
            star_on: Vec::new(),
            control: 1.0,
            higher_intent: false,
        }
    }
}

impl Hit {
    /// A blow of `amount` split by `split` (kind, share).
    pub fn split(amount: f64, split: &[(Element, f64)]) -> Hit {
        let mut h = Hit::default();
        for &(e, s) in split {
            h.base[e.index()] += amount * s;
        }
        h
    }
}

/// The rolls a blow uses, each in [0, 1), drawn by the caller from the fight's dice in this order: evasion, crit,
/// double damage, then the stars. A roll under its chance succeeds; 1 never does.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rolls {
    pub evade: f64,
    pub crit: f64,
    /// Under [`Hit::double`]: the blow deals double (step 5).
    pub double: f64,
    /// One roll for every `Stat::StarOn` chance of the blow (the striker's and the target's): a star of each kind
    /// whose chance is above it.
    pub star: f64,
}

impl Rolls {
    /// Nothing happens by chance: no dodge, no crit, no double, no star.
    pub const NONE: Rolls = Rolls {
        evade: 1.0,
        crit: 1.0,
        double: 1.0,
        star: 1.0,
    };
}

/// What a blow did.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Outcome {
    pub evaded: bool,
    pub crit: bool,
    /// The blow dealt double (step 5).
    pub doubled: bool,
    /// Culling took the rest of the target's life (step 11).
    pub executed: bool,
    /// Each kind after step 7 (what ailments read).
    pub before: Parts,
    /// Each kind after step 10 (what the bars take).
    pub dealt: Parts,
    /// The fused part, after step 10, and what made it.
    pub fusion: Option<(Fusion, f64)>,
    pub to_shield: f64,
    pub to_life: f64,
    pub to_sense: f64,
    /// Stacks laid by ailment (1 for a burn or bleed instance laid), and those that filled.
    pub stacks: [f64; crate::ailment::AILMENTS],
    /// The burn and bleed instances laid, as snapshot.
    pub dots: Vec<Dot>,
    pub full: Vec<Ailment>,
    /// Ailments an overcoming blow cleared, and how many stacks.
    pub cleared: Vec<(Ailment, f64)>,
    /// Hard controls laid, after grit.
    pub controls: Vec<(Control, f64)>,
    /// What the striker takes back (duality), and drinks (leech, wood).
    pub recoil: f64,
    pub leech: f64,
    /// Qi the target takes from a same-phase part.
    pub target_qi: f64,
    /// What the striker gains: its on-hit gains, and the qi it drinks.
    pub gains: Gains,
    /// Stars the striker earns (`Stat::StarOn`), for the fight to add.
    pub stars: Vec<Star>,
}

impl Outcome {
    pub fn total(&self) -> f64 {
        self.dealt.iter().sum::<f64>() + self.fusion.map_or(0.0, |f| f.1)
    }
}

/// A part of damage and every kind it has been (step 3 to 4).
struct Bucket {
    to: usize,
    been: u16,
    amount: f64,
}

/// Steps 1 to 3: the parts by kind, each with every kind it has been; then what is gained as extra (`Hit::extra`), a
/// new part that has been both its source's kinds and its own.
fn buckets(hit: &Hit) -> Vec<Bucket> {
    // steps 1 and 2
    let mut buckets: Vec<Bucket> = (0..N)
        .map(|i| Bucket {
            to: i,
            been: 1 << i,
            amount: hit.base[i] + hit.added[i] * hit.effectiveness,
        })
        .filter(|b| b.amount > 0.0)
        .collect();
    // step 3: kind by kind, forward only, scaled down past 100%
    for (from, row) in hit.convert.iter().enumerate() {
        let out: f64 = row[from + 1..].iter().map(|s| s.max(0.0)).sum();
        if out <= 0.0 {
            continue;
        }
        let k = if out > 1.0 { 1.0 / out } else { 1.0 };
        let mut moved = Vec::new();
        for b in buckets.iter_mut().filter(|b| b.to == from) {
            let whole = b.amount;
            for (to, s) in row.iter().enumerate().skip(from + 1) {
                let share = s.max(0.0) * k;
                if share > 0.0 {
                    moved.push(Bucket {
                        to,
                        been: b.been | (1 << to),
                        amount: whole * share,
                    });
                    b.amount -= whole * share;
                }
            }
        }
        buckets.extend(moved);
    }
    // gain as extra, after conversion: from the parts as they stand, nothing gained gains again
    let mut gained = Vec::new();
    for b in &buckets {
        for (to, &share) in hit.extra[b.to].iter().enumerate() {
            if to != b.to && share > 0.0 {
                gained.push(Bucket {
                    to,
                    been: b.been | (1 << to),
                    amount: b.amount * share,
                });
            }
        }
    }
    buckets.extend(gained);
    buckets
}

/// The per-kind increase and more of a part that has been the kinds in `been` (a bit per [`Element::index`]).
fn by_kind(hit: &Hit, been: u16) -> (f64, f64) {
    let mut inc = 0.0;
    let mut more = 1.0;
    for (e, (i, m)) in hit.increased_by.iter().zip(&hit.more_by).enumerate() {
        if been & (1 << e) != 0 {
            inc += i;
            more *= 1.0 + m;
        }
    }
    (inc, more)
}

/// Steps 1 to 4: the parts by kind, each scaled by the increases and mores of every kind it has been.
pub fn scale(hit: &Hit, power: f64) -> Parts {
    let more: f64 = hit.more.iter().map(|m| 1.0 + m).product();
    let mut parts = [0.0; N];
    for b in &buckets(hit) {
        let (inc, by) = by_kind(hit, b.been);
        parts[b.to] += b.amount * (1.0 + hit.increased + inc).max(0.0) * more * by * power;
    }
    parts
}

/// Steps 1 to 4 for ailment `a`'s damage over time: the part of `a`'s kind, each piece scaled by the striker's
/// damage-over-time modifiers ([`Hit::dot`]) and by `DamageOf` of every kind it has been, never by the hit-only
/// ones. A burn whose fire came from converted force still takes "increased force".
pub fn dot_scale(hit: &Hit, a: Ailment, power: f64) -> f64 {
    let d = hit.dot[a.index()];
    let k = a.element().index();
    buckets(hit)
        .iter()
        .filter(|b| b.to == k)
        .map(|b| {
            let (inc, by) = by_kind(hit, b.been);
            b.amount * (1.0 + d.increased + inc).max(0.0) * d.more * by * power
        })
        .sum()
}

/// The target's side of a blow: its sheet (items, tree, keystones), and what its conditions read (how far the
/// striker stands, whether the target moves, whether the striker is cursed).
#[derive(Clone, Copy, Debug)]
pub struct Defence<'a> {
    pub sheet: &'a Sheet,
    pub distance: f64,
    pub moving: bool,
    pub striker_cursed: bool,
}

impl Default for Defence<'_> {
    /// The empty sheet, the striker far, the target still.
    fn default() -> Self {
        Defence {
            sheet: &EMPTY,
            distance: f64::INFINITY,
            moving: false,
            striker_cursed: false,
        }
    }
}

/// The target's defensive table for `hit` from `by`: its sheet, its timed effects (curses, intents pressing on it,
/// domains) and its state (stars, ailments), gathered with the target as `me`, the striker as `other` and the
/// blow's tags as the skill. Step 8 reads `Taken`, `TakenOf`, `Resist`, `ResistCap` and `Armour` from it, and the
/// dodge `Evasion`.
pub fn defence_table(
    target: &Cultivator,
    by: Option<&Cultivator>,
    hit: &Hit,
    def: &Defence,
) -> Table {
    let ctx = Ctx {
        other: by,
        other_cursed: def.striker_cursed,
        distance: def.distance,
        moving: def.moving,
        skill: hit.tags,
        ..Ctx::alone(target)
    };
    def.sheet.table(&ctx, [])
}

/// A blow from `by` (its intent and age, via [`Cultivator::power`]) lands on `target`, which wears nothing (the
/// empty sheet; its timed effects and state still count). `by` is `None` for damage with no striker (a lava zone, a
/// rebound). [`resolve_with`] for a target with a sheet.
pub fn resolve(
    hit: &Hit,
    by: Option<&Cultivator>,
    target: &mut Cultivator,
    rolls: Rolls,
) -> Outcome {
    resolve_with(hit, by, target, &Defence::default(), rolls)
}

/// [`resolve`] against a target defended by `def`. The dodge and step 8 read the target's gathered table
/// ([`defence_table`]), each when it comes, so what steps 6 and 7 changed (ailments cleared) counts.
pub fn resolve_with(
    hit: &Hit,
    by: Option<&Cultivator>,
    target: &mut Cultivator,
    def: &Defence,
    rolls: Rolls,
) -> Outcome {
    let mut out = Outcome::default();
    let power = by.map_or(1.0, Cultivator::power);
    let life_before = target.life;

    // dodge first: evasion takes no part of an instant
    let first = defence_table(target, by, hit, def);
    let evasion = first.get(Stat::Evasion, target.evasion);
    if hit.evadable && !hit.instant && rolls.evade < evasion {
        out.evaded = true;
        target.add_star(Star::Speed, first.get(Stat::StarsMax, RES.stars_max));
        return out;
    }

    // steps 1-4, and beside them each damage-over-time part as step 4 would make it for that ailment
    let mut parts = scale(hit, power);
    let mut dots = [0.0; N];
    for a in [
        Ailment::Burn,
        Ailment::of(Element::Force, hit.force_ailment),
    ] {
        if dot::is_dot(a) {
            dots[a.element().index()] = dot_scale(hit, a, power);
        }
    }
    if hit.illusory {
        for p in &mut parts {
            *p *= ILLUSION;
        }
    }

    // step 5: the crit, then double damage on its own roll
    if !hit.instant && rolls.crit < hit.crit {
        out.crit = true;
        for p in parts.iter_mut().chain(dots.iter_mut()) {
            *p *= hit.crit_multi;
        }
    }
    if rolls.double < hit.double {
        out.doubled = true;
        for p in parts.iter_mut().chain(dots.iter_mut()) {
            *p *= 2.0;
        }
    }

    // step 6: one pair a blow, the window's side spent
    for f in Fusion::ALL {
        let (a, b) = f.pair();
        let (ia, ib) = (a.index(), b.index());
        let made = if parts[ib] > 0.0 && target.in_window(a) > 0.0 {
            let v = FUSE.share * target.in_window(a).min(parts[ib]);
            target.spend_window(a);
            v
        } else if parts[ia] > 0.0 && target.in_window(b) > 0.0 {
            let v = FUSE.share * target.in_window(b).min(parts[ia]);
            target.spend_window(b);
            v
        } else {
            0.0
        };
        if made > 0.0 {
            out.fusion = Some((f, made));
            break;
        }
    }
    target.fill_window(&parts);

    // step 7: against the root, then against the ailments on the target. Black fire is dao fire whether or not the
    // flag was set with it.
    let dao_fire = hit.dao_fire || hit.fire_tier >= DAO_FIRE_TIER;
    let mut fed = [1.0; crate::ailment::AILMENTS];
    for e in Element::ALL {
        let i = e.index();
        if parts[i] <= 0.0 {
            continue;
        }
        let root = root_factor(e, target.root, dao_fire, target.root_weak);
        parts[i] *= root;
        dots[i] *= root;
        if target.root == Some(e) && e.is_phase() {
            out.target_qi += parts[i] * ROOT.same_qi;
        }
        if hit.illusory {
            continue;
        }
        // overcoming clears; thunder conducts through soak. The cash-in: a stacking ailment's stacks × a share of
        // full life; burn and bleed instances, the damage they would still have dealt (design 4.1, 17.5)
        let beaten = match e {
            Element::Thunder => Some(Ailment::Soak),
            _ => e.overcomes().map(|o| Ailment::of(o, ForceAilment::Bleed)),
        };
        if let Some(b) = beaten
            && target.ailments.stacks(b) > 0.0
        {
            let left = target.ailments.dots(b).map(dot::Instances::remaining);
            let n = target.ailments.clear(b);
            let cash = match left {
                Some(d) => d * dot::OVERCOME_REMAINING,
                None => n * AIL.overcome * target.life_max,
            };
            parts[i] += cash;
            if dots[i] > 0.0 {
                dots[i] += cash;
            }
            out.cleared.push((b, n));
        }
        if let Some(src) = e.fed_by()
            && target
                .ailments
                .stacks(Ailment::of(src, ForceAilment::Bleed))
                > 0.0
        {
            fed[Ailment::of(e, hit.force_ailment).index()] = 1.0 + AIL.fed;
        }
    }
    out.before = parts;

    // step 8: the target's table as it stands now (jolt, soak, wither, sunder, body stars, curses, intents...)
    let t = defence_table(target, by, hit, def);
    let taken = target.taken * t.get(Stat::Taken, 1.0) * t.get(Stat::HitTaken, 1.0);
    for e in Element::ALL {
        let i = e.index();
        if parts[i] <= 0.0 {
            continue;
        }
        if target.immune == Some(e) {
            parts[i] = 0.0;
            continue;
        }
        let cut = match e {
            Element::Force => {
                let pen = if hit.illusory { 0.0 } else { hit.armour_pen };
                let armour = t.get(Stat::Armour, target.armour).max(0.0);
                let h = armour * (1.0 - pen).max(0.0);
                let red = h / (h + BASE.armour_k * parts[i]);
                1.0 - red.min(BASE.armour_cap)
            }
            _ => {
                let cap = t.get(Stat::ResistCap(e), BASE.resist_cap);
                let resist = t
                    .get(Stat::Resist(e), target.resist[i])
                    .clamp(BASE.resist_floor, cap.max(BASE.resist_floor));
                let r = (resist - hit.pen[i]).max(BASE.resist_floor);
                1.0 - r
            }
        };
        parts[i] *= cut * taken * t.get(Stat::TakenOf(e), 1.0);
    }
    let mut fused = out.fusion.map(|(f, v)| (f, v * taken));

    // step 10
    let frame = if hit.frame == Frame::Plain {
        1.0
    } else {
        FRAME_MORE
    };
    for p in &mut parts {
        *p *= frame;
    }
    if let Some((_, v)) = &mut fused {
        *v *= frame;
    }
    out.dealt = parts;
    out.fusion = fused;

    // step 11
    for e in Element::ALL {
        let v = parts[e.index()];
        if v <= 0.0 {
            continue;
        }
        match e {
            Element::Yin => out.to_life += target.lose_life(v),
            Element::Soul => {
                let (s, l) = target.hurt_soul(v);
                out.to_sense += s;
                out.to_life += l;
            }
            _ => {
                let past = v * hit.shield_bypass.clamp(0.0, 1.0);
                let (s, l) = target.hurt(v - past);
                out.to_shield += s;
                out.to_life += l + target.lose_life(past);
            }
        }
    }
    if let Some((f, v)) = out.fusion {
        let (s, l) = target.hurt(v);
        out.to_shield += s;
        out.to_life += l;
        match f {
            Fusion::Duality => out.recoil += v * FUSE.duality_recoil,
            Fusion::Wind => target.wind_lock = target.wind_lock.max(FUSE.wind_lock),
            Fusion::Stormfire | Fusion::Lava => {}
        }
    }
    target.qi = (target.qi + out.target_qi).min(target.qi_max);
    // culling: what the blow left at or under the line dies
    if hit.execute > 0.0 && target.life > 0.0 && target.life <= hit.execute * target.life_max {
        out.to_life += target.lose_life(target.life);
        out.executed = true;
    }
    let stars_max = t.get(Stat::StarsMax, RES.stars_max);
    if out.to_shield + out.to_life > RES.star_body_hit * target.life_max {
        target.add_star(Star::Body, stars_max);
    }
    // the target's stars on being hit (and crit)
    for s in [Star::Body, Star::Speed, Star::Mind] {
        let mut chance = t.flat(Stat::StarOn(Happened::BeenHit, s));
        if out.crit {
            chance += t.flat(Stat::StarOn(Happened::BeenCrit, s));
        }
        if rolls.star < chance {
            target.add_star(s, stars_max);
        }
    }

    // step 12: from the parts before mitigation. Cruelty: damage over time more by the hit's size.
    let cruelty = 1.0
        + hit.cruelty.max(0.0)
            * (out.before.iter().sum::<f64>() / (CRUELTY_LIFE * target.life_max.max(1.0))).min(1.0);
    let controls = t.get(Stat::ControlTaken, 1.0).max(0.0) * hit.control.max(0.0);
    if !hit.illusory {
        for e in Element::ALL {
            let v = out.before[e.index()];
            // a kind the target is immune to lays nothing
            if v <= 0.0 || target.immune == Some(e) {
                continue;
            }
            let a = Ailment::of(e, hit.force_ailment);
            let ai = a.index();
            // an ailment of no effect is not laid (Ngũ Hành Ngưng)
            if hit.ailment_effect[ai] <= 0.0 {
                continue;
            }
            let res = t
                .get(Stat::AilmentResist(a), target.ailment_resist[ai])
                .min(BASE.ailment_resist_cap);
            if dot::is_dot(a) {
                // design 17.5: one instance, its damage a second snapshot now; fed means more effect
                let tier = if a == Ailment::Burn {
                    dot::tier_factor(hit.fire_tier)
                } else {
                    1.0
                };
                let dps = dot::rate(a)
                    * dots[e.index()]
                    * hit.dot[ai].multi
                    * hit.ailment_effect[ai].max(0.0)
                    * tier
                    * (1.0 - res)
                    * fed[ai]
                    * cruelty;
                let mut d = Dot::new(
                    a,
                    dps,
                    dot::base_time(a) * hit.ailment_duration[ai].max(0.0),
                );
                if a == Ailment::Burn {
                    d.tier = hit.fire_tier;
                    target.ailments.hollow_fire |= hit.hollow_fire;
                }
                if d.time <= 0.0 {
                    continue;
                }
                out.stacks[ai] = 1.0;
                out.dots.push(d);
                if target.ailments.lay(d) {
                    out.full.push(a);
                    full_effect_with(a, hit, target, &mut out, controls);
                }
                continue;
            }
            let mut n = a.data().k * v / target.life_max
                * AIL.scale
                * (1.0 - res)
                * hit.ailment_effect[ai].max(0.0)
                * fed[ai];
            // light and yin cancel stack for stack
            if let Some(o) = e.opposes() {
                let other = Ailment::of(o, hit.force_ailment);
                let gone = target.ailments.remove(other, libm::floor(n));
                n -= gone;
            }
            out.stacks[ai] = n;
            if target.ailments.add(a, n, hit.ailment_duration[ai].max(0.0)) {
                out.full.push(a);
                full_effect_with(a, hit, target, &mut out, controls);
            }
        }
        // what every landing blow lays besides its parts (stacking ailments)
        for a in Ailment::ALL {
            let ai = a.index();
            let n = hit.inflict[ai] * hit.ailment_effect[ai].max(0.0);
            if n <= 0.0 || dot::is_dot(a) {
                continue;
            }
            out.stacks[ai] += n;
            if target.ailments.add(a, n, hit.ailment_duration[ai].max(0.0)) {
                out.full.push(a);
                full_effect_with(a, hit, target, &mut out, controls);
            }
        }
        for d in &hit.debuffs {
            if d.on.is_none_or(|a| out.stacks[a.index()] > 0.0) {
                target.put_on(Active {
                    source: d.source,
                    mods: d.mods.to_vec(),
                    left: Some(d.seconds),
                });
            }
        }
        if out.before[Element::Wood.index()] > 0.0 {
            out.leech += out.dealt[Element::Wood.index()]
                * target.ailments.stacks(Ailment::Bind)
                * AIL.bind_leech;
        }
    }
    out.leech += hit.leech * out.total();
    out.gains = Gains {
        qi: hit.on_hit.qi + hit.qi_leech * out.total(),
        ..hit.on_hit
    };
    // the striker's stars
    let killed = life_before > 0.0 && target.life <= 0.0;
    for &(h, s, chance) in &hit.star_on {
        let on = match h {
            Happened::Hit => true,
            Happened::Crit => out.crit,
            Happened::Kill => killed,
            Happened::LaidControl => !out.controls.is_empty(),
            _ => false,
        };
        if on && rolls.star < chance && !out.stars.contains(&s) {
            out.stars.push(s);
        }
    }
    out
}

/// What a full ailment does at once.
pub fn full_effect(a: Ailment, hit: &Hit, target: &mut Cultivator, out: &mut Outcome) {
    full_effect_with(a, hit, target, out, hit.control);
}

/// [`full_effect`] with the hard control it lays lasting `control` times as long before grit (the striker's
/// `Stat::ControlDuration` times the target's `Stat::ControlTaken`).
pub fn full_effect_with(
    a: Ailment,
    hit: &Hit,
    target: &mut Cultivator,
    out: &mut Outcome,
    control: f64,
) {
    match a {
        Ailment::Sunder => target.break_shield(),
        Ailment::Soak => target.choke = a.data().seconds,
        Ailment::Wither => target.rot = a.data().seconds,
        Ailment::Burn if target.ailments.burn_tier() >= DAO_FIRE_TIER => {
            let (s, _) = target.hurt_soul(AIL.dao_fire_sense.min(target.sense));
            out.to_sense += s;
        }
        _ => {}
    }
    if let Some(c) = target.full_control_for(a, hit.higher_intent, control) {
        out.controls.push(c);
    }
}

/// A skill's base by its level (hỏa hầu 1..20): `base1 × 1.07^(level − 1)`.
pub fn spell_base(base1: f64, level: u8) -> f64 {
    base1 * libm::pow(1.07, f64::from(level.clamp(1, 20)) - 1.0)
}

/// An attack's base: the striker's might times the skill's effectiveness.
pub fn attack_base(might: f64, effectiveness: f64) -> f64 {
    might * effectiveness
}

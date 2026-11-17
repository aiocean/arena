//! One blow through the twelve steps (design, section 5). The order is the system's depth: never swap two steps.
//! Step 9 (rank against shields) needs the field's things and lives in `act`; a caller that lets a shield stop the
//! blow never calls [`resolve`].

use super::ailment::{AIL, Ailment, Control, ForceAilment};
use super::cultivator::{BASE, Cultivator, RES, Star};
use super::element::{Element, FUSE, Fusion, N, Parts, ROOT, root_factor};
use super::grade::{Grade, Rank};

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

/// A blow, built from a skill, its supports, the keystones and the striker (`gem::build`).
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Step 1: the base, split by kind.
    pub base: Parts,
    /// Step 2: flat added damage, scaled by `effectiveness`.
    pub added: Parts,
    pub effectiveness: f64,
    /// Step 3: `convert[from][to]`, only to a later kind.
    pub convert: [[f64; N]; N],
    /// Step 4: increased from tags that touch every part, and by kind (a part counts every kind it has been).
    pub increased: f64,
    pub increased_by: Parts,
    /// Step 4: each "more" on its own, and more by kind.
    pub more: Vec<f64>,
    pub more_by: Parts,
    /// Step 5.
    pub crit: f64,
    pub crit_multi: f64,
    /// Thuấn: never dodged, never a crit.
    pub instant: bool,
    /// Evasion can dodge it (an attack or a projectile, not instant, not Chắc Trúng).
    pub evadable: bool,
    /// Step 7: fire of the fourth tier (dao fire) overcomes a water root.
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
    pub ailment_more: f64,
    /// Chân giả: an illusory blow deals `ILLUSION` of its damage and lays nothing.
    pub illusory: bool,
    /// Life the striker drinks, as a share of what lands.
    pub leech: f64,
    /// The striker's intent rank is higher than the target's (controls halve).
    pub higher_intent: bool,
}

/// An illusion's share of a true blow; a believed one turns true and passes this much armour.
pub const ILLUSION: f64 = 0.5;
pub const ILLUSION_TRUE_PEN: f64 = 0.3;

impl Default for Hit {
    fn default() -> Self {
        Hit {
            base: [0.0; N],
            added: [0.0; N],
            effectiveness: 1.0,
            convert: [[0.0; N]; N],
            increased: 0.0,
            increased_by: [0.0; N],
            more: Vec::new(),
            more_by: [0.0; N],
            crit: BASE.crit,
            crit_multi: BASE.crit_multi,
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
            ailment_more: 0.0,
            illusory: false,
            leech: 0.0,
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

/// The two rolls a blow uses, drawn by the caller from the fight's dice in this order: evasion, then crit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rolls {
    pub evade: f64,
    pub crit: f64,
}

/// What a blow did.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Outcome {
    pub evaded: bool,
    pub crit: bool,
    /// Each kind after step 7 (what ailments read).
    pub before: Parts,
    /// Each kind after step 10 (what the bars take).
    pub dealt: Parts,
    /// The fused part, after step 10, and what made it.
    pub fusion: Option<(Fusion, f64)>,
    pub to_shield: f64,
    pub to_life: f64,
    pub to_sense: f64,
    /// Stacks laid by ailment, and those that filled.
    pub stacks: [f64; super::ailment::AILMENTS],
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

/// Steps 1 to 4: the parts by kind, each scaled by the increases and mores of every kind it has been.
pub fn scale(hit: &Hit, power: f64) -> Parts {
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
    for from in 0..N {
        let out: f64 = (from + 1..N).map(|to| hit.convert[from][to].max(0.0)).sum();
        if out <= 0.0 {
            continue;
        }
        let k = if out > 1.0 { 1.0 / out } else { 1.0 };
        let mut moved = Vec::new();
        for b in buckets.iter_mut().filter(|b| b.to == from) {
            let whole = b.amount;
            for to in from + 1..N {
                let share = hit.convert[from][to].max(0.0) * k;
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
    // step 4
    let more: f64 = hit.more.iter().map(|m| 1.0 + m).product();
    let mut parts = [0.0; N];
    for b in &buckets {
        let mut inc = hit.increased;
        let mut by = 1.0;
        for e in 0..N {
            if b.been & (1 << e) != 0 {
                inc += hit.increased_by[e];
                by *= 1.0 + hit.more_by[e];
            }
        }
        parts[b.to] += b.amount * (1.0 + inc).max(0.0) * more * by * power;
    }
    parts
}

/// A blow from `by` (its intent and age, via [`Cultivator::power`]) lands on `target`. `by` is `None` for damage
/// with no striker (a lava zone, a rebound).
pub fn resolve(hit: &Hit, by: Option<&Cultivator>, target: &mut Cultivator, rolls: Rolls) -> Outcome {
    let mut out = Outcome::default();
    let power = by.map_or(1.0, Cultivator::power);

    // dodge first: evasion takes no part of an instant
    if hit.evadable && !hit.instant && rolls.evade < target.evasion {
        out.evaded = true;
        target.add_star(Star::Speed, RES.stars_max);
        return out;
    }

    // steps 1-4
    let mut parts = scale(hit, power);
    if hit.illusory {
        for p in &mut parts {
            *p *= ILLUSION;
        }
    }

    // step 5
    if !hit.instant && rolls.crit < hit.crit {
        out.crit = true;
        for p in &mut parts {
            *p *= hit.crit_multi;
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

    // step 7: against the root, then against the ailments on the target
    let mut fed = [1.0; super::ailment::AILMENTS];
    for e in Element::ALL {
        let i = e.index();
        if parts[i] <= 0.0 {
            continue;
        }
        parts[i] *= root_factor(e, target.root, hit.dao_fire, target.root_weak);
        if target.root == Some(e) && e.is_phase() {
            out.target_qi += parts[i] * ROOT.same_qi;
        }
        if hit.illusory {
            continue;
        }
        // overcoming clears; thunder conducts through soak
        let beaten = match e {
            Element::Thunder => Some(Ailment::Soak),
            _ => e.overcomes().map(|o| Ailment::of(o, ForceAilment::Bleed)),
        };
        if let Some(b) = beaten
            && target.ailments.stacks(b) > 0.0
        {
            let n = target.ailments.clear(b);
            parts[i] += n * AIL.overcome * target.life_max;
            out.cleared.push((b, n));
        }
        if let Some(src) = e.fed_by()
            && target.ailments.stacks(Ailment::of(src, ForceAilment::Bleed)) > 0.0
        {
            fed[Ailment::of(e, hit.force_ailment).index()] = 1.0 + AIL.fed;
        }
    }
    out.before = parts;

    // step 8
    let jolt = 1.0 + target.ailments.stacks(Ailment::Jolt) * AIL.jolt_taken;
    let taken = target.taken_of() * jolt;
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
                let sunder = 1.0 - target.ailments.stacks(Ailment::Sunder) * AIL.sunder;
                let pen = if hit.illusory { 0.0 } else { hit.armour_pen };
                let h = target.armour * sunder.max(0.0) * (1.0 - pen).max(0.0);
                let red = h / (h + BASE.armour_k * parts[i]);
                1.0 - red.min(BASE.armour_cap)
            }
            _ => {
                let r = (target.resist_of(e) - hit.pen[i]).max(BASE.resist_floor);
                1.0 - r
            }
        };
        let more_taken = match e {
            Element::Thunder => 1.0 + target.ailments.stacks(Ailment::Soak) * AIL.soak_thunder,
            Element::Yin => 1.0 + target.ailments.stacks(Ailment::Wither) * AIL.wither_yin,
            _ => 1.0,
        };
        parts[i] *= cut * taken * more_taken;
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
                let (s, l) = target.hurt(v);
                out.to_shield += s;
                out.to_life += l;
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
    if out.to_shield + out.to_life > RES.star_body_hit * target.life_max {
        target.add_star(Star::Body, RES.stars_max);
    }

    // step 12: from the parts before mitigation
    if !hit.illusory {
        for e in Element::ALL {
            let v = out.before[e.index()];
            if v <= 0.0 {
                continue;
            }
            let a = Ailment::of(e, hit.force_ailment);
            let ai = a.index();
            let res = target.ailment_resist[ai].min(BASE.ailment_resist_cap);
            let mut n = a.data().k * v / target.life_max * AIL.scale
                * (1.0 - res)
                * (1.0 + hit.ailment_more)
                * fed[ai];
            // light and yin cancel stack for stack
            if let Some(o) = e.opposes() {
                let other = Ailment::of(o, hit.force_ailment);
                let gone = target.ailments.remove(other, libm::floor(n));
                n -= gone;
            }
            if a == Ailment::Burn {
                if hit.fire_tier >= target.ailments.burn_tier {
                    target.ailments.burn_tier = hit.fire_tier;
                }
                target.ailments.hollow_fire |= hit.hollow_fire;
            }
            out.stacks[ai] = n;
            if target.ailments.add(a, n, fed[ai].max(1.0)) {
                out.full.push(a);
                full_effect(a, hit, target, &mut out);
            }
        }
        if out.before[Element::Wood.index()] > 0.0 {
            out.leech += out.dealt[Element::Wood.index()]
                * target.ailments.stacks(Ailment::Bind)
                * AIL.bind_leech;
        }
    }
    out.leech += hit.leech * out.total();
    out
}

/// What a full ailment does at once.
fn full_effect(a: Ailment, hit: &Hit, target: &mut Cultivator, out: &mut Outcome) {
    match a {
        Ailment::Sunder => target.break_shield(),
        Ailment::Soak => target.choke = a.data().seconds,
        Ailment::Wither => target.rot = a.data().seconds,
        Ailment::Burn if target.ailments.burn_tier >= 3 => {
            let (s, _) = target.hurt_soul(AIL.dao_fire_sense.min(target.sense));
            out.to_sense += s;
        }
        _ => {}
    }
    if let Some(c) = target.full_control(a, hit.higher_intent) {
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

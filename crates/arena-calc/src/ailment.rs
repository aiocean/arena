//! Ailments (design, 4 and 5.1): each kind of damage lays its own, in stacks that expire one by one; a full stack
//! fires its full effect, empties, and locks for a while. Burn and bleed are damage over time instead (design,
//! 17.5; [`crate::dot`]): each blow lays an instance with its own damage and time, and their "stacks" are the
//! instances. No ailment freezes.

use crate::dot::{self, BLEED_TIME, BURN_TIME, DOT_CAP, Dot, HOLLOW_FIRE_EFFECT, Instances};
use crate::element::Element;

/// How many ailments there are.
pub const AILMENTS: usize = 11;

/// An ailment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Ailment {
    /// Thương (force, from swords, fingers and strikes): bleeds, twice as fast while moving.
    Bleed,
    /// Chấn (force, from fists, palms and slams): full, a stun.
    Daze,
    /// Phá (metal): each stack cuts armour; full, the energy shield breaks.
    Sunder,
    /// Triền (wood): slows; full, roots.
    Bind,
    /// Thấm (water): slows qi regen, raises thunder taken; full, Ngạt (no qi regen, slow cooldowns).
    Soak,
    /// Thiêu (fire): burns past armour and shield.
    Burn,
    /// Trọng (earth): shortens dashes and escapes, drops flyers; full, a knockdown.
    Heavy,
    /// Tê (thunder): raises all damage taken; full, a jolt that breaks a windup.
    Jolt,
    /// Chiếu (light): shows illusions and the hidden, weakens yin buffs; full, wipes them.
    Glare,
    /// Tịch (yin): cuts healing, raises yin taken; full, healing hurts.
    Wither,
    /// Nhiếp (soul): lengthens windups.
    Dread,
}

impl Ailment {
    pub const ALL: [Ailment; AILMENTS] = [
        Ailment::Bleed,
        Ailment::Daze,
        Ailment::Sunder,
        Ailment::Bind,
        Ailment::Soak,
        Ailment::Burn,
        Ailment::Heavy,
        Ailment::Jolt,
        Ailment::Glare,
        Ailment::Wither,
        Ailment::Dread,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Ailment::Bleed => "bleed",
            Ailment::Daze => "daze",
            Ailment::Sunder => "sunder",
            Ailment::Bind => "bind",
            Ailment::Soak => "soak",
            Ailment::Burn => "burn",
            Ailment::Heavy => "heavy",
            Ailment::Jolt => "jolt",
            Ailment::Glare => "glare",
            Ailment::Wither => "wither",
            Ailment::Dread => "dread",
        }
    }

    pub fn data(self) -> &'static AilmentData {
        &AILMENT[self.index()]
    }

    /// The element whose damage lays it.
    pub fn element(self) -> Element {
        match self {
            Ailment::Bleed | Ailment::Daze => Element::Force,
            Ailment::Sunder => Element::Metal,
            Ailment::Bind => Element::Wood,
            Ailment::Soak => Element::Water,
            Ailment::Burn => Element::Fire,
            Ailment::Heavy => Element::Earth,
            Ailment::Jolt => Element::Thunder,
            Ailment::Glare => Element::Light,
            Ailment::Wither => Element::Yin,
            Ailment::Dread => Element::Soul,
        }
    }

    /// The ailment a part of `e` lays; force lays bleed or daze by the blow's form.
    pub fn of(e: Element, force: ForceAilment) -> Ailment {
        match e {
            Element::Force => match force {
                ForceAilment::Bleed => Ailment::Bleed,
                ForceAilment::Daze => Ailment::Daze,
            },
            Element::Metal => Ailment::Sunder,
            Element::Wood => Ailment::Bind,
            Element::Water => Ailment::Soak,
            Element::Fire => Ailment::Burn,
            Element::Earth => Ailment::Heavy,
            Element::Thunder => Ailment::Jolt,
            Element::Light => Ailment::Glare,
            Element::Yin => Ailment::Wither,
            Element::Soul => Ailment::Dread,
        }
    }
}

/// Which ailment a blow's force lays: swords, fingers and strikes cut (bleed); fists, palms and slams shake (daze).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ForceAilment {
    #[default]
    Bleed,
    Daze,
}

/// A hard control a full ailment (or a skill) lays; each is shortened by grit (Kiên).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Control {
    Stun,
    Root,
    Knockdown,
    Jolt,
    /// Định thân: held in place, the cast broken.
    Hold,
    /// Thất hồn: silenced and the intents put out.
    Soulless,
}

/// One ailment's numbers: `k` scales the stacks a part lays, `cap` the stacks that make it full, `time` each stack's
/// life; `control` and `seconds` what a full one does, if it is a hard control. Burn and bleed lay instances, not
/// stacks: their `k` is unused, their `cap` is [`DOT_CAP`] and their `time` an instance's base ([`crate::dot`]).
pub struct AilmentData {
    pub k: f64,
    pub cap: f64,
    pub time: f64,
    pub control: Option<Control>,
    pub seconds: f64,
}

pub const AILMENT: [AilmentData; AILMENTS] = [
    AilmentData {
        k: 1.0,
        cap: DOT_CAP as f64,
        time: BLEED_TIME,
        control: None,
        seconds: 0.0,
    },
    AilmentData {
        k: 1.2,
        cap: 10.0,
        time: 3.0,
        control: Some(Control::Stun),
        seconds: 0.5,
    },
    AilmentData {
        k: 1.0,
        cap: 5.0,
        time: 5.0,
        control: None,
        seconds: 0.0,
    },
    AilmentData {
        k: 1.0,
        cap: 6.0,
        time: 3.0,
        control: Some(Control::Root),
        seconds: 0.8,
    },
    AilmentData {
        k: 1.0,
        cap: 6.0,
        time: 4.0,
        control: None,
        seconds: 2.0,
    },
    AilmentData {
        k: 0.8,
        cap: DOT_CAP as f64,
        time: BURN_TIME,
        control: None,
        seconds: 0.0,
    },
    AilmentData {
        k: 1.0,
        cap: 5.0,
        time: 3.0,
        control: Some(Control::Knockdown),
        seconds: 0.6,
    },
    AilmentData {
        k: 1.0,
        cap: 5.0,
        time: 3.0,
        control: Some(Control::Jolt),
        seconds: 0.3,
    },
    AilmentData {
        k: 1.0,
        cap: 3.0,
        time: 3.0,
        control: None,
        seconds: 0.0,
    },
    AilmentData {
        k: 1.0,
        cap: 6.0,
        time: 5.0,
        control: None,
        seconds: 3.0,
    },
    AilmentData {
        k: 1.0,
        cap: 5.0,
        time: 3.0,
        control: None,
        seconds: 0.0,
    },
];

/// The numbers every ailment shares and each stack's effect.
pub struct Ail {
    /// Stacks a part lays per whole of the target's full life, before `k`.
    pub scale: f64,
    /// After a full effect, no stacks for this long.
    pub lock: f64,
    /// Sunder: armour lost per stack.
    pub sunder: f64,
    /// Bind: run speed lost per stack, and the share of a wood blow the striker drinks.
    pub bind_slow: f64,
    pub bind_leech: f64,
    /// Soak: qi regen lost per stack, thunder taken per stack.
    pub soak_qi: f64,
    pub soak_thunder: f64,
    /// Ngạt (full soak): cooldowns run at this speed.
    pub choke_cooldown: f64,
    /// Dao fire (black) draws this much sense when the burn fills.
    pub dao_fire_sense: f64,
    /// Heavy: dash and escape reach lost per stack; a flyer falls this many T per stack.
    pub heavy_reach: f64,
    pub heavy_fall: f64,
    /// Jolt: all damage taken per stack.
    pub jolt_taken: f64,
    /// Glare: yin buffs' effect lost per stack.
    pub glare_yin: f64,
    /// Wither: healing lost per stack, yin taken per stack.
    pub wither_heal: f64,
    pub wither_yin: f64,
    /// Dread: windup longer per stack.
    pub dread_windup: f64,
    /// An overcoming blow clears the overcome ailment and adds this share of full life per stack it cleared. Burn and
    /// bleed cash in the damage their instances would still have dealt instead (`dot::OVERCOME_REMAINING`).
    pub overcome: f64,
    /// A fed ailment lays this many more stacks; a fed burn or bleed instance has this much more effect.
    pub fed: f64,
}
pub const AIL: Ail = Ail {
    scale: 20.0,
    lock: 2.0,
    sunder: 0.1,
    bind_slow: 0.08,
    bind_leech: 0.02,
    soak_qi: 0.08,
    soak_thunder: 0.05,
    choke_cooldown: 0.5,
    dao_fire_sense: 20.0,
    heavy_reach: 0.1,
    heavy_fall: 0.5,
    jolt_taken: 0.04,
    glare_yin: 0.15,
    wither_heal: 0.1,
    wither_yin: 0.03,
    dread_windup: 0.06,
    overcome: 0.02,
    fed: 0.5,
};

/// One ailment on a fighter: each stack's time left (oldest first), the fraction carried toward the next stack, and
/// the lock after a full effect.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Stacks {
    pub left: Vec<f64>,
    pub carry: f64,
    pub lock: f64,
}

impl Stacks {
    pub fn count(&self) -> f64 {
        self.left.len() as f64
    }
}

/// Every ailment on a fighter: the stacks of each (empty for burn and bleed), the burn and bleed instances, and hollow
/// fire.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Ailments {
    pub on: [Stacks; AILMENTS],
    /// Thiêu: only the strongest deals damage.
    pub burns: Instances,
    /// Thương: every one deals damage.
    pub bleeds: Instances,
    /// Hư hỏa: each cast the bearer makes while burning lays a copy of the strongest burn ([`HOLLOW_FIRE_EFFECT`]).
    pub hollow_fire: bool,
}

impl Ailments {
    /// The stacks of `a`: for burn and bleed, the instances it carries.
    pub fn stacks(&self, a: Ailment) -> f64 {
        match self.dots(a) {
            Some(d) => d.len() as f64,
            None => self.on[a.index()].count(),
        }
    }

    /// The instances of a damage-over-time ailment (burn, bleed); `None` for the others.
    pub fn dots(&self, a: Ailment) -> Option<&Instances> {
        match a {
            Ailment::Burn => Some(&self.burns),
            Ailment::Bleed => Some(&self.bleeds),
            _ => None,
        }
    }

    fn dots_mut(&mut self, a: Ailment) -> Option<&mut Instances> {
        match a {
            Ailment::Burn => Some(&mut self.burns),
            Ailment::Bleed => Some(&mut self.bleeds),
            _ => None,
        }
    }

    /// Lays a burn or bleed instance ([`Instances::lay`]); returns whether it made the ailment full. Any other
    /// ailment is ignored.
    pub fn lay(&mut self, d: Dot) -> bool {
        self.dots_mut(d.ailment).is_some_and(|v| v.lay(d))
    }

    /// The highest fire tier among the burns (0 with none).
    pub fn burn_tier(&self) -> u8 {
        self.burns.0.iter().map(|d| d.tier).max().unwrap_or(0)
    }

    /// Adds `amount` stacks (fractions carry) lasting `time_scale` × the ailment's time. Returns whether it filled:
    /// then the stacks are gone and the ailment is locked. For burn and bleed it lays whole bare instances (no
    /// damage, the base time × `time_scale`): stacks the five-phase rules count but that deal nothing, for a rule or
    /// a test with no blow behind it.
    pub fn add(&mut self, a: Ailment, amount: f64, time_scale: f64) -> bool {
        let d = a.data();
        if dot::is_dot(a) {
            if amount <= 0.0 {
                return false;
            }
            let mut carry = self.on[a.index()].carry + amount;
            let mut full = false;
            while carry >= 1.0 {
                carry -= 1.0;
                full |= self.lay(Dot::new(a, 0.0, dot::base_time(a) * time_scale));
            }
            self.on[a.index()].carry = carry;
            return full;
        }
        let s = &mut self.on[a.index()];
        if s.lock > 0.0 || amount <= 0.0 {
            return false;
        }
        s.carry += amount;
        while s.carry >= 1.0 {
            s.carry -= 1.0;
            s.left.push(d.time * time_scale);
            if s.count() >= d.cap {
                s.left.clear();
                s.carry = 0.0;
                s.lock = AIL.lock;
                return true;
            }
        }
        false
    }

    /// Clears an ailment; returns how many stacks (instances, for burn and bleed) it had.
    pub fn clear(&mut self, a: Ailment) -> f64 {
        let n = self.stacks(a);
        let s = &mut self.on[a.index()];
        s.left.clear();
        s.carry = 0.0;
        if let Some(v) = self.dots_mut(a) {
            v.0.clear();
        }
        if a == Ailment::Burn {
            self.hollow_fire = false;
        }
        n
    }

    /// Removes up to `n` of the oldest stacks (the earliest laid instances); returns how many it removed.
    pub fn remove(&mut self, a: Ailment, n: f64) -> f64 {
        let k = n.max(0.0) as usize;
        if let Some(v) = self.dots_mut(a) {
            return v.remove(k) as f64;
        }
        let s = &mut self.on[a.index()];
        let k = k.min(s.left.len());
        s.left.drain(..k);
        k as f64
    }

    /// Hư hỏa on a cast: while burning with hollow fire, lays a copy of the strongest burn at
    /// [`HOLLOW_FIRE_EFFECT`] of its damage, its tier, and a fresh full time. Returns whether it laid one.
    pub fn hollow_cast(&mut self) -> bool {
        if !self.hollow_fire {
            return false;
        }
        let Some(top) = self.burns.strongest().copied() else {
            return false;
        };
        let mut copy = Dot::new(Ailment::Burn, top.dps * HOLLOW_FIRE_EFFECT, top.time);
        copy.tier = top.tier;
        self.burns.lay(copy);
        true
    }

    /// Time passes: stacks, instances and locks run out.
    pub fn tick(&mut self, dt: f64) {
        for s in &mut self.on {
            s.lock = (s.lock - dt).max(0.0);
            for t in &mut s.left {
                *t -= dt;
            }
            s.left.retain(|&t| t > 0.0);
        }
        self.burns.tick(dt);
        self.bleeds.tick(dt);
        if self.burns.is_empty() {
            self.hollow_fire = false;
        }
    }

    /// Washes every ailment off.
    pub fn cleanse(&mut self) {
        for a in Ailment::ALL {
            self.clear(a);
        }
    }
}

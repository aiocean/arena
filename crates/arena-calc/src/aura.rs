//! Ý cảnh (aura) and tranh đạo niệm (rules 7.1 and 7.2, research 6.2 and 6.3). An aura holds sense (`sense_reserved`
//! on the [`Cultivator`]) while it is lit, reaches [`AURA`]'s radius around its holder, and goes out when the holder
//! loses the soul (thất hồn). A self aura (ý cảnh thân) helps its holder; a pressing aura (ý cảnh áp) weighs on a foe
//! inside it. Two sides' pressing auras overlapping open a [`Contest`]: a bar pushed by each side's force until one
//! loses the soul. What an aura does to a stat is a modifier ([`Aura::holder_mods`], [`Aura::foe_mods`]) that
//! [`sync`] puts on the holder or the foe each tick. Positions are in T along the floor. Pure and deterministic: no
//! dice.

use crate::cultivator::{Active, BASE, Cultivator};
use crate::grade::{Grade, Rank, dispels, wipes_field, zone_touches};
use crate::sheet::Sheet;
use crate::stat::{Ctx, Mod, Source, Stat};

/// The numbers every aura shares.
pub struct AuraRules {
    /// The zone's radius in T (Trung, 3T).
    pub radius: f64,
    /// Auras a fighter can hold: at least, at most (1 to 3, by its dao map).
    pub slots_min: usize,
    pub slots_max: usize,
    /// Seconds from lighting (or relighting after thất hồn) until an aura works.
    pub windup: f64,
    /// Hỉ Nộ Ai Oán: seconds each mood lasts before the next.
    pub mood_period: f64,
}
pub const AURA: AuraRules = AuraRules {
    radius: 3.0,
    slots_min: 1,
    slots_max: 3,
    windup: 1.0,
    mood_period: 3.0,
};

/// How many kinds of aura there are.
pub const AURA_KINDS: usize = 15;

/// An aura. The first four are self auras, the rest pressing ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum AuraKind {
    /// Sát lục: the holder's damage rises with its killing intent.
    Slaughter,
    /// Tuế nguyệt: a foe's projectiles in the zone fly slower (TN267).
    Years,
    /// Niềm tin: the holder's fist chain cannot be broken (Thập Băng).
    Belief,
    /// Chiến: the holder does not flinch while attacking.
    War,
    /// An nhàn: a foe inside attacks slower and loses killing intent (TN260).
    Ease,
    /// Vô tình: the holder's blows spill onto a foe's sense (TN299).
    Dispassion,
    /// Điên cuồng: a foe casting inside gives the holder qi (TN258).
    Frenzy,
    /// Hỉ Nộ Ai Oán: four moods in turn, each weighing differently (CM5).
    Moods,
    /// Luân hồi: a foe inside heals less.
    Rebirth,
    /// Dục (dâm tà): a foe inside is slowed and weakened.
    Lust,
    /// Tỏa Thần ("vạn tà bất xâm"): a foe inside regrows sense slower.
    SoulLock,
    /// Hồi ức: a foe inside deals less.
    Memory,
    /// Si: a foe inside is slowed.
    Infatuation,
    /// Tuyệt tình: a foe inside takes more.
    Severance,
    /// Mê hoặc: a foe inside is slowed and takes more. Named in the counters table (research 6.3).
    Charm,
}

impl AuraKind {
    pub const ALL: [AuraKind; AURA_KINDS] = [
        AuraKind::Slaughter,
        AuraKind::Years,
        AuraKind::Belief,
        AuraKind::War,
        AuraKind::Ease,
        AuraKind::Dispassion,
        AuraKind::Frenzy,
        AuraKind::Moods,
        AuraKind::Rebirth,
        AuraKind::Lust,
        AuraKind::SoulLock,
        AuraKind::Memory,
        AuraKind::Infatuation,
        AuraKind::Severance,
        AuraKind::Charm,
    ];

    /// Its place in [`AuraKind::ALL`] and [`AURAS`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Its numbers and rules.
    pub fn data(self) -> &'static AuraData {
        &AURAS[self.index()]
    }

    /// A pressing aura (ý cảnh áp), the kind that enters a contest.
    pub fn pressing(self) -> bool {
        self.data().pressing
    }
}

/// What a pressing aura does to a foe inside it: shares lost (speed, damage dealt, qi regen, healing, sense regen),
/// damage taken more, and killing intent lost a second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Press {
    pub speed: f64,
    pub damage: f64,
    pub taken: f64,
    pub intent_drain: f64,
    pub qi_regen: f64,
    pub heal: f64,
    pub sense_regen: f64,
}

impl Press {
    /// No weight at all.
    pub const NONE: Press = Press {
        speed: 0.0,
        damage: 0.0,
        taken: 0.0,
        intent_drain: 0.0,
        qi_regen: 0.0,
        heal: 0.0,
        sense_regen: 0.0,
    };
}

impl Press {
    /// Its stat shares as modifiers from `source`, each aura's its own multiplier: speed and damage dealt less,
    /// damage taken more, qi regen and healing less. Killing intent drained and sense regen lost are rates the fight
    /// applies itself (no stat holds them).
    pub fn mods(&self, source: Source) -> Vec<Mod> {
        [
            (Stat::Speed, -self.speed),
            (Stat::Damage, -self.damage),
            (Stat::Taken, self.taken),
            (Stat::QiRegen, -self.qi_regen),
            (Stat::Healing, -self.heal),
        ]
        .into_iter()
        .filter(|&(_, v)| v != 0.0)
        .map(|(stat, v)| Mod::more(stat, v, source))
        .collect()
    }
}

impl core::ops::Add for Press {
    type Output = Press;

    /// Two presses together: each share adds.
    fn add(self, o: Press) -> Press {
        Press {
            speed: self.speed + o.speed,
            damage: self.damage + o.damage,
            taken: self.taken + o.taken,
            intent_drain: self.intent_drain + o.intent_drain,
            qi_regen: self.qi_regen + o.qi_regen,
            heal: self.heal + o.heal,
            sense_regen: self.sense_regen + o.sense_regen,
        }
    }
}

/// One kind of aura's numbers. Starting numbers: the ladder sets the real ones.
pub struct AuraData {
    /// Its name in the novels.
    pub name: &'static str,
    /// A pressing aura (ý cảnh áp) rather than a self aura (ý cảnh thân).
    pub pressing: bool,
    /// An emotion (An nhàn, Hỉ Nộ Ai Oán, Dục, Si, Mê hoặc): Tâm Xám is immune to it.
    pub emotional: bool,
    /// Sense held while lit (20 to 35).
    pub reserve: f64,
    /// Sát lục: the holder's damage more per point of killing intent.
    pub more_per_intent: f64,
    /// Tuế nguyệt: a foe's projectile inside flies this share slower.
    pub slow_shots: f64,
    /// Niềm tin: the holder's fist chain cannot be broken.
    pub unbroken_fists: bool,
    /// Chiến: no flinch while attacking.
    pub no_flinch: bool,
    /// On a foe inside.
    pub press: Press,
    /// Vô tình: this share of the holder's blows is dealt to the foe's sense as well.
    pub spill_sense: f64,
    /// Điên cuồng: qi the holder gains each time a foe inside casts.
    pub qi_per_foe_cast: f64,
}

const SELF: AuraData = AuraData {
    name: "",
    pressing: false,
    emotional: false,
    reserve: 20.0,
    more_per_intent: 0.0,
    slow_shots: 0.0,
    unbroken_fists: false,
    no_flinch: false,
    press: Press::NONE,
    spill_sense: 0.0,
    qi_per_foe_cast: 0.0,
};
const PRESSING: AuraData = AuraData {
    pressing: true,
    ..SELF
};

/// Every aura's numbers, in [`AuraKind::ALL`]'s order.
pub const AURAS: [AuraData; AURA_KINDS] = [
    AuraData {
        name: "Sát lục",
        reserve: 25.0,
        more_per_intent: 0.003,
        ..SELF
    },
    AuraData {
        name: "Tuế nguyệt",
        reserve: 30.0,
        slow_shots: 0.4,
        ..SELF
    },
    AuraData {
        name: "Niềm tin",
        reserve: 20.0,
        unbroken_fists: true,
        ..SELF
    },
    AuraData {
        name: "Chiến",
        reserve: 20.0,
        no_flinch: true,
        ..SELF
    },
    AuraData {
        name: "An nhàn",
        emotional: true,
        reserve: 25.0,
        press: Press {
            speed: 0.15,
            intent_drain: 3.0,
            ..Press::NONE
        },
        ..PRESSING
    },
    AuraData {
        name: "Vô tình",
        reserve: 30.0,
        spill_sense: 0.15,
        ..PRESSING
    },
    AuraData {
        name: "Điên cuồng",
        reserve: 25.0,
        qi_per_foe_cast: 8.0,
        ..PRESSING
    },
    AuraData {
        name: "Hỉ Nộ Ai Oán",
        emotional: true,
        reserve: 35.0,
        ..PRESSING
    },
    AuraData {
        name: "Luân hồi",
        reserve: 35.0,
        press: Press {
            heal: 0.3,
            ..Press::NONE
        },
        ..PRESSING
    },
    AuraData {
        name: "Dục",
        emotional: true,
        reserve: 25.0,
        press: Press {
            speed: 0.1,
            damage: 0.05,
            ..Press::NONE
        },
        ..PRESSING
    },
    AuraData {
        name: "Tỏa Thần",
        reserve: 30.0,
        press: Press {
            sense_regen: 0.5,
            ..Press::NONE
        },
        ..PRESSING
    },
    AuraData {
        name: "Hồi ức",
        reserve: 25.0,
        press: Press {
            damage: 0.08,
            ..Press::NONE
        },
        ..PRESSING
    },
    AuraData {
        name: "Si",
        emotional: true,
        reserve: 20.0,
        press: Press {
            speed: 0.12,
            ..Press::NONE
        },
        ..PRESSING
    },
    AuraData {
        name: "Tuyệt tình",
        reserve: 30.0,
        press: Press {
            taken: 0.08,
            ..Press::NONE
        },
        ..PRESSING
    },
    AuraData {
        name: "Mê hoặc",
        emotional: true,
        reserve: 20.0,
        press: Press {
            speed: 0.1,
            taken: 0.05,
            ..Press::NONE
        },
        ..PRESSING
    },
];

/// Hỉ Nộ Ai Oán's four moods, in the order they turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Mood {
    /// Hỉ: a foe inside deals less.
    Joy,
    /// Nộ: a foe inside takes more.
    Anger,
    /// Ai: a foe inside regrows qi slower.
    Sorrow,
    /// Oán: a foe inside heals less.
    Resentment,
}

impl Mood {
    pub const ALL: [Mood; 4] = [Mood::Joy, Mood::Anger, Mood::Sorrow, Mood::Resentment];

    /// The mood after `age` seconds lit.
    pub fn at(age: f64) -> Mood {
        let turn = libm::floor(age.max(0.0) / AURA.mood_period) as usize;
        Mood::ALL[turn % 4]
    }

    /// What this mood weighs on a foe inside.
    pub fn press(self) -> Press {
        match self {
            Mood::Joy => Press {
                damage: 0.1,
                ..Press::NONE
            },
            Mood::Anger => Press {
                taken: 0.1,
                ..Press::NONE
            },
            Mood::Sorrow => Press {
                qi_regen: 0.5,
                ..Press::NONE
            },
            Mood::Resentment => Press {
                heal: 0.5,
                ..Press::NONE
            },
        }
    }
}

/// Which aura overcomes which in a contest (research 6.3): the first overcomes every one in its list.
pub const COUNTERS: &[(AuraKind, &[AuraKind])] = &[
    (AuraKind::Rebirth, &[AuraKind::Severance, AuraKind::Years]),
    (AuraKind::Lust, &[AuraKind::Dispassion]),
    (AuraKind::Slaughter, &[AuraKind::Ease, AuraKind::Charm]),
    (AuraKind::SoulLock, &[AuraKind::Lust, AuraKind::Charm]),
    (
        AuraKind::Dispassion,
        &[AuraKind::Memory, AuraKind::Infatuation],
    ),
];

/// Whether `a` overcomes `b`.
pub fn counters(a: AuraKind, b: AuraKind) -> bool {
    COUNTERS
        .iter()
        .any(|(k, list)| *k == a && list.contains(&b))
}

/// Whether a holder at `holder` reaches `x`.
pub fn reaches(holder: f64, x: f64) -> bool {
    (x - holder).abs() <= AURA.radius
}

/// One aura on a fighter.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Aura {
    pub kind: AuraKind,
    /// The rank of the skill that lit it.
    pub rank: Rank,
    /// Tự ngộ (the fighter's own) rather than borrowed from a treasure: wins a contest of equal rank.
    pub self_realised: bool,
    /// Seconds of windup left; it works at 0.
    pub windup: f64,
    /// Seconds it has worked.
    pub age: f64,
    /// The sense it holds: its [`AuraData::reserve`] times the holder's reservation when lit ([`Auras::light_with`]).
    pub reserve: f64,
}

impl Aura {
    /// An aura about to be lit: the whole windup ahead.
    pub fn new(kind: AuraKind, rank: Rank, self_realised: bool) -> Aura {
        Aura {
            kind,
            rank,
            self_realised,
            windup: AURA.windup,
            age: 0.0,
            reserve: kind.data().reserve,
        }
    }

    /// Past its windup.
    pub fn lit(&self) -> bool {
        self.windup <= 0.0
    }

    /// Its numbers.
    pub fn data(&self) -> &'static AuraData {
        self.kind.data()
    }

    /// Hỉ Nộ Ai Oán's mood now, if it is that aura.
    pub fn mood(&self) -> Option<Mood> {
        (self.kind == AuraKind::Moods).then(|| Mood::at(self.age))
    }

    /// Where its modifiers come from.
    pub fn source(&self) -> Source {
        Source::Aura(self.data().name)
    }

    /// Its modifiers on its holder now, a self intent's (nothing until lit): Sát lục's damage more from killing
    /// intent. The other self intents' effects are not stats (Tuế nguyệt's slowed shots, Niềm tin, Chiến).
    pub fn holder_mods(&self, holder: &Cultivator) -> Vec<Mod> {
        let per = self.data().more_per_intent;
        if !self.lit() || per == 0.0 {
            return Vec::new();
        }
        vec![Mod::more(
            Stat::Damage,
            per * holder.intent.max(0.0),
            self.source(),
        )]
    }

    /// Its modifiers on a foe inside now, a pressing intent's ([`Press::mods`] of [`Aura::press`]).
    pub fn foe_mods(&self) -> Vec<Mod> {
        self.press().mods(self.source())
    }

    /// What it weighs on a foe inside now (nothing until lit).
    pub fn press(&self) -> Press {
        if !self.lit() {
            return Press::NONE;
        }
        match self.mood() {
            Some(m) => m.press(),
            None => self.data().press,
        }
    }
}

/// Why an aura did not light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Unlit {
    /// Thất hồn.
    SoulLost,
    /// Every slot is taken.
    NoSlot,
    /// Not enough sense left to hold it.
    NoSense,
    /// It is already on.
    AlreadyOn,
}

/// A fighter's auras: those on (lit or in windup), those thất hồn put out (to relight), and the slots its dao map
/// gives.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Auras {
    pub on: Vec<Aura>,
    pub out: Vec<Aura>,
    pub slots: usize,
}

impl Default for Auras {
    fn default() -> Self {
        Auras::new(AURA.slots_min)
    }
}

impl Auras {
    /// No aura, `slots` of them allowed (held to 1 to 3).
    pub fn new(slots: usize) -> Auras {
        Auras {
            on: Vec::new(),
            out: Vec::new(),
            slots: slots.clamp(AURA.slots_min, AURA.slots_max),
        }
    }

    /// Sense the auras on hold.
    pub fn reserved(&self) -> f64 {
        self.on.iter().map(|a| a.reserve).sum()
    }

    /// Lights an aura: it holds its sense at once and works after the windup. The sense above the new cap is lost.
    pub fn light(
        &mut self,
        kind: AuraKind,
        rank: Rank,
        self_realised: bool,
        c: &mut Cultivator,
    ) -> Result<(), Unlit> {
        self.light_with(kind, rank, self_realised, c, 1.0)
    }

    /// [`light`](Auras::light) holding `reservation` times its sense (the skill's `Stat::Reservation`, from
    /// `build::Built::reservation`: Khai Ngộ holds less).
    pub fn light_with(
        &mut self,
        kind: AuraKind,
        rank: Rank,
        self_realised: bool,
        c: &mut Cultivator,
        reservation: f64,
    ) -> Result<(), Unlit> {
        if c.soul_lost > 0.0 {
            return Err(Unlit::SoulLost);
        }
        if self.on.iter().any(|a| a.kind == kind) {
            return Err(Unlit::AlreadyOn);
        }
        if self.on.len() >= self.slots {
            return Err(Unlit::NoSlot);
        }
        let reserve = kind.data().reserve * reservation.max(0.0);
        if c.sense_max - c.sense_reserved < reserve {
            return Err(Unlit::NoSense);
        }
        c.sense_reserved += reserve;
        c.sense = c.sense.min((c.sense_max - c.sense_reserved).max(0.0));
        self.out.retain(|a| a.kind != kind);
        let mut a = Aura::new(kind, rank, self_realised);
        a.reserve = reserve;
        self.on.push(a);
        Ok(())
    }

    /// Puts an aura out by choice; its sense is freed. Returns whether it was on.
    pub fn put_out(&mut self, kind: AuraKind, c: &mut Cultivator) -> bool {
        let Some(i) = self.on.iter().position(|a| a.kind == kind) else {
            return false;
        };
        let a = self.on.remove(i);
        c.sense_reserved = (c.sense_reserved - a.reserve).max(0.0);
        true
    }

    /// Thất hồn: every aura goes out and frees its sense; they wait in `out` to be relit.
    pub fn extinguish(&mut self, c: &mut Cultivator) {
        c.sense_reserved = (c.sense_reserved - self.reserved()).max(0.0);
        for a in self.on.drain(..) {
            if !self.out.iter().any(|o| o.kind == a.kind) {
                let mut o = Aura::new(a.kind, a.rank, a.self_realised);
                o.reserve = a.reserve;
                self.out.push(o);
            }
        }
    }

    /// Relights what thất hồn put out, each with the windup again and the sense it held. Returns how many lit; the
    /// rest stay out.
    pub fn relight(&mut self, c: &mut Cultivator) -> usize {
        let out = core::mem::take(&mut self.out);
        let mut n = 0;
        for a in &out {
            let full = a.data().reserve;
            let reservation = if full > 0.0 { a.reserve / full } else { 1.0 };
            if self
                .light_with(a.kind, a.rank, a.self_realised, c, reservation)
                .is_ok()
            {
                n += 1;
            } else {
                self.out.push(*a);
            }
        }
        n
    }

    /// Time passes: windups run down, lit auras age; a holder without its soul loses them all.
    pub fn tick(&mut self, dt: f64, c: &mut Cultivator) {
        if c.soul_lost > 0.0 && !self.on.is_empty() {
            self.extinguish(c);
        }
        for a in &mut self.on {
            if a.lit() {
                a.age += dt;
            } else {
                a.windup = (a.windup - dt).max(0.0);
            }
        }
    }

    /// A dispel of `by` takes every aura not higher than it (rules 5.2.2). Returns how many went.
    pub fn dispel(&mut self, by: Rank, c: &mut Cultivator) -> usize {
        let (gone, kept): (Vec<Aura>, Vec<Aura>) = self
            .on
            .iter()
            .partition(|a| dispels(by, Grade::Ranked(a.rank), false));
        let freed: f64 = gone.iter().map(|a| a.reserve).sum();
        c.sense_reserved = (c.sense_reserved - freed).max(0.0);
        self.on = kept;
        gone.len()
    }

    /// A Faith skill wipes every aura on the field. Returns how many went.
    pub fn wipe(&mut self, by: Rank, c: &mut Cultivator) -> usize {
        if !wipes_field(by) {
            return 0;
        }
        let n = self.on.len();
        c.sense_reserved = (c.sense_reserved - self.reserved()).max(0.0);
        self.on.clear();
        self.out.clear();
        n
    }

    /// Whether `kind` is lit.
    pub fn lit(&self, kind: AuraKind) -> bool {
        self.on.iter().any(|a| a.kind == kind && a.lit())
    }

    /// The lit auras.
    pub fn lit_auras(&self) -> impl Iterator<Item = &Aura> {
        self.on.iter().filter(|a| a.lit())
    }

    /// The lit pressing auras.
    pub fn lit_pressing(&self) -> impl Iterator<Item = &Aura> {
        self.lit_auras().filter(|a| a.kind.pressing())
    }

    /// The pressing aura that leads a contest: the highest rank, then self-realised over borrowed.
    pub fn leading(&self) -> Option<&Aura> {
        self.lit_pressing()
            .fold(None, |best: Option<&Aura>, a| match best {
                Some(b) if (b.rank, b.self_realised) >= (a.rank, a.self_realised) => Some(b),
                _ => Some(a),
            })
    }

    /// What the pressing auras of a holder at `holder` weigh on a foe at `foe`. `calm` (Tâm Xám) shrugs off the
    /// emotions.
    pub fn press_on(&self, holder: f64, foe: f64, calm: bool) -> Press {
        if !reaches(holder, foe) {
            return Press::NONE;
        }
        self.lit_pressing()
            .filter(|a| !(calm && a.data().emotional))
            .fold(Press::NONE, |p, a| p + a.press())
    }

    /// The speed factor of a projectile of `shot` at `x`, for a holder at `holder` (Tuế nguyệt). Only a foe's
    /// projectile is touched, and only one not higher than the aura (rules 5.2.3).
    pub fn shot_speed(&self, holder: f64, x: f64, shot: Grade, foe_shot: bool) -> f64 {
        if !foe_shot || !reaches(holder, x) {
            return 1.0;
        }
        self.lit_auras()
            .filter(|a| a.data().slow_shots > 0.0 && zone_touches(Grade::Ranked(a.rank), shot))
            .fold(1.0, |s, a| s * (1.0 - a.data().slow_shots))
    }

    /// Sát lục: the holder's damage factor from its killing intent.
    pub fn more(&self, c: &Cultivator) -> f64 {
        let per: f64 = self.lit_auras().map(|a| a.data().more_per_intent).sum();
        1.0 + per * c.intent.max(0.0)
    }

    /// Niềm tin: a fist chain cannot be broken.
    pub fn unbroken_fists(&self) -> bool {
        self.lit_auras().any(|a| a.data().unbroken_fists)
    }

    /// Chiến: no flinch while attacking.
    pub fn no_flinch(&self, attacking: bool) -> bool {
        attacking && self.lit_auras().any(|a| a.data().no_flinch)
    }

    /// Vô tình: the share of the holder's blows also dealt to the foe's sense.
    pub fn spill_sense(&self) -> f64 {
        self.lit_auras().map(|a| a.data().spill_sense).sum()
    }

    /// Điên cuồng: a foe at `foe` cast; the holder at `holder` gains qi (to its cap). Returns the qi gained.
    pub fn on_foe_cast(&self, holder: f64, foe: f64, c: &mut Cultivator) -> f64 {
        if !reaches(holder, foe) {
            return 0.0;
        }
        let gain: f64 = self.lit_pressing().map(|a| a.data().qi_per_foe_cast).sum();
        let gain = gain.min((c.qi_max - c.qi).max(0.0));
        c.qi += gain;
        gain
    }
}

/// Puts `auras`' modifiers on the fighters, each tick: a self intent's on its `holder` (at `holder_at`), a pressing
/// intent's on the `foe` (at `foe_at`) while the foe stands inside its radius (and not an emotion on a `calm` foe,
/// Tâm Xám). Each is an [`Active`] held until the next sync, which takes off what went out or out of reach, scaled
/// by the holder's [`Stat::AuraEffect`] from its `sheet`, timed effects and state. A blow, a heal, a speed then
/// gather them like any other modifier.
pub fn sync(
    auras: &Auras,
    sheet: &Sheet,
    holder: &mut Cultivator,
    holder_at: f64,
    foe: &mut Cultivator,
    foe_at: f64,
    calm: bool,
) {
    for kind in AuraKind::ALL {
        let src = Source::Aura(kind.data().name);
        if kind.pressing() {
            foe.take_off(src);
        } else {
            holder.take_off(src);
        }
    }
    let effect = sheet
        .table(&Ctx::alone(holder), [])
        .get(Stat::AuraEffect, 1.0)
        .max(0.0);
    let mut own = Vec::new();
    for a in auras.lit_auras() {
        let scaled =
            |mods: Vec<Mod>| -> Vec<Mod> { mods.into_iter().map(|m| m.scaled(effect)).collect() };
        if a.kind.pressing() {
            if reaches(holder_at, foe_at) && !(calm && a.data().emotional) {
                let mods = scaled(a.foe_mods());
                if !mods.is_empty() {
                    foe.put_on(Active {
                        source: a.source(),
                        mods,
                        left: None,
                    });
                }
            }
        } else {
            let mods = scaled(a.holder_mods(holder));
            if !mods.is_empty() {
                own.push(Active {
                    source: a.source(),
                    mods,
                    left: None,
                });
            }
        }
    }
    for a in own {
        holder.put_on(a);
    }
}

/// The numbers of tranh đạo niệm (rules 7.2).
pub struct ContestRules {
    /// The bar moves this much a second per point of force between the sides.
    pub push: f64,
    /// Force for a self-realised leading aura, and for overcoming the foe's aura.
    pub self_realised: f64,
    pub counter: f64,
    /// Sense current / this adds to force.
    pub sense: f64,
    /// The bar's ends.
    pub end: f64,
    /// Nghịch chuyển: qi needed at least (all of it is spent).
    pub reversal_qi: f64,
}
pub const CONTEST: ContestRules = ContestRules {
    push: 0.25,
    self_realised: 0.5,
    counter: 1.0,
    sense: 100.0,
    end: 1.0,
    reversal_qi: 50.0,
};

/// Whether the pressing zones of holders at `xa` and `xb` overlap: both have a lit pressing aura and stand closer
/// than the sum of their radii.
pub fn overlap(a: &Auras, xa: f64, b: &Auras, xb: f64) -> bool {
    a.leading().is_some() && b.leading().is_some() && (xa - xb).abs() < 2.0 * AURA.radius
}

/// One side's force in a contest: the leading aura's rank, +0.5 if self-realised, +1 if any lit aura of its own
/// overcomes any lit aura of the foe's, + sense / 100. 0 with no pressing aura lit.
pub fn force(mine: &Auras, theirs: &Auras, c: &Cultivator) -> f64 {
    let Some(lead) = mine.leading() else {
        return 0.0;
    };
    let realised = if lead.self_realised {
        CONTEST.self_realised
    } else {
        0.0
    };
    let overcomes = mine
        .lit_auras()
        .any(|m| theirs.lit_auras().any(|t| counters(m.kind, t.kind)));
    let counter = if overcomes { CONTEST.counter } else { 0.0 };
    f64::from(lead.rank.level()) + realised + counter + c.sense.max(0.0) / CONTEST.sense
}

/// One side of a contest: its auras, its fighter and where it stands.
pub struct Side<'a> {
    pub auras: &'a mut Auras,
    pub fighter: &'a mut Cultivator,
    pub at: f64,
}

/// Loses the soul to a contest: thất hồn for `BASE.soulless`, every aura out and its sense freed.
pub fn lose_soul(auras: &mut Auras, c: &mut Cultivator) {
    auras.extinguish(c);
    c.soul_lost = BASE.soulless;
    c.sense_reserved = 0.0;
}

/// Tranh đạo niệm between side A (0) and side B (1): the bar `x` in [−1, 1], positive toward B's loss.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Contest {
    pub x: f64,
    /// The zones overlap now.
    pub open: bool,
    /// Each side's force at the last tick, as a bot reads it.
    pub force: [f64; 2],
    /// Nghịch chuyển spent this round, by side.
    pub reversed: [bool; 2],
}

impl Contest {
    /// A new round: the bar at 0, Nghịch chuyển back.
    pub fn new_round(&mut self) {
        *self = Contest::default();
    }

    /// Time passes. While the zones overlap the bar moves by `push × (force A − force B)` a second; at an end the
    /// side it points at loses the soul and the bar goes back to 0. Without overlap it rests at 0. Returns the
    /// side that lost the soul, if one did.
    pub fn tick(&mut self, dt: f64, a: Side, b: Side) -> Option<usize> {
        self.open = overlap(a.auras, a.at, b.auras, b.at);
        self.force = [
            force(a.auras, b.auras, a.fighter),
            force(b.auras, a.auras, b.fighter),
        ];
        if !self.open {
            self.x = 0.0;
            return None;
        }
        self.x += CONTEST.push * (self.force[0] - self.force[1]) * dt;
        self.x = self.x.clamp(-CONTEST.end, CONTEST.end);
        let loser = if self.x >= CONTEST.end {
            lose_soul(b.auras, b.fighter);
            Some(1)
        } else if self.x <= -CONTEST.end {
            lose_soul(a.auras, a.fighter);
            Some(0)
        } else {
            None
        };
        if loser.is_some() {
            self.x = 0.0;
            self.open = false;
        }
        loser
    }

    /// Whether the bar leans against `side`.
    pub fn against(&self, side: usize) -> bool {
        if side == 0 {
            self.x < 0.0
        } else {
            self.x > 0.0
        }
    }

    /// Nghịch chuyển, once a round: spends all of `side`'s qi (50 at least) to set the bar back to 0. Only while the
    /// contest is open and the bar leans against `side`. Returns whether it happened.
    pub fn reverse(&mut self, side: usize, c: &mut Cultivator) -> bool {
        let Some(used) = self.reversed.get(side).copied() else {
            return false;
        };
        if used || !self.open || !self.against(side) || c.qi < CONTEST.reversal_qi {
            return false;
        }
        c.qi = 0.0;
        self.x = 0.0;
        self.reversed[side] = true;
        true
    }

    /// What a bot reads: the bar and both forces, from `side`'s view (its own force first, the bar positive when
    /// it leans toward the foe's loss).
    pub fn view(&self, side: usize) -> [f64; 3] {
        if side == 0 {
            [self.x, self.force[0], self.force[1]]
        } else {
            [-self.x, self.force[1], self.force[0]]
        }
    }
}

//! Lĩnh vực (domain; rules 7.3, research 6.4): a skill that turns the field to its owner's rules for a while. Each
//! side holds at most one; a newer one of the same side replaces the older. Two sides' domains split the field at
//! the midpoint of their centres, each half under its own. Ly Phong overrides any other domain of a rank not above
//! its own; a Faith skill wipes them all. A domain that collapses (runs out or is removed) locks blinks for everyone
//! inside it for a second ("đánh lúc sân sụp"). What a domain does to a stat is a modifier ([`Rules::mods`]) that
//! [`Field::sync`] puts on each fighter each tick. Positions are in T along the floor. Pure and deterministic: no
//! dice.

use crate::cultivator::{Active, Cultivator};
use crate::grade::{Grade, Rank, dispels, wipes_field, zone_touches};
use crate::stat::{Mod, Source, Stat};

/// The numbers every domain shares.
pub struct DomainRules {
    /// Seconds a collapse locks blinks for those inside.
    pub collapse_lock: f64,
    /// Lifetimes lie in this range (6 to 10 s).
    pub life_min: f64,
    pub life_max: f64,
    /// Tàn Dạ is free in a round's first seconds ("bình minh").
    pub dawn: f64,
    /// How far from the floor's centre a centre-only domain (Uy Áp Chân Giới) may be raised, in T.
    pub centre_reach: f64,
    /// Forms a domain grows through; Hoàng Tuyền pulls from `pull_form`.
    pub forms: u8,
    pub pull_form: u8,
}
pub const DOMAIN: DomainRules = DomainRules {
    collapse_lock: 1.0,
    life_min: 6.0,
    life_max: 10.0,
    dawn: 3.0,
    centre_reach: 1.0,
    forms: 3,
    pull_form: 3,
};

/// How many kinds of domain there are.
pub const DOMAIN_KINDS: usize = 6;

/// A domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum DomainKind {
    /// Hoàng Tuyền: a river of grudges under the floor; slows and swallows foe projectiles, its third form pulls foes
    /// to the centre, minions dying inside become the owner's (TN3-4).
    YellowSprings,
    /// Tàn Dạ: night over the field; sunrise deals Light that cannot be dodged. Free in a round's first seconds
    /// (TN986, TN1213).
    RemnantNight,
    /// Ly Phong: windless; no one regrows qi or casts a qi skill, projectiles and wind are gone (CM, Ly Phong Tam
    /// Thức).
    StillWind,
    /// Định Quy Tắc: whoever moves inside is bound: no escape, no flight (TN1755).
    FixedRule,
    /// Uy Áp Chân Giới: foes inside use 40% of their power; raised only at the floor's centre (CM6).
    TruePressure,
    /// Hô Phong Hoán Vũ: foes draw no qi from the world; a minion dying inside gives the owner incense (TN6).
    WindRain,
}

impl DomainKind {
    pub const ALL: [DomainKind; DOMAIN_KINDS] = [
        DomainKind::YellowSprings,
        DomainKind::RemnantNight,
        DomainKind::StillWind,
        DomainKind::FixedRule,
        DomainKind::TruePressure,
        DomainKind::WindRain,
    ];

    /// Its place in [`DomainKind::ALL`] and [`DOMAINS`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Its numbers and rules.
    pub fn data(self) -> &'static DomainData {
        &DOMAINS[self.index()]
    }
}

/// One kind of domain's rules. Starting numbers: the ladder sets the real ones.
pub struct DomainData {
    /// Its name in the novels.
    pub name: &'static str,
    /// Seconds it stands (6 to 10).
    pub life: f64,
    /// A foe's projectile inside flies at this share of its speed.
    pub foe_shot_speed: f64,
    /// A foe's projectile inside this many seconds is swallowed.
    pub swallow_after: Option<f64>,
    /// T a second a foe inside is pulled toward the centre, from the pulling form.
    pub pull: f64,
    /// A foe's minion dying inside becomes the owner's.
    pub turns_minions: bool,
    /// Incense (hương hỏa) the owner gains when any minion dies inside.
    pub incense: f64,
    /// Night; on running out, sunrise deals this base of Light, undodgeable, to foes inside.
    pub night: bool,
    pub sunrise: f64,
    /// Free to raise in a round's first [`DOMAIN`]`.dawn` seconds.
    pub free_dawn: bool,
    /// Windless (Ly Phong): no qi regen and no qi skills for anyone inside; every projectile and wind removed; other
    /// domains of a rank not above it go.
    pub windless: bool,
    /// Whoever moves inside cannot escape or fly.
    pub binds_movers: bool,
    /// A foe inside uses this share of its power.
    pub foe_power: f64,
    /// A foe inside draws no qi from the world.
    pub no_world_qi: bool,
    /// Raised only within [`DOMAIN`]`.centre_reach` of the floor's centre.
    pub centre_only: bool,
}

const PLAIN: DomainData = DomainData {
    name: "",
    life: 8.0,
    foe_shot_speed: 1.0,
    swallow_after: None,
    pull: 0.0,
    turns_minions: false,
    incense: 0.0,
    night: false,
    sunrise: 0.0,
    free_dawn: false,
    windless: false,
    binds_movers: false,
    foe_power: 1.0,
    no_world_qi: false,
    centre_only: false,
};

/// Every domain's rules, in [`DomainKind::ALL`]'s order.
pub const DOMAINS: [DomainData; DOMAIN_KINDS] = [
    DomainData {
        name: "Hoàng Tuyền",
        life: 10.0,
        foe_shot_speed: 0.5,
        swallow_after: Some(0.75),
        pull: 1.5,
        turns_minions: true,
        ..PLAIN
    },
    DomainData {
        name: "Tàn Dạ",
        life: 8.0,
        night: true,
        sunrise: 500.0,
        free_dawn: true,
        ..PLAIN
    },
    DomainData {
        name: "Ly Phong",
        life: 6.0,
        windless: true,
        ..PLAIN
    },
    DomainData {
        name: "Định Quy Tắc",
        life: 7.0,
        binds_movers: true,
        ..PLAIN
    },
    DomainData {
        name: "Uy Áp Chân Giới",
        life: 8.0,
        foe_power: 0.4,
        centre_only: true,
        ..PLAIN
    },
    DomainData {
        name: "Hô Phong Hoán Vũ",
        life: 9.0,
        no_world_qi: true,
        incense: 1.0,
        ..PLAIN
    },
];

/// A domain standing on the field.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Domain {
    pub kind: DomainKind,
    /// The side that raised it (0 or 1).
    pub owner: usize,
    /// Where it was raised, in T.
    pub centre: f64,
    /// The rank of the skill that raised it.
    pub rank: Rank,
    /// Seconds left.
    pub left: f64,
    /// Its form (thức), 1 to [`DOMAIN`]`.forms`.
    pub form: u8,
}

impl Domain {
    /// Its rules.
    pub fn data(&self) -> &'static DomainData {
        self.kind.data()
    }
}

/// A domain that went, and who it locked.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Collapse {
    pub kind: DomainKind,
    pub owner: usize,
    pub rank: Rank,
    /// Each fighter inside it when it went: blinks locked for [`DOMAIN`]`.collapse_lock` s.
    pub locked: [bool; 2],
    /// Tàn Dạ ran out: sunrise deals its Light (undodgeable) to the foe if `locked` holds it.
    pub sunrise: bool,
}

/// What raising a domain did.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Raised {
    /// It stands; the foe's domains it overrode collapsed.
    Stands(Vec<Collapse>),
    /// A foe's Ly Phong of a rank not below it overrides it: it never stands.
    Overridden,
    /// It must be raised at the floor's centre.
    OffCentre,
}

/// What happens to a minion dying at a spot.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum MinionFate {
    /// It just dies.
    Dies,
    /// Hoàng Tuyền: it rises as a grudge-ghost of this side.
    Turns(usize),
    /// Hô Phong Hoán Vũ: this side gains this much incense.
    Incense(usize, f64),
}

/// The rules on a fighter at a spot.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Rules {
    /// The domain governing the spot, its owner and rank.
    pub domain: Option<DomainKind>,
    pub owner: Option<usize>,
    pub rank: Option<Rank>,
    pub qi_regen: bool,
    pub qi_skills: bool,
    /// Qi drawn from the world (pickups, environment).
    pub world_qi: bool,
    pub wind: bool,
    /// The share of power the fighter uses.
    pub power: f64,
    pub escape: bool,
    pub fly: bool,
    /// Blinks, off while a collapse lock holds the fighter.
    pub blink: bool,
    pub night: bool,
    /// T a second toward `pull_to`.
    pub pull: f64,
    pub pull_to: f64,
}

impl Rules {
    /// No domain, no lock.
    pub const FREE: Rules = Rules {
        domain: None,
        owner: None,
        rank: None,
        qi_regen: true,
        qi_skills: true,
        world_qi: true,
        wind: true,
        power: 1.0,
        escape: true,
        fly: true,
        blink: true,
        night: false,
        pull: 0.0,
        pull_to: 0.0,
    };
}

impl Rules {
    /// Its stat modifiers on the fighter it governs ([`Source::Domain`] the domain's name): the share of power a foe
    /// uses as damage less (Uy Áp Chân Giới), no qi regen as qi regen to nothing (Ly Phong). Escape, flight, blinks,
    /// world qi, wind, night and the pull are rules, not stats: the fight reads them here.
    pub fn mods(&self) -> Vec<Mod> {
        let Some(kind) = self.domain else {
            return Vec::new();
        };
        let src = Source::Domain(kind.data().name);
        let mut out = Vec::new();
        if self.power != 1.0 {
            out.push(Mod::more(Stat::Damage, self.power - 1.0, src));
        }
        if !self.qi_regen {
            out.push(Mod::more(Stat::QiRegen, -1.0, src));
        }
        out
    }
}

/// What a domain does to a projectile at a spot.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ShotRule {
    /// Its speed share.
    pub speed: f64,
    /// Swallowed after this many seconds inside.
    pub swallow_after: Option<f64>,
    /// Gone at once (Ly Phong).
    pub removed: bool,
}

impl ShotRule {
    /// Untouched.
    pub const FREE: ShotRule = ShotRule {
        speed: 1.0,
        swallow_after: None,
        removed: false,
    };
}

/// Whether `kind` may be raised at `x` on a floor whose centre is `mid`.
pub fn can_raise(kind: DomainKind, x: f64, mid: f64) -> bool {
    !kind.data().centre_only || (x - mid).abs() <= DOMAIN.centre_reach
}

/// The share of its cost raising `kind` takes `round_time` seconds into a round: Tàn Dạ is free at dawn.
pub fn cost_factor(kind: DomainKind, round_time: f64) -> f64 {
    if kind.data().free_dawn && round_time < DOMAIN.dawn {
        0.0
    } else {
        1.0
    }
}

/// The field's domains, one a side at most, and each fighter's collapse lock.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Field {
    pub domains: [Option<Domain>; 2],
    /// Seconds each fighter's blinks stay locked by a collapse.
    pub lock: [f64; 2],
}

impl Field {
    /// The domain governing `x`: the only one, or with two, the one on `x`'s side of the midpoint of their centres
    /// (the lower centre governs below it; equal centres give the lower half to side 0).
    pub fn governing(&self, x: f64) -> Option<&Domain> {
        match (&self.domains[0], &self.domains[1]) {
            (None, None) => None,
            (Some(d), None) | (None, Some(d)) => Some(d),
            (Some(a), Some(b)) => {
                let mid = (a.centre + b.centre) / 2.0;
                let (low, high) = if b.centre < a.centre { (b, a) } else { (a, b) };
                Some(if x < mid { low } else { high })
            }
        }
    }

    /// Whether `x` lies in `side`'s domain.
    pub fn in_domain_of(&self, side: usize, x: f64) -> bool {
        self.governing(x).is_some_and(|d| d.owner == side)
    }

    /// Whether fighter `f`'s blinks are locked by a collapse.
    pub fn locked(&self, f: usize) -> bool {
        self.lock.get(f).is_some_and(|&t| t > 0.0)
    }

    /// The rules on fighter `f` at `x`, `moving` or standing.
    pub fn rules(&self, f: usize, x: f64, moving: bool) -> Rules {
        let mut r = Rules {
            blink: !self.locked(f),
            ..Rules::FREE
        };
        let Some(d) = self.governing(x) else {
            return r;
        };
        let data = d.data();
        let foe = d.owner != f;
        r.domain = Some(d.kind);
        r.owner = Some(d.owner);
        r.rank = Some(d.rank);
        r.night = data.night;
        if data.windless {
            r.qi_regen = false;
            r.qi_skills = false;
            r.wind = false;
        }
        if data.binds_movers && moving {
            r.escape = false;
            r.fly = false;
        }
        if foe {
            r.power = data.foe_power;
            r.world_qi = !data.no_world_qi;
            if d.form >= DOMAIN.pull_form && data.pull > 0.0 {
                r.pull = data.pull;
                r.pull_to = d.centre;
            }
        }
        r
    }

    /// What the domain at `x` does to a projectile of `shot` fired by side `shooter`. A projectile above the
    /// domain's rank passes untouched (rules 5.2.3); Ly Phong removes every other one, Hoàng Tuyền slows and swallows
    /// a foe's.
    pub fn shot(&self, x: f64, shooter: usize, shot: Grade) -> ShotRule {
        let Some(d) = self.governing(x) else {
            return ShotRule::FREE;
        };
        if !zone_touches(Grade::Ranked(d.rank), shot) {
            return ShotRule::FREE;
        }
        let data = d.data();
        if data.windless {
            return ShotRule {
                removed: true,
                ..ShotRule::FREE
            };
        }
        if shooter == d.owner {
            return ShotRule::FREE;
        }
        ShotRule {
            speed: data.foe_shot_speed,
            swallow_after: data.swallow_after,
            removed: false,
        }
    }

    /// A minion of side `minion_side` dies at `x`.
    pub fn minion_dies(&self, x: f64, minion_side: usize) -> MinionFate {
        let Some(d) = self.governing(x) else {
            return MinionFate::Dies;
        };
        let data = d.data();
        if data.turns_minions && minion_side != d.owner {
            MinionFate::Turns(d.owner)
        } else if data.incense > 0.0 {
            MinionFate::Incense(d.owner, data.incense)
        } else {
            MinionFate::Dies
        }
    }

    /// Who stands inside `side`'s domain now.
    fn inside(&self, side: usize, pos: [f64; 2]) -> [bool; 2] {
        [
            self.in_domain_of(side, pos[0]),
            self.in_domain_of(side, pos[1]),
        ]
    }

    /// Takes `side`'s domain down, blink-locking the fighters in `locked`. `ran_out` when its time ended.
    fn take_down(&mut self, side: usize, locked: [bool; 2], ran_out: bool) -> Option<Collapse> {
        let d = (*self.domains.get(side)?)?;
        self.domains[side] = None;
        for (f, &l) in locked.iter().enumerate() {
            if l {
                self.lock[f] = self.lock[f].max(DOMAIN.collapse_lock);
            }
        }
        Some(Collapse {
            kind: d.kind,
            owner: d.owner,
            rank: d.rank,
            locked,
            sunrise: ran_out && d.data().night,
        })
    }

    /// Takes `side`'s domain down: those inside it now are blink-locked.
    fn collapse(&mut self, side: usize, pos: [f64; 2], ran_out: bool) -> Option<Collapse> {
        let locked = self.inside(side, pos);
        self.take_down(side, locked, ran_out)
    }

    /// Side `owner` raises `kind` of `rank` at `centre`, the fighters at `pos`, on a floor whose centre is `mid`.
    /// Its own older domain gives way without collapsing. A foe's Ly Phong of a rank not below it overrides it;
    /// raised as Ly Phong it overrides the foe's other domain of a rank not above it, and raised at Faith it wipes
    /// the foe's whatever it is. What it overrides collapses.
    pub fn raise(
        &mut self,
        owner: usize,
        kind: DomainKind,
        rank: Rank,
        centre: f64,
        pos: [f64; 2],
        mid: f64,
    ) -> Raised {
        if owner > 1 {
            return Raised::Overridden;
        }
        if !can_raise(kind, centre, mid) {
            return Raised::OffCentre;
        }
        let foe = 1 - owner;
        if let Some(f) = &self.domains[foe]
            && f.kind == DomainKind::StillWind
            && kind != DomainKind::StillWind
            && f.rank >= rank
            && !wipes_field(rank)
        {
            return Raised::Overridden;
        }
        let mut gone = Vec::new();
        if let Some(f) = self.domains[foe] {
            let overrides =
                kind == DomainKind::StillWind && f.kind != DomainKind::StillWind && f.rank <= rank;
            if (overrides || wipes_field(rank))
                && let Some(c) = self.collapse(foe, pos, false)
            {
                gone.push(c);
            }
        }
        self.domains[owner] = Some(Domain {
            kind,
            owner,
            centre,
            rank,
            left: kind.data().life.clamp(DOMAIN.life_min, DOMAIN.life_max),
            form: 1,
        });
        Raised::Stands(gone)
    }

    /// Side `side`'s domain grows a form (to [`DOMAIN`]`.forms`). Returns the form, 0 with no domain.
    pub fn grow(&mut self, side: usize) -> u8 {
        match self.domains.get_mut(side) {
            Some(Some(d)) => {
                d.form = (d.form + 1).min(DOMAIN.forms);
                d.form
            }
            _ => 0,
        }
    }

    /// Removes `side`'s domain (it collapses).
    pub fn remove(&mut self, side: usize, pos: [f64; 2]) -> Option<Collapse> {
        self.collapse(side, pos, false)
    }

    /// A dispel of `by` on `side`'s domain: it collapses when not higher.
    pub fn dispel(&mut self, side: usize, by: Rank, pos: [f64; 2]) -> Option<Collapse> {
        let d = (*self.domains.get(side)?)?;
        if dispels(by, Grade::Ranked(d.rank), false) {
            self.collapse(side, pos, false)
        } else {
            None
        }
    }

    /// A Faith skill wipes every domain; each collapses.
    pub fn wipe(&mut self, by: Rank, pos: [f64; 2]) -> Vec<Collapse> {
        if !wipes_field(by) {
            return Vec::new();
        }
        // Who stands inside is read before either goes: the field is split until both are down.
        let inside = [self.inside(0, pos), self.inside(1, pos)];
        let mut gone = Vec::new();
        for (side, locked) in inside.into_iter().enumerate() {
            if let Some(c) = self.take_down(side, locked, false) {
                gone.push(c);
            }
        }
        gone
    }

    /// Puts the rules' modifiers ([`Rules::mods`]) on each fighter, each tick: fighter `f` stands at `pos[f]`,
    /// moving or not. Each is an [`Active`] held until the next sync, which takes off a domain gone or left.
    pub fn sync(&self, fighters: [&mut Cultivator; 2], pos: [f64; 2], moving: [bool; 2]) {
        for (f, c) in fighters.into_iter().enumerate() {
            for kind in DomainKind::ALL {
                c.take_off(Source::Domain(kind.data().name));
            }
            let r = self.rules(f, pos[f], moving[f]);
            let mods = r.mods();
            if let (Some(kind), false) = (r.domain, mods.is_empty()) {
                c.put_on(Active {
                    source: Source::Domain(kind.data().name),
                    mods,
                    left: None,
                });
            }
        }
    }

    /// A new round: no domain, no lock.
    pub fn new_round(&mut self) {
        *self = Field::default();
    }

    /// Time passes: locks run down, domains run out and collapse (side 0's first). Returns the collapses.
    pub fn tick(&mut self, dt: f64, pos: [f64; 2]) -> Vec<Collapse> {
        for l in &mut self.lock {
            *l = (*l - dt).max(0.0);
        }
        let mut ran_out = [false; 2];
        for (side, d) in self.domains.iter_mut().enumerate() {
            if let Some(d) = d {
                d.left -= dt;
                ran_out[side] = d.left <= 0.0;
            }
        }
        let inside = [self.inside(0, pos), self.inside(1, pos)];
        let mut gone = Vec::new();
        for side in 0..2 {
            if ran_out[side]
                && let Some(c) = self.take_down(side, inside[side], true)
            {
                gone.push(c);
            }
        }
        gone
    }
}

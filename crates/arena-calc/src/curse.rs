//! Curses (nguyền): the rules of `docs/specs/2026-11-17-skill-system-rules.md` 6.9 (conduits, the cap, replacing,
//! spreading to minions, backlash, blessing) and the curse list of the research's 6.1 as data and behaviour. Pure
//! state: the fight owns a [`Curses`] per fighter (and per minion) and a [`Conduits`] per striker, feeds them the
//! casts and the clock, and turns the [`Event`]s they return into blows ([`strike_hit`]) and controls (through
//! [`Cultivator::control`]). What a curse does to a stat (damage taken, resistance, speed) is a modifier on its
//! bearer's effects ([`Curses::sync`], run by [`Curses::tick`]), so a blow on the bearer gathers it without the fight
//! adding it. No dice inside: a roll comes in as an argument.

use crate::ailment::Control;
use crate::cultivator::{Active, Cultivator};
use crate::dao::{Id, Tether, dao_hit, drain_lifespan};
use crate::element::Element;
use crate::grade::{Grade, Rank, dispels};
use crate::hit::Hit;
use crate::sheet::Sheet;
use crate::stat::{Ctx, Mod, Source, Stat, Table};
use crate::tag::{Tag, Tags};

/// The rules every curse shares (rules, 6.9).
pub struct Rules {
    /// Vật dẫn: a conduit (blood, hair) taken by a Near hit is held this many seconds...
    pub conduit_time: f64,
    /// ...and a striker holds at most this many of one opponent's.
    pub conduits: usize,
    /// Curses a fighter carries at once...
    pub cap: usize,
    /// ...or under the Vu Chú keystone.
    pub cap_vu_chu: usize,
    /// Lan cùng nguồn: a target's minions and doubles take its curse at this share of its effect.
    pub spread: f64,
    /// Phản phệ: cursing back one's curser makes the new curse this much stronger (and the first one falls).
    pub backlash: f64,
    /// Chúc phúc: a blessed-away curse turns into its opposite buff for this many seconds.
    pub boon_time: f64,
}
pub const RULES: Rules = Rules {
    conduit_time: 20.0,
    conduits: 3,
    cap: 1,
    cap_vu_chu: 2,
    spread: 0.5,
    backlash: 1.5,
    boon_time: 3.0,
};

/// The tags that let a curse be laid from beyond Near, spending a conduit: Phù, Hồn (the form), and soul or yin
/// damage (research, 6.1: "Hồn, Âm hay Phù").
pub const FAR_TAGS: &[Tag] = &[Tag::Talisman, Tag::Spirit, Tag::Soul, Tag::Yin];

/// Each curse's numbers (research, 6.1). Every share of life is of the bearer's full life; "potency" (1, 0.5 when
/// spread, 1.5 on backlash) multiplies every effect but no duration.
pub struct Numbers {
    /// Thảo Kết Ký Sự: how long the doll lasts, in seconds.
    pub straw_time: f64,
    /// Thảo Kết: from this knot the target's blows may misfire...
    pub straw_misfire_at: u8,
    /// ...missing with this chance...
    pub straw_miss: f64,
    /// ...or striking its own striker with this chance (after the miss).
    pub straw_self: f64,
    /// Thảo Kết: the last knot, which holds the target (Định thân) for `straw_hold` s and spends the doll.
    pub straw_last: u8,
    pub straw_hold: f64,
    /// Thảo Kết: from this knot on, each knot costs the curser `straw_bleed` of its full life.
    pub straw_bleed_from: u8,
    pub straw_bleed: f64,
    /// Cửu Tử: beats, the seconds between them, the yin of a beat, and how much heavier each Guard the target used
    /// makes every later beat.
    pub nine_beats: u8,
    pub nine_beat: f64,
    pub nine_damage: f64,
    pub nine_guard: f64,
    /// Trớ Thuật Tiên Tổ: how long it lasts; each blow the target deals adds a stack of `doom_step` more damage
    /// taken, up to `doom_max` stacks; a stack falls for each `doom_rest` s the target does not strike.
    pub doom_time: f64,
    pub doom_step: f64,
    pub doom_max: f64,
    pub doom_rest: f64,
    /// Tử Ấn Tổ Quyết: how long the tether curse lasts (the tether's numbers are in `dao::DAO`).
    pub seal_time: f64,
    /// Ngũ Tạng: the seconds between beats; beat 5 (the heart) deals `organs_heart` as yin.
    pub organs_beat: f64,
    pub organs_heart: f64,
    /// Bói Ba Nhịp Thở: the countdown, the soul damage when it runs out, and what a broken mark sends back at the
    /// diviner (a share of the diviner's full life, as soul).
    pub breaths_time: f64,
    pub breaths: f64,
    pub breaths_backfire: f64,
    /// Lời Giam Cầm: how long the word binds; breaking it cracks the heart (Tâm Nứt) for `crack_time` s, cutting
    /// soul resistance by `crack_soul` and opening Đoạt Xá.
    pub word_time: f64,
    pub crack_time: f64,
    pub crack_soul: f64,
    /// Mộng Đạo: how long the hidden dream waits, and the stall it lays on the target's strongest skill.
    pub dream_time: f64,
    pub dream_stall: f64,
    /// Phong Thần: how long a damage tag stays sealed, and the share of the curser's full life it pays for it.
    pub god_time: f64,
    pub god_cost: f64,
    /// Hóa Ma: how long it turns the target's immortal buffs.
    pub demon_time: f64,
    /// Tuế Nguyệt: how long it ages, its slow, and the lifespan it drains a second.
    pub aging_time: f64,
    pub aging_slow: f64,
    pub aging_drain: f64,
    /// Khắc Ấn: how long it marks, and the resistance it takes from one kind.
    pub expose_time: f64,
    pub expose: f64,
    /// A blessed-away curse with no numeric mirror gives this much more damage instead.
    pub boon_more: f64,
}
pub const NUMBERS: Numbers = Numbers {
    straw_time: 12.0,
    straw_misfire_at: 10,
    straw_miss: 0.2,
    straw_self: 0.1,
    straw_last: 11,
    straw_hold: 0.5,
    straw_bleed_from: 7,
    straw_bleed: 0.02,
    nine_beats: 9,
    nine_beat: 1.0,
    nine_damage: 0.01,
    nine_guard: 0.5,
    doom_time: 10.0,
    doom_step: 0.04,
    doom_max: 10.0,
    doom_rest: 1.5,
    seal_time: 8.0,
    organs_beat: 2.0,
    organs_heart: 0.15,
    breaths_time: 3.0,
    breaths: 0.25,
    breaths_backfire: 0.15,
    word_time: 8.0,
    crack_time: 5.0,
    crack_soul: 0.3,
    dream_time: 15.0,
    dream_stall: 0.5,
    god_time: 4.0,
    god_cost: 0.05,
    demon_time: 8.0,
    aging_time: 6.0,
    aging_slow: 0.2,
    aging_drain: 2.0,
    expose_time: 8.0,
    expose: 0.2,
    boon_more: 0.1,
};

/// A curse, with what it was laid on.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Kind {
    /// Thảo Kết Ký Sự (straw doll): each knot the curser ties (a short channel) adds one; 10 knots, the target's
    /// blows may miss or turn back; 11, a short hold. From the 7th, each knot bleeds the curser.
    StrawKnot,
    /// Cửu Tử: nine beats of yin, heavier for every Guard the target uses. Only another curse or outliving the nine
    /// beats lifts it.
    NineDeaths,
    /// Trớ Thuật Tiên Tổ (doom): each blow the target deals makes it take more; only resting lowers it.
    AncestorDoom,
    /// Tử Ấn Tổ Quyết: a tether (`dao::Tether`) to the curser.
    DeathSeal,
    /// Ngũ Tạng: five beats, each breaking an organ ([`Organ`]); the fifth is a heavy blow.
    FiveOrgans,
    /// Bói Ba Nhịp Thở: a three-second countdown to heavy soul damage; breaking it turns it on the diviner.
    ThreeBreaths,
    /// Lời Giam Cầm: using a skill with this tag while it lasts cracks the heart (Tâm Nứt).
    BindingWord(Tag),
    /// Mộng Đạo: hidden; the target's next strongest skill stalls.
    DreamDao,
    /// Phong Thần: every skill with this damage tag is locked; paid in the curser's full life.
    SealGod(Tag),
    /// Hóa Ma: the target's immortal buffs work against it. Pure-yang thunder (thunder with light) drives it out.
    Demonize,
    /// Tuế Nguyệt: ages the target: slows it and drains lifespan. A still tree watcher is untouched.
    Aging,
    /// Khắc Ấn: shows the target, draws projectiles to it, and cuts its resistance to this kind.
    ExposeMark(Element),
}

impl Kind {
    /// One slot per curse, whatever it was laid with.
    pub fn index(self) -> usize {
        match self {
            Kind::StrawKnot => 0,
            Kind::NineDeaths => 1,
            Kind::AncestorDoom => 2,
            Kind::DeathSeal => 3,
            Kind::FiveOrgans => 4,
            Kind::ThreeBreaths => 5,
            Kind::BindingWord(_) => 6,
            Kind::DreamDao => 7,
            Kind::SealGod(_) => 8,
            Kind::Demonize => 9,
            Kind::Aging => 10,
            Kind::ExposeMark(_) => 11,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::StrawKnot => "strawknot",
            Kind::NineDeaths => "ninedeaths",
            Kind::AncestorDoom => "ancestorcurse",
            Kind::DeathSeal => "deathseal",
            Kind::FiveOrgans => "fiveorgans",
            Kind::ThreeBreaths => "threebreaths",
            Kind::BindingWord(_) => "bindingword",
            Kind::DreamDao => "dreamdao",
            Kind::SealGod(_) => "sealgod",
            Kind::Demonize => "demonize",
            Kind::Aging => "aging",
            Kind::ExposeMark(_) => "exposemark",
        }
    }

    /// How long it lasts when laid, in seconds (beat curses end on their last beat).
    pub fn time(self) -> f64 {
        let n = &NUMBERS;
        match self {
            Kind::StrawKnot => n.straw_time,
            Kind::NineDeaths => f64::from(n.nine_beats) * n.nine_beat,
            Kind::AncestorDoom => n.doom_time,
            Kind::DeathSeal => n.seal_time,
            Kind::FiveOrgans => 5.0 * n.organs_beat,
            Kind::ThreeBreaths => n.breaths_time,
            Kind::BindingWord(_) => n.word_time,
            Kind::DreamDao => n.dream_time,
            Kind::SealGod(_) => n.god_time,
            Kind::Demonize => n.demon_time,
            Kind::Aging => n.aging_time,
            Kind::ExposeMark(_) => n.expose_time,
        }
    }

    /// One of each kind, for what is keyed by [`Kind::name`] alone.
    pub const SAMPLES: [Kind; 12] = [
        Kind::StrawKnot,
        Kind::NineDeaths,
        Kind::AncestorDoom,
        Kind::DeathSeal,
        Kind::FiveOrgans,
        Kind::ThreeBreaths,
        Kind::BindingWord(Tag::Attack),
        Kind::DreamDao,
        Kind::SealGod(Tag::Attack),
        Kind::Demonize,
        Kind::Aging,
        Kind::ExposeMark(Element::Force),
    ];

    /// Ends on its beats, not its clock.
    fn beats(self) -> bool {
        matches!(self, Kind::NineDeaths | Kind::FiveOrgans)
    }

    /// A dispel or a blessing can lift it (Cửu Tử: only another curse or its nine beats).
    pub fn liftable(self) -> bool {
        self != Kind::NineDeaths
    }

    /// Shown on the bearer's status bar (Mộng Đạo is hidden).
    pub fn shown(self) -> bool {
        self != Kind::DreamDao
    }
}

/// Ngũ Tạng's organs, in the order the beats break them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Organ {
    /// Can (liver): no healing, no Recovery skill.
    Liver,
    /// Phế (lungs): no shout: no Warcry or Sound skill.
    Lungs,
    /// Tỳ (spleen): no qi regen.
    Spleen,
    /// Thận (kidneys): no sense regen.
    Kidneys,
    /// Tâm (heart): the heavy blow.
    Heart,
}

impl Organ {
    pub const ALL: [Organ; 5] = [
        Organ::Liver,
        Organ::Lungs,
        Organ::Spleen,
        Organ::Kidneys,
        Organ::Heart,
    ];
}

/// A curse's running counters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct State {
    /// Thảo Kết: knots tied.
    pub knots: u8,
    /// Cửu Tử: Guards the target used.
    pub guards: u32,
    /// Cửu Tử, Ngũ Tạng: beats struck, and seconds since the last.
    pub beats: u8,
    pub clock: f64,
    /// Trớ Thuật: doom stacks, and seconds since the target last struck.
    pub stacks: f64,
    pub idle: f64,
    /// Lời Giam Cầm: seconds of Tâm Nứt left.
    pub cracked: f64,
    /// Tử Ấn Tổ Quyết.
    pub tether: Option<Tether>,
}

/// A curse on one bearer.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Curse {
    pub kind: Kind,
    /// The rank of the skill that laid it.
    pub rank: Rank,
    /// The curser.
    pub by: Id,
    /// 1, 0.5 spread to a minion, 1.5 on backlash.
    pub potency: f64,
    /// Seconds left.
    pub left: f64,
    /// Spread from its first bearer.
    pub spread: bool,
    pub state: State,
}

impl Curse {
    /// A curse of `kind` from `by` on `on`, at full potency.
    pub fn new(kind: Kind, rank: Rank, by: Id, on: Id) -> Curse {
        let mut state = State::default();
        if kind == Kind::DeathSeal {
            state.tether = Some(Tether::new(by, on, rank));
        }
        Curse {
            kind,
            rank,
            by,
            potency: 1.0,
            left: kind.time(),
            spread: false,
            state,
        }
    }

    /// Its stat modifiers on its bearer ([`Source::Curse`] its name), scaled by its potency: Trớ Thuật's stacks of
    /// damage taken, Khắc Ấn's resistance cut, Tâm Nứt's soul resistance cut, Tuế Nguyệt's slow (not on a still tree
    /// watcher, `tree_immune`). Its other effects are events and locks, not stats.
    pub fn mods(&self, tree_immune: bool) -> Vec<Mod> {
        let n = &NUMBERS;
        let src = Source::Curse(self.kind.name());
        let out = match self.kind {
            Kind::AncestorDoom if self.state.stacks > 0.0 => {
                vec![Mod::more(Stat::Taken, self.state.stacks * n.doom_step, src)]
            }
            Kind::ExposeMark(e) => vec![Mod::flat(Stat::Resist(e), -n.expose, src)],
            Kind::BindingWord(_) if self.state.cracked > 0.0 => {
                vec![Mod::flat(Stat::Resist(Element::Soul), -n.crack_soul, src)]
            }
            Kind::Aging if !tree_immune => vec![
                Mod::inc(Stat::Speed, -n.aging_slow, src),
                Mod::inc(Stat::MoveSpeed, -n.aging_slow, src),
            ],
            _ => Vec::new(),
        };
        out.into_iter().map(|m| m.scaled(self.potency)).collect()
    }

    /// The copy a minion or double of its bearer takes: [`Rules::spread`] of the effect, the same time left. A
    /// tether binds two fighters and does not spread.
    pub fn spread_to(&self, minion: Id) -> Option<Curse> {
        if self.kind == Kind::DeathSeal {
            return None;
        }
        let mut c = Curse::new(self.kind, self.rank, self.by, minion);
        c.potency = self.potency * RULES.spread;
        c.left = self.left;
        c.spread = true;
        Some(c)
    }
}

/// A curse turned into its opposite by a blessing.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Boon {
    pub from: Kind,
    pub potency: f64,
    /// Doom stacks the curse had (at least one).
    pub strength: f64,
    pub left: f64,
}

impl Boon {
    /// Its stat modifiers on the blessed fighter ([`Source::Blessing`] the curse's name): each curse's opposite.
    pub fn mods(&self) -> Vec<Mod> {
        let src = Source::Blessing(self.from.name());
        match self.from {
            Kind::ExposeMark(e) => vec![Mod::flat(Stat::Resist(e), self.resist(e), src)],
            Kind::AncestorDoom => vec![Mod::more(Stat::Taken, self.taken() - 1.0, src)],
            Kind::Aging => vec![Mod::inc(Stat::Speed, self.haste(), src)],
            _ => vec![Mod::more(Stat::Damage, self.more(), src)],
        }
    }

    /// Khắc Ấn's mirror: resistance to its kind.
    pub fn resist(&self, e: Element) -> f64 {
        match self.from {
            Kind::ExposeMark(x) if x == e => NUMBERS.expose * self.potency,
            _ => 0.0,
        }
    }

    /// Trớ Thuật's mirror: damage taken, down by its stacks.
    pub fn taken(&self) -> f64 {
        match self.from {
            Kind::AncestorDoom => (1.0 - NUMBERS.doom_step * self.strength * self.potency).max(0.0),
            _ => 1.0,
        }
    }

    /// Tuế Nguyệt's mirror: haste, the share speed rises by.
    pub fn haste(&self) -> f64 {
        match self.from {
            Kind::Aging => NUMBERS.aging_slow * self.potency,
            _ => 0.0,
        }
    }

    /// Every other curse's mirror: more damage dealt.
    pub fn more(&self) -> f64 {
        match self.from {
            Kind::ExposeMark(_) | Kind::AncestorDoom | Kind::Aging => 0.0,
            _ => NUMBERS.boon_more,
        }
    }
}

/// What a curse did, for the fight to carry out.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Event {
    /// Damage on the bearer: `share` of its full life, of kind `element` (see [`strike_hit`]).
    Strike {
        kind: Kind,
        element: Element,
        share: f64,
    },
    /// Damage turned back on the curser `by`: `share` of the curser's full life.
    Backfire {
        kind: Kind,
        by: Id,
        element: Element,
        share: f64,
    },
    /// A hard control on the bearer, before grit.
    Control {
        kind: Kind,
        control: Control,
        seconds: f64,
    },
    /// The bearer's cast stalls this many seconds.
    Stall(f64),
    /// Tâm Nứt.
    Cracked,
    /// Ngũ Tạng broke an organ.
    Organ(Organ),
    /// The curse is gone.
    Ended(Kind),
}

/// A strike or backfire as a blow: `share` of `life_max` of one kind, undodgeable, no crit.
pub fn strike_hit(element: Element, share: f64, life_max: f64) -> Hit {
    dao_hit(element, share * life_max)
}

/// What laying a curse did.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Lay {
    Added,
    /// It took the place of this curse.
    Replaced(Kind),
    /// Every curse held was higher: the new one fails (its cost is still paid).
    Failed,
}

/// What a Thảo Kết curse does to a blow of its bearer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Misfire {
    None,
    Miss,
    /// The blow lands on its own striker.
    SelfHarm,
}

/// Whether a curse can reach its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Reach {
    /// Within Near: no conduit needed.
    Near,
    /// From afar, one conduit spent.
    Spent,
    /// Out of reach: not a far curse, or no conduit.
    Out,
}

/// Vật dẫn: the conduits a striker holds of one opponent, each with its seconds left.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Conduits {
    pub left: Vec<f64>,
}

impl Conduits {
    pub fn new() -> Conduits {
        Conduits::default()
    }

    pub fn count(&self) -> usize {
        self.left.len()
    }

    /// A blow of `tags` landed: a Near one takes a conduit; past the cap the oldest goes.
    pub fn on_hit(&mut self, tags: Tags) {
        if !tags.has(Tag::Near) {
            return;
        }
        if self.left.len() >= RULES.conduits {
            self.spend();
        }
        self.left.push(RULES.conduit_time);
    }

    /// Spends the oldest; false with none.
    pub fn spend(&mut self) -> bool {
        let oldest = self
            .left
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i);
        match oldest {
            Some(i) => {
                self.left.remove(i);
                true
            }
            None => false,
        }
    }

    /// Time passes.
    pub fn tick(&mut self, dt: f64) {
        for t in &mut self.left {
            *t -= dt;
        }
        self.left.retain(|&t| t > 0.0);
    }

    /// A curse skill of `tags` cast at a target `near` or not: within Near it always reaches; beyond, only a
    /// [`FAR_TAGS`] curse, spending a conduit.
    pub fn reach(&mut self, tags: Tags, near: bool) -> Reach {
        if near {
            Reach::Near
        } else if tags.any(FAR_TAGS) && self.spend() {
            Reach::Spent
        } else {
            Reach::Out
        }
    }
}

/// What a blessing did.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Blessed {
    pub boon: Boon,
    /// A broken Bói Ba Nhịp Thở's backfire.
    pub events: Vec<Event>,
}

/// The curses on one bearer, and the boons blessings made of them.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Curses {
    pub owner: Id,
    /// How many it may carry.
    pub cap: usize,
    pub on: Vec<Curse>,
    pub boons: Vec<Boon>,
}

impl Curses {
    /// The curses of `owner`: one at a time, two under Vu Chú.
    pub fn new(owner: Id, vu_chu: bool) -> Curses {
        Curses {
            owner,
            cap: if vu_chu { RULES.cap_vu_chu } else { RULES.cap },
            on: Vec::new(),
            boons: Vec::new(),
        }
    }

    /// The curse of this kind, if carried.
    pub fn get(&self, kind: Kind) -> Option<&Curse> {
        self.on.iter().find(|c| c.kind.index() == kind.index())
    }

    /// Lays `curse`. A curse of the same kind is replaced when the new rank is not lower; with room, it is added;
    /// full, it replaces the lowest-ranked one held (the oldest of equals) when not lower; otherwise it fails.
    pub fn lay(&mut self, curse: Curse) -> Lay {
        let slot = match self
            .on
            .iter()
            .position(|c| c.kind.index() == curse.kind.index())
        {
            Some(i) => Some(i),
            None if self.on.len() < self.cap => {
                self.on.push(curse);
                return Lay::Added;
            }
            None => self
                .on
                .iter()
                .enumerate()
                .min_by_key(|&(i, c)| (c.rank, i))
                .map(|(i, _)| i),
        };
        match slot {
            Some(i) if curse.rank >= self.on[i].rank => {
                let old = self.on[i].kind;
                self.on[i] = curse;
                Lay::Replaced(old)
            }
            _ => Lay::Failed,
        }
    }

    /// Time passes: beats, countdowns, drains. `tree_immune` is the bearer's Người Xem Cây standing still.
    pub fn tick(&mut self, dt: f64, bearer: &mut Cultivator, tree_immune: bool) -> Vec<Event> {
        let n = &NUMBERS;
        let mut out = Vec::new();
        for c in &mut self.on {
            c.left -= dt;
            let s = &mut c.state;
            match c.kind {
                Kind::NineDeaths => {
                    s.clock += dt;
                    while s.clock >= n.nine_beat && s.beats < n.nine_beats {
                        s.clock -= n.nine_beat;
                        s.beats += 1;
                        let share =
                            n.nine_damage * (1.0 + n.nine_guard * f64::from(s.guards)) * c.potency;
                        out.push(Event::Strike {
                            kind: c.kind,
                            element: Element::Yin,
                            share,
                        });
                    }
                }
                Kind::FiveOrgans => {
                    s.clock += dt;
                    while s.clock >= n.organs_beat && (s.beats as usize) < Organ::ALL.len() {
                        s.clock -= n.organs_beat;
                        let organ = Organ::ALL[s.beats as usize];
                        s.beats += 1;
                        out.push(Event::Organ(organ));
                        if organ == Organ::Heart {
                            out.push(Event::Strike {
                                kind: c.kind,
                                element: Element::Yin,
                                share: n.organs_heart * c.potency,
                            });
                        }
                    }
                }
                Kind::AncestorDoom => {
                    s.idle += dt;
                    while s.idle >= n.doom_rest && s.stacks > 0.0 {
                        s.idle -= n.doom_rest;
                        s.stacks = (s.stacks - 1.0).max(0.0);
                    }
                }
                Kind::ThreeBreaths if c.left <= 0.0 => {
                    out.push(Event::Strike {
                        kind: c.kind,
                        element: Element::Soul,
                        share: n.breaths * c.potency,
                    });
                }
                Kind::BindingWord(_) if s.cracked > 0.0 => {
                    s.cracked = (s.cracked - dt).max(0.0);
                    // a cracked heart outlasts the word
                    c.left = c.left.max(s.cracked);
                }
                Kind::Aging if !tree_immune => {
                    let time = Tags::of(&[Tag::Time, Tag::Curse]);
                    drain_lifespan(time, bearer, n.aging_drain * c.potency * dt);
                }
                Kind::DeathSeal => {
                    if let Some(t) = &mut s.tether {
                        t.tick(dt);
                    }
                }
                _ => {}
            }
        }
        let mut kept = Vec::with_capacity(self.on.len());
        for c in self.on.drain(..) {
            let done = if c.kind.beats() {
                match c.kind {
                    Kind::NineDeaths => c.state.beats >= n.nine_beats,
                    _ => c.state.beats as usize >= Organ::ALL.len(),
                }
            } else {
                c.left <= 0.0
            };
            if done {
                out.push(Event::Ended(c.kind));
            } else {
                kept.push(c);
            }
        }
        self.on = kept;
        for b in &mut self.boons {
            b.left -= dt;
        }
        self.boons.retain(|b| b.left > 0.0);
        self.sync(bearer, tree_immune);
        out
    }

    /// Every stat modifier the curses and boons give their bearer now.
    pub fn mods(&self, tree_immune: bool) -> Vec<Mod> {
        self.on
            .iter()
            .flat_map(|c| c.mods(tree_immune))
            .chain(self.boons.iter().flat_map(Boon::mods))
            .collect()
    }

    /// Puts the curses' and boons' modifiers on `bearer`'s effects, one [`Active`] a curse and one a kind of boon,
    /// held until the next sync (which takes off what ended). [`Curses::tick`] syncs at its end; a fight that lays,
    /// blesses or lifts between ticks may sync at once. These own every `Curse` and `Blessing` source named after a
    /// curse on the bearer.
    pub fn sync(&self, bearer: &mut Cultivator, tree_immune: bool) {
        let ours = |s: &Source| match *s {
            Source::Curse(n) | Source::Blessing(n) => Kind::SAMPLES.iter().any(|k| k.name() == n),
            _ => false,
        };
        bearer.effects.retain(|e| !ours(&e.source));
        let mut groups: Vec<Active> = Vec::new();
        for m in self.mods(tree_immune) {
            match groups.iter_mut().find(|g| g.source == m.source) {
                Some(g) => g.mods.push(m),
                None => groups.push(Active {
                    source: m.source,
                    mods: vec![m],
                    left: None,
                }),
            }
        }
        bearer.effects.extend(groups);
    }

    /// The gathered table of [`Curses::mods`] alone.
    fn table(&self, tree_immune: bool) -> Table {
        let mods = self.mods(tree_immune);
        let c = Cultivator::new();
        Table::gather(mods.iter(), &Ctx::alone(&c))
    }

    /// The bearer cast a skill of `tags` (`strongest`: its top-rank one): Cửu Tử counts Guards, Trớ Thuật counts
    /// blows, Lời Giam Cầm cracks on its word, Mộng Đạo stalls, and pure-yang thunder drives out Hóa Ma.
    pub fn on_cast(&mut self, tags: Tags, strongest: bool) -> Vec<Event> {
        let n = &NUMBERS;
        let mut out = Vec::new();
        let blow = tags.any(&[Tag::Attack, Tag::Spell]);
        for c in &mut self.on {
            let s = &mut c.state;
            match c.kind {
                Kind::NineDeaths if tags.has(Tag::Guard) => s.guards += 1,
                Kind::AncestorDoom if blow => {
                    s.stacks = (s.stacks + 1.0).min(n.doom_max);
                    s.idle = 0.0;
                }
                Kind::BindingWord(word) if tags.has(word) && s.cracked <= 0.0 => {
                    s.cracked = n.crack_time;
                    c.left = c.left.max(n.crack_time);
                    out.push(Event::Cracked);
                }
                Kind::DreamDao if strongest => {
                    out.push(Event::Stall(n.dream_stall * c.potency));
                    c.left = 0.0;
                    out.push(Event::Ended(c.kind));
                }
                Kind::Demonize if tags.all(&[Tag::Thunder, Tag::Light]) => {
                    c.left = 0.0;
                    out.push(Event::Ended(c.kind));
                }
                _ => {}
            }
        }
        self.on
            .retain(|c| c.left > 0.0 || matches!(c.kind, Kind::NineDeaths | Kind::FiveOrgans));
        out
    }

    /// The curser `by` ties a knot on its Thảo Kết doll; from [`Numbers::straw_bleed_from`] each knot costs the
    /// curser life; the last knot holds the bearer and spends the doll.
    pub fn knot(&mut self, by: Id, curser: &mut Cultivator) -> Vec<Event> {
        let n = &NUMBERS;
        let mut out = Vec::new();
        let Some(i) = self
            .on
            .iter()
            .position(|c| c.kind == Kind::StrawKnot && c.by == by)
        else {
            return out;
        };
        let c = &mut self.on[i];
        c.state.knots = (c.state.knots + 1).min(n.straw_last);
        if c.state.knots >= n.straw_bleed_from {
            curser.pay_life(n.straw_bleed * curser.life_max);
        }
        if c.state.knots >= n.straw_last {
            out.push(Event::Control {
                kind: c.kind,
                control: Control::Hold,
                seconds: n.straw_hold * c.potency,
            });
            out.push(Event::Ended(c.kind));
            self.on.remove(i);
        }
        out
    }

    /// A blow of the bearer under a Thảo Kết of 10 knots or more, by `roll` in [0, 1): a miss, a blow on itself, or
    /// nothing.
    pub fn misfire(&self, roll: f64) -> Misfire {
        let n = &NUMBERS;
        let Some(c) = self
            .on
            .iter()
            .find(|c| c.kind == Kind::StrawKnot && c.state.knots >= n.straw_misfire_at)
        else {
            return Misfire::None;
        };
        if roll < n.straw_miss * c.potency {
            Misfire::Miss
        } else if roll < (n.straw_miss + n.straw_self) * c.potency {
            Misfire::SelfHarm
        } else {
            Misfire::None
        }
    }

    /// The highest-ranked liftable curse a lift of `rank` reaches, oldest of equals.
    fn liftable(&self, rank: Rank) -> Option<usize> {
        self.on
            .iter()
            .enumerate()
            .filter(|(_, c)| c.kind.liftable() && dispels(rank, Grade::Ranked(c.rank), false))
            .fold(None::<(usize, Rank)>, |best, (i, c)| match best {
                Some((_, r)) if r >= c.rank => best,
                _ => Some((i, c.rank)),
            })
            .map(|(i, _)| i)
    }

    /// Removes the curse at `i`; a Bói Ba Nhịp Thở broken this way turns on its diviner.
    fn lift(&mut self, i: usize) -> (Curse, Vec<Event>) {
        let c = self.on.remove(i);
        let mut out = Vec::new();
        if c.kind == Kind::ThreeBreaths {
            out.push(Event::Backfire {
                kind: c.kind,
                by: c.by,
                element: Element::Soul,
                share: NUMBERS.breaths_backfire * c.potency,
            });
        }
        out.push(Event::Ended(c.kind));
        (c, out)
    }

    /// A dispel of `rank` lifts the highest curse it reaches.
    pub fn dispel(&mut self, rank: Rank) -> Option<(Kind, Vec<Event>)> {
        let i = self.liftable(rank)?;
        let (c, out) = self.lift(i);
        Some((c.kind, out))
    }

    /// Chúc phúc of `rank` on oneself: lifts the highest curse of that rank or lower and turns it into its opposite
    /// for [`Rules::boon_time`] s.
    pub fn bless(&mut self, rank: Rank) -> Option<Blessed> {
        let i = self.liftable(rank)?;
        let (c, events) = self.lift(i);
        let boon = Boon {
            from: c.kind,
            potency: c.potency,
            strength: c.state.stacks.max(1.0),
            left: RULES.boon_time,
        };
        self.boons.push(boon);
        Some(Blessed { boon, events })
    }

    /// The curser `by` died: its curses fall; a Tử Ấn's bearer loses half its current life (`dao::Tether`).
    /// Returns the life lost.
    pub fn curser_died(&mut self, by: Id, bearer: &mut Cultivator) -> f64 {
        let seal = self
            .on
            .iter()
            .any(|c| c.by == by && c.kind == Kind::DeathSeal);
        self.on.retain(|c| c.by != by);
        if seal {
            Tether::curser_died(bearer)
        } else {
            0.0
        }
    }

    /// Tam Mệnh's revival cuts every karma on the bearer: all curses, the tether with them. Returns how many fell.
    pub fn sever(&mut self) -> usize {
        let n = self.on.len();
        self.on.clear();
        n
    }

    /// The Tử Ấn tether on the bearer, if any.
    pub fn tether(&self) -> Option<&Tether> {
        self.on.iter().find_map(|c| c.state.tether.as_ref())
    }

    /// A sword of `tags` and `blow` grade cuts a showing tether: the Tử Ấn falls. Returns whether it did.
    pub fn cut_tether(&mut self, tags: Tags, blow: Grade) -> bool {
        let before = self.on.len();
        self.on
            .retain(|c| !c.state.tether.is_some_and(|t| t.cut_by(tags, blow)));
        self.on.len() < before
    }

    /// Damage the bearer takes, scaled (Trớ Thuật's stacks, a doom boon), from [`Curses::mods`].
    pub fn taken(&self) -> f64 {
        self.table(false).get(Stat::Taken, 1.0)
    }

    /// The bearer's resistance to `e`, moved by curses (Khắc Ấn, Tâm Nứt on soul) and boons, from
    /// [`Curses::mods`].
    pub fn resist(&self, e: Element) -> f64 {
        self.table(false).flat(Stat::Resist(e))
    }

    /// Tâm Nứt opens the bearer to Đoạt Xá.
    pub fn cracked(&self) -> bool {
        self.on
            .iter()
            .any(|c| matches!(c.kind, Kind::BindingWord(_)) && c.state.cracked > 0.0)
    }

    /// The bearer's speed factor under Tuế Nguyệt (1 for a still tree watcher), and haste from boons, from
    /// [`Curses::mods`].
    pub fn speed(&self, tree_immune: bool) -> f64 {
        self.table(tree_immune).get(Stat::Speed, 1.0)
    }

    /// More damage the bearer deals, from boons ([`Curses::mods`]).
    pub fn more(&self) -> f64 {
        self.table(false).more(Stat::Damage) - 1.0
    }

    /// A skill of `tags` is locked: Phong Thần's sealed tag, Ngũ Tạng's broken liver (Recovery) or lungs (Warcry,
    /// Sound).
    pub fn barred(&self, tags: Tags) -> bool {
        self.on.iter().any(|c| match c.kind {
            Kind::SealGod(t) => tags.has(t),
            Kind::FiveOrgans => {
                (c.state.beats >= 1 && tags.has(Tag::Recovery))
                    || (c.state.beats >= 2 && tags.any(&[Tag::Warcry, Tag::Sound]))
            }
            _ => false,
        })
    }

    /// Whether Ngũ Tạng has broken `organ`.
    pub fn broken(&self, organ: Organ) -> bool {
        let need = Organ::ALL
            .iter()
            .position(|&o| o == organ)
            .map_or(u8::MAX, |i| i as u8 + 1);
        self.on
            .iter()
            .any(|c| c.kind == Kind::FiveOrgans && c.state.beats >= need)
    }

    /// No healing (Ngũ Tạng, the liver).
    pub fn heal_blocked(&self) -> bool {
        self.broken(Organ::Liver)
    }

    /// No qi regen (Ngũ Tạng, the spleen).
    pub fn qi_blocked(&self) -> bool {
        self.broken(Organ::Spleen)
    }

    /// No sense regen (Ngũ Tạng, the kidneys).
    pub fn sense_blocked(&self) -> bool {
        self.broken(Organ::Kidneys)
    }

    /// Khắc Ấn: the bearer is shown (no hiding) and projectiles home on it.
    pub fn exposed(&self) -> bool {
        self.on
            .iter()
            .any(|c| matches!(c.kind, Kind::ExposeMark(_)))
    }

    /// Hóa Ma: a buff of `tags` the bearer holds works against it (an immortal one).
    pub fn turns(&self, buff: Tags) -> bool {
        buff.has(Tag::Immortal) && self.on.iter().any(|c| c.kind == Kind::Demonize)
    }

    /// The curses a viewer sees on the bearer: Mộng Đạo only under glare.
    pub fn shown(&self, glared: bool) -> impl Iterator<Item = &Curse> {
        self.on.iter().filter(move |c| glared || c.kind.shown())
    }
}

/// The curser's curse effect: its [`Stat::CurseEffect`] from its `sheet`, timed effects and state, with no target and
/// no skill (1 for none, never below 0).
pub fn curse_effect(curser: &Cultivator, sheet: &Sheet) -> f64 {
    sheet
        .table(&Ctx::alone(curser), [])
        .get(Stat::CurseEffect, 1.0)
        .max(0.0)
}

/// How many curses a fighter whose curses are laid by `curser` (wearing `sheet`) can carry at once: the rule's
/// ([`RULES`]`.cap`, or `cap_vu_chu` under Vu Chú) plus the curser's whole [`Stat::CurseCount`]; at least 1. What
/// [`Curses::cap`] is set to.
pub fn curse_limit(curser: &Cultivator, sheet: &Sheet, vu_chu: bool) -> usize {
    let base = if vu_chu { RULES.cap_vu_chu } else { RULES.cap };
    let more = sheet
        .table(&Ctx::alone(curser), [])
        .flat(Stat::CurseCount)
        .clamp(0.0, 8.0);
    base.saturating_add(libm::floor(more) as usize).max(1)
}

/// [`cast`] by a curser whose curse effect is `effect` ([`curse_effect`]): the curse's potency is multiplied by it
/// before backlash, so every effect it has (its modifiers, its strikes, its drains) grows with it; its time does not.
pub fn cast_with(mut curse: Curse, target: &mut Curses, caster: &mut Curses, effect: f64) -> Lay {
    curse.potency *= effect.max(0.0);
    cast(curse, target, caster)
}

/// `caster` lays `curse` on `target`, with backlash: when the target is cursing the caster, the new curse is
/// [`Rules::backlash`] times as strong and the target's curses on the caster fall. The curse comes at its own
/// potency: [`cast_with`] for a curser with a curse effect.
pub fn cast(mut curse: Curse, target: &mut Curses, caster: &mut Curses) -> Lay {
    let backlash = caster.on.iter().any(|c| c.by == target.owner);
    if backlash {
        curse.potency *= RULES.backlash;
    }
    let lay = target.lay(curse);
    if backlash && lay != Lay::Failed {
        caster.on.retain(|c| c.by != target.owner);
    }
    lay
}

/// Phong Thần's price: the curser's full life falls by [`Numbers::god_cost`] (life follows down). Returns how much.
pub fn seal_cost(curser: &mut Cultivator) -> f64 {
    let v = NUMBERS.god_cost * curser.life_max;
    curser.life_max -= v;
    curser.life = curser.life.min(curser.life_max);
    v
}

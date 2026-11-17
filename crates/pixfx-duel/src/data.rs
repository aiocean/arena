//! The duel's tables and numbers: skills, schools, stats, tiers, traits, luck, and the constants every rule reads.
//! `tests/data.rs` pins each against `tests/fixtures/js_numbers.json`.

use crate::body::BodySpec;
use crate::js::js_min;

/// Effects the arena itself shows when they are in the registry: the lamps and the night.
pub const DECOR: [&str; 2] = ["fire/brazier", "weather/fireflies"];

/// Flying: the highest a fighter goes, and the qi it costs a second.
pub struct Fly {
    pub top: f64,
    pub cost: f64,
}
pub const FLY: Fly = Fly {
    top: 384.0,
    cost: 3.0,
};

/// Rage is a small whole-number pool: a point for each blow landed; when full, the school's ultimate.
pub const RAGE: f64 = 4.0;
/// Frozen: no moving, no acting, in seconds.
pub const FREEZE: f64 = 0.9;

/// Chill: each chilling hit adds a stack (at most `max`), all fading `time` seconds after the last; each slows
/// walking by `slow`, and the last freezes and clears them.
pub struct Chill {
    pub max: f64,
    pub time: f64,
    pub slow: f64,
}
pub const CHILL: Chill = Chill {
    max: 3.0,
    time: 2.5,
    slow: 0.85,
};

/// Shock: each shocking hit adds a stack (at most `max`); a shocked fighter twitches every
/// `every × fast^(stacks − 1)` seconds, spending a stack, losing `share` of the caster's attack and held `hold` s.
pub struct Shock {
    pub max: f64,
    pub every: f64,
    pub fast: f64,
    pub share: f64,
    pub hold: f64,
}
pub const SHOCK: Shock = Shock {
    max: 5.0,
    every: 0.6,
    fast: 0.854,
    share: 0.03,
    hold: 0.12,
};

/// While the foe carries a status, the share of draws that come from the skills that spend it.
pub const PAY_DRAW: f64 = 1.0 / 3.0;

/// A status a payoff skill spends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Shock,
    Chill,
}

/// Skills that spend the foe's stacks of a status to hit harder: this much more power per stack.
pub const PAYOFF: &[(&str, Status, f64)] = &[
    ("palm", Status::Shock, 0.12),
    ("burst", Status::Shock, 0.12),
    ("shatter", Status::Chill, 0.15),
    ("lotus", Status::Chill, 0.15),
    ("gather", Status::Shock, 0.12),
    ("yellowspring", Status::Chill, 0.15),
];

/// The payoff of spell `name`, if it is one.
pub fn payoff(name: &str) -> Option<(Status, f64)> {
    PAYOFF
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|&(_, s, per)| (s, per))
}

/// Grit: each hard control adds `per`; grit falls `fall` a second. A control lasts `seconds / (1 + grit)`, and from
/// grit `thaw` a freeze is only a chill stack.
pub struct Grit {
    pub per: f64,
    pub fall: f64,
    pub thaw: f64,
}
pub const GRIT: Grit = Grit {
    per: 1.0,
    fall: 0.4,
    thaw: 2.0,
};

pub const DEFENCE: f64 = 500.0;
pub const SPREAD: f64 = 0.03;

/// Fury: each blow a fury fighter lands raises its attack by `per`, up to `max` stacks, for the rest of the round.
pub struct Fury {
    pub per: f64,
    pub max: f64,
}
pub const FURY: Fury = Fury {
    per: 0.04,
    max: 5.0,
};

/// Defence broken by a spell: this much defence lost.
pub const DEF_DOWN: f64 = 0.3;

/// Burn: a share of the caster's attack every tick, ignoring defence.
pub struct Burn {
    pub time: f64,
    pub every: f64,
    pub share: f64,
}
pub const BURN: Burn = Burn {
    time: 3.0,
    every: 0.5,
    share: 0.05,
};

/// A ward holds 1 to `charges` charges for `time` seconds; each erases one whole blow.
pub struct Ward {
    pub charges: f64,
    pub time: f64,
}
pub const WARD: Ward = Ward {
    charges: 3.0,
    time: 6.0,
};

/// A skill's tier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Rarity {
    Common = 0,
    Rare = 1,
    Epic = 2,
    Legendary = 3,
}

impl Rarity {
    /// The tiers, lowest first: a duplicate draw lifts a skill one step, up to the last.
    pub const ALL: [Rarity; 4] = [
        Rarity::Common,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Legendary,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
        }
    }

    /// One tier up; a legendary stays legendary.
    pub fn up(self) -> Rarity {
        match self {
            Rarity::Common => Rarity::Rare,
            Rarity::Rare => Rarity::Epic,
            Rarity::Epic | Rarity::Legendary => Rarity::Legendary,
        }
    }
}

/// How likely a draw is to bring a tier, how much more power, and how many traits it brings (a common one: `chance`
/// of one). A legendary is never drawn: a skill reaches it only by duplicates.
#[derive(Clone, Copy, Debug)]
pub struct RarityData {
    pub odds: f64,
    pub power: f64,
    pub traits: usize,
    pub chance: Option<f64>,
}
pub const RARITY: [RarityData; 4] = [
    RarityData {
        odds: 0.7,
        power: 1.0,
        traits: 0,
        chance: Some(0.4),
    },
    RarityData {
        odds: 0.22,
        power: 1.15,
        traits: 1,
        chance: None,
    },
    RarityData {
        odds: 0.08,
        power: 1.3,
        traits: 2,
        chance: None,
    },
    RarityData {
        odds: 0.0,
        power: 1.5,
        traits: 3,
        chance: None,
    },
];

/// A trait a bar skill can be drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Trait {
    Swift,
    Heavy,
    Piercing,
    Lingering,
    Cheap,
}

impl Trait {
    /// In the order traits are rolled from.
    pub const ALL: [Trait; 5] = [
        Trait::Swift,
        Trait::Heavy,
        Trait::Piercing,
        Trait::Lingering,
        Trait::Cheap,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Trait::Swift => "swift",
            Trait::Heavy => "heavy",
            Trait::Piercing => "piercing",
            Trait::Lingering => "lingering",
            Trait::Cheap => "cheap",
        }
    }
}

/// What each trait does to the skill it is drawn on.
pub struct Traits {
    pub swift_cooldown: f64,
    pub heavy_power: f64,
    pub lingering_stacks: f64,
    pub cheap_cost: f64,
}
pub const TRAITS: Traits = Traits {
    swift_cooldown: 0.6,
    heavy_power: 1.3,
    lingering_stacks: 1.0,
    cheap_cost: 0.5,
};

/// What luck does to a cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Luck {
    /// Hits twice as hard.
    Double,
    /// Costs nothing.
    Free,
    /// Ready again at once.
    Again,
}

impl Luck {
    pub fn name(self) -> &'static str {
        match self {
            Luck::Double => "double",
            Luck::Free => "free",
            Luck::Again => "again",
        }
    }
}

/// A cast of a bar skill is lucky this often.
pub struct Lucky {
    pub chance: f64,
    pub kinds: [Luck; 3],
}
pub const LUCKY: Lucky = Lucky {
    chance: 0.08,
    kinds: [Luck::Double, Luck::Free, Luck::Again],
};

/// The kinds of blow a guard can turn aside (a zone or a field falls past it).
pub const GUARDED: [Kind; 5] = [Kind::Melee, Kind::Blast, Kind::Dash, Kind::Bolt, Kind::Orb];

/// Whether `t` does anything to spell `name`: heavy needs a blow, piercing a blow a guard can stop, lingering a chill
/// or a shock.
pub fn fits(t: Trait, name: &str) -> bool {
    let Some(s) = spell(name) else { return false };
    let damage = s.damage.unwrap_or(0.0);
    match t {
        Trait::Heavy => damage > 0.0,
        Trait::Piercing => damage > 0.0 && GUARDED.contains(&s.kind),
        Trait::Lingering => s.chill || s.shock,
        Trait::Swift | Trait::Cheap => true,
    }
}

/// Power a skill gains for each point of life its caster spends on it (`Spell::blood`).
pub const BLOOD_POWER: f64 = 0.03;

/// An evolved skill is this many times as strong.
pub const EVOLVED: f64 = 1.5;

/// The power a bar skill strikes with: its tier, heavy, and evolution.
pub fn slot_power(rarity: Rarity, traits: &[Trait], evolved: bool) -> f64 {
    RARITY[rarity as usize].power
        * (if traits.contains(&Trait::Heavy) {
            TRAITS.heavy_power
        } else {
            1.0
        })
        * (if evolved { EVOLVED } else { 1.0 })
}

/// How many of its school's pool a fighter plays in one fight.
pub const POOL_SIZE: usize = 8;

/// The rotating bar: `slots` skills at a time; an emptied slot counts down `wait + spread × roll` seconds.
pub struct Bar {
    pub slots: usize,
    pub wait: f64,
    pub spread: f64,
}
pub const BAR: Bar = Bar {
    slots: 3,
    wait: 1.5,
    spread: 1.0,
};

/// Seconds a bar skill may sit ready and uncast before it leaves for another draw.
pub const STALE: f64 = 6.0;

/// The pairs that evolve, per school.
pub fn evolves(school: School) -> &'static [[&'static str; 2]] {
    match school {
        School::Thunder => &[
            ["bolt", "seal"],
            ["palm", "step"],
            ["quake", "burst"],
            ["fireball", "vermilion"],
            ["jithunder", "finger"],
            ["gravity", "earthsplit"],
            ["gather", "heartbeat"],
            ["slashluo", "onestep"],
            ["mountain", "ancientgod"],
            ["ancientthunder", "tigerflag"],
            ["demonfinger", "kunjiwhip"],
            ["fireescape", "flameorigin"],
            ["warprint", "heavenlychop"],
            ["fingerdeath", "celestialslaughter"],
        ],
        School::Frost => &[
            ["shard", "icicle"],
            ["shatter", "flurry"],
            ["lotus", "jet"],
            ["summonrain", "summonwind"],
            ["yinmoon", "remnantnight"],
            ["bodyfix", "lifedeath"],
            ["soulsearch", "dreamdao"],
            ["thirdeye", "sevencolor"],
            ["restriction", "plumblossom"],
            ["starfall", "heavenflip"],
            ["yellowspring", "devour"],
            ["timerestrict", "lifedeathrestrict"],
            ["soulbanner", "servitude"],
            ["voidnail", "seize"],
            ["ghostsummon", "soulvortex"],
            ["soulrefine", "bodyformation"],
            ["starrotation", "meritspirit"],
            ["daoyellow", "springsoul"],
        ],
    }
}

/// A school.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum School {
    Thunder,
    Frost,
}

impl School {
    pub fn name(self) -> &'static str {
        match self {
            School::Thunder => "thunder",
            School::Frost => "frost",
        }
    }
}

/// Each school's fighter: full life, attack, the two defences, crit chance and damage, dodge, and its passives.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Stats {
    pub hp: f64,
    pub atk: f64,
    pub pdef: f64,
    pub mdef: f64,
    pub crit: f64,
    pub crit_damage: f64,
    pub dodge: f64,
    pub fury: bool,
    pub unyielding: bool,
    pub attuned: bool,
}

pub fn stats(school: School) -> &'static Stats {
    static THUNDER: Stats = Stats {
        hp: 2500.0,
        atk: 345.0,
        pdef: 130.0,
        mdef: 110.0,
        crit: 0.18,
        crit_damage: 1.6,
        dodge: 0.05,
        fury: true,
        unyielding: true,
        attuned: false,
    };
    static FROST: Stats = Stats {
        hp: 3300.0,
        atk: 520.0,
        pdef: 145.0,
        mdef: 165.0,
        crit: 0.15,
        crit_damage: 1.7,
        dodge: 0.11,
        fury: false,
        unyielding: false,
        attuned: true,
    };
    match school {
        School::Thunder => &THUNDER,
        School::Frost => &FROST,
    }
}

pub const QI_REGEN: f64 = 3.0;
pub const MEDITATE_REGEN: f64 = 16.0;

/// Each fighter's temperament for arena-mind.
pub struct Temperament {
    pub spread: f64,
    pub risk: f64,
    pub floor: f64,
    pub lead: f64,
    pub prior: f64,
    pub reaction: f64,
    pub think: f64,
}
pub const TEMPERAMENT: Temperament = Temperament {
    spread: 0.8,
    risk: 0.6,
    floor: 0.3,
    lead: 0.8,
    prior: 6.0,
    reaction: 0.16,
    think: 0.12,
};

/// Qi a block costs.
pub const GUARD_COST: f64 = 8.0;
/// The share of a blocked blow's damage that still lands.
pub const CHIP: f64 = 0.2;
/// Guard by the size of the blow: a guarded blow of `power` is turned aside with chance `min(0.5r, 0.5√r)`.
pub const GUARD: f64 = 20.0;

/// The chance a guard turns aside a blow of `power`.
pub fn block_chance(power: f64) -> f64 {
    // JS `!(power > 0)` holds for NaN too
    if power.is_nan() || power <= 0.0 {
        return 1.0;
    }
    let r = GUARD / power;
    js_min(js_min(1.0, 0.5 * r), 0.5 * libm::sqrt(r))
}

pub const KNOCKBACK: f64 = 520.0;

/// The arena's pixels to the engine's: every length, speed and pull in these rules is four times the engine's site
/// duel, so an HD fighter (a 256-pixel canvas, about 240 pixels tall) stands its full height. The minds still think in the engine's pixels
/// (`sense`).
pub const UNIT: f64 = 4.0;

/// A fighter's body: the engine's site duel ([`BodySpec::default`]) in the arena's pixels; its times and shares as
/// they are.
pub fn body_spec() -> BodySpec {
    let d = BodySpec::default();
    BodySpec {
        body: d.body * UNIT,
        chest: d.chest * UNIT,
        half_width: d.half_width * UNIT,
        room: d.room * UNIT,
        walk: d.walk * UNIT,
        jump_speed: d.jump_speed * UNIT,
        gravity: d.gravity * UNIT,
        apex_speed: d.apex_speed * UNIT,
        fly_max: d.fly_max * UNIT,
        launch_speed: d.launch_speed * UNIT,
        launch_per_damage: d.launch_per_damage * UNIT,
        launch_pop: d.launch_pop * UNIT,
        knockdown_bounce: d.knockdown_bounce * UNIT,
        knockdown_roll_distance: d.knockdown_roll_distance * UNIT,
        duck: d.duck * UNIT,
        bounce: d.bounce * UNIT,
        hard_landing: d.hard_landing * UNIT,
        knockback: d.knockback * UNIT,
        soft_landing: d.soft_landing * UNIT,
        ..d
    }
}
pub const ROUND_PAUSE: f64 = 3.2;
pub const CAST_CIRCLE: &str = "arcane/rune-circle";
/// How long a barrier flares after it turns a blow aside, in seconds.
pub const RING: f64 = 0.22;
/// How urgent a threat must be before a fighter spends its barrier on it.
pub const BARRIER_THREAT: f64 = 10.0;
pub const CLASH: &str = "impacts/parry";

/// What being struck at a weak moment adds to a blow: its power is multiplied by `power` (the power damage and launch
/// read), and a blow that staggers rather than throws holds its target `stagger` seconds longer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bonus {
    pub power: f64,
    pub stagger: f64,
}

/// Frame data, the way Street Fighter has it: a fighter struck while it gathers a cast takes a COUNTER, one struck
/// while it recovers takes a PUNISH (a field's strike gives neither); the skills in `unsafe_on_block` recover
/// `blocked_recover` seconds longer when their blow is guarded; an ultimate cannot be hit for its windup's first
/// `ultimate_invincible` seconds. The feel answers a counter and a punish with its own longer hit-stop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frames {
    pub counter: Bonus,
    pub punish: Bonus,
    pub blocked_recover: f64,
    pub unsafe_on_block: &'static [&'static str],
    pub ultimate_invincible: f64,
}
pub const FRAMES: Frames = Frames {
    counter: Bonus {
        power: 1.2,
        stagger: 0.1,
    },
    punish: Bonus {
        power: 1.2,
        stagger: 0.2,
    },
    blocked_recover: 0.15,
    // dive, dash and the close blasts; an ultimate falls as a zone, which no guard stops
    unsafe_on_block: &[
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
    ],
    ultimate_invincible: 0.3,
};

/// What a spell does when it lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Kind {
    Melee,
    Dash,
    Blast,
    Bolt,
    Orb,
    Zone,
    Void,
    Heal,
    Ward,
    Barrier,
    Blink,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Melee => "melee",
            Kind::Dash => "dash",
            Kind::Blast => "blast",
            Kind::Bolt => "bolt",
            Kind::Orb => "orb",
            Kind::Zone => "zone",
            Kind::Void => "void",
            Kind::Heal => "heal",
            Kind::Ward => "ward",
            Kind::Barrier => "barrier",
            Kind::Blink => "blink",
        }
    }
}

/// Which defence a blow is cut by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Blow {
    Physical,
    Magic,
}

impl Blow {
    pub fn name(self) -> &'static str {
        match self {
            Blow::Physical => "physical",
            Blow::Magic => "magic",
        }
    }
}

/// A golden bell's answer to a close blow.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Counter {
    pub damage: f64,
    pub stun: f64,
}

/// A spell: what it costs, how long the fighter is committed before (`windup`) and after (`recover`) it lands, its
/// cooldown, and its own numbers. Fields a spell does without are `None` or `false`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Spell {
    #[cfg_attr(feature = "serde", serde(skip))]
    pub name: &'static str,
    pub kind: Kind,
    #[cfg_attr(
        feature = "serde",
        serde(rename = "type", skip_serializing_if = "Option::is_none")
    )]
    pub blow: Option<Blow>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub fx: Option<&'static str>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub hit_fx: Option<&'static str>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub end_fx: Option<&'static str>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub slam_fx: Option<&'static str>,
    pub cost: f64,
    pub windup: f64,
    pub recover: f64,
    pub cooldown: f64,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub range: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub distance: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub damage: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub sigil: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub impact: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub radius: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub notice: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub def_down: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub life: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub height: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub pull: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub reach: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub under: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub tick: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub freeze: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub speed: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub silence: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub pull_until: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub steal_rage: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub strikes: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub every: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub aim_ahead: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub amount: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub time: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub spray: Option<f64>,
    /// Life the caster spends to cast it (never its last point): each point adds `BLOOD_POWER` to the blow's power.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub blood: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub shock: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub chill: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub dive: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub burn: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub grounded: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub ultimate: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub seals: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub reflect: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub feet: bool,
    /// A blink that trades places with the foe instead of moving to a point.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub swap: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub counter: Option<Counter>,
}

const BASE: Spell = Spell {
    name: "",
    kind: Kind::Melee,
    blow: None,
    fx: None,
    hit_fx: None,
    end_fx: None,
    slam_fx: None,
    cost: 0.0,
    windup: 0.0,
    recover: 0.0,
    cooldown: 0.0,
    range: None,
    distance: None,
    damage: None,
    sigil: None,
    impact: None,
    radius: None,
    notice: None,
    def_down: None,
    life: None,
    height: None,
    pull: None,
    reach: None,
    under: None,
    tick: None,
    freeze: None,
    speed: None,
    silence: None,
    pull_until: None,
    steal_rage: None,
    strikes: None,
    every: None,
    aim_ahead: None,
    amount: None,
    time: None,
    spray: None,
    blood: None,
    shock: false,
    chill: false,
    dive: false,
    burn: false,
    grounded: false,
    ultimate: false,
    seals: false,
    reflect: false,
    feet: false,
    swap: false,
    counter: None,
};

use Blow::{Magic, Physical};

/// Every spell, in the duel's order.
static SPELLS: [Spell; 99] = [
    Spell {
        name: "slash",
        kind: Kind::Melee,
        blow: Some(Physical),
        cost: 0.0,
        windup: 0.2,
        recover: 0.32,
        cooldown: 0.7,
        range: Some(96.0),
        damage: Some(5.0),
        ..BASE
    },
    // Thunder: a flash of lightning that carries the body through the foe and out the other side
    Spell {
        name: "step",
        kind: Kind::Dash,
        blow: Some(Physical),
        fx: Some("spawn/poof"),
        cost: 24.0,
        windup: 0.26,
        recover: 0.45,
        cooldown: 3.4,
        distance: Some(288.0),
        damage: Some(15.0),
        ..BASE
    },
    // Thunder: a gold seal opens on the ground under the foe, and a thunderclap bursts from it
    Spell {
        name: "seal",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 30.0,
        windup: 0.3,
        recover: 0.3,
        cooldown: 2.4,
        sigil: Some(0.4),
        impact: Some(0.08),
        radius: Some(64.0),
        damage: Some(22.0),
        notice: Some(0.0),
        shock: true,
        ..BASE
    },
    // Frost: a lotus of ice shards blooms from the palm at close range, and cracks the foe's guard
    Spell {
        name: "lotus",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("holy/feather-burst"),
        cost: 24.0,
        windup: 0.3,
        recover: 0.45,
        cooldown: 2.2,
        range: Some(160.0),
        damage: Some(18.0),
        def_down: Some(3.0),
        ..BASE
    },
    // Frost: a pale fire that does not warm, lit under the foe; it burns whoever stands in it
    Spell {
        name: "frostfire",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("fire/spirit-flame"),
        cost: 33.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 5.0,
        life: Some(2.6),
        height: Some(0.0),
        pull: Some(0.0),
        reach: Some(72.0),
        under: Some(72.0),
        tick: Some(5.0),
        damage: Some(8.0),
        chill: true,
        ..BASE
    },
    // Frost: a blue seal under the foe that shatters into a burst of cold, freezing it
    Spell {
        name: "shatter",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 33.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.0,
        sigil: Some(0.35),
        impact: Some(0.08),
        radius: Some(64.0),
        damage: Some(25.0),
        notice: Some(0.0),
        freeze: Some(0.6),
        ..BASE
    },
    // from above: drop onto the foe sword first, and crack the ground if it lands there
    Spell {
        name: "dive",
        kind: Kind::Melee,
        blow: Some(Physical),
        dive: true,
        cost: 12.0,
        windup: 0.22,
        recover: 0.5,
        cooldown: 1.6,
        range: Some(104.0),
        damage: Some(11.0),
        speed: Some(1040.0),
        slam_fx: Some("explosions/dirt-explosion"),
        ..BASE
    },
    Spell {
        name: "bolt",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 18.0,
        windup: 0.28,
        recover: 0.22,
        cooldown: 1.3,
        speed: Some(920.0),
        damage: Some(13.0),
        shock: true,
        ..BASE
    },
    Spell {
        name: "orb",
        kind: Kind::Orb,
        blow: Some(Magic),
        fx: Some("lightning/ball-lightning"),
        hit_fx: Some("explosions/plasma-explosion"),
        burn: true,
        shock: true,
        cost: 45.0,
        windup: 0.6,
        recover: 0.3,
        cooldown: 4.5,
        speed: Some(104.0),
        life: Some(5.5),
        damage: Some(14.0),
        radius: Some(40.0),
        ..BASE
    },
    // icicle-fall drops its icicle at 0.944 s and only starts to tremble, which is when a fighter notices it, at 0.3 s
    Spell {
        name: "icicle",
        kind: Kind::Zone,
        blow: Some(Magic),
        def_down: Some(4.0),
        fx: Some("ice/icicle-fall"),
        cost: 33.0,
        windup: 0.42,
        recover: 0.28,
        cooldown: 2.2,
        impact: Some(0.944),
        radius: Some(64.0),
        damage: Some(28.0),
        notice: Some(0.3),
        grounded: true,
        freeze: Some(FREEZE),
        ..BASE
    },
    // implosion sucks in until 0.8 s and pops at 1.02 s
    Spell {
        name: "collapse",
        kind: Kind::Zone,
        blow: Some(Magic),
        silence: Some(1.5),
        fx: Some("explosions/implosion"),
        cost: 39.0,
        windup: 0.5,
        recover: 0.3,
        cooldown: 2.4,
        impact: Some(1.02),
        radius: Some(72.0),
        damage: Some(25.0),
        notice: Some(0.15),
        pull: Some(240.0),
        reach: Some(208.0),
        pull_until: Some(0.8),
        ..BASE
    },
    // a thunderclap from the open palm, at arm's length
    Spell {
        name: "palm",
        kind: Kind::Blast,
        blow: Some(Physical),
        steal_rage: Some(1.0),
        fx: Some("explosions/plasma-explosion"),
        cost: 27.0,
        windup: 0.32,
        recover: 0.5,
        cooldown: 2.8,
        range: Some(152.0),
        damage: Some(20.0),
        shock: true,
        ..BASE
    },
    // ultimates, cast with full rage. Thunder: three thunderclaps fall one after another where the foe will be.
    Spell {
        name: "tribulation",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        ultimate: true,
        cost: 0.0,
        windup: 0.7,
        recover: 0.8,
        cooldown: 0.0,
        impact: Some(0.08),
        radius: Some(48.0),
        damage: Some(14.0),
        notice: Some(0.0),
        strikes: Some(3.0),
        every: Some(0.7),
        aim_ahead: Some(0.5),
        ..BASE
    },
    // Frost: a great collapse that drags the foe in from far off and freezes it where it pops.
    Spell {
        name: "prison",
        kind: Kind::Zone,
        blow: Some(Magic),
        silence: Some(2.0),
        fx: Some("explosions/implosion"),
        ultimate: true,
        cost: 0.0,
        windup: 0.7,
        recover: 0.8,
        cooldown: 0.0,
        impact: Some(1.02),
        radius: Some(104.0),
        damage: Some(31.0),
        notice: Some(0.1),
        pull: Some(280.0),
        reach: Some(440.0),
        pull_until: Some(0.9),
        seals: true,
        freeze: Some(FREEZE * 1.4),
        ..BASE
    },
    // void-orb hangs 62 px over its origin and loops every 3.2 s
    Spell {
        name: "void",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("shadow/void-orb"),
        end_fx: Some("explosions/implosion"),
        cost: 60.0,
        windup: 0.9,
        recover: 0.45,
        cooldown: 11.0,
        life: Some(3.2),
        height: Some(248.0),
        pull: Some(136.0),
        reach: Some(360.0),
        under: Some(88.0),
        tick: Some(4.0),
        damage: Some(14.0),
        ..BASE
    },
    Spell {
        name: "heal",
        kind: Kind::Heal,
        fx: Some("holy/feather-burst"),
        cost: 35.0,
        windup: 0.85,
        recover: 0.45,
        cooldown: 9.0,
        amount: Some(20.0),
        ..BASE
    },
    // a shell of frost qi that takes blows for the caster until it breaks
    Spell {
        name: "ward",
        kind: Kind::Ward,
        fx: Some("holy/feather-burst"),
        cost: 45.0,
        windup: 0.5,
        recover: 0.25,
        cooldown: 8.0,
        ..BASE
    },
    // the defences: a dome of qi for a moment that turns every blow aside
    Spell {
        name: "bell",
        kind: Kind::Barrier,
        fx: Some("arcane/rune-circle"),
        cost: 18.0,
        windup: 0.06,
        recover: 0.1,
        cooldown: 6.5,
        time: Some(2.0),
        counter: Some(Counter {
            damage: 9.0,
            stun: 0.6,
        }),
        ..BASE
    },
    Spell {
        name: "mirror",
        kind: Kind::Barrier,
        fx: Some("arcane/rune-circle"),
        cost: 18.0,
        windup: 0.06,
        recover: 0.1,
        cooldown: 6.5,
        time: Some(2.2),
        reflect: true,
        ..BASE
    },
    Spell {
        name: "blink",
        kind: Kind::Blink,
        fx: Some("spawn/poof"),
        cost: 20.0,
        windup: 0.06,
        recover: 0.12,
        cooldown: 1.8,
        distance: Some(224.0),
        ..BASE
    },
    // Thunder: a quick spark off the fingertips, light but fast
    Spell {
        name: "spark",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 10.0,
        windup: 0.18,
        recover: 0.18,
        cooldown: 0.9,
        speed: Some(1200.0),
        damage: Some(8.0),
        shock: true,
        ..BASE
    },
    // Thunder: a palm that bursts with everything the foe's shocks have gathered
    Spell {
        name: "burst",
        kind: Kind::Blast,
        blow: Some(Physical),
        fx: Some("explosions/plasma-explosion"),
        cost: 30.0,
        windup: 0.36,
        recover: 0.5,
        cooldown: 3.0,
        range: Some(120.0),
        damage: Some(24.0),
        ..BASE
    },
    // Thunder: the ground under the foe cracks and blows up
    Spell {
        name: "quake",
        kind: Kind::Zone,
        blow: Some(Physical),
        fx: Some("explosions/small-explosion"),
        cost: 26.0,
        windup: 0.3,
        recover: 0.3,
        cooldown: 2.6,
        sigil: Some(0.3),
        impact: Some(0.05),
        radius: Some(56.0),
        damage: Some(18.0),
        notice: Some(0.0),
        grounded: true,
        ..BASE
    },
    // Thunder: a dome of hex cells that takes blows for the caster; it stands on the caster's feet
    Spell {
        name: "hex",
        kind: Kind::Ward,
        fx: Some("arcane/hex-shield"),
        feet: true,
        cost: 40.0,
        windup: 0.45,
        recover: 0.25,
        cooldown: 8.0,
        ..BASE
    },
    // Frost: a sliver of ice, light and quick, that chills where it strikes
    Spell {
        name: "shard",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 10.0,
        windup: 0.18,
        recover: 0.18,
        cooldown: 0.9,
        speed: Some(1200.0),
        damage: Some(7.0),
        chill: true,
        ..BASE
    },
    // Frost: a jet of freezing water from the palm, at middle reach; it sprays for `spray` seconds, then stops
    Spell {
        name: "jet",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("water/water-jet"),
        spray: Some(0.6),
        cost: 22.0,
        windup: 0.3,
        recover: 0.35,
        cooldown: 2.4,
        range: Some(224.0),
        damage: Some(14.0),
        chill: true,
        ..BASE
    },
    // Frost: a short flurry of snow under the foe: two ticks that chill whoever stands in it, then its blow
    Spell {
        name: "flurry",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("weather/snow"),
        cost: 30.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 5.0,
        life: Some(1.2),
        height: Some(0.0),
        pull: Some(0.0),
        reach: Some(88.0),
        under: Some(88.0),
        tick: Some(3.0),
        damage: Some(10.0),
        chill: true,
        ..BASE
    },
    // Thunder: Fireball Technique: A ball of fire that flies at the foe and sets it burning.
    Spell {
        name: "fireball",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 14.0,
        windup: 0.26,
        recover: 0.24,
        cooldown: 1.5,
        damage: Some(12.0),
        speed: Some(840.0),
        burn: true,
        ..BASE
    },
    // Thunder: Gravity Technique / Attractive Force: Pulls the foe in toward a point, then crushes it there.
    Spell {
        name: "gravity",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/swirl-blast"),
        cost: 22.0,
        windup: 0.3,
        recover: 0.3,
        cooldown: 2.6,
        damage: Some(20.0),
        impact: Some(0.35),
        radius: Some(56.0),
        notice: Some(0.1),
        pull: Some(200.0),
        reach: Some(184.0),
        pull_until: Some(0.3),
        ..BASE
    },
    // Thunder: Earth Split / Lands Collapse: The ground under the foe splits and blows up, breaking its defence. Only hits on the ground.
    Spell {
        name: "earthsplit",
        kind: Kind::Zone,
        blow: Some(Physical),
        fx: Some("explosions/small-explosion"),
        cost: 28.0,
        windup: 0.32,
        recover: 0.3,
        cooldown: 3.2,
        damage: Some(26.0),
        sigil: Some(0.35),
        impact: Some(0.05),
        radius: Some(72.0),
        notice: Some(0.0),
        def_down: Some(3.0),
        grounded: true,
        ..BASE
    },
    // Thunder: Finger of Annihilation: A thin, fast beam from one finger that hits hard.
    Spell {
        name: "finger",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 28.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 3.5,
        damage: Some(21.0),
        speed: Some(1520.0),
        ..BASE
    },
    // Thunder: Ji Thunder: A heavy bolt of the Ji Realm that shocks.
    Spell {
        name: "jithunder",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 24.0,
        windup: 0.3,
        recover: 0.25,
        cooldown: 2.0,
        damage: Some(15.0),
        speed: Some(1040.0),
        shock: true,
        ..BASE
    },
    // Thunder: Thunder Gathering Seal: Gathers thunder under the foe and releases it in one burst: shocks, and hits harder per shock stack on the foe.
    Spell {
        name: "gather",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 36.0,
        windup: 0.4,
        recover: 0.35,
        cooldown: 4.2,
        damage: Some(34.0),
        sigil: Some(0.3),
        impact: Some(0.08),
        radius: Some(80.0),
        notice: Some(0.0),
        shock: true,
        ..BASE
    },
    // Thunder: Ancient God Blood: The body swells into an Ancient God phantom for one heavy blow at close range.
    Spell {
        name: "ancientgod",
        kind: Kind::Blast,
        blow: Some(Physical),
        fx: Some("explosions/swirl-blast"),
        cost: 36.0,
        windup: 0.5,
        recover: 0.55,
        cooldown: 5.0,
        range: Some(160.0),
        damage: Some(28.0),
        ..BASE
    },
    // Thunder: Slash Luo Art (Zhanluo): A sword form that cuts through space: dashes through the foe along a long line.
    Spell {
        name: "slashluo",
        kind: Kind::Dash,
        blow: Some(Physical),
        fx: Some("arcane/sword-slash"),
        cost: 30.0,
        windup: 0.3,
        recover: 0.5,
        cooldown: 3.8,
        distance: Some(360.0),
        damage: Some(22.0),
        ..BASE
    },
    // Thunder: Magic Arsenal: A ring of flying swords turns over the foe and cuts whoever stands under it, then strikes.
    Spell {
        name: "arsenal",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("arcane/sword-formation"),
        end_fx: Some("arcane/sword-strike"),
        cost: 48.0,
        windup: 0.6,
        recover: 0.4,
        cooldown: 9.0,
        damage: Some(12.0),
        life: Some(3.0),
        height: Some(160.0),
        pull: Some(0.0),
        reach: Some(160.0),
        under: Some(104.0),
        tick: Some(9.0),
        ..BASE
    },
    // Thunder: Shrinking the Earth into an Inch / Great Teleportation Restriction Art: A long blink: the ground folds and the caster steps out far away.
    Spell {
        name: "inchearth",
        kind: Kind::Blink,
        fx: Some("spawn/swirl-portal"),
        cost: 26.0,
        windup: 0.06,
        recover: 0.12,
        cooldown: 3.0,
        distance: Some(368.0),
        ..BASE
    },
    // Thunder: One Step Tread the Heaven: One stride that carries the caster through the foe, a long way, cutting it on the way.
    Spell {
        name: "onestep",
        kind: Kind::Dash,
        blow: Some(Physical),
        fx: Some("spawn/poof"),
        cost: 30.0,
        windup: 0.2,
        recover: 0.4,
        cooldown: 4.0,
        distance: Some(480.0),
        damage: Some(14.0),
        ..BASE
    },
    // Thunder: Mountain Collapse / Mountain Crumbles: A peak falls on the foe: a huge blow after a long mark. Only hits on the ground.
    Spell {
        name: "mountain",
        kind: Kind::Zone,
        blow: Some(Physical),
        fx: Some("explosions/small-explosion"),
        cost: 44.0,
        windup: 0.6,
        recover: 0.45,
        cooldown: 5.5,
        damage: Some(34.0),
        sigil: Some(0.4),
        impact: Some(0.05),
        radius: Some(104.0),
        notice: Some(0.0),
        grounded: true,
        ..BASE
    },
    // Thunder: Heartbeat Thunder Sound: A thunderclap at the heart, at close range: shocks.
    Spell {
        name: "heartbeat",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 20.0,
        windup: 0.24,
        recover: 0.35,
        cooldown: 2.0,
        range: Some(176.0),
        damage: Some(12.0),
        shock: true,
        ..BASE
    },
    // Thunder: Light and Shadow Shield: A dome of gathered light that sends the next bolt or orb back at its caster.
    Spell {
        name: "lightshield",
        kind: Kind::Barrier,
        fx: Some("arcane/bagua-mirror"),
        cost: 20.0,
        windup: 0.06,
        recover: 0.1,
        cooldown: 7.0,
        time: Some(2.2),
        reflect: true,
        ..BASE
    },
    // Thunder: Nine Revolutions Immortal Refining Art: Spirals power inside the body, then releases it in one huge blast at close range. Long to gather.
    Spell {
        name: "nineturn",
        kind: Kind::Blast,
        blow: Some(Physical),
        fx: Some("arcane/vortex-ring"),
        cost: 40.0,
        windup: 0.85,
        recover: 0.5,
        cooldown: 6.5,
        range: Some(184.0),
        damage: Some(34.0),
        ..BASE
    },
    // Thunder: Ancient Clan Indestructible Body: Recovery outpaces destruction: heals and washes off burn, silence, defence down, chill and shock.
    Spell {
        name: "indestructible",
        kind: Kind::Heal,
        fx: Some("feedback/power-up"),
        cost: 40.0,
        windup: 0.7,
        recover: 0.4,
        cooldown: 11.0,
        amount: Some(26.0),
        ..BASE
    },
    // Thunder: Ancient Protection / Ancestral Soul Protection: A golden bell of ancestral souls: 1 to 3 charges, each stops a whole blow. Cleanses.
    Spell {
        name: "ancientprotect",
        kind: Kind::Ward,
        fx: Some("holy/golden-bell"),
        feet: true,
        cost: 42.0,
        windup: 0.5,
        recover: 0.25,
        cooldown: 9.0,
        ..BASE
    },
    // Thunder: Vermilion Bird Nine Mysteries Transformation: A burning bird of fire sweeps out of the palm at close range and sets the foe burning.
    Spell {
        name: "vermilion",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("explosions/swirl-blast"),
        cost: 30.0,
        windup: 0.35,
        recover: 0.4,
        cooldown: 3.0,
        range: Some(208.0),
        damage: Some(18.0),
        burn: true,
        ..BASE
    },
    // Frost: Summon Rain: Rain falls on the foe for two seconds, chilling whoever stands in it.
    Spell {
        name: "summonrain",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("water/splash"),
        cost: 30.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 5.0,
        damage: Some(6.0),
        life: Some(2.0),
        height: Some(0.0),
        pull: Some(0.0),
        reach: Some(120.0),
        under: Some(120.0),
        tick: Some(3.0),
        chill: true,
        ..BASE
    },
    // Frost: Summon Wind: A black wind-dragon exhales yin wind at the foe at middle reach: chills.
    Spell {
        name: "summonwind",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("explosions/swirl-blast"),
        cost: 26.0,
        windup: 0.3,
        recover: 0.4,
        cooldown: 3.0,
        range: Some(232.0),
        damage: Some(12.0),
        chill: true,
        ..BASE
    },
    // Frost: Beans to Soldiers: Souls of the slain rise as soldiers round the foe, dragging it in and cutting it while it stands there.
    Spell {
        name: "beans",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("critters/wisp"),
        cost: 36.0,
        windup: 0.5,
        recover: 0.3,
        cooldown: 7.0,
        damage: Some(8.0),
        life: Some(3.2),
        height: Some(0.0),
        pull: Some(48.0),
        reach: Some(176.0),
        under: Some(96.0),
        tick: Some(6.0),
        ..BASE
    },
    // Frost: Yin Moon Has Clear Sky / Dark Moon: Marks the foe with a moon sign, then cracks break open: breaks its defence.
    Spell {
        name: "yinmoon",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/comma-seal"),
        cost: 32.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.4,
        damage: Some(20.0),
        impact: Some(0.45),
        radius: Some(72.0),
        notice: Some(0.15),
        def_down: Some(5.0),
        ..BASE
    },
    // Frost: Soul Searching Technique: Pulls at the foe's soul at close range: silences it and steals a point of rage.
    Spell {
        name: "soulsearch",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("arcane/vortex-ring"),
        cost: 28.0,
        windup: 0.4,
        recover: 0.4,
        cooldown: 4.0,
        range: Some(176.0),
        damage: Some(8.0),
        silence: Some(1.2),
        steal_rage: Some(1.0),
        ..BASE
    },
    // Frost: Spirit Transformation Technique: A dart that wipes the foe's memory of its spells: silences.
    Spell {
        name: "spiritchange",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 20.0,
        windup: 0.28,
        recover: 0.25,
        cooldown: 3.0,
        damage: Some(6.0),
        speed: Some(1040.0),
        silence: Some(1.4),
        ..BASE
    },
    // Frost: Body Fixation Art / Immobilization Technique / Stop Spell: A seal under the foe that holds its body in place: a long freeze, little damage.
    Spell {
        name: "bodyfix",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 36.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 5.5,
        damage: Some(10.0),
        sigil: Some(0.35),
        impact: Some(0.08),
        radius: Some(56.0),
        notice: Some(0.0),
        freeze: Some(0.8),
        ..BASE
    },
    // Frost: Destruction Restriction: Rings of runes write themselves round the foe, locking blinking until they close, then crush. Only hits on the ground.
    Spell {
        name: "restriction",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/restriction"),
        cost: 40.0,
        windup: 0.5,
        recover: 0.35,
        cooldown: 5.0,
        damage: Some(26.0),
        impact: Some(0.9),
        radius: Some(88.0),
        notice: Some(0.1),
        reach: Some(200.0),
        grounded: true,
        seals: true,
        ..BASE
    },
    // Frost: Eighteen Plum Blossom Restrictions: A self-running restriction of rings round the foe that breaks its defence when it closes.
    Spell {
        name: "plumblossom",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/comet-rings"),
        cost: 34.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.6,
        damage: Some(24.0),
        impact: Some(0.5),
        radius: Some(80.0),
        notice: Some(0.15),
        def_down: Some(3.0),
        ..BASE
    },
    // Frost: Life and Death Seal: Left hand life, right hand death: a seal that closes on the foe and freezes it briefly.
    Spell {
        name: "lifedeath",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/twin-shuriken"),
        cost: 36.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.6,
        damage: Some(28.0),
        impact: Some(0.5),
        radius: Some(64.0),
        notice: Some(0.15),
        freeze: Some(0.5),
        ..BASE
    },
    // Frost: Cause and Effect Seal: A dome that answers the next close blow with a stunning counter.
    Spell {
        name: "karma",
        kind: Kind::Barrier,
        fx: Some("holy/lotus"),
        cost: 22.0,
        windup: 0.06,
        recover: 0.1,
        cooldown: 7.0,
        time: Some(2.0),
        counter: Some(Counter {
            damage: 12.0,
            stun: 0.8,
        }),
        ..BASE
    },
    // Frost: Dream Return to Ancient Times: Runes close round the caster: 1 to 3 charges, each stops a whole blow. Cleanses.
    Spell {
        name: "dreamback",
        kind: Kind::Ward,
        fx: Some("arcane/restriction"),
        feet: true,
        cost: 44.0,
        windup: 0.5,
        recover: 0.25,
        cooldown: 8.5,
        ..BASE
    },
    // Frost: Dream Dao: Drags the foe into a dream at middle reach: silences it.
    Spell {
        name: "dreamdao",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("explosions/puff-burst"),
        cost: 26.0,
        windup: 0.4,
        recover: 0.4,
        cooldown: 3.6,
        range: Some(208.0),
        damage: Some(10.0),
        silence: Some(1.6),
        ..BASE
    },
    // Frost: Flowing Moon: Turns time back on the caster: heals and washes off burn, silence, defence down, chill and shock.
    Spell {
        name: "flowingmoon",
        kind: Kind::Heal,
        fx: Some("holy/feather-burst"),
        cost: 38.0,
        windup: 0.8,
        recover: 0.4,
        cooldown: 10.0,
        amount: Some(24.0),
        ..BASE
    },
    // Frost: Remnant Night / Sundered Night: The night sky cracks over the foe and falls: a heavy blow that silences. Slow to land; only hits on the ground.
    Spell {
        name: "remnantnight",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("shadow/winter-ordeal"),
        cost: 46.0,
        windup: 0.5,
        recover: 0.4,
        cooldown: 6.0,
        damage: Some(30.0),
        impact: Some(1.78),
        radius: Some(96.0),
        notice: Some(0.12),
        silence: Some(1.0),
        grounded: true,
        ..BASE
    },
    // Frost: Heaven Flipping Seal: A giant seal overturns heaven onto the foe: a huge blow that breaks its defence.
    Spell {
        name: "heavenflip",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/shuriken-seal"),
        cost: 50.0,
        windup: 0.6,
        recover: 0.4,
        cooldown: 7.5,
        damage: Some(32.0),
        impact: Some(0.5),
        radius: Some(120.0),
        notice: Some(0.15),
        def_down: Some(4.0),
        ..BASE
    },
    // Frost: Third Eye: A beam from the third eye that breaks the defence of whatever it hits.
    Spell {
        name: "thirdeye",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 20.0,
        windup: 0.3,
        recover: 0.25,
        cooldown: 2.4,
        damage: Some(12.0),
        def_down: Some(3.0),
        speed: Some(1360.0),
        ..BASE
    },
    // Frost: Seven-Colored Dao Spear: A spear of seven-coloured light that carries the caster through the foe.
    Spell {
        name: "sevencolor",
        kind: Kind::Dash,
        blow: Some(Magic),
        fx: Some("arcane/sword-strike"),
        cost: 30.0,
        windup: 0.28,
        recover: 0.45,
        cooldown: 3.6,
        distance: Some(336.0),
        damage: Some(17.0),
        ..BASE
    },
    // Frost: World-Burning Ancient Umbrella: An ancient umbrella sweeps fire over the foe: burns.
    Spell {
        name: "umbrella",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/swirl-blast"),
        cost: 34.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 3.8,
        damage: Some(22.0),
        sigil: Some(0.3),
        impact: Some(0.45),
        radius: Some(88.0),
        notice: Some(0.0),
        burn: true,
        ..BASE
    },
    // Frost: Spirit-Devouring Body-Consuming Heaven-Severing Great Art / Devouring: Devours the foe at close range: steals a point of rage.
    Spell {
        name: "devour",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("impacts/star-converge"),
        cost: 26.0,
        windup: 0.35,
        recover: 0.4,
        cooldown: 3.2,
        range: Some(160.0),
        damage: Some(16.0),
        steal_rage: Some(1.0),
        ..BASE
    },
    // Frost: Falling Star: Two stars fall one after another where the foe will be.
    Spell {
        name: "starfall",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 40.0,
        windup: 0.4,
        recover: 0.35,
        cooldown: 5.0,
        damage: Some(15.0),
        sigil: Some(0.3),
        impact: Some(0.08),
        radius: Some(48.0),
        notice: Some(0.0),
        strikes: Some(2.0),
        every: Some(0.5),
        aim_ahead: Some(0.3),
        ..BASE
    },
    // Frost: Foundation Seizing Great Art: A dart that takes a point of the foe's rage.
    Spell {
        name: "seize",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 18.0,
        windup: 0.25,
        recover: 0.25,
        cooldown: 3.2,
        damage: Some(6.0),
        speed: Some(1120.0),
        steal_rage: Some(1.0),
        ..BASE
    },
    // Frost: Yellow Spring Finger / Underworld Finger: A finger of the underworld's cold at middle reach: chills, and hits harder per chill stack on the foe.
    Spell {
        name: "yellowspring",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("impacts/twin-converge"),
        cost: 24.0,
        windup: 0.3,
        recover: 0.4,
        cooldown: 2.4,
        range: Some(200.0),
        damage: Some(14.0),
        chill: true,
        ..BASE
    },
    // Thunder: Ancient Thunder Realm / Thunder Origin Technique: An ancient thunder dragon falls twice on the foe, one strike after another, and shocks it.
    Spell {
        name: "ancientthunder",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 38.0,
        windup: 0.4,
        recover: 0.35,
        cooldown: 5.0,
        damage: Some(16.0),
        sigil: Some(0.3),
        impact: Some(0.08),
        radius: Some(56.0),
        notice: Some(0.0),
        strikes: Some(2.0),
        every: Some(0.45),
        aim_ahead: Some(0.25),
        shock: true,
        ..BASE
    },
    // Thunder: Demon Transformation Finger: A finger of demonic qi that silences what it hits.
    Spell {
        name: "demonfinger",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 22.0,
        windup: 0.3,
        recover: 0.25,
        cooldown: 2.8,
        damage: Some(12.0),
        speed: Some(1200.0),
        silence: Some(1.0),
        ..BASE
    },
    // Thunder: Kunji Whip / Cause and Effect Whip: A whip that strikes the soul directly at middle reach: silences, and shocks.
    Spell {
        name: "kunjiwhip",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("explosions/swirl-blast"),
        cost: 26.0,
        windup: 0.32,
        recover: 0.4,
        cooldown: 3.0,
        range: Some(224.0),
        damage: Some(13.0),
        silence: Some(0.8),
        shock: true,
        ..BASE
    },
    // Thunder: Heavenly Tiger Flag: A heavenly tiger leaps from the flag in a straight line and mauls what it meets.
    Spell {
        name: "tigerflag",
        kind: Kind::Bolt,
        blow: Some(Physical),
        fx: Some("projectiles/laser-bolt"),
        cost: 26.0,
        windup: 0.34,
        recover: 0.3,
        cooldown: 3.0,
        damage: Some(17.0),
        speed: Some(1200.0),
        ..BASE
    },
    // Frost: Time Restriction: A restriction of time round the foe: locks blinking while it runs, then closes and holds the foe. Only hits on the ground.
    Spell {
        name: "timerestrict",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/restriction"),
        cost: 38.0,
        windup: 0.45,
        recover: 0.35,
        cooldown: 5.5,
        damage: Some(20.0),
        impact: Some(1.2),
        radius: Some(80.0),
        notice: Some(0.1),
        reach: Some(240.0),
        freeze: Some(0.4),
        grounded: true,
        seals: true,
        ..BASE
    },
    // Frost: Life and Death Restriction: Turns the vitality of whoever stands in it into death energy: a cloud that burns for three seconds.
    Spell {
        name: "lifedeathrestrict",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("nature/poison-cloud"),
        cost: 40.0,
        windup: 0.5,
        recover: 0.3,
        cooldown: 7.5,
        damage: Some(7.0),
        life: Some(3.0),
        height: Some(0.0),
        pull: Some(0.0),
        reach: Some(136.0),
        under: Some(136.0),
        tick: Some(5.0),
        burn: true,
        ..BASE
    },
    // Frost: Billion Soul Banner / Soul Banner method: Spirits swirl in a vortex over the foe, dragging it in and cutting it while it stands there.
    Spell {
        name: "soulbanner",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("critters/wisp"),
        cost: 44.0,
        windup: 0.55,
        recover: 0.3,
        cooldown: 8.0,
        damage: Some(8.0),
        life: Some(3.0),
        height: Some(0.0),
        pull: Some(96.0),
        reach: Some(320.0),
        under: Some(88.0),
        tick: Some(5.0),
        ..BASE
    },
    // Frost: Spirit Servitude Seal: A seal under the foe that drags it in, then closes on it: silences.
    Spell {
        name: "servitude",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/comma-seal"),
        cost: 34.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 4.5,
        damage: Some(18.0),
        impact: Some(0.7),
        radius: Some(72.0),
        notice: Some(0.1),
        pull: Some(160.0),
        reach: Some(240.0),
        silence: Some(1.0),
        pull_until: Some(0.6),
        ..BASE
    },
    // Frost: Seven-Colored Divine Void Nail: A nail of void driven at the foe at great speed: breaks its defence.
    Spell {
        name: "voidnail",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 34.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 4.5,
        damage: Some(20.0),
        def_down: Some(4.0),
        speed: Some(1680.0),
        ..BASE
    },
    // Thunder: Finger of Death / 100 Fingers of Death: Three fingers of death fall one after another where the foe will be.
    Spell {
        name: "fingerdeath",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 34.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 4.0,
        damage: Some(9.0),
        sigil: Some(0.25),
        impact: Some(0.08),
        radius: Some(44.0),
        notice: Some(0.0),
        strikes: Some(3.0),
        every: Some(0.25),
        aim_ahead: Some(0.2),
        ..BASE
    },
    // Thunder: Heavenly Chop (sword divine ability): A sword cut that splits heaven at close range: a heavy blow.
    Spell {
        name: "heavenlychop",
        kind: Kind::Blast,
        blow: Some(Physical),
        fx: Some("arcane/sword-slash"),
        cost: 34.0,
        windup: 0.45,
        recover: 0.5,
        cooldown: 4.5,
        range: Some(224.0),
        damage: Some(26.0),
        ..BASE
    },
    // Thunder: Nine Mysterious Spell, Fire Escape: Turns to flame and dashes through the foe, setting it burning.
    Spell {
        name: "fireescape",
        kind: Kind::Dash,
        blow: Some(Magic),
        fx: Some("spawn/poof"),
        cost: 28.0,
        windup: 0.22,
        recover: 0.4,
        cooldown: 3.6,
        distance: Some(320.0),
        damage: Some(10.0),
        burn: true,
        ..BASE
    },
    // Thunder: Rise Three Realm Flame Origin: The origin of flame rises under the foe and burns it.
    Spell {
        name: "flameorigin",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/swirl-blast"),
        cost: 34.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 3.8,
        damage: Some(19.0),
        sigil: Some(0.35),
        impact: Some(0.4),
        radius: Some(80.0),
        notice: Some(0.0),
        burn: true,
        ..BASE
    },
    // Thunder: War Spirit Print (Cloud Sea divine ability): A seal of war stamped on the foe at close range: a heavy blow that breaks its defence.
    Spell {
        name: "warprint",
        kind: Kind::Blast,
        blow: Some(Physical),
        fx: Some("explosions/swirl-blast"),
        cost: 32.0,
        windup: 0.4,
        recover: 0.5,
        cooldown: 4.0,
        range: Some(184.0),
        damage: Some(22.0),
        def_down: Some(3.0),
        ..BASE
    },
    // Thunder: Earth Escape Technique: Sinks into the ground and comes out a few steps away.
    Spell {
        name: "earthescape",
        kind: Kind::Blink,
        fx: Some("explosions/puff-burst"),
        cost: 14.0,
        windup: 0.06,
        recover: 0.1,
        cooldown: 1.4,
        distance: Some(160.0),
        ..BASE
    },
    // Thunder: Celestial Slaughter Art (All Seer spell): A bolt of slaughter intent, slow to gather and heavy.
    Spell {
        name: "celestialslaughter",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 36.0,
        windup: 0.5,
        recover: 0.35,
        cooldown: 4.5,
        damage: Some(26.0),
        speed: Some(1440.0),
        ..BASE
    },
    // Frost: Ghost Summoning Technique: Ghosts rise round the foe and drag it in while they cut it.
    Spell {
        name: "ghostsummon",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("critters/wisp"),
        cost: 34.0,
        windup: 0.45,
        recover: 0.3,
        cooldown: 6.0,
        damage: Some(7.0),
        life: Some(2.6),
        height: Some(0.0),
        pull: Some(32.0),
        reach: Some(200.0),
        under: Some(112.0),
        tick: Some(5.0),
        ..BASE
    },
    // Frost: Soul Vortex: A vortex of souls pulls the foe in, then bursts.
    Spell {
        name: "soulvortex",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("arcane/vortex-ring"),
        cost: 30.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.6,
        damage: Some(16.0),
        impact: Some(0.5),
        radius: Some(64.0),
        notice: Some(0.1),
        pull: Some(180.0),
        reach: Some(224.0),
        pull_until: Some(0.45),
        ..BASE
    },
    // Frost: Soul Refining Technique: Refines a soul into life: heals and washes off burn, silence, defence down, chill and shock.
    Spell {
        name: "soulrefine",
        kind: Kind::Heal,
        fx: Some("holy/feather-burst"),
        cost: 28.0,
        windup: 0.7,
        recover: 0.4,
        cooldown: 8.0,
        amount: Some(16.0),
        ..BASE
    },
    // Frost: Star Rotation: Three stars turn down on the foe one after another where it will be.
    Spell {
        name: "starrotation",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 38.0,
        windup: 0.4,
        recover: 0.35,
        cooldown: 4.5,
        damage: Some(11.0),
        sigil: Some(0.25),
        impact: Some(0.08),
        radius: Some(44.0),
        notice: Some(0.0),
        strikes: Some(3.0),
        every: Some(0.35),
        aim_ahead: Some(0.25),
        ..BASE
    },
    // Frost: Merit Spirit (Ancient God spell): A spirit of merit stands over the caster: 1 to 3 charges, each stops a whole blow. Cleanses.
    Spell {
        name: "meritspirit",
        kind: Kind::Ward,
        fx: Some("holy/lotus"),
        feet: true,
        cost: 42.0,
        windup: 0.5,
        recover: 0.25,
        cooldown: 8.5,
        ..BASE
    },
    // Frost: Body Formation (Celestial Emperor Qing Lin spell): Forms the body anew: heals and washes off burn, silence, defence down, chill and shock.
    Spell {
        name: "bodyformation",
        kind: Kind::Heal,
        fx: Some("feedback/power-up"),
        cost: 34.0,
        windup: 0.75,
        recover: 0.4,
        cooldown: 9.5,
        amount: Some(20.0),
        ..BASE
    },
    // Frost: True and False Domain / True and False Seal: Open eyes are truth, closed eyes falsehood: a seal on the foe that silences it and breaks its defence.
    Spell {
        name: "truefalse",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("impacts/twin-converge"),
        cost: 36.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 4.0,
        damage: Some(20.0),
        impact: Some(0.5),
        radius: Some(64.0),
        notice: Some(0.1),
        def_down: Some(2.0),
        silence: Some(0.8),
        ..BASE
    },
    // Thunder: Ancient God Cauldron: Swaps places with the foe, wherever it stands.
    Spell {
        name: "ancientcauldron",
        kind: Kind::Blink,
        fx: Some("arcane/vortex-ring"),
        swap: true,
        cost: 30.0,
        windup: 0.2,
        recover: 0.3,
        cooldown: 4.5,
        distance: Some(160.0),
        ..BASE
    },
    // Thunder: Eight Extremes Dao: Eight Dao paths fall on the foe one after another, four strikes that burn.
    Spell {
        name: "eightextremes",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("explosions/plasma-explosion"),
        cost: 40.0,
        windup: 0.45,
        recover: 0.35,
        cooldown: 5.5,
        damage: Some(11.0),
        sigil: Some(0.2),
        impact: Some(0.08),
        radius: Some(44.0),
        notice: Some(0.0),
        strikes: Some(4.0),
        every: Some(0.3),
        aim_ahead: Some(0.2),
        burn: true,
        ..BASE
    },
    // Thunder: Dao Art Fusion: Treasures, abilities and rules fused into one blow: slow to gather, heavy, and it shocks.
    Spell {
        name: "daofusion",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("explosions/swirl-blast"),
        cost: 46.0,
        windup: 0.8,
        recover: 0.5,
        cooldown: 7.0,
        range: Some(216.0),
        damage: Some(36.0),
        shock: true,
        ..BASE
    },
    // Frost: Dao Transformation Yellow Spring: A reincarnation cycle forms over the foe: a cold field that chills whoever stands in it for three seconds.
    Spell {
        name: "daoyellow",
        kind: Kind::Void,
        blow: Some(Magic),
        fx: Some("critters/wisp"),
        cost: 42.0,
        windup: 0.5,
        recover: 0.3,
        cooldown: 7.5,
        damage: Some(6.0),
        life: Some(3.0),
        height: Some(0.0),
        pull: Some(0.0),
        reach: Some(136.0),
        under: Some(136.0),
        tick: Some(5.0),
        chill: true,
        ..BASE
    },
    // Frost: Condensed Spring Soul: A hundred million souls condensed into one dart that chills what it hits.
    Spell {
        name: "springsoul",
        kind: Kind::Bolt,
        blow: Some(Magic),
        fx: Some("projectiles/laser-bolt"),
        cost: 26.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 3.0,
        damage: Some(14.0),
        speed: Some(680.0),
        chill: true,
        ..BASE
    },
    // Frost: Star Soul Extraction: Pulls the soul out of the foe at close range: steals a point of rage.
    Spell {
        name: "starsoul",
        kind: Kind::Blast,
        blow: Some(Magic),
        fx: Some("arcane/vortex-ring"),
        cost: 30.0,
        windup: 0.45,
        recover: 0.4,
        cooldown: 4.5,
        range: Some(184.0),
        damage: Some(12.0),
        steal_rage: Some(1.0),
        ..BASE
    },
    // Frost: Life Destruction Spirit Summoning: Spends 20 of the caster's life to call spirits down in two waves that burn the foe, each stronger for the life paid.
    Spell {
        name: "spiritsummon",
        kind: Kind::Zone,
        blow: Some(Magic),
        fx: Some("impacts/twin-converge"),
        cost: 36.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 4.5,
        damage: Some(14.0),
        impact: Some(0.3),
        radius: Some(48.0),
        notice: Some(0.0),
        strikes: Some(2.0),
        every: Some(0.4),
        burn: true,
        blood: Some(20.0),
        ..BASE
    },
    // Frost: Star Compass: The compass turns and the caster and the foe trade places, quicker and cheaper than the cauldron.
    Spell {
        name: "starcompass",
        kind: Kind::Blink,
        fx: Some("explosions/puff-burst"),
        swap: true,
        cost: 18.0,
        windup: 0.1,
        recover: 0.2,
        cooldown: 2.5,
        distance: Some(160.0),
        ..BASE
    },
];

/// Every spell, in the duel's order.
pub fn spells() -> &'static [Spell] {
    &SPELLS
}

/// Whether `b` is false: a spell's flag JS leaves out when it is off.
#[cfg(feature = "serde")]
fn is_false(b: &bool) -> bool {
    !*b
}

/// The spell called `name`.
pub fn spell(name: &str) -> Option<&'static Spell> {
    SPELLS.iter().find(|s| s.name == name)
}

/// How a school tints an effect its spells play.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    pub palette: Option<&'static [&'static str]>,
    pub accent: Option<&'static [&'static str]>,
    pub stars: Option<&'static [&'static str]>,
    pub fire: Option<&'static [&'static str]>,
    pub glow: Option<&'static str>,
    pub width: Option<f64>,
    pub height: Option<f64>,
}

const TINT: Tint = Tint {
    palette: None,
    accent: None,
    stars: None,
    fire: None,
    glow: None,
    width: None,
    height: None,
};

/// A school: its spells (`basics`, the `pool` its bar draws from, its `ultimate`), its temperament for the brain, and
/// its colours.
pub struct SchoolData {
    pub name: &'static str,
    pub basics: [&'static str; 3],
    pub pool: &'static [&'static str],
    pub ultimate: &'static str,
    pub signature: Kind,
    pub postures: [f64; 8],
    pub range: f64,
    pub dry: f64,
    pub aggression: f64,
    pub tempo: f64,
    pub sky: f64,
    pub bolt: [&'static str; 8],
    pub circle: [&'static str; 7],
    pub tint: &'static [(&'static str, Tint)],
}

const THUNDER_CIRCLE: [&str; 7] = [
    "#2a1a08", "#6a4410", "#b07a1a", "#e8b030", "#ffd84a", "#fff6c4", "#ffffff",
];
const FROST_CIRCLE: [&str; 7] = [
    "#151a3d", "#212f6b", "#2c58aa", "#3a8bd8", "#62c8f4", "#dcfbff", "#ffffff",
];

static THUNDER: SchoolData = SchoolData {
    name: "Thunder",
    basics: ["slash", "dive", "blink"],
    pool: &[
        "bolt",
        "palm",
        "step",
        "seal",
        "orb",
        "bell",
        "heal",
        "spark",
        "burst",
        "quake",
        "hex",
        // Renegade Immortal (docs/specs/2026-11-16-xianni-spells-design.md)
        "fireball",
        "gravity",
        "earthsplit",
        "finger",
        "jithunder",
        "gather",
        "ancientgod",
        "slashluo",
        "arsenal",
        "inchearth",
        "onestep",
        "mountain",
        "heartbeat",
        "lightshield",
        "nineturn",
        "indestructible",
        "ancientprotect",
        "vermilion",
        "ancientthunder",
        "demonfinger",
        "kunjiwhip",
        "tigerflag",
        "fingerdeath",
        "heavenlychop",
        "fireescape",
        "flameorigin",
        "warprint",
        "earthescape",
        "celestialslaughter",
        "ancientcauldron",
        "eightextremes",
        "daofusion",
    ],
    ultimate: "tribulation",
    signature: Kind::Blast,
    postures: [1.0; 8],
    range: 64.0,
    // slash's range less 4: Thunder presses in to sword's length while its qi gathers
    dry: 20.0,
    aggression: 1.15,
    tempo: 0.8,
    sky: 22.0,
    bolt: [
        "#2a1a08", "#6a4410", "#b07a1a", "#e8b030", "#ffd84a", "#ffe98a", "#fff6c4", "#ffffff",
    ],
    circle: THUNDER_CIRCLE,
    tint: &[
        // the Renegade Immortal spells' effects take the school's colours
        (
            "arcane/sword-formation",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/sword-slash",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/sword-strike",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/vortex-ring",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "explosions/small-explosion",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "explosions/swirl-blast",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "spawn/swirl-portal",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "explosions/plasma-explosion",
            Tint {
                palette: Some(&[
                    "#1a0e04", "#4a2a08", "#9a5a10", "#e0a020", "#ffd84a", "#fff3b0", "#ffffff",
                ]),
                accent: Some(&["#2a1408", "#7a3a10", "#e07a1a", "#ffc23d", "#fff0b0"]),
                ..TINT
            },
        ),
        (
            "spawn/poof",
            Tint {
                palette: Some(&[
                    "#3a2a10", "#7a5a20", "#c09040", "#ffd84a", "#fff3b0", "#ffffff",
                ]),
                stars: Some(&["#ffd84a", "#fff3b0", "#ffffff"]),
                ..TINT
            },
        ),
        (
            "holy/feather-burst",
            Tint {
                palette: Some(&[
                    "#6a4410", "#b07a1a", "#e8b030", "#ffd84a", "#fff6c4", "#ffffff",
                ]),
                ..TINT
            },
        ),
        (
            "explosions/dirt-explosion",
            Tint {
                fire: Some(&[
                    "#4a2a08", "#9a5a10", "#e0a020", "#ffc23d", "#ffd84a", "#ffe98a", "#fff6c4",
                    "#ffffff",
                ]),
                ..TINT
            },
        ),
        // a school's barrier rings in its circle's colours, and so does a blow it turns aside
        (
            "arcane/rune-circle",
            Tint {
                palette: Some(&THUNDER_CIRCLE),
                ..TINT
            },
        ),
        (
            "impacts/parry",
            Tint {
                palette: Some(&["#b07a1a", "#e8b030", "#ffd84a", "#fff6c4", "#ffffff"]),
                ..TINT
            },
        ),
    ],
};

static FROST: SchoolData = SchoolData {
    name: "Frost",
    basics: ["slash", "dive", "blink"],
    pool: &[
        "lotus",
        "shatter",
        "icicle",
        "collapse",
        "frostfire",
        "void",
        "mirror",
        "ward",
        "shard",
        "jet",
        "flurry",
        // Renegade Immortal (docs/specs/2026-11-16-xianni-spells-design.md)
        "summonrain",
        "summonwind",
        "beans",
        "yinmoon",
        "soulsearch",
        "spiritchange",
        "bodyfix",
        "restriction",
        "plumblossom",
        "lifedeath",
        "karma",
        "dreamback",
        "dreamdao",
        "flowingmoon",
        "remnantnight",
        "heavenflip",
        "thirdeye",
        "sevencolor",
        "umbrella",
        "devour",
        "starfall",
        "seize",
        "yellowspring",
        "timerestrict",
        "lifedeathrestrict",
        "soulbanner",
        "servitude",
        "voidnail",
        "ghostsummon",
        "soulvortex",
        "soulrefine",
        "starrotation",
        "meritspirit",
        "bodyformation",
        "truefalse",
        "daoyellow",
        "springsoul",
        "starsoul",
        "spiritsummon",
        "starcompass",
    ],
    ultimate: "prison",
    signature: Kind::Zone,
    postures: [1.0; 8],
    range: 100.0,
    dry: 80.0,
    aggression: 1.1,
    tempo: 2.0,
    sky: 48.0,
    bolt: [
        "#151a3d", "#212f6b", "#2c58aa", "#3a8bd8", "#62c8f4", "#a6ecff", "#dcfbff", "#ffffff",
    ],
    circle: FROST_CIRCLE,
    tint: &[
        // the Renegade Immortal spells' effects take the school's colours
        (
            "arcane/comet-rings",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/comma-seal",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/restriction",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/shuriken-seal",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/sword-strike",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/twin-shuriken",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "arcane/vortex-ring",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "critters/wisp",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "explosions/puff-burst",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "explosions/swirl-blast",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "impacts/star-converge",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "impacts/twin-converge",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "water/splash",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "explosions/plasma-explosion",
            Tint {
                palette: Some(&[
                    "#0a1430", "#16306a", "#2c58aa", "#4a9ae0", "#8fd8ff", "#dcfbff", "#ffffff",
                ]),
                accent: Some(&["#0a1830", "#1a4a80", "#3a8bd8", "#a6ecff", "#ffffff"]),
                ..TINT
            },
        ),
        (
            "explosions/implosion",
            Tint {
                palette: Some(&[
                    "#060a18", "#10224a", "#1e4a8a", "#3a8bd8", "#8fd8ff", "#dcfbff", "#ffffff",
                ]),
                ..TINT
            },
        ),
        (
            "holy/feather-burst",
            Tint {
                palette: Some(&[
                    "#1e3f80", "#2c58aa", "#62c8f4", "#a6ecff", "#dcfbff", "#ffffff",
                ]),
                glow: Some("#a6ecff"),
                ..TINT
            },
        ),
        (
            "fire/spirit-flame",
            Tint {
                palette: Some(&[
                    "#0a1430", "#16306a", "#2c58aa", "#3a8bd8", "#62c8f4", "#a6ecff", "#dcfbff",
                    "#ffffff",
                ]),
                ..TINT
            },
        ),
        (
            "spawn/poof",
            Tint {
                palette: Some(&[
                    "#16306a", "#2c58aa", "#62c8f4", "#a6ecff", "#dcfbff", "#ffffff",
                ]),
                stars: Some(&["#a6ecff", "#dcfbff", "#ffffff"]),
                ..TINT
            },
        ),
        // a flurry falls on a patch round the foe, not the whole sky
        (
            "weather/snow",
            Tint {
                width: Some(192.0),
                height: Some(288.0),
                ..TINT
            },
        ),
        // a school's barrier rings in its circle's colours, and so does a blow it turns aside
        (
            "arcane/rune-circle",
            Tint {
                palette: Some(&FROST_CIRCLE),
                ..TINT
            },
        ),
        (
            "impacts/parry",
            Tint {
                palette: Some(&["#2c58aa", "#3a8bd8", "#62c8f4", "#dcfbff", "#ffffff"]),
                ..TINT
            },
        ),
    ],
};

pub fn school(s: School) -> &'static SchoolData {
    match s {
        School::Thunder => &THUNDER,
        School::Frost => &FROST,
    }
}

/// How school `s` tints effect `fx`, if it does.
pub fn tint(s: School, fx: &str) -> Option<&'static Tint> {
    school(s)
        .tint
        .iter()
        .find(|(name, _)| *name == fx)
        .map(|(_, t)| t)
}

/// ball-lightning's ball relative to its origin at time t (pixelfx lightning/ball-lightning.js `at`, P = 4).
pub fn orb_offset(t: f64) -> (f64, f64) {
    let a = (t / 4.0) * core::f64::consts::PI * 2.0;
    (
        152.0 * libm::sin(a) + 36.0 * libm::sin(2.0 * a + 1.0),
        -168.0 + 52.0 * libm::sin(2.0 * a + 0.4) + 24.0 * libm::sin(3.0 * a + 0.5),
    )
}

/// `base` to the whole power `n` (n ≥ 0), by multiplying.
pub fn powi(base: f64, n: f64) -> f64 {
    let mut out = 1.0;
    let mut i = 0.0;
    while i < n {
        out *= base;
        i += 1.0;
    }
    out
}

/// Seconds between shock ticks with `stacks` stacks.
pub fn shock_interval(stacks: f64) -> f64 {
    SHOCK.every * powi(SHOCK.fast, stacks - 1.0)
}

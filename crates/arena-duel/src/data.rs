//! The duel's tables and numbers: the motions skills borrow (how they move and look), the skills of the shared pool
//! as the duel casts them, the looks, and the constants every rule reads. `tests/data.rs` pins each against
//! `tests/fixtures/js_numbers.json`. How much a skill deals, what it lays and what it costs come from `arena-calc`
//! (`docs/specs/2026-11-17-damage-system-design.md`); the duel decides how it travels.

use crate::body::BodySpec;
use crate::js::js_min;
use std::sync::OnceLock;

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

/// A ward holds 1 to `charges` charges for `time` seconds; each erases one whole blow.
pub struct Ward {
    pub charges: f64,
    pub time: f64,
}
pub const WARD: Ward = Ward {
    charges: 3.0,
    time: 6.0,
};

/// The kinds of blow a guard can turn aside (a zone or a field falls past it).
pub const GUARDED: [Kind; 5] = [Kind::Melee, Kind::Blast, Kind::Dash, Kind::Bolt, Kind::Orb];

/// How many of its loadout's actives a fighter's bar draws from (`arena_calc::loadout::ACTIVES`).
pub const POOL_SIZE: usize = arena_calc::loadout::ACTIVES;

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

/// The level (hỏa hầu) every skill is cast at: `arena_calc::gem::BASE_LEVEL`, where a spell's base deals what the
/// duel's skills dealt before the damage system.
pub const SKILL_LEVEL: u8 = arena_calc::gem::BASE_LEVEL;

/// One point of a skill's old power in damage: the mean attack times the mean magic defence factor over 10
/// (`arena_calc::gem`'s base). A blow's power for the body (how hard it throws) and for the minds is what it deals
/// over this.
pub const POWER: f64 = arena_calc::gem::TODAY_ATK * arena_calc::gem::TODAY_DEFENCE / 10.0;

/// T, the damage system's unit of length (a fighter's height, design 7), in the arena's pixels: the engine's 60 px
/// times `UNIT`.
pub const T: f64 = 60.0 * UNIT;

/// How a fighter looks: the sprites and colours the game draws it in. The rules never decide anything by it; they
/// only play a fighter's effects in its look's colours. The looks are the characters: seat 0 is Vương Lâm (the
/// cultivator), seat 1 Thất Thải Đạo Nhân (the Seven-Colored Daoist). Their colours keep the old schools' palettes
/// (`look`: Vương Lâm's gold, Thất Thải's blue).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Look {
    #[cfg_attr(feature = "serde", serde(rename = "vuong-lam"))]
    VuongLam,
    #[cfg_attr(feature = "serde", serde(rename = "that-thai"))]
    ThatThai,
}

impl Look {
    /// Its id: `vuong-lam`, `that-thai`.
    pub fn name(self) -> &'static str {
        match self {
            Look::VuongLam => "vuong-lam",
            Look::ThatThai => "that-thai",
        }
    }

    /// The character's name as the game writes it.
    pub fn display(self) -> &'static str {
        match self {
            Look::VuongLam => "Vương Lâm",
            Look::ThatThai => "Thất Thải Đạo Nhân",
        }
    }

    /// The look of seat `i`.
    pub fn of_seat(i: usize) -> Look {
        if i == 0 {
            Look::VuongLam
        } else {
            Look::ThatThai
        }
    }
}

/// How the brain (`arena-mind`) fights, the same for both seats: the two old schools' mean.
pub struct MindProfile {
    pub postures: [f64; 8],
    pub range: f64,
    pub dry: f64,
    pub aggression: f64,
    pub tempo: f64,
    pub sky: f64,
}
pub const MIND: MindProfile = MindProfile {
    postures: [1.0; 8],
    range: 82.0,
    dry: 50.0,
    aggression: 1.125,
    tempo: 1.4,
    sky: 35.0,
};

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

/// The HD fighters' bodies as drawn, in the arena's pixels, measured on the sheets' standing frames
/// (`arena-game/assets/fighters`): 243 pixels from the feet to the top of the hair or crown, the chest's band about
/// 50 wide, 156 to 185 crouched. The body a blow must touch is a little inside the figure: up to the brow, not the hair
/// (`HD_BODY`); blows are felt and shots leave at the shoulders (`HD_CHEST`); a crouch is `HD_DUCK` tall. The width
/// stays the engine's (`half_width` 6, 48 wide), inside the torso.
pub const HD_BODY: f64 = 228.0;
pub const HD_CHEST: f64 = 152.0;
pub const HD_DUCK: f64 = 160.0;

/// A fighter's body: the engine's site duel ([`BodySpec::default`]) in the arena's pixels, its height, chest and crouch
/// those of the HD figures as drawn (`HD_BODY`, `HD_CHEST`, `HD_DUCK`); its times and shares as they are.
pub fn body_spec() -> BodySpec {
    let d = BodySpec::default();
    BodySpec {
        body: HD_BODY,
        chest: HD_CHEST,
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
        duck: HD_DUCK,
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
/// `blocked_recover` seconds longer when their blow is guarded. The feel answers a counter and a punish with its own longer hit-stop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frames {
    pub counter: Bonus,
    pub punish: Bonus,
    pub blocked_recover: f64,
    pub unsafe_on_block: &'static [&'static str],
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
    // dive, dash and the close blasts
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
    pub speed: Option<f64>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub pull_until: Option<f64>,
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
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub dive: bool,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "is_false"))]
    pub grounded: bool,
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
    life: None,
    height: None,
    pull: None,
    reach: None,
    under: None,
    tick: None,
    speed: None,
    pull_until: None,
    strikes: None,
    every: None,
    aim_ahead: None,
    amount: None,
    time: None,
    spray: None,
    dive: false,
    grounded: false,
    reflect: false,
    feet: false,
    swap: false,
    counter: None,
};

/// Every spell, in the duel's order.
/// Every motion a skill may borrow, in the duel's order.
static SPELLS: [Spell; 93] = [
    Spell {
        name: "slash",
        kind: Kind::Melee,
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
        fx: Some("lightning/lightning-dash"),
        cost: 24.0,
        windup: 0.26,
        recover: 0.45,
        cooldown: 3.4,
        distance: Some(288.0),
        damage: Some(13.0),
        ..BASE
    },
    // Thunder: a gold seal opens on the ground under the foe, and a thunderclap bursts from it
    Spell {
        name: "seal",
        kind: Kind::Zone,
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
        ..BASE
    },
    // Frost: a lotus of ice shards blooms from the palm at close range, and cracks the foe's guard
    Spell {
        name: "lotus",
        kind: Kind::Blast,
        fx: Some("ice/frost-nova"),
        cost: 24.0,
        windup: 0.3,
        recover: 0.45,
        cooldown: 2.2,
        range: Some(160.0),
        damage: Some(18.0),
        ..BASE
    },
    // from above: drop onto the foe sword first, and crack the ground if it lands there
    Spell {
        name: "dive",
        kind: Kind::Melee,
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
        fx: Some("projectiles/laser-bolt"),
        cost: 18.0,
        windup: 0.28,
        recover: 0.22,
        cooldown: 1.3,
        speed: Some(920.0),
        damage: Some(13.0),
        ..BASE
    },
    Spell {
        name: "orb",
        kind: Kind::Orb,
        fx: Some("lightning/ball-lightning"),
        hit_fx: Some("explosions/plasma-explosion"),
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
    // implosion sucks in until 0.8 s and pops at 1.02 s
    Spell {
        name: "collapse",
        kind: Kind::Zone,
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
        fx: Some("impacts/shockwave-ring"),
        cost: 27.0,
        windup: 0.32,
        recover: 0.5,
        cooldown: 2.8,
        range: Some(152.0),
        damage: Some(20.0),
        ..BASE
    },
    // the top tier's motion (đỉnh cấp; the gem sets its cost and cooldown). Thunder: three thunderclaps fall one after
    // another where the foe will be.
    Spell {
        name: "tribulation",
        kind: Kind::Zone,
        fx: Some("lightning/chain-lightning"),
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
    // void-orb hangs 62 px over its origin and loops every 3.2 s
    Spell {
        name: "void",
        kind: Kind::Void,
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
        fx: Some("holy/angel-feathers"),
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
        fx: Some("water/bubble-shield"),
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
        fx: Some("scifi/bullet-tracer"),
        cost: 10.0,
        windup: 0.18,
        recover: 0.18,
        cooldown: 0.9,
        speed: Some(1200.0),
        damage: Some(8.0),
        ..BASE
    },
    // Thunder: a palm that bursts with everything the foe's shocks have gathered
    Spell {
        name: "burst",
        kind: Kind::Blast,
        fx: Some("lightning/arc-discharge"),
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
    // Frost: a jet of freezing water from the palm, at middle reach; it sprays for `spray` seconds, then stops
    Spell {
        name: "jet",
        kind: Kind::Blast,
        fx: Some("water/water-jet"),
        spray: Some(0.6),
        cost: 22.0,
        windup: 0.3,
        recover: 0.35,
        cooldown: 2.4,
        range: Some(224.0),
        damage: Some(14.0),
        ..BASE
    },
    // Thunder: Fireball Technique: A ball of fire that flies at the foe and sets it burning.
    Spell {
        name: "fireball",
        kind: Kind::Bolt,
        fx: Some("projectiles/fireball-flight"),
        cost: 14.0,
        windup: 0.26,
        recover: 0.24,
        cooldown: 1.5,
        damage: Some(12.0),
        speed: Some(840.0),
        ..BASE
    },
    // Thunder: Gravity Technique / Attractive Force: Pulls the foe in toward a point, then crushes it there.
    Spell {
        name: "gravity",
        kind: Kind::Zone,
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
        fx: Some("nature/fissure"),
        cost: 28.0,
        windup: 0.32,
        recover: 0.3,
        cooldown: 3.2,
        damage: Some(26.0),
        sigil: Some(0.35),
        impact: Some(0.05),
        radius: Some(72.0),
        notice: Some(0.0),
        grounded: true,
        ..BASE
    },
    // Thunder: Finger of Annihilation: A thin, fast beam from one finger that hits hard.
    Spell {
        name: "finger",
        kind: Kind::Bolt,
        fx: Some("scifi/laser-beam"),
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
        fx: Some("scifi/plasma-bolt-flight"),
        cost: 24.0,
        windup: 0.3,
        recover: 0.25,
        cooldown: 2.0,
        damage: Some(15.0),
        speed: Some(1040.0),
        ..BASE
    },
    // Thunder: Thunder Gathering Seal: Gathers thunder under the foe and releases it in one burst: shocks, and hits harder per shock stack on the foe.
    Spell {
        name: "gather",
        kind: Kind::Zone,
        fx: Some("lightning/static-burst"),
        cost: 36.0,
        windup: 0.4,
        recover: 0.35,
        cooldown: 4.2,
        damage: Some(34.0),
        sigil: Some(0.3),
        impact: Some(0.08),
        radius: Some(80.0),
        notice: Some(0.0),
        ..BASE
    },
    // Thunder: Ancient God Blood: The body swells into an Ancient God phantom for one heavy blow at close range.
    Spell {
        name: "ancientgod",
        kind: Kind::Blast,
        fx: Some("melee/hammer-smash"),
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
        fx: Some("arcane/sword-formation"),
        end_fx: Some("arcane/sword-strike"),
        cost: 48.0,
        windup: 0.6,
        recover: 0.4,
        cooldown: 9.0,
        damage: Some(8.0),
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
        fx: Some("arcane/blink-dash"),
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
        fx: Some("fire/meteor-strike"),
        cost: 44.0,
        windup: 0.6,
        recover: 0.45,
        cooldown: 5.5,
        damage: Some(20.0),
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
        fx: Some("wind/sonic-boom"),
        cost: 20.0,
        windup: 0.24,
        recover: 0.35,
        cooldown: 2.0,
        range: Some(176.0),
        damage: Some(12.0),
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
        fx: Some("explosions/arcane-burst"),
        cost: 40.0,
        windup: 0.85,
        recover: 0.5,
        cooldown: 6.5,
        range: Some(264.0),
        damage: Some(34.0),
        ..BASE
    },
    // Thunder: Ancient Clan Indestructible Body: Recovery outpaces destruction: heals and washes off burn, silence, defence down, chill and shock.
    Spell {
        name: "indestructible",
        kind: Kind::Heal,
        fx: Some("holy/resurrect"),
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
        fx: Some("fire/phoenix-burst"),
        cost: 30.0,
        windup: 0.35,
        recover: 0.4,
        cooldown: 3.0,
        range: Some(208.0),
        damage: Some(10.0),
        ..BASE
    },
    // Frost: Summon Rain: Rain falls on the foe for two seconds, chilling whoever stands in it.
    Spell {
        name: "summonrain",
        kind: Kind::Void,
        fx: Some("ambient/rain"),
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
        ..BASE
    },
    // Frost: Summon Wind: A black wind-dragon exhales yin wind at the foe at middle reach: chills.
    Spell {
        name: "summonwind",
        kind: Kind::Blast,
        fx: Some("wind/wind-gust"),
        cost: 26.0,
        windup: 0.3,
        recover: 0.4,
        cooldown: 3.0,
        range: Some(232.0),
        damage: Some(12.0),
        ..BASE
    },
    // Frost: Beans to Soldiers: Souls of the slain rise as soldiers round the foe, dragging it in and cutting it while it stands there.
    Spell {
        name: "beans",
        kind: Kind::Void,
        fx: Some("ambient/magic-motes"),
        cost: 36.0,
        windup: 0.5,
        recover: 0.3,
        cooldown: 7.0,
        damage: Some(3.0),
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
        fx: Some("arcane/comma-seal"),
        cost: 32.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.4,
        damage: Some(20.0),
        impact: Some(0.45),
        radius: Some(72.0),
        notice: Some(0.15),
        ..BASE
    },
    // Frost: Soul Searching Technique: Pulls at the foe's soul at close range: silences it.
    Spell {
        name: "soulsearch",
        kind: Kind::Blast,
        fx: Some("shadow/shadow-tendrils"),
        cost: 28.0,
        windup: 0.4,
        recover: 0.4,
        cooldown: 4.0,
        range: Some(176.0),
        damage: Some(8.0),
        ..BASE
    },
    // Frost: Spirit Transformation Technique: A dart that wipes the foe's memory of its spells: silences.
    Spell {
        name: "spiritchange",
        kind: Kind::Bolt,
        fx: Some("arcane/arcane-orb-projectile"),
        cost: 20.0,
        windup: 0.28,
        recover: 0.25,
        cooldown: 3.0,
        damage: Some(6.0),
        speed: Some(1040.0),
        ..BASE
    },
    // Frost: Body Fixation Art / Immobilization Technique / Stop Spell: A seal under the foe that holds its body in place: a long freeze, little damage.
    Spell {
        name: "bodyfix",
        kind: Kind::Zone,
        fx: Some("status/petrified"),
        cost: 36.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 5.5,
        damage: Some(10.0),
        sigil: Some(0.35),
        impact: Some(0.08),
        radius: Some(56.0),
        notice: Some(0.0),
        ..BASE
    },
    // Frost: Destruction Restriction: Rings of runes write themselves round the foe, locking blinking until they close, then crush. Only hits on the ground.
    Spell {
        name: "restriction",
        kind: Kind::Zone,
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
        ..BASE
    },
    // Frost: Eighteen Plum Blossom Restrictions: A self-running restriction of rings round the foe that breaks its defence when it closes.
    Spell {
        name: "plumblossom",
        kind: Kind::Zone,
        fx: Some("arcane/comet-rings"),
        cost: 34.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.6,
        damage: Some(24.0),
        impact: Some(0.5),
        radius: Some(80.0),
        notice: Some(0.15),
        ..BASE
    },
    // Frost: Life and Death Seal: Left hand life, right hand death: a seal that closes on the foe and freezes it briefly.
    Spell {
        name: "lifedeath",
        kind: Kind::Zone,
        fx: Some("arcane/twin-shuriken"),
        cost: 36.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 3.6,
        damage: Some(28.0),
        impact: Some(0.5),
        radius: Some(64.0),
        notice: Some(0.15),
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
        fx: Some("arcane/mana-shield"),
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
        fx: Some("arcane/polymorph-poof"),
        cost: 26.0,
        windup: 0.4,
        recover: 0.4,
        cooldown: 3.6,
        range: Some(208.0),
        damage: Some(10.0),
        ..BASE
    },
    // Frost: Flowing Moon: Turns time back on the caster: heals and washes off burn, silence, defence down, chill and shock.
    Spell {
        name: "flowingmoon",
        kind: Kind::Heal,
        fx: Some("holy/purify"),
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
        fx: Some("shadow/winter-ordeal"),
        cost: 46.0,
        windup: 0.5,
        recover: 0.4,
        cooldown: 6.0,
        damage: Some(30.0),
        impact: Some(1.78),
        radius: Some(96.0),
        notice: Some(0.12),
        grounded: true,
        ..BASE
    },
    // Frost: Heaven Flipping Seal: A giant seal overturns heaven onto the foe: a huge blow that breaks its defence.
    Spell {
        name: "heavenflip",
        kind: Kind::Zone,
        fx: Some("arcane/shuriken-seal"),
        cost: 50.0,
        windup: 0.6,
        recover: 0.4,
        cooldown: 7.5,
        damage: Some(32.0),
        impact: Some(0.5),
        radius: Some(120.0),
        notice: Some(0.15),
        ..BASE
    },
    // Frost: Third Eye: A beam from the third eye that breaks the defence of whatever it hits.
    Spell {
        name: "thirdeye",
        kind: Kind::Bolt,
        fx: Some("arcane/arcane-beam"),
        cost: 20.0,
        windup: 0.3,
        recover: 0.25,
        cooldown: 2.4,
        damage: Some(12.0),
        speed: Some(1360.0),
        ..BASE
    },
    // Frost: Seven-Colored Dao Spear: A spear of seven-coloured light that carries the caster through the foe.
    Spell {
        name: "sevencolor",
        kind: Kind::Dash,
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
        fx: Some("fire/flame-ring"),
        cost: 34.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 3.8,
        damage: Some(12.0),
        sigil: Some(0.3),
        impact: Some(0.45),
        radius: Some(88.0),
        notice: Some(0.0),
        ..BASE
    },
    // Frost: Spirit-Devouring Body-Consuming Heaven-Severing Great Art / Devouring: Devours the foe at close range.
    Spell {
        name: "devour",
        kind: Kind::Blast,
        fx: Some("impacts/star-converge"),
        cost: 26.0,
        windup: 0.35,
        recover: 0.4,
        cooldown: 3.2,
        range: Some(160.0),
        damage: Some(16.0),
        ..BASE
    },
    // Frost: Falling Star: Two stars fall one after another where the foe will be.
    Spell {
        name: "starfall",
        kind: Kind::Zone,
        fx: Some("arcane/arcane-rain"),
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
    // Frost: Foundation Seizing Great Art: A dart.
    Spell {
        name: "seize",
        kind: Kind::Bolt,
        fx: Some("projectiles/acid-glob-flight"),
        cost: 18.0,
        windup: 0.25,
        recover: 0.25,
        cooldown: 3.2,
        damage: Some(6.0),
        speed: Some(1120.0),
        ..BASE
    },
    // Frost: Yellow Spring Finger / Underworld Finger: A finger of the underworld's cold at middle reach: chills, and hits harder per chill stack on the foe.
    Spell {
        name: "yellowspring",
        kind: Kind::Blast,
        fx: Some("melee/shadow-claw"),
        cost: 24.0,
        windup: 0.3,
        recover: 0.4,
        cooldown: 2.4,
        range: Some(200.0),
        damage: Some(14.0),
        ..BASE
    },
    // Thunder: Ancient Thunder Realm / Thunder Origin Technique: An ancient thunder dragon falls twice on the foe, one strike after another, and shocks it.
    Spell {
        name: "ancientthunder",
        kind: Kind::Zone,
        fx: Some("lightning/lightning-strike"),
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
        ..BASE
    },
    // Thunder: Demon Transformation Finger: A finger of demonic qi that silences what it hits.
    Spell {
        name: "demonfinger",
        kind: Kind::Bolt,
        fx: Some("shadow/shadow-bolt"),
        cost: 22.0,
        windup: 0.3,
        recover: 0.25,
        cooldown: 2.8,
        damage: Some(12.0),
        speed: Some(1200.0),
        ..BASE
    },
    // Thunder: Kunji Whip / Cause and Effect Whip: A whip that strikes the soul directly at middle reach: silences, and shocks.
    Spell {
        name: "kunjiwhip",
        kind: Kind::Blast,
        fx: Some("lightning/lightning-whip"),
        cost: 26.0,
        windup: 0.32,
        recover: 0.4,
        cooldown: 3.0,
        range: Some(224.0),
        damage: Some(13.0),
        ..BASE
    },
    // Thunder: Heavenly Tiger Flag: A heavenly tiger leaps from the flag in a straight line and mauls what it meets.
    Spell {
        name: "tigerflag",
        kind: Kind::Bolt,
        fx: Some("projectiles/holy-lance-flight"),
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
        fx: Some("arcane/time-stop"),
        cost: 38.0,
        windup: 0.45,
        recover: 0.35,
        cooldown: 5.5,
        damage: Some(20.0),
        impact: Some(1.2),
        radius: Some(80.0),
        notice: Some(0.1),
        reach: Some(240.0),
        grounded: true,
        ..BASE
    },
    // Frost: Life and Death Restriction: Turns the vitality of whoever stands in it into death energy: a cloud that burns for three seconds.
    Spell {
        name: "lifedeathrestrict",
        kind: Kind::Void,
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
        ..BASE
    },
    // Frost: Billion Soul Banner / Soul Banner method: Spirits swirl in a vortex over the foe, dragging it in and cutting it while it stands there.
    Spell {
        name: "soulbanner",
        kind: Kind::Void,
        fx: Some("lightning/storm-field"),
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
        fx: Some("shadow/curse"),
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
        pull_until: Some(0.6),
        ..BASE
    },
    // Frost: Seven-Colored Divine Void Nail: A nail of void driven at the foe at great speed: breaks its defence.
    Spell {
        name: "voidnail",
        kind: Kind::Bolt,
        fx: Some("projectiles/shuriken-flight"),
        cost: 34.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 4.5,
        damage: Some(20.0),
        speed: Some(1680.0),
        ..BASE
    },
    // Thunder: Finger of Death / 100 Fingers of Death: Three fingers of death fall one after another where the foe will be.
    Spell {
        name: "fingerdeath",
        kind: Kind::Zone,
        fx: Some("shadow/shadow-spikes"),
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
        fx: Some("melee/spin-slash"),
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
        fx: Some("fire/fire-dash"),
        cost: 28.0,
        windup: 0.22,
        recover: 0.4,
        cooldown: 3.6,
        distance: Some(320.0),
        damage: Some(10.0),
        ..BASE
    },
    // Thunder: Rise Three Realm Flame Origin: The origin of flame rises under the foe and burns it.
    Spell {
        name: "flameorigin",
        kind: Kind::Zone,
        fx: Some("fire/fire-pillar"),
        cost: 34.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 3.8,
        damage: Some(12.0),
        sigil: Some(0.35),
        impact: Some(0.4),
        radius: Some(80.0),
        notice: Some(0.0),
        ..BASE
    },
    // Thunder: War Spirit Print (Cloud Sea divine ability): A seal of war stamped on the foe at close range: a heavy blow that breaks its defence.
    Spell {
        name: "warprint",
        kind: Kind::Blast,
        fx: Some("impacts/crater-impact"),
        cost: 32.0,
        windup: 0.4,
        recover: 0.5,
        cooldown: 4.0,
        range: Some(184.0),
        damage: Some(22.0),
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
        fx: Some("scifi/drone-thruster"),
        cost: 36.0,
        windup: 0.5,
        recover: 0.35,
        cooldown: 4.5,
        damage: Some(22.0),
        speed: Some(1440.0),
        ..BASE
    },
    // Frost: Ghost Summoning Technique: Ghosts rise round the foe and drag it in while they cut it.
    Spell {
        name: "ghostsummon",
        kind: Kind::Void,
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
        fx: Some("nature/bloom-heal"),
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
        fx: Some("explosions/firework-burst"),
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
        fx: Some("holy/halo-crown"),
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
        fx: Some("holy/regen-aura"),
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
        fx: Some("impacts/twin-converge"),
        cost: 36.0,
        windup: 0.35,
        recover: 0.3,
        cooldown: 4.0,
        damage: Some(20.0),
        impact: Some(0.5),
        radius: Some(64.0),
        notice: Some(0.1),
        ..BASE
    },
    // Thunder: Ancient God Cauldron: Swaps places with the foe, wherever it stands.
    Spell {
        name: "ancientcauldron",
        kind: Kind::Blink,
        fx: Some("scifi/warp-jump"),
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
        fx: Some("fire/lava-burst"),
        cost: 40.0,
        windup: 0.45,
        recover: 0.35,
        cooldown: 5.5,
        damage: Some(5.8),
        sigil: Some(0.2),
        impact: Some(0.08),
        radius: Some(44.0),
        notice: Some(0.0),
        strikes: Some(4.0),
        every: Some(0.3),
        aim_ahead: Some(0.2),
        ..BASE
    },
    // Thunder: Dao Art Fusion: Treasures, abilities and rules fused into one blow: slow to gather, heavy, and it shocks.
    Spell {
        name: "daofusion",
        kind: Kind::Blast,
        fx: Some("holy/holy-nova"),
        cost: 46.0,
        windup: 0.8,
        recover: 0.5,
        cooldown: 7.0,
        range: Some(216.0),
        damage: Some(36.0),
        ..BASE
    },
    // Frost: Dao Transformation Yellow Spring: A reincarnation cycle forms over the foe: a cold field that chills whoever stands in it for three seconds.
    Spell {
        name: "daoyellow",
        kind: Kind::Void,
        fx: Some("ice/frozen-ground"),
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
        ..BASE
    },
    // Frost: Condensed Spring Soul: A hundred million souls condensed into one dart that chills what it hits.
    Spell {
        name: "springsoul",
        kind: Kind::Bolt,
        fx: Some("projectiles/water-bolt-flight"),
        cost: 26.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 3.0,
        damage: Some(14.0),
        speed: Some(680.0),
        ..BASE
    },
    // Frost: Star Soul Extraction: Pulls the soul out of the foe at close range.
    Spell {
        name: "starsoul",
        kind: Kind::Blast,
        fx: Some("shadow/shadow-cast"),
        cost: 30.0,
        windup: 0.45,
        recover: 0.4,
        cooldown: 4.5,
        range: Some(184.0),
        damage: Some(12.0),
        ..BASE
    },
    // Frost: Life Destruction Spirit Summoning: Spends 20 of the caster's life to call spirits down in two waves that burn the foe, each stronger for the life paid.
    Spell {
        name: "spiritsummon",
        kind: Kind::Zone,
        fx: Some("fire/incinerate"),
        cost: 36.0,
        windup: 0.4,
        recover: 0.3,
        cooldown: 4.5,
        damage: Some(8.0),
        impact: Some(0.3),
        radius: Some(48.0),
        notice: Some(0.0),
        strikes: Some(2.0),
        every: Some(0.4),
        ..BASE
    },
    // Frost: Star Compass: The compass turns and the caster and the foe trade places, quicker and cheaper than the cauldron.
    Spell {
        name: "starcompass",
        kind: Kind::Blink,
        fx: Some("arcane/teleport-in"),
        swap: true,
        cost: 18.0,
        windup: 0.1,
        recover: 0.2,
        cooldown: 2.5,
        distance: Some(160.0),
        ..BASE
    },
];

/// Every skill of the shared pool as the duel casts it: the motion it borrows (`Gem::spell`: how it moves and looks),
/// with its own name, qi cost, timings and power (`POWER`s of what it is expected to deal). Built once, in the pool's
/// order.
fn skills() -> &'static [Spell] {
    static SKILLS: OnceLock<Vec<Spell>> = OnceLock::new();
    SKILLS.get_or_init(|| {
        use arena_calc::gem::{self, Effect, Resource};
        gem::GEMS
            .iter()
            .filter_map(|g| {
                let m = motion(g.spell)?;
                let qi = g
                    .costs
                    .iter()
                    .filter(|c| c.0 == Resource::Qi)
                    .map(|c| c.1)
                    .sum();
                let heal = g.effects.iter().find_map(|e| match e {
                    Effect::Heal(h) => Some(*h * 100.0),
                    _ => None,
                });
                Some(Spell {
                    name: g.id,
                    fx: gem_fx(g.id).or(m.fx),
                    cost: qi,
                    windup: g.windup,
                    recover: g.recover,
                    cooldown: g.cooldown,
                    damage: g.deals_damage().then(|| gem::expected_damage(g) / POWER),
                    amount: heal.or(m.amount),
                    ..*m
                })
            })
            .collect()
    })
}

/// Skill `id` of the shared pool as the duel casts it (see `skills`).
/// The effect skill `id` plays of its own when it borrows another skill's motion (`Gem::spell`).
pub fn gem_fx(id: &str) -> Option<&'static str> {
    Some(match id {
        "hundredfist" => "impacts/punch-hit",
        "tenfist" => "melee/uppercut",
        "manshen" => "melee/axe-smear",
        "devourbody" => "shadow/shadow-step",
        "manspear" => "melee/spear-thrust",
        "autumn" => "melee/scythe-reap",
        "godquake" => "melee/shield-bash",
        "banspear" => "melee/quickdraw-slash",
        "starbreak" => "explosions/fire-nova",
        "bloodpowder" => "impacts/blood-splat",
        "threeslay" => "melee/sword-combo",
        "liphong" => "fire/fire-breath",
        "bindingword" => "status/silenced",
        "extremesense" => "scifi/scanner-sweep",
        "silentfinger" => "projectiles/poison-dart",
        "fivearrows" => "projectiles/arrow-flight",
        "libow" => "scifi/railgun-shot",
        "godfinger" => "projectiles/magic-missile",
        "slaughterbolt" => "scifi/rocket-exhaust",
        "timereverse" => "ui/treasure-glint",
        "manhair" => "projectiles/thorn-shot-flight",
        "skyqueen" => "projectiles/ice-shard",
        "movemountain" => "nature/rockfall",
        "threerestrict" => "nature/thorn-wall",
        "karmaseal" => "holy/divine-beam",
        "remnantdusk" => "explosions/cluster-bomb",
        "sukiep" => "explosions/barrel-explosion",
        "thunderchart" => "scifi/emp-pulse",
        "fourforms" => "nature/earth-spike",
        "soulburst" => "shadow/shadow-burst",
        "heaventhread" => "lightning/lightning-cast",
        "bannertribulation" => "scifi/mech-explosion",
        "cursewords" => "explosions/toxic-explosion",
        "ninedeaths" => "impacts/death-poof",
        "fiveorgans" => "nature/spore-burst",
        "threebreaths" => "arcane/arcane-cast",
        "strawknot" => "nature/nature-cast",
        "ancestorcurse" => "ui/debuff-arrow",
        "agingcurse" => "smoke/soot-burst",
        "exposemark" => "status/target-mark",
        "deathseal" => "shadow/doom-mark",
        "sealgod" => "arcane/shield-shatter",
        "wanderingsouls" => "ambient/dust-motes",
        "restrictbanner" => "arcane/gravity-well",
        "yellowfinger" => "water/whirlpool",
        "moonwing" => "ambient/rising-embers",
        "soulmerge" => "lightning/thunder-cloud",
        "realfake" => "scifi/cloak-shimmer",
        "defiance" => "scifi/shield-hit",
        "warseal" => "ice/frost-armor",
        "godform" => "ui/level-up",
        "fate" => "holy/holy-cast",
        "soularmour" => "ice/ice-cast",
        "deathlessbreath" => "ui/heal-plus",
        "teleport" => "scifi/teleport-beam",
        "stillwind" => "wind/tornado",
        "fixedrule" => "ice/freeze-encase",
        "truepressure" => "nature/quicksand",
        "windrain" => "ice/blizzard",
        "slaughteraura" => "status/bleeding",
        "timeaura" => "status/slowed",
        "fourseasons" => "nature/leaf-storm",
        "belief" => "status/blessed",
        "waraura" => "status/berserk",
        "ease" => "wind/updraft",
        "dispassion" => "status/frozen-chill",
        "frenzy" => "status/burning-aura",
        "moods" => "status/confused",
        "rebirth" => "water/ripple",
        "desire" => "fire/fire-cast",
        "soullock" => "status/electrified",
        "memory" => "status/sleep",
        "infatuation" => "status/poisoned",
        "severance" => "wind/whirlwind",
        "charm" => "status/charm-hearts",
        _ => return None,
    })
}

pub fn spell(id: &str) -> Option<&'static Spell> {
    skills().iter().find(|s| s.name == id)
}

/// Every skill of the shared pool the duel can cast, in the pool's order.
pub fn spells() -> &'static [Spell] {
    skills()
}

/// Every motion, in the duel's order.
pub fn motions() -> &'static [Spell] {
    &SPELLS
}

/// Whether `b` is false: a spell's flag JS leaves out when it is off.
#[cfg(feature = "serde")]
fn is_false(b: &bool) -> bool {
    !*b
}

/// The motion called `name`: how a skill that borrows it moves and looks (`arena_calc::gem::Gem::spell`).
pub fn motion(name: &str) -> Option<&'static Spell> {
    SPELLS.iter().find(|s| s.name == name)
}

/// How a look tints an effect its fighter's skills play.
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

/// How a fighter looks: the colours its effects are played in (a bolt's, a rune circle's, and tints by effect). The
/// rules play a fighter's effects in its look's colours and decide nothing by it.
pub struct LookData {
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

static THUNDER: LookData = LookData {
    bolt: [
        "#2a1a08", "#6a4410", "#b07a1a", "#e8b030", "#ffd84a", "#ffe98a", "#fff6c4", "#ffffff",
    ],
    circle: THUNDER_CIRCLE,
    tint: &[
        // the Renegade Immortal skills' effects take the look's colours
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
        // a look's barrier rings in its circle's colours, and so does a blow it turns aside
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

static FROST: LookData = LookData {
    bolt: [
        "#151a3d", "#212f6b", "#2c58aa", "#3a8bd8", "#62c8f4", "#a6ecff", "#dcfbff", "#ffffff",
    ],
    circle: FROST_CIRCLE,
    tint: &[
        // the Renegade Immortal skills' effects take the look's colours
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
        // a look's barrier rings in its circle's colours, and so does a blow it turns aside
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

/// A look's colours.
pub fn look(l: Look) -> &'static LookData {
    match l {
        Look::VuongLam => &THUNDER,
        Look::ThatThai => &FROST,
    }
}

/// How look `l` tints effect `fx`, if it does.
pub fn tint(l: Look, fx: &str) -> Option<&'static Tint> {
    look(l)
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

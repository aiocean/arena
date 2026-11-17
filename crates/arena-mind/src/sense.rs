//! What a fighter could perceive this tick, as the game hands it in: the true state. The mind applies its own
//! reaction time to the foe and the threats.

use crate::policy::SLOTS;
use crate::skill::SkillKind;

/// The damage system's lists the policy sees one place for each of, in the game's fixed orders (the duel's: its
/// tests tie each to arena-calc's own): ailments, supports, curses, auras, keystones and roots.
pub const AILMENTS: usize = 11;
pub const SUPPORTS: usize = 119;
pub const CURSES: usize = 12;
pub const AURAS: usize = 15;
pub const KEYSTONES: usize = 20;
pub const ROOTS: usize = 7;
// a slot's supports are a u128's bits (`SlotX::supports`)
const _: () = assert!(SUPPORTS <= 128);

/// The arena's walls and ceiling, in game pixels.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Arena {
    pub left: f64,
    pub right: f64,
    pub top: f64,
}

/// Which half of a skill a fighter is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum ActPhase {
    Windup,
    Recover,
}

/// What the foe is doing.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase",
        deny_unknown_fields
    )
)]
pub enum Doing {
    Idle,
    /// Walking: `dir` -1 left, 1 right.
    Move {
        dir: f64,
    },
    Guard,
    Meditate,
    Fly,
    /// Using a skill: `t` seconds into its `phase`.
    Act {
        kind: SkillKind,
        phase: ActPhase,
        t: f64,
        windup: f64,
        recover: f64,
        reach: f64,
        damage: f64,
    },
}

/// A skill of the fighter's own under way: `t` seconds into its `phase`, of `windup` and `recover` seconds.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Acting {
    pub phase: ActPhase,
    pub t: f64,
    pub windup: f64,
    pub recover: f64,
}

/// The fighter itself.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Me {
    pub x: f64,
    /// Height of the feet over the ground.
    pub air: f64,
    /// -1 facing left, 1 right.
    pub face: f64,
    /// Life, 0 to 100.
    pub hp: f64,
    /// Qi, 0 to 100: a share of the most it can hold.
    pub qi: f64,
    /// The most qi it can hold, in the qi skills cost (100 for most loadouts; a keystone can raise it far).
    #[cfg_attr(feature = "serde", serde(default = "full_qi"))]
    pub qi_max: f64,
    pub flying: bool,
    pub hover: f64,
    pub vx: f64,
    /// Free to act: not in a skill, reeling, crouching to jump, ducking, frozen, shocked or knocked out.
    pub can_act: bool,
    pub silenced: bool,
    /// Blink is sealed.
    pub sealed: bool,
    pub barrier: bool,
    pub shield: bool,
    /// A seeker of its own still hunts the foe.
    pub seeker_out: bool,
    /// The skill it is using, if any.
    #[cfg_attr(feature = "serde", serde(default))]
    pub act: Option<Acting>,
}

/// The most qi a fighter holds unless its loadout says otherwise, in the qi skills cost.
pub const FULL_QI: f64 = 100.0;

#[cfg(feature = "serde")]
fn full_qi() -> f64 {
    FULL_QI
}

impl Me {
    /// How full of qi it is for fighting, 0 to 100: its share of what it can hold, but of `FULL_QI` at most, so a
    /// fighter that can hold far more (a keystone's 10000) counts as full once it holds what a fighter usually does,
    /// enough for its skills, instead of meditating toward a share it would take minutes to fill.
    pub fn qi_level(&self) -> f64 {
        if self.qi_max <= FULL_QI || self.qi_max <= 0.0 {
            return self.qi;
        }
        (self.qi * self.qi_max / FULL_QI).min(100.0)
    }
}

/// The foe.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Foe {
    pub x: f64,
    pub air: f64,
    pub flying: bool,
    pub hp: f64,
    pub qi: f64,
    pub doing: Doing,
    /// Seconds until it can act again (staggered, falling, lying, getting up).
    pub out_for: f64,
    /// Seconds until a blow can touch it.
    pub open_in: f64,
    pub frozen: f64,
    pub shocked: f64,
    pub barrier: bool,
    /// Whether each of its skills (as handed to `Mind::meet`) is ready now: what it could answer with.
    pub ready: Vec<bool>,
}

/// The shape of a threat in flight, as the mind reasons about it. Heights from the ground, lengths in game pixels.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        tag = "type",
        rename_all = "camelCase",
        rename_all_fields = "camelCase",
        deny_unknown_fields
    )
)]
pub enum ThreatShape {
    /// Flies level at height `y`.
    Projectile { x: f64, y: f64, vx: f64 },
    /// Lands on `x` in `lands_in` seconds, hurting within `radius` (and drawing in from `reach`).
    Area {
        x: f64,
        radius: f64,
        reach: f64,
        lands_in: f64,
        grounded: bool,
    },
    /// Hunts the foe; at `x`, `y` now.
    Seeker { x: f64, y: f64, radius: f64 },
    /// Stays on `x`, hurting within `reach`.
    Field { x: f64, reach: f64 },
    /// Strikes that follow the one they fall on; the next lands in `lands_in`.
    Barrage { lands_in: f64 },
}

/// A foe's shot that could reach me.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Threat {
    /// Seconds it has been there to see.
    pub age: f64,
    pub damage: f64,
    pub shape: ThreatShape,
}

/// Everything a fighter could perceive this tick.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct Sense {
    /// The fight's clock, seconds.
    pub time: f64,
    pub me: Me,
    pub foe: Foe,
    pub arena: Arena,
    pub threats: Vec<Threat>,
    /// Whether each skill handed to `Mind::new` is ready now, in that order.
    pub ready: Vec<bool>,
    /// How hard each skill handed to `Mind::new` strikes now, as a share of its `damage` (a game whose skills grow
    /// or shrink in a fight says so here), in that order; a skill past the end of the list strikes at 1.
    #[cfg_attr(feature = "serde", serde(default))]
    pub power: Vec<f64>,
    /// How many times each skill handed to `Mind::new` lands its stacking statuses (`Inflicts::chill`, `shock`) now,
    /// as a multiple of what it inflicts (a game whose skills grow says so here), in that order; a skill past the
    /// end of the list lands them once.
    #[cfg_attr(feature = "serde", serde(default))]
    pub stacks: Vec<f64>,
    /// The game's castable slots in its own fixed layout, each the index of a skill handed to `Mind::new` or `None`
    /// for an empty slot. Only a policy reads it (`policy::observe`, `policy::legal`); the utility mind does not.
    #[cfg_attr(feature = "serde", serde(default))]
    pub slots: Vec<Option<usize>>,
    /// The frame data of its own cast and the foe's (true; the policy shows the foe's only once it sees the foe
    /// acting, through `Delayed`). `None` when not casting.
    #[cfg_attr(feature = "serde", serde(default))]
    pub me_act: Option<ActFrames>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub foe_act: Option<ActFrames>,
    /// What each of `slots` builds to now, in the same order (`None` for an empty slot).
    #[cfg_attr(feature = "serde", serde(default))]
    pub slotx: [Option<SlotX>; SLOTS],
    /// What lies on it and on the foe: ailments, controls, curses, auras, buffs, shields, keystones, root.
    #[cfg_attr(feature = "serde", serde(default))]
    pub effects: [Effects; 2],
    /// The traps, mines, totems and minions standing, its own and the foe's, in any order.
    #[cfg_attr(feature = "serde", serde(default))]
    pub units: Vec<UnitSeen>,
    /// The foe's castable slots in the same layout as `slots` (its basics, then its bar), each what it builds to.
    #[cfg_attr(feature = "serde", serde(default))]
    pub foe_slots: [Option<FoeSlot>; SLOTS],
}

/// A cast under way as its fighter's clip times it. Its active window opens when its blow is released (the end of
/// its wind-up) and stays open while the clip's active frames show, by the recovery's progress; a clip with no
/// active window has none (`to_window` and `window` 0, never `in_window`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields, default)
)]
pub struct ActFrames {
    pub in_window: bool,
    /// Seconds until the window opens: 0 in it, past it, or with none.
    pub to_window: f64,
    /// The window's length, seconds.
    pub window: f64,
    /// Seconds until its recovery begins, when it can be punished: 0 once it recovers.
    pub to_punish: f64,
    /// Its recovery's whole span, seconds (longer for an unsafe blow that was guarded).
    pub recover: f64,
    /// The box its blade strikes with now or on its active frame, `[x0, y0, x1, y1]`: x in game pixels, y up from
    /// the ground. `None` for a clip drawn with no blade out ahead.
    pub hitbox: Option<[f64; 4]>,
}

/// A slot's skill as it builds now, with every support, keystone and stat its fighter carries. Times in seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields, default)
)]
pub struct SlotX {
    /// Seconds to its next use (0 with one stored), and its whole cooldown.
    pub cooldown_left: f64,
    pub cooldown: f64,
    /// Its stored uses and their most.
    pub stored: f64,
    pub max_uses: f64,
    pub windup: f64,
    pub recover: f64,
    /// Seconds its recovery runs longer when its blow is guarded (0 for a safe skill).
    pub blocked_extra: f64,
    /// When its clip's active window opens, from the cast's start, and how long it stays open (0 for a clip with
    /// none).
    pub window_start: f64,
    pub window: f64,
    /// What it is expected to deal (a heal: to heal), in the units of `Skill::damage`.
    pub damage: f64,
    /// The qi it costs.
    pub qi: f64,
    /// Projectiles besides the first, times its blow lands, its area's factor (1 as cast), pierces, chains, forks.
    pub projectiles: f64,
    pub strikes: f64,
    pub area: f64,
    pub pierce: f64,
    pub chain: f64,
    pub fork: f64,
    /// Supports that do nothing on it (their tags do not fit).
    pub inert: f64,
    /// Each ailment's stacks a landing blow lays besides its parts', over the stacks that make it full, in the
    /// game's ailment order.
    pub inflict: [f64; AILMENTS],
    /// The supports it carries, a bit for each in the game's support order.
    pub supports: u128,
}

/// One of the foe's slots: its kind, whether it is ready, its reach (game pixels) and what it builds to.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct FoeSlot {
    pub kind: SkillKind,
    pub ready: bool,
    pub reach: f64,
    pub built: SlotX,
}

/// One ailment on a fighter: its stacks (for a damage over time, its instances), the stacks that make it full, the
/// seconds its longest has left, and for a burn or a bleed the strongest one's damage a second as a share of full
/// life.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields, default)
)]
pub struct AilmentOn {
    pub stacks: f64,
    pub cap: f64,
    pub left: f64,
    pub strength: f64,
}

/// What lies on a fighter. Times in seconds left, 0 when it is not on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields, default)
)]
pub struct Effects {
    /// In the game's ailment order.
    pub ailments: [AilmentOn; AILMENTS],
    /// Stunned, rooted, down (until it can act), shocked, its soul lost, blinking sealed, no guard, choked (no qi
    /// regen), rotting (healing hurts).
    pub stun: f64,
    pub root: f64,
    pub down: f64,
    pub shocked: f64,
    pub soul_lost: f64,
    pub sealed: f64,
    pub no_guard: f64,
    pub choke: f64,
    pub rot: f64,
    /// Each kind of curse's time left, in the game's curse order.
    pub curses: [f64; CURSES],
    /// Each kind of aura it holds, in the game's aura order.
    pub auras: [bool; AURAS],
    /// Its skills' timed buffs: how many, the longest's time left, and their sizes summed (a stance's damage more,
    /// a blessing's damage taken less).
    pub buffs: f64,
    pub buff_left: f64,
    pub buff_size: f64,
    /// Its energy shield as a share of the most, its ward's charges, its barrier's time left.
    pub shield: f64,
    pub ward: f64,
    pub barrier: f64,
    /// The keystones its loadout holds, in the game's keystone order.
    pub keystones: [bool; KEYSTONES],
    /// Its root (linh căn), as an index in the game's roots; `None` for none.
    pub element: Option<usize>,
}

/// What a thing on the field is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum UnitKind {
    #[default]
    Trap,
    Mine,
    Totem,
    Minion,
}

/// A trap, mine, totem or minion standing: whose, what, where (game pixels, `y` its centre's height over the
/// ground), its life as a share (0 to 1), seconds until it falls apart (`None` for never), armed, and
/// seconds to its next blow (0 for a trap or a mine, which strike when sprung).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct UnitSeen {
    pub mine: bool,
    pub kind: UnitKind,
    pub x: f64,
    pub y: f64,
    pub hp: f64,
    pub left: Option<f64>,
    pub armed: bool,
    pub next: f64,
}

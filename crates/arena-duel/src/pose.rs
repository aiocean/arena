//! What a fighter is doing, as the painter draws it.

/// A fighter's pose. Each has its own clock (`pose_t`), restarted when the pose changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Pose {
    Idle,
    Walk,
    Jump,
    Fly,
    /// Recovering from a landing.
    Land,
    /// The squat before a jump, or the last part of getting up.
    Crouch,
    /// Ducking under something flying at head height.
    Duck,
    /// Winding up a blade blow.
    Draw,
    /// The blade blow's follow-through.
    Slash,
    /// Gathering a spell.
    Channel,
    /// A spell's follow-through.
    Cast,
    Guard,
    Meditate,
    /// Struck and staggered where it stands.
    Hurt,
    Shocked,
    /// Thrown up by a blow, still rising.
    Launched,
    /// Thrown hard enough to spin, before it bounces.
    Tumble,
    Fall,
    /// Lying on the floor.
    Down,
    /// A quick rise: rolling away.
    Roll,
    /// Getting up: sitting up.
    Sit,
    /// Getting up: on one knee.
    Kneel,
    Win,
}

/// How a spell is gathered, which sets the arms of a channel pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Gather {
    /// Both arms raised in a V (zones, voids, orbs).
    Summon,
    /// One palm drawn back (bolts, blasts).
    Palm,
    /// Leaning forward to dash.
    Lunge,
    /// Hands joined (heals, wards).
    Prayer,
    /// Arms crossed (barriers, blinks).
    Cross,
}

/// How a spell of `kind` is gathered: a summoning raises both arms, a palm draws back, a prayer joins the hands.
pub fn gather_of(kind: crate::data::Kind) -> Gather {
    use crate::data::Kind;
    match kind {
        Kind::Zone | Kind::Void | Kind::Orb => Gather::Summon,
        Kind::Dash => Gather::Lunge,
        Kind::Heal | Kind::Ward => Gather::Prayer,
        Kind::Barrier | Kind::Blink => Gather::Cross,
        Kind::Bolt | Kind::Blast | Kind::Melee => Gather::Palm,
    }
}

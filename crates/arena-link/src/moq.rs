//! How the link is laid onto MoQ, agreed by both ends.
//!
//! The server publishes each match as a **broadcast** named [`broadcast`]`(match_id)`, with one **track** named
//! [`TRACK`]. A page that wants the list of live matches reads the broadcasts announced under [`NAMESPACE`]; to watch
//! one, it subscribes to that broadcast's track. Within the track, the server rolls a new **group** every
//! [`GROUP_TICKS`] ticks, each group's first frame a [`crate::Frame::Keyframe`], the rest its decisions.

/// The namespace every match broadcast lives under. A page lists live matches by watching the announcements here.
pub const NAMESPACE: &str = "arena/match";

/// The one track each match broadcast carries.
pub const TRACK: &str = "fight";

/// Fighting ticks per group. A new group starts with a fresh keyframe, so a late joiner's cached group (what the relay
/// replays to it) never grows past this many decisions. The rules run at 60 ticks a second, so this is one second.
pub const GROUP_TICKS: u32 = 60;

/// The broadcast name for a match: the namespace and the match id.
pub fn broadcast(match_id: &str) -> String {
    format!("{NAMESPACE}/{match_id}")
}

/// Ticks between two `Frame::Through` marks: a page may step only as far as the last one, so this (with the network's
/// delay) is the least a page must lag the fight. Six ticks is a tenth of a second.
pub const THROUGH_EVERY: u32 = 6;

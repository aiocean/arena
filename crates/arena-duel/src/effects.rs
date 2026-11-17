//! The effect ids a duel can play, and which of them loop: the pixfx engine's registry at rev ad1612b, where the
//! duel's effects came from. A spell whose effect is not in a duel's list is never cast, and some plays roll the
//! dice only when their effect is there, so this list is part of the rules: every golden is pinned with all of it.

/// Every effect id, and whether it loops (and so must be stopped).
pub const EFFECTS: &[(&str, bool)] = &[
    ("fire/candle", true),
    ("fire/campfire", true),
    ("fire/brazier", true),
    ("fire/spirit-flame", true),
    ("impacts/hit", false),
    ("impacts/parry", false),
    ("explosions/dirt-explosion", false),
    ("explosions/small-explosion", false),
    ("explosions/implosion", false),
    ("explosions/plasma-explosion", false),
    ("explosions/body-burst", false),
    ("arcane/rune-circle", true),
    ("arcane/hex-shield", false),
    ("arcane/restriction", false),
    ("arcane/sword-formation", true),
    ("arcane/flying-sword", true),
    ("arcane/held-sword", true),
    ("arcane/sword-strike", false),
    ("arcane/sword-slash", false),
    ("arcane/bagua-mirror", false),
    ("arcane/bagua-mirror-strike", true),
    ("arcane/bagua-mirror-flying", false),
    ("arcane/bagua-mirror-burst", false),
    ("critters/wisp", true),
    ("feedback/heal-sparkle", true),
    ("feedback/outline", true),
    ("feedback/power-up", false),
    ("spawn/poof", false),
    ("holy/feather-burst", false),
    ("holy/golden-bell", false),
    ("holy/lotus", false),
    ("ice/icicle-fall", false),
    ("scifi/jetpack", true),
    ("lightning/ball-lightning", true),
    ("nature/poison-cloud", true),
    ("pickups/coin-spin", true),
    ("projectiles/laser-bolt", true),
    ("projectiles/trail", true),
    ("transitions/burn-away", false),
    ("transitions/crt-off", false),
    ("shadow/blood-gush", false),
    ("shadow/blood-shield", false),
    ("shadow/void-orb", true),
    ("shadow/winter-ordeal", false),
    ("water/splash", false),
    ("water/water-jet", true),
    ("weather/snow", true),
    ("weather/fireflies", true),
    ("arcane/blade-pinwheel", false),
    ("impacts/star-converge", false),
    ("impacts/twin-converge", false),
    ("projectiles/drop-dart", false),
    ("arcane/comet-rings", false),
    ("arcane/comma-seal", false),
    ("arcane/shuriken-seal", false),
    ("arcane/vortex-ring", false),
    ("explosions/swirl-blast", false),
    ("spawn/swirl-portal", false),
    ("arcane/twin-shuriken", false),
    ("explosions/puff-burst", false),
];

/// Every effect id, as a duel's config takes them.
pub fn all() -> Vec<String> {
    EFFECTS.iter().map(|(id, _)| (*id).to_string()).collect()
}

/// Whether effect `fx` loops.
pub fn loops(fx: &str) -> bool {
    EFFECTS.iter().any(|&(id, looping)| id == fx && looping)
}

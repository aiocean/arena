//! The arcane effects the duel plays, after the pixfx engine's `arcane` category: the cast circle (rune circle), the
//! barriers (hex shield, restriction, bagua mirror) and the tone-painted seals (comma seal, shuriken seal, comet
//! rings, vortex ring). Their shared helpers (the category's `lib.rs` and the engine's tone painting) are private
//! here.

mod bagua_mirror;
mod hex_shield;
mod common;
mod restriction;
mod rune_circle;
mod seals;
mod tone;

use crate::vfx::kit::Recipe;

pub static ALL: &[Recipe] = &[
    Recipe::new("arcane/rune-circle", true, rune_circle::D, rune_circle::make),
    Recipe::new("arcane/hex-shield", false, 3.9, hex_shield::make),
    Recipe::new("arcane/restriction", false, 3.8, restriction::make),
    Recipe::new("arcane/bagua-mirror", false, 3.6, bagua_mirror::make),
    Recipe::new(
        "arcane/comma-seal",
        false,
        seals::MOMENT * 14.0,
        seals::comma_seal,
    )
    .fps(50),
    Recipe::new(
        "arcane/shuriken-seal",
        false,
        seals::MOMENT * 14.0,
        seals::shuriken_seal,
    )
    .fps(50),
    Recipe::new(
        "arcane/comet-rings",
        false,
        seals::MOMENT * 14.0,
        seals::comet_rings,
    )
    .fps(50),
    Recipe::new(
        "arcane/vortex-ring",
        false,
        seals::MOMENT * 13.0,
        seals::vortex_ring,
    )
    .fps(50),
];

//! Blasts: the explosions (a buried charge, a collapse and its pop, a plasma ball, a puff, a ground blast, a swirl)
//! and the spawn puffs (a poof, a swirl portal), after the pixfx engine's `explosions/*` and `spawn/*`.
//!
//! `common` holds what several share (the engine's explosions `lib.rs` and its tone painting); each effect is its own
//! module.

use crate::vfx::kit::Recipe;

mod common;
mod dirt;
mod implosion;
mod plasma;
mod poof;
mod portal;
mod puff;
mod small;
mod swirl;

pub static ALL: &[Recipe] = &[
    Recipe::new("explosions/dirt-explosion", false, dirt::DURATION, dirt::make),
    Recipe::new("explosions/implosion", false, implosion::DURATION, implosion::make),
    Recipe::new("explosions/plasma-explosion", false, plasma::DURATION, plasma::make),
    Recipe::new("explosions/puff-burst", false, puff::DURATION, puff::make).fps(50),
    Recipe::new("explosions/small-explosion", false, small::DURATION, small::make),
    Recipe::new("explosions/swirl-blast", false, swirl::DURATION, swirl::make).fps(50),
    Recipe::new("spawn/poof", false, poof::DURATION, poof::make),
    Recipe::new("spawn/swirl-portal", false, portal::DURATION, portal::make).fps(50),
];

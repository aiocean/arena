//! The elements: an icicle that drops, a ball of lightning, the duel's bolt, a void orb and the winter sky that falls.
//! Each written after the pixfx engine's effect of the same id (`pixfx-effects/src/<category>/<name>.rs`).

mod ball;
mod bolt;
mod icicle;
mod void;
mod winter;

use crate::vfx::kit::Recipe;

pub static ALL: &[Recipe] = &[
    icicle::RECIPE,
    ball::RECIPE,
    bolt::RECIPE,
    void::RECIPE,
    winter::RECIPE,
];

//! Blades and the marks they leave: the spirit swords (`arcane/sword-formation`, `arcane/sword-strike`,
//! `arcane/sword-slash`), the hand-drawn stroke marks (`arcane/twin-shuriken`, `impacts/star-converge`,
//! `impacts/twin-converge`) and the sparks of a blow (`impacts/parry`, the duel's clash; `impacts/hit`), each written
//! after the pixfx engine's effect of the same id.

mod clash;
mod marks;
mod sword;
mod tone;

use crate::vfx::kit::Recipe;

pub static ALL: &[Recipe] = &[
    Recipe::new(
        "arcane/sword-formation",
        true,
        sword::FORMATION_SECS,
        sword::formation,
    )
    .intro(sword::FAN_END),
    Recipe::new("arcane/sword-strike", false, 0.45, sword::strike),
    Recipe::new("arcane/sword-slash", false, sword::SLASH_SECS, sword::slash),
    Recipe::new("arcane/twin-shuriken", false, marks::SECS, marks::twin_shuriken).fps(50),
    Recipe::new("impacts/star-converge", false, marks::SECS, marks::star_converge).fps(50),
    Recipe::new("impacts/twin-converge", false, marks::SECS, marks::twin_converge).fps(50),
    Recipe::new("impacts/parry", false, 1.0, clash::parry),
    Recipe::new("impacts/hit", false, 0.45, clash::hit),
];

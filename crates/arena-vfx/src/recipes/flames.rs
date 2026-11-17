//! The flames family: the lamps' brazier, the spirit flame, the wisp, and the holy light (feathers, the golden bell,
//! the lotus) and the power-up aura. Each after the pixfx engine's effect of the same id.

mod bell;
mod brazier;
mod feathers;
mod firelib;
mod lotus;
mod power;
mod spirit;
mod wisp;

use crate::kit::Recipe;

pub static ALL: &[Recipe] = &[
    Recipe::new("fire/brazier", true, brazier::D, brazier::make),
    Recipe::new("fire/spirit-flame", true, spirit::D, spirit::make),
    Recipe::new("holy/feather-burst", false, feathers::D, feathers::make),
    Recipe::new("holy/golden-bell", false, bell::D, bell::make),
    Recipe::new("holy/lotus", false, lotus::D, lotus::make),
    Recipe::new("feedback/power-up", false, power::D, power::make).fps(60),
    Recipe::new("critters/wisp", true, wisp::D, wisp::make),
];

//! Weather and water: the poison cloud, the splash, the water jet, the fireflies (the stage's night) and the snow,
//! after the pixfx engine's effects of the same ids (`nature/`, `water/`, `weather/`).

mod fireflies;
mod poison_cloud;
mod sky;
mod snow;
mod splash;
mod water_jet;

use crate::kit::Recipe;

pub static ALL: &[Recipe] = &[
    Recipe::new("nature/poison-cloud", true, 4.0, poison_cloud::make),
    Recipe::new("water/splash", false, 2.0, splash::make),
    Recipe::new("water/water-jet", true, 1.0, water_jet::make),
    Recipe::new("weather/fireflies", true, 12.0, fireflies::make),
    Recipe::new("weather/snow", true, 12.0, snow::make),
];

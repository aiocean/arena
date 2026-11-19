//! Every effect the duel plays, as particles: each a [`Recipe`] (its id, whether it loops, how long, how a playing of
//! it draws), written after the pixfx engine's effect of the same id. They live in `recipes/`, a file a family.

use super::Effect;
use super::kit::Recipe;

mod arcane;
mod blades;
mod blasts;
mod elements;
mod flames;
mod weather;

/// Every recipe, by family.
static FAMILIES: [&[Recipe]; 6] = [
    arcane::ALL,
    blades::ALL,
    blasts::ALL,
    elements::ALL,
    flames::ALL,
    weather::ALL,
];

/// Every recipe the game has.
pub fn all() -> impl Iterator<Item = &'static Recipe> {
    FAMILIES.iter().flat_map(|f| f.iter())
}

/// The recipe with id `id`, if the game has it.
pub fn recipe(id: &str) -> Option<&'static Recipe> {
    all().find(|r| r.id == id)
}

/// The effect with id `id`, if the game has it.
pub fn find(id: &str) -> Option<&'static dyn Effect> {
    recipe(id).map(|r| r as &dyn Effect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::Canvas;
    use crate::{Paint, Spawn};

    /// The effects the duel plays (and a plain hit): each must be here.
    const WANTED: [&str; 45] = [
        "arcane/bagua-mirror",
        "arcane/comet-rings",
        "arcane/comma-seal",
        "arcane/hex-shield",
        "arcane/restriction",
        "arcane/rune-circle",
        "arcane/shuriken-seal",
        "arcane/sword-formation",
        "arcane/sword-slash",
        "arcane/sword-strike",
        "arcane/twin-shuriken",
        "arcane/vortex-ring",
        "critters/wisp",
        "explosions/dirt-explosion",
        "explosions/implosion",
        "explosions/plasma-explosion",
        "explosions/puff-burst",
        "explosions/small-explosion",
        "explosions/swirl-blast",
        "feedback/power-up",
        "fire/brazier",
        "fire/spirit-flame",
        "holy/feather-burst",
        "holy/golden-bell",
        "holy/lotus",
        "ice/icicle-fall",
        "impacts/parry",
        "impacts/star-converge",
        "impacts/twin-converge",
        "lightning/ball-lightning",
        "nature/poison-cloud",
        "projectiles/laser-bolt",
        "shadow/void-orb",
        "shadow/winter-ordeal",
        "spawn/poof",
        "spawn/swirl-portal",
        "spawn/trap-poof",
        "spawn/mine-poof",
        "spawn/totem-poof",
        "spawn/minion-poof",
        "water/splash",
        "water/water-jet",
        "weather/fireflies",
        "weather/snow",
        "impacts/hit",
    ];

    #[test]
    fn every_effect_is_here_and_loops_as_the_rules_say() {
        let missing: Vec<_> = WANTED.iter().filter(|id| find(id).is_none()).collect();
        assert!(missing.is_empty(), "missing: {missing:?}");
        for r in all() {
            assert_eq!(
                r.looping,
                arena_duel::effects::loops(r.id),
                "{} loops wrongly",
                r.id
            );
        }
    }

    #[test]
    fn every_effect_plays_and_ends() {
        let (mut back, mut front) = (Canvas::new(320, 180), Canvas::new(320, 180));
        for r in all() {
            let at = Spawn {
                x: 160.0,
                y: 120.0,
                ground: 150.0,
                width: Some(320.0),
                height: Some(160.0),
                seed: 7,
                ..Spawn::default()
            };
            let mut i = r.spawn(&at);
            let mut alive = true;
            for k in 0..240 {
                if k == 120 {
                    i.stop();
                }
                if k % 20 == 0 {
                    i.set_position(160.0 + k as f32 * 0.2, 120.0);
                }
                alive = i.update(1.0 / 60.0);
                i.paint(&mut Paint {
                    back: &mut back,
                    front: &mut front,
                });
                if !alive {
                    break;
                }
            }
            assert!(!alive, "{} never ends", r.id);
        }
    }
}

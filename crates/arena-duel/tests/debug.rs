//! The debug view's shapes: a fresh fight shows each fighter's body as wide as the hit tests take it.

use arena_duel::debug::{Mark, Shape, shapes};
use arena_duel::duel::{Duel, DuelConfig};
use arena_duel::seat::Brain;

#[test]
fn a_fresh_fight_shows_both_bodies_at_the_width_the_hit_tests_use() -> Result<(), String> {
    let config = DuelConfig {
        seed: 7,
        left: 88.0,
        right: 1192.0,
        ground: 632.0,
        sky: 196.0,
        effects: arena_duel::effects::all(),
    };
    let duel = Duel::new(config, [Brain::make(), Brain::make()]);
    let state = duel.state();
    let bodies: Vec<_> = shapes(state)
        .into_iter()
        .filter(|d| d.mark == Mark::Body)
        .collect();
    assert_eq!(bodies.len(), 2);
    for d in bodies {
        let me = d.owner.ok_or("a body with no owner")?;
        let b = &state.fighters[me].body;
        let Shape::Rect { x0, x1, y0, y1 } = d.shape else {
            return Err("a body that is not a box".into());
        };
        assert!((x1 - x0 - 2.0 * b.spec.half_width).abs() < 1e-9);
        assert!(((x0 + x1) / 2.0 - b.x).abs() < 1e-9);
        assert!(y1 > y0);
    }
    Ok(())
}

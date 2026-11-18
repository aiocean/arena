//! Every bolt flies as thick as its drawn body (`shot::body_radius`, from `bolts_data`): a bolt motion missing from
//! the table would fly as a point.

use arena_duel::data::{Kind, motions};
use arena_duel::shot::body_radius;

#[test]
fn every_bolt_motion_has_a_drawn_body() {
    let thin: Vec<&str> = motions()
        .iter()
        .filter(|m| m.kind == Kind::Bolt && body_radius(m.name) <= 0.0)
        .map(|m| m.name)
        .collect();
    assert!(
        thin.is_empty(),
        "bolts with no drawn body (run spell_hitbox --bolts): {thin:?}"
    );
}

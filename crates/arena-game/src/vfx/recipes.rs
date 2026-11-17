//! Every effect the duel plays, as particles.

use super::Effect;

/// The effect with id `id`, if the game has it.
pub fn find(_id: &str) -> Option<&'static dyn Effect> {
    None
}

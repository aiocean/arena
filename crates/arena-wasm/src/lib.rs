//! The arena's duel for JavaScript. The page loads this beside the pixfx engine's wasm (`pixfx-web`): the engine
//! draws, plays the effects and moves the bodies; this module holds only the rules (`pixfx-duel`) and the trained
//! policies a corner seats. It translates; it decides nothing.

pub mod duel;
mod js;

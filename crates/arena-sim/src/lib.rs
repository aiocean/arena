//! A headless fight simulator over `arena-calc`, to measure balance before the real fight exists. **This is not the
//! real fight**: the real fight is the duel's Bevy port, with bodies, jumps, guards, sprites and its own rules for
//! how a skill travels. This crate stands two scripted fighters on a 1D field, lets each cast its loadout through
//! `arena_calc::build` and `arena_calc::hit::resolve_with`, and plays best-of-3 matches, fast and deterministic from
//! (seed, loadout A, loadout B). The numbers it gives rank skills, supports and keystones against each other under
//! one simple brain; they say nothing about how a trained bot or a player would use them.
//!
//! - `delivery`: how a skill reaches the foe, from its tags (the simulator's own documented choices).
//! - `fighter`: a fighter's loadout made into skills, its place, its timers.
//! - `fight`: the field, the tick in a fixed order, rounds and matches.
//! - `units`: traps, mines, totems and minions on the field, bodies the foe can strike.
//! - `brain`: the scripted brain that plays every loadout.
//! - `report`: what a match measured, by skill.
//! - `rng`: the dice.
//! - `balance`: random loadout pairs played both ways, measured per skill, support and keystone, and tiered.
//!
//! The field is `FIELD` T wide (T, a fighter's height) between two walls; the fighters start `START` apart.

#![forbid(unsafe_code)]

pub mod balance;
pub mod brain;
pub mod delivery;
pub mod fight;
pub mod fighter;
pub mod report;
pub mod rng;
pub mod units;

pub use fight::{Fight, MatchResult, RoundResult, play_match};
pub use report::{SideReport, SkillLine};
pub use rng::Rng;

/// Ticks a second, as the duel (rules, "A fight").
pub const TICKS: f64 = 60.0;
/// One tick, in seconds.
pub const DT: f64 = 1.0 / TICKS;
/// The field's width in T, wall to wall.
pub const FIELD: f64 = 12.0;
/// Where the two fighters stand as a round begins (seat 0 on the left).
pub const START: [f64; 2] = [3.0, 9.0];
/// The closest two fighters' centres come (half a body each); a dash passes through.
pub const BODY: f64 = 0.5;
/// How close to a wall a fighter's centre comes.
pub const WALL: f64 = 0.25;
/// Run speed in T a second, before the bearer's `MoveSpeed`.
pub const RUN_SPEED: f64 = 3.0;
/// The level (hỏa hầu) every skill is cast at: `gem::BASE_LEVEL`, where a spell's base deals today's damage.
pub const SKILL_LEVEL: u8 = arena_calc::gem::BASE_LEVEL;
/// Seconds a round lasts before it is cut (the higher share of life wins).
pub const ROUND_TIME: f64 = 60.0;
/// Rage points that fill the ultimate (design 10).
pub const RAGE_MAX: u8 = 4;
/// Rounds a side must win to take the match (best of 3), and the most rounds a match plays (drawn rounds can
/// stretch it; past this the match is drawn).
pub const ROUNDS_TO_WIN: u8 = 2;
pub const MAX_ROUNDS: u8 = 5;
/// Seconds a landing blow staggers its target (it cannot act), and the extra for a counter and a punish (rules,
/// "Frame data").
pub const HITSTUN: f64 = 0.1;
pub const COUNTER_STAGGER: f64 = 0.1;
pub const PUNISH_STAGGER: f64 = 0.2;
/// The first seconds of an ultimate's windup, during which a blow passes through it (rules, "Frame data").
pub const ULTIMATE_OPENING: f64 = 0.3;

//! How bodies move on the field (design, 17.8): the run speed a fighter's table gives ([`Stat::MoveSpeed`]), a
//! knockback's push ([`Stat::Knockback`]), a dash's reach under Trọng, and a step along the floor between the walls.
//! The fight decides when a body moves and where it wants to go; these say how far.
//!
//! Run speed reads the bearer's table (`cultivator::Cultivator::bearer_table`), so every slow there counts and
//! nothing is added by hand: Triền (`ailment::AIL.bind_slow` increased less a stack), Phế Cước's maim
//! (`support::MAIM_SLOW`), Tuế Nguyệt's curse (`curse::NUMBERS.aging_slow`). Speed stars raise cast and attack
//! speed (`Stat::Speed`), not the run. A root (full Triền, `ailment::Control::Root`) holds the body: zero.
//!
//! Trọng shortens dashes and escapes (`ailment::AIL.heavy_reach` a stack), not a knockback: the ailment rules say
//! nothing of pushes, so a heavy body flies as far as any other.

use crate::ailment::{AIL, Ailment};
use crate::cultivator::Cultivator;
use crate::play::Field;
use crate::sheet::Sheet;
use crate::stat::{Stat, Table};

/// A fighter's run speed at the base, T a second, for a fight with no number of its own.
pub const RUN_SPEED: f64 = 3.0;

/// The run speed from `base` (T a second) and the bearer's table: `MoveSpeed`'s increases and mores, never below
/// zero; zero while `rooted`.
pub fn run_speed(base: f64, bearer: &Table, rooted: bool) -> f64 {
    if rooted {
        return 0.0;
    }
    bearer.get(Stat::MoveSpeed, base).max(0.0)
}

/// [`run_speed`] for `c` wearing `sheet`, read from its bearer table now (`moving` for the conditions that read it).
pub fn run_speed_of(c: &Cultivator, sheet: &Sheet, base: f64, moving: bool, rooted: bool) -> f64 {
    run_speed(base, &c.bearer_table(sheet, moving), rooted)
}

/// The T a blow pushes its target: the skill's own push (`base`) after the striker's table (`Stat::Knockback`,
/// flat T then its increases and mores), never below zero.
pub fn knockback_distance(base: f64, striker: &Table) -> f64 {
    striker.get(Stat::Knockback, base).max(0.0)
}

/// Where a body of `radius` at `x` lands, pushed `distance` T away from `from_x` (toward `+x` when they stand at
/// the same place), held by the walls.
pub fn knockback(x: f64, radius: f64, from_x: f64, distance: f64, field: &Field) -> f64 {
    let away = if x < from_x { -1.0 } else { 1.0 };
    field.clamp_x(x + away * distance.max(0.0), radius)
}

/// A dash's or an escape's reach under Trọng: `base` less [`AIL`]`.heavy_reach` for each stack, never below zero.
pub fn dash_reach(base: f64, heavy: f64) -> f64 {
    base * (1.0 - AIL.heavy_reach * heavy.max(0.0)).max(0.0)
}

/// [`dash_reach`] with the stacks of Trọng `c` carries.
pub fn dash_reach_of(c: &Cultivator, base: f64) -> f64 {
    dash_reach(base, c.ailments.stacks(Ailment::Heavy))
}

/// Where a body of `radius` at `x` is after running at `velocity` T a second (signed: `-` toward the left wall) for
/// `dt` seconds, held by the walls.
pub fn step_x(x: f64, radius: f64, velocity: f64, dt: f64, field: &Field) -> f64 {
    field.clamp_x(x + velocity * dt.max(0.0), radius)
}

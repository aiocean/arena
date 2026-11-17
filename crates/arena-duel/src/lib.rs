//! The landing duel's rules: two cultivators, each with a loadout from the shared pool (`arena-calc`), fight with
//! skills drawn onto a rotating bar, with wards and barriers, traps, mines, totems and minions, round after round.
//! Every blow goes through the damage system. Everything is decided here and deterministic from a seed; a shell plays
//! the effect commands and feels the events `Duel::step` returns.
//! See `AGENTS.md` and `docs/specs/2026-11-15-duel-rules-in-rust-design.md`.

#![forbid(unsafe_code)]

pub mod act;
pub mod bar;
pub mod body;
pub mod data;
pub mod debug;
pub mod duel;
pub mod effects;
pub mod fighter;
pub mod fingerprint;
pub mod frames;
#[rustfmt::skip]
mod frames_data;
pub mod first_mind;
pub mod js;
pub mod kit;
pub mod ladder;
pub mod out;
pub mod pick;
pub mod pose;
pub mod scripted;
pub mod seat;
pub mod sense;
pub mod shot;
pub mod strike;
pub mod style;
pub mod train;
pub mod unit;
pub mod view;

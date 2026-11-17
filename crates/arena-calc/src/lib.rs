//! The damage system: ten kinds of damage, their ailments, fusion and the five phases, ranks, the three living bars
//! and the resources, the daos, curses, intents and domains, supports and keystones, the shared pool of skills and the
//! loadout a player builds from it, one blow's twelve steps, and what the field-bound stats do on plain geometry
//! (projectiles, areas, pushes, runs, stored uses, devices and minions). Pure and deterministic, with no fight in
//! it: a fight (the duel, or its Bevy port) builds blows here and applies what comes back. The
//! design is `docs/specs/2026-11-17-damage-system-design.md`, built on the research in
//! `docs/specs/2026-11-17-skill-system-design.md` and `docs/specs/2026-11-17-skill-system-rules.md`.

#![forbid(unsafe_code)]

pub mod ailment;
pub mod aura;
pub mod build;
pub mod card;
pub mod charge;
pub mod cultivator;
pub mod curse;
pub mod dao;
pub mod device;
pub mod domain;
pub mod dot;
pub mod element;
pub mod event;
pub mod gem;
pub mod grade;
pub mod hit;
pub mod item;
pub mod keystone;
pub mod loadout;
pub mod motion;
pub mod passive;
pub mod play;
pub mod projectile;
pub mod sheet;
pub mod stat;
pub mod support;
pub mod tag;

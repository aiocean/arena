//! The damage system: ten kinds of damage, their ailments, fusion and the five phases, ranks, the three living bars
//! and the resources, the daos, curses, intents and domains, supports and keystones, the shared pool of skills and the
//! loadout a player builds from it, and one blow's twelve steps. Pure and deterministic; the fight calls it. The
//! design is `docs/specs/2026-11-17-damage-system-design.md`, built on the research in
//! `docs/specs/2026-11-17-skill-system-design.md` and `docs/specs/2026-11-17-skill-system-rules.md`.

pub mod ailment;
pub mod aura;
pub mod build;
pub mod card;
pub mod cultivator;
pub mod curse;
pub mod dao;
pub mod domain;
pub mod element;
pub mod gem;
pub mod grade;
pub mod hit;
pub mod keystone;
pub mod loadout;
pub mod support;
pub mod tag;

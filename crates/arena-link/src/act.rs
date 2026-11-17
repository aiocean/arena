//! `Act`: a decision on the wire. It mirrors [`arena_mind::Choice`] exactly, but as a plain externally tagged enum so
//! postcard can carry it (ADR-0095's rule: a type on the link has no internally tagged enum, no `flatten`, no
//! `deny_unknown_fields`). `Choice`'s own serde is internally tagged for the page's JSON, so it cannot ride postcard;
//! `Act` is the postcard form, and the two convert losslessly. The round-trip test in `tests/act.rs` pins that every
//! `Choice` variant survives the trip, so a new variant that is not mirrored here fails the build's tests.

use arena_mind::Choice;
use serde::{Deserialize, Serialize};

/// What a fighter does on a tick, as carried on the link. One-to-one with [`arena_mind::Choice`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Act {
    /// Walk: `dir` -1 left, 1 right.
    Move {
        dir: f64,
    },
    Jump,
    Duck {
        hold: f64,
    },
    Guard {
        hold: f64,
    },
    /// Fly at `height` over the ground; 0 lands.
    Fly {
        height: f64,
    },
    Meditate,
    Wait,
    /// Use the skill of that name; `target` is an x for areas and blinks.
    Cast {
        skill: String,
        target: Option<f64>,
    },
}

impl From<&Choice> for Act {
    fn from(c: &Choice) -> Act {
        match c {
            Choice::Move { dir } => Act::Move { dir: *dir },
            Choice::Jump => Act::Jump,
            Choice::Duck { hold } => Act::Duck { hold: *hold },
            Choice::Guard { hold } => Act::Guard { hold: *hold },
            Choice::Fly { height } => Act::Fly { height: *height },
            Choice::Meditate => Act::Meditate,
            Choice::Wait => Act::Wait,
            Choice::Cast { skill, target } => Act::Cast {
                skill: skill.clone(),
                target: *target,
            },
        }
    }
}

impl From<Act> for Choice {
    fn from(a: Act) -> Choice {
        match a {
            Act::Move { dir } => Choice::Move { dir },
            Act::Jump => Choice::Jump,
            Act::Duck { hold } => Choice::Duck { hold },
            Act::Guard { hold } => Choice::Guard { hold },
            Act::Fly { height } => Choice::Fly { height },
            Act::Meditate => Choice::Meditate,
            Act::Wait => Choice::Wait,
            Act::Cast { skill, target } => Choice::Cast { skill, target },
        }
    }
}

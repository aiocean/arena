//! Who decides for a fighter: a seat. The duel asks it each tick and tells it how its skills went; the rules never
//! call it themselves (they queue what to tell in `State::told`).

use crate::data::{TEMPERAMENT, UNIT, school};
use crate::fighter::State;
use crate::sense::{chosen, sense_of, skill_of};
use crate::style::{Style, tag};
use arena_mind::policy::{ACTIONS, OBS, PolicyMind};
use arena_mind::{BodySize, Choice, Mind, Outcome, Posture, Profile, Thought};
use std::cell::RefCell;
use std::rc::Rc;

/// A fighter's decider.
pub trait Seat {
    /// What fighter `me` does this tick, if it decides anything.
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice>;
    fn tell(&mut self, outcome: &Outcome);
    /// A new round.
    fn reset(&mut self);
    /// Think nothing for `seconds`.
    fn hold(&mut self, seconds: f64);
    fn posture(&self) -> Option<Posture> {
        None
    }
    /// Whether it gives up the round now: a bot module that trapped or ran out of fuel (`BotSeat`).
    fn forfeit(&self) -> bool {
        false
    }
}

/// Makes fighter `me`'s seat once the fighters exist (a brain is told both fighters' spells).
pub type MakeSeat = Box<dyn FnOnce(&State, usize) -> Box<dyn Seat>>;

/// pixfx-mind's brain with its school's skills and temperament.
pub struct Brain {
    mind: Mind,
    /// Its last thought.
    pub thought: Option<Thought>,
}

impl Brain {
    pub fn new(state: &State, me: usize) -> Brain {
        let f = &state.fighters[me];
        let d = school(f.school);
        let t = TEMPERAMENT;
        let profile = Profile {
            aggression: d.aggression,
            tempo: d.tempo,
            spread: t.spread,
            range: d.range,
            dry: d.dry,
            sky: d.sky,
            risk: t.risk,
            floor: t.floor,
            lead: t.lead,
            prior: t.prior,
            reaction: t.reaction,
            think: t.think,
            posture_scale: d.postures,
        };
        // in the engine's pixels, as the mind sees everything
        let spec = &f.body.spec;
        let body = BodySize {
            half_width: spec.half_width / UNIT,
            chest: spec.chest / UNIT,
            body: spec.body / UNIT,
            duck: spec.duck / UNIT,
            walk: spec.walk / UNIT,
        };
        let skills = f.spells.iter().map(|n| skill_of(n)).collect();
        let mut mind = Mind::new(
            profile,
            body,
            skills,
            state.seed.wrapping_mul(2).wrapping_add(me as u32),
        );
        mind.meet(
            state.fighters[1 - me]
                .spells
                .iter()
                .map(|n| skill_of(n))
                .collect(),
        );
        Brain {
            mind,
            thought: None,
        }
    }

    /// A maker for `Duel::new`.
    pub fn make() -> MakeSeat {
        Box::new(|state, me| Box::new(Brain::new(state, me)))
    }
}

impl Seat for Brain {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        let thought = self.mind.think(&sense_of(state, me), dt)?;
        let choice = chosen(thought.choice.clone());
        self.thought = Some(thought);
        Some(choice)
    }

    fn tell(&mut self, outcome: &Outcome) {
        self.mind.tell(outcome);
    }

    fn reset(&mut self) {
        self.mind.reset();
    }

    fn hold(&mut self, seconds: f64) {
        self.mind.hold(seconds);
    }

    fn posture(&self) -> Option<Posture> {
        Some(self.mind.posture())
    }
}

/// Fighter `me`'s policy mind in `style`: its spells, its tag, the brain's reaction and think times, dice of its own.
pub fn policy_mind(state: &State, me: usize, style: Style) -> PolicyMind {
    let f = &state.fighters[me];
    let skills = f.spells.iter().map(|n| skill_of(n)).collect();
    let t = TEMPERAMENT;
    let seed = state.seed.wrapping_mul(2).wrapping_add(me as u32) ^ 0x9e37_79b9;
    PolicyMind::new(skills, tag(f.school, style), t.reaction, t.think, seed)
}

/// Why a bot module's call failed: it trapped or ran out of fuel. It loses the round it happened in.
#[derive(Clone, Debug, PartialEq)]
pub struct Fault(pub String);

/// A bot that answers the decisions a `PolicyMind` makes due: the duel's side of a module of the `arena-bot-1` ABI
/// (`docs/bot.md`, "The bot module"), so the rules do not depend on a wasm host (`arena-bot` runs one).
pub trait Decider {
    /// A new fight of the match; `seed` is the bot's own dice for it.
    fn start(&mut self, seed: u32) -> Result<(), Fault>;
    /// A new round.
    fn round(&mut self) -> Result<(), Fault>;
    /// The act for a due decision: an index into the acts; any other number is a wait.
    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault>;
}

/// One bot's instance, shared by every fight of a match: it remembers across them.
pub type SharedDecider = Rc<RefCell<dyn Decider>>;

/// A bot module in a fighter's seat: the policy's clock, view and dice (`PolicyMind`) around the module's answers.
/// A fault gives up the round it happened in; one in `start` gives up the fight's first round.
pub struct BotSeat {
    decider: SharedDecider,
    pub mind: PolicyMind,
    started: bool,
    forfeit: bool,
}

impl BotSeat {
    /// Fighter `me`'s seat for `decider`, playing `style`; the fight starts for the bot here.
    pub fn new(decider: SharedDecider, state: &State, me: usize, style: Style) -> BotSeat {
        let seed = state.seed.wrapping_mul(2).wrapping_add(me as u32) ^ 0x7f4a_7c15;
        let started = decider.borrow_mut().start(seed).is_ok();
        BotSeat {
            decider,
            mind: policy_mind(state, me, style),
            started,
            forfeit: false,
        }
    }

    /// A maker for `Duel::new`: the bot in `style`, or the style the fight drew for the fighter.
    pub fn make(decider: SharedDecider, style: Option<Style>) -> MakeSeat {
        Box::new(move |state, me| {
            let style = style.unwrap_or(state.fighters[me].style);
            Box::new(BotSeat::new(decider, state, me, style))
        })
    }
}

impl Seat for BotSeat {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        if self.forfeit {
            return None;
        }
        let sense = sense_of(state, me);
        let now = self.mind.begin(&sense, dt);
        if let Some(d) = self.mind.due(&sense) {
            match self.decider.borrow_mut().decide(&d.obs, &d.legal, d.u) {
                Ok(act) => self.mind.queue(
                    usize::try_from(act)
                        .ok()
                        .filter(|&a| a < ACTIONS)
                        .unwrap_or(0),
                ),
                Err(_) => self.forfeit = true,
            }
        }
        now.map(chosen)
    }

    fn tell(&mut self, _outcome: &Outcome) {}

    fn reset(&mut self) {
        self.mind.reset();
        // a fight whose start failed gives up its first round without asking the bot
        self.forfeit = !std::mem::replace(&mut self.started, true)
            || self.decider.borrow_mut().round().is_err();
    }

    fn hold(&mut self, seconds: f64) {
        self.mind.hold(seconds);
    }

    fn forfeit(&self) -> bool {
        self.forfeit
    }
}

/// What a scripted seat runs: the state and the fighter it decides for, to a choice.
pub type Decide = Box<dyn FnMut(&State, usize) -> Option<Choice>>;

/// A seat run by a closure, for tests: told nothing, never held.
pub struct Script(pub Decide);

impl Seat for Script {
    fn think(&mut self, state: &State, me: usize, _dt: f64) -> Option<Choice> {
        (self.0)(state, me)
    }

    fn tell(&mut self, _outcome: &Outcome) {}

    fn reset(&mut self) {}

    fn hold(&mut self, _seconds: f64) {}
}

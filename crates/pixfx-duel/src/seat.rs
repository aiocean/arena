//! Who decides for a fighter: a seat. The duel asks it each tick and tells it how its skills went; the rules never
//! call it themselves (they queue what to tell in `State::told`).

use crate::data::{TEMPERAMENT, school};
use crate::fighter::State;
use crate::sense::{sense_of, skill_of};
use crate::style::{Style, tag};
use pixfx_mind::policy::{ACTIONS, Net, NetError, OBS, PolicyMind};
use pixfx_mind::{BodySize, Choice, Mind, Outcome, Posture, Profile, Thought};
use std::fmt;
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
    /// What stopped this seat for good, if anything did.
    fn fault(&self) -> Option<&Fault> {
        None
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
        let spec = &f.body.spec;
        let body = BodySize {
            half_width: spec.half_width,
            chest: spec.chest,
            body: spec.body,
            duck: spec.duck,
            walk: spec.walk,
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
        let choice = thought.choice.clone();
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

/// The trained policy (`weights/policy.bin`), compiled in for native callers: the tests and the ladder. The page
/// fetches the same file beside the wasm and hands its bytes to the wasm `Policy`.
pub fn shipped() -> Result<Rc<Net>, NetError> {
    Net::load(include_bytes!("../weights/policy.bin")).map(Rc::new)
}

/// Fighter `me`'s policy mind in `style`: its spells, its tag, the brain's reaction and think times, dice of its own.
pub fn policy_mind(state: &State, me: usize, style: Style) -> PolicyMind {
    let f = &state.fighters[me];
    let skills = f.spells.iter().map(|n| skill_of(n)).collect();
    let t = TEMPERAMENT;
    PolicyMind::new(
        skills,
        tag(f.school, style),
        t.reaction,
        t.think,
        policy_seed(state, me),
    )
}

/// Fighter `me`'s dice seed for a policy mind, and for a bot's `start`.
fn policy_seed(state: &State, me: usize) -> u32 {
    state.seed.wrapping_mul(2).wrapping_add(me as u32) ^ 0x9e37_79b9
}

/// The trained policy, in the fighter's style, seeing as late and thinking as often as the brain.
pub struct PolicySeat {
    net: Rc<Net>,
    pub mind: PolicyMind,
}

impl PolicySeat {
    /// Fighter `me`'s policy seat, playing `style`.
    pub fn new(net: Rc<Net>, state: &State, me: usize, style: Style) -> PolicySeat {
        PolicySeat {
            net,
            mind: policy_mind(state, me, style),
        }
    }

    /// A maker for `Duel::new`: the policy in the style the fight drew for the fighter.
    pub fn make(net: Rc<Net>) -> MakeSeat {
        Box::new(move |state, me| {
            let style = state.fighters[me].style;
            Box::new(PolicySeat::new(net, state, me, style))
        })
    }

    /// A maker for `Duel::new`: the policy in `style`, whatever the fight drew.
    pub fn with_style(net: Rc<Net>, style: Style) -> MakeSeat {
        Box::new(move |state, me| Box::new(PolicySeat::new(net, state, me, style)))
    }
}

impl Seat for PolicySeat {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        self.mind.think(&self.net, &sense_of(state, me), dt)
    }

    fn tell(&mut self, _outcome: &Outcome) {}

    fn reset(&mut self) {
        self.mind.reset();
    }

    fn hold(&mut self, seconds: f64) {
        self.mind.hold(seconds);
    }
}

/// Why a bot stopped: it trapped or ran out of fuel.
#[derive(Clone, Debug, PartialEq)]
pub enum Fault {
    Trap(String),
    OutOfFuel,
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Trap(why) => write!(f, "trap: {why}"),
            Fault::OutOfFuel => f.write_str("out of fuel"),
        }
    }
}

/// What decides a bot's acts: the network's place, run by whoever hosts the bot. `decide` answers one due decision
/// with an act index (outside 0..ACTIONS is a wait).
pub trait Decider {
    /// Once, before the first round, with the bot's dice seed.
    fn start(&mut self, seed: u32) -> Result<(), Fault>;
    /// Each round after the first.
    fn round(&mut self) -> Result<(), Fault>;
    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault>;
}

/// A bot: the policy mind's clock, view and dice, with a `Decider` for the network. A fault stops it for good.
pub struct BotSeat {
    mind: PolicyMind,
    decider: Box<dyn Decider>,
    seed: u32,
    started: bool,
    fault: Option<Fault>,
}

impl BotSeat {
    /// A maker for `Duel::new`: a fresh decider from `new`, in `style` (`None`: the style the fight drew).
    pub fn make(new: Rc<dyn Fn() -> Box<dyn Decider>>, style: Option<Style>) -> MakeSeat {
        Box::new(move |state, me| {
            let style = style.unwrap_or(state.fighters[me].style);
            Box::new(BotSeat {
                mind: policy_mind(state, me, style),
                decider: new(),
                seed: policy_seed(state, me),
                started: false,
                fault: None,
            })
        })
    }
}

impl Seat for BotSeat {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        if self.fault.is_some() {
            return None;
        }
        let sense = sense_of(state, me);
        let now = self.mind.begin(&sense, dt);
        if let Some(due) = self.mind.due(&sense) {
            match self.decider.decide(&due.obs, &due.legal, due.u) {
                Ok(act) => {
                    let act = usize::try_from(act).ok().filter(|&a| a < ACTIONS);
                    self.mind.queue(act.unwrap_or(0));
                }
                Err(fault) => self.fault = Some(fault),
            }
        }
        now
    }

    fn tell(&mut self, _outcome: &Outcome) {}

    fn reset(&mut self) {
        self.mind.reset();
        if self.fault.is_some() {
            return;
        }
        let ran = if self.started {
            self.decider.round()
        } else {
            self.started = true;
            self.decider.start(self.seed)
        };
        if let Err(fault) = ran {
            self.fault = Some(fault);
        }
    }

    fn hold(&mut self, seconds: f64) {
        self.mind.hold(seconds);
    }

    fn fault(&self) -> Option<&Fault> {
        self.fault.as_ref()
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

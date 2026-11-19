//! Who decides for a fighter: a seat. The duel asks it each tick and tells it how its skills went; the rules never
//! call it themselves (they queue what to tell in `State::told`).

use crate::data::{MIND, TEMPERAMENT, UNIT};
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

/// arena-mind's brain with its fighter's skills and the one temperament both seats share.
pub struct Brain {
    mind: Mind,
    /// Its last thought.
    pub thought: Option<Thought>,
}

impl Brain {
    pub fn new(state: &State, me: usize) -> Brain {
        let f = &state.fighters[me];
        let d = MIND;
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
    PolicyMind::new(
        skills,
        tag(state.fighters[me].look, style),
        t.reaction,
        t.think,
        seed,
    )
}

/// Why a bot module's call failed: it trapped or ran out of fuel. It loses the round it happened in.
#[derive(Clone, Debug, PartialEq)]
pub struct Fault(pub String);

/// A bot that answers the decisions a `PolicyMind` makes due: the duel's side of a module of the `arena-bot-4` ABI
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

/// A corner's decisions over a match: each is the choice it made (`Some`) or nothing, at a fighting tick. A tick counts
/// this seat's `think` calls from the match's first, which the rules make once per fighting tick, so the same count on
/// a server that fights and a page that replays lines them up (`arena_link`, `docs/specs/2026-11-17-live-matches-design.md`).
#[derive(Clone, Debug, Default)]
pub struct Log {
    /// `(tick, choice)` for every tick this corner decided something, in tick order.
    pub decisions: Vec<(u32, Choice)>,
    /// The first tick this corner gave up a round (a bot module's fault), if any.
    pub forfeit: Option<u32>,
}

/// A shared handle on a [`Recorder`]'s log, drained by the thing driving the fight.
pub type Recording = Rc<RefCell<Log>>;

/// A seat that plays a real seat and writes down what it decided, so a match can be replayed from its seed and this log
/// alone. It counts fighting ticks (one `think` a tick) and records each decision and the tick a forfeit began.
pub struct Recorder {
    inner: Box<dyn Seat>,
    tick: u32,
    forfeited: bool,
    log: Recording,
}

impl Recorder {
    /// Wrap `inner` (the corner `me`'s real seat); its log fills through the returned handle.
    pub fn new(inner: Box<dyn Seat>) -> (Recorder, Recording) {
        let log = Rc::new(RefCell::new(Log::default()));
        let rec = Recorder {
            inner,
            tick: 0,
            forfeited: false,
            log: Rc::clone(&log),
        };
        (rec, log)
    }
}

impl Seat for Recorder {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        let choice = self.inner.think(state, me, dt);
        let t = self.tick;
        self.tick += 1;
        if let Some(c) = &choice {
            self.log.borrow_mut().decisions.push((t, c.clone()));
        }
        // a forfeit shows the tick it first turns true; `think` runs every fighting tick, so this edge is not missed
        if !self.forfeited && self.inner.forfeit() {
            self.forfeited = true;
            self.log.borrow_mut().forfeit = Some(t);
        }
        choice
    }

    fn tell(&mut self, outcome: &Outcome) {
        self.inner.tell(outcome);
    }

    fn reset(&mut self) {
        self.inner.reset();
    }

    fn hold(&mut self, seconds: f64) {
        self.inner.hold(seconds);
    }

    fn posture(&self) -> Option<Posture> {
        self.inner.posture()
    }

    fn forfeit(&self) -> bool {
        self.inner.forfeit()
    }
}

/// A seat that replays a corner's [`Log`]: no mind, no bot, it hands back the choice recorded for each fighting tick
/// and forfeits from the tick the log says. Given the same seed, two of these step the fight the server fought.
///
/// The log is shared: a page watching a match live keeps appending to it as decisions arrive (`Replay::live`), and the
/// seat reads on from where it left off. The page only steps the fight as far as the decisions it holds reach
/// (`arena_link::Frame::Through`), so a decision is never asked for before it has come.
pub struct Replay {
    source: Recording,
    next: usize,
    tick: u32,
    forfeited: bool,
}

impl Replay {
    /// Replay a whole log known up front.
    pub fn new(log: Log) -> Replay {
        Replay::live(Rc::new(RefCell::new(log)))
    }

    /// Replay a log that keeps growing as the match comes in.
    pub fn live(source: Recording) -> Replay {
        Replay {
            source,
            next: 0,
            tick: 0,
            forfeited: false,
        }
    }
}

impl Seat for Replay {
    fn think(&mut self, _state: &State, _me: usize, _dt: f64) -> Option<Choice> {
        let t = self.tick;
        self.tick += 1;
        let log = self.source.borrow();
        if log.forfeit.is_some_and(|at| t >= at) {
            self.forfeited = true;
        }
        match log.decisions.get(self.next) {
            Some((at, c)) if *at == t => {
                self.next += 1;
                Some(c.clone())
            }
            _ => None,
        }
    }

    fn tell(&mut self, _outcome: &Outcome) {}

    fn reset(&mut self) {}

    fn hold(&mut self, _seconds: f64) {}

    fn forfeit(&self) -> bool {
        self.forfeited
    }
}

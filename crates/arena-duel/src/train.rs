//! The training environment: the trainer's learner in one corner of a fresh duel, an opponent from the league in the
//! other, one round a episode. The learner decides through a `PolicyMind` with the timing the page plays, so what
//! it learns is what it does on the page.
//!
//! Reward, per decision: +1 for the round won and -1 for the round lost (a round cut at `limit` seconds is lost too,
//! so stalling never pays); `shaping` x (dealt - taken) / 100, which the trainer anneals to 0; minus
//! `STALL` a decision once `QUIET` seconds pass with no damage either way; minus `REPEAT` when one act (other than
//! waiting) is chosen `RUN` times in a row; minus `HABIT` when the act chosen is already `SHARE` or more of the last
//! `HABITS` decisions; minus the trainer's `whiff` cost for each damaging cast that lands nothing (no hit, not guarded,
//! not turned aside).
//!
//! It also counts the learner's casts by kind (`Casts`): how many, how many began out of reach (a blade, blast or dash
//! whose reach does not get to the foe's hurtboxes), how many landed nothing, and the damage they dealt. Only a skill
//! cast as its motion strikes for this: a device's or a domain's blows come later, from the field.

use crate::duel::{Duel, DuelConfig, TICK};
use crate::fighter::State;
use crate::first_mind::FirstMind;
use crate::scripted::Scripted;
use crate::seat::{BotSeat, Brain, MakeSeat, Seat, SharedDecider, policy_mind};
use crate::sense::{chosen, sense_of};
use crate::style::Style;
use arena_mind::policy::{ACTIONS, Due, OBS, PolicyMind};
use arena_mind::{Choice, Outcome};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// The penalty a decision for a fight gone quiet, and how long it must be quiet.
const STALL: f32 = 0.002;
const QUIET: f64 = 6.0;
/// The penalty for choosing one act `RUN` times in a row.
const REPEAT: f32 = 0.002;
const RUN: u32 = 6;
/// What choosing an act that already fills `SHARE` of the last `HABITS` decisions (about 5 s of play) costs.
const HABIT: f32 = 0.004;
const HABITS: usize = 40;
const SHARE: f64 = 0.4;

/// The kinds of cast `Casts` counts, in `data::Kind` order.
pub const CAST_KINDS: usize = 11;

/// The learner's casts of one kind: begun, begun out of reach, landed nothing, and the damage they dealt (shares of
/// the foe's full life, 0 to 100 a life).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CastLog {
    pub casts: u32,
    pub out_of_reach: u32,
    pub whiffs: u32,
    pub damage: f64,
}

/// The learner's casts by kind (`data::Kind` order).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Casts(pub [CastLog; CAST_KINDS]);

impl Casts {
    pub fn add(&mut self, o: &Casts) {
        for (a, b) in self.0.iter_mut().zip(o.0.iter()) {
            a.casts += b.casts;
            a.out_of_reach += b.out_of_reach;
            a.whiffs += b.whiffs;
            a.damage += b.damage;
        }
    }
}

/// The learner's cast in progress: its skill, its kind's index, whether it deals damage, and what it has done.
#[derive(Clone, Copy, Debug)]
struct Open {
    spell: &'static str,
    kind: usize,
    strikes: bool,
    landed: bool,
}

/// Who the learner fights.
#[derive(Clone)]
pub enum Opponent {
    Brain,
    First,
    Aggressive,
    Turtle,
    /// A bot (the learner's past or present self as ONNX, or a module) in `EnvConfig::foe_style`.
    Bot(SharedDecider),
}

impl std::fmt::Debug for Opponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Opponent::Brain => "Brain",
            Opponent::First => "First",
            Opponent::Aggressive => "Aggressive",
            Opponent::Turtle => "Turtle",
            Opponent::Bot(_) => "Bot",
        })
    }
}

/// One episode: the fight's seed, the learner's corner (0 Thunder, 1 Frost) and style, the opponent and its style,
/// the seconds a round may last, and the effects in the registry (the page's whole registry: some rules play only
/// when their effect is there).
#[derive(Clone, Debug)]
pub struct EnvConfig {
    pub seed: u32,
    pub learner: usize,
    pub style: Style,
    pub foe_style: Style,
    pub opponent: Opponent,
    pub limit: f64,
    pub effects: Vec<String>,
}

/// What one decision earned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub reward: f32,
    pub done: bool,
    /// At a knockout, whether the learner won; `None` while the round goes on or when it was cut.
    pub won: Option<bool>,
    /// The round's length so far, seconds.
    pub length: f64,
}

/// What passes between the env and its learner's seat.
#[derive(Default)]
struct Wire {
    ask: Option<Due>,
    answer: Option<usize>,
}

/// The learner's seat: a policy mind whose decisions the trainer answers.
struct Learner {
    mind: PolicyMind,
    wire: Rc<RefCell<Wire>>,
}

impl Seat for Learner {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        let sense = sense_of(state, me);
        let mut wire = self.wire.borrow_mut();
        if let Some(a) = wire.answer.take() {
            self.mind.queue(a);
        }
        let now = self.mind.begin(&sense, dt);
        if let Some(due) = self.mind.due(&sense) {
            wire.ask = Some(due);
        }
        now.map(chosen)
    }

    fn tell(&mut self, _outcome: &Outcome) {}

    fn reset(&mut self) {
        self.mind.reset();
    }

    fn hold(&mut self, seconds: f64) {
        self.mind.hold(seconds);
    }
}

/// A training episode.
pub struct Env {
    duel: Duel,
    me: usize,
    limit: f64,
    wire: Rc<RefCell<Wire>>,
    /// The decision the trainer answers next.
    due: Due,
    quiet: f64,
    last: Option<usize>,
    run: u32,
    /// The last `HABITS` acts it chose.
    recent: VecDeque<usize>,
    done: bool,
    /// The cast under way, the casts ended whose shots or later blows are still out, the learner's casts since the
    /// trainer last took them, and the whiffs not yet paid for.
    open: Option<Open>,
    trailing: Vec<Open>,
    casts: Casts,
    whiffed: u32,
}

impl Env {
    /// A fresh duel, run to the learner's first decision.
    pub fn new(cfg: EnvConfig) -> Env {
        let wire = Rc::new(RefCell::new(Wire::default()));
        let (style, w) = (cfg.style, Rc::clone(&wire));
        let learner: MakeSeat = Box::new(move |state, me| {
            Box::new(Learner {
                mind: policy_mind(state, me, style),
                wire: w,
            })
        });
        let foe: MakeSeat = match cfg.opponent {
            Opponent::Brain => Brain::make(),
            Opponent::First => FirstMind::make(),
            Opponent::Aggressive => Scripted::aggressive(),
            Opponent::Turtle => Scripted::turtle(),
            Opponent::Bot(bot) => BotSeat::make(bot, Some(cfg.foe_style)),
        };
        let me = cfg.learner.min(1);
        let seats = if me == 0 {
            [learner, foe]
        } else {
            [foe, learner]
        };
        let config = DuelConfig {
            seed: cfg.seed,
            left: 0.0,
            right: 880.0,
            ground: 600.0,
            sky: 384.0,
            effects: cfg.effects,
        };
        let blank = Due {
            obs: [0.0; OBS],
            legal: [false; ACTIONS],
            u: 0.0,
        };
        let mut env = Env {
            duel: Duel::new_with_looks(config, seats, crate::ladder::looks_of(cfg.seed)),
            me,
            limit: cfg.limit,
            wire,
            due: blank,
            quiet: 0.0,
            last: None,
            run: 0,
            recent: VecDeque::with_capacity(HABITS + 1),
            done: false,
            open: None,
            trailing: Vec::new(),
            casts: Casts::default(),
            whiffed: 0,
        };
        env.advance();
        env
    }

    /// What the learner sees now and what it may do.
    pub fn observe(&self) -> (&[f32; OBS], &[bool; ACTIONS]) {
        (&self.due.obs, &self.due.legal)
    }

    /// Answer the learner's decision with `action` (an act it may not do is a wait), run the duel to its next
    /// decision or the round's end, and pay for it. `shaping` weighs the damage term (the trainer anneals it to 0).
    pub fn step(&mut self, action: usize, shaping: f32) -> Step {
        self.step_with(action, shaping, 0.0)
    }

    /// The learner's casts since this was last called, by kind.
    pub fn take_casts(&mut self) -> Casts {
        core::mem::take(&mut self.casts)
    }

    /// As `step`, with `whiff` taken from the reward for each damaging cast that landed nothing.
    pub fn step_with(&mut self, action: usize, shaping: f32, whiff: f32) -> Step {
        let length = self.duel.state().time;
        if self.done {
            return Step {
                reward: 0.0,
                done: true,
                won: None,
                length,
            };
        }
        let act = if self.due.legal.get(action).copied().unwrap_or(false) {
            action
        } else {
            0
        };
        let mut reward = 0.0;
        if act != 0 && self.last == Some(act) {
            self.run += 1;
        } else {
            self.run = 1;
        }
        self.last = Some(act);
        if act != 0 && self.run >= RUN {
            reward -= REPEAT;
        }
        if self.quiet >= QUIET {
            reward -= STALL;
        }
        let same = self.recent.iter().filter(|&&a| a == act).count();
        if self.recent.len() == HABITS && same as f64 >= SHARE * HABITS as f64 {
            reward -= HABIT;
        }
        self.recent.push_back(act);
        if self.recent.len() > HABITS {
            self.recent.pop_front();
        }
        self.wire.borrow_mut().answer = Some(act);
        let (dealt, taken, won) = self.advance();
        reward += shaping * ((dealt - taken) / 100.0) as f32;
        reward -= whiff * core::mem::take(&mut self.whiffed) as f32;
        let length = self.duel.state().time;
        let cut = won.is_none() && length >= self.limit;
        if won.is_some() || cut {
            self.done = true;
            reward += if won == Some(true) { 1.0 } else { -1.0 };
        }
        Step {
            reward,
            done: self.done,
            won,
            length,
        }
    }

    /// Run whole ticks until the learner's next decision, the round's end or the limit: the damage it dealt and
    /// took on the way, and, at a knockout, whether it won.
    fn advance(&mut self) -> (f64, f64, Option<bool>) {
        let (me, foe) = (self.me, 1 - self.me);
        let (mut dealt, mut taken) = (0.0, 0.0);
        loop {
            let before = [
                self.duel.state().fighters[0].hp(),
                self.duel.state().fighters[1].hp(),
            ];
            let out = self.duel.step(TICK);
            self.follow(
                &out,
                (before[foe] - self.duel.state().fighters[foe].hp()).max(0.0),
            );
            let f = &self.duel.state().fighters;
            let (d, t) = (
                (before[foe] - f[foe].hp()).max(0.0),
                (before[me] - f[me].hp()).max(0.0),
            );
            dealt += d;
            taken += t;
            self.quiet = if d > 0.0 || t > 0.0 {
                0.0
            } else {
                self.quiet + TICK
            };
            if let Some(w) = self.duel.state().winner {
                return (dealt, taken, Some(w == me));
            }
            if self.duel.state().time >= self.limit {
                return (dealt, taken, None);
            }
            if let Some(due) = self.wire.borrow_mut().ask.take() {
                self.due = due;
                return (dealt, taken, None);
            }
        }
    }

    /// Follow the learner's casts over one tick of `out`, `dealt` of the foe's life taken in it: a new cast opens
    /// (counted, out of reach or not), its blows land or are stopped, and a cast that has ended, with none of its shots
    /// or later blows still out, having landed nothing is a whiff.
    fn follow(&mut self, out: &[crate::out::Out], dealt: f64) {
        use crate::data::{Kind, spell};
        use crate::out::{Note, Out};
        let (me, foe) = (self.me, 1 - self.me);
        let state = self.duel.state();
        let act = state.fighters[me].act;
        let fresh = act.filter(|a| a.phase == crate::fighter::Phase::Windup && a.t <= TICK * 1.5);
        let ended = match (&self.open, act) {
            (Some(o), Some(a)) => fresh.is_some() || a.spell != o.spell,
            (Some(_), None) => true,
            _ => false,
        };
        if ended && let Some(o) = self.open.take() {
            self.trailing.push(o);
        }
        if let Some(a) = fresh
            && self.open.is_none()
            && let Some(sp) = spell(a.spell)
        {
            // a skill cast as its motion: a device's or a domain's blows come later, from the field
            let ks = state.fighters[me].kit.get(a.spell);
            let motion = ks.is_none_or(|k| k.way == crate::kit::Way::Motion);
            let mo = ks.map_or(sp, |k| k.motion);
            let k = mo.kind;
            let kind = k as usize;
            let strikes = motion
                && matches!(
                    k,
                    Kind::Melee
                        | Kind::Dash
                        | Kind::Blast
                        | Kind::Bolt
                        | Kind::Orb
                        | Kind::Zone
                        | Kind::Void
                );
            // from the caster's x to the nearest edge of the foe's hurtboxes
            let x = state.fighters[me].body.x;
            let gap = crate::frames::hurtboxes(state, foe)
                .iter()
                .map(|h| (h[0] - x).max(x - h[2]).max(0.0))
                .fold(f64::INFINITY, f64::min);
            let reach = match k {
                Kind::Melee | Kind::Blast if motion => mo.range,
                Kind::Dash if motion => mo.distance,
                _ => None,
            };
            let log = &mut self.casts.0[kind.min(CAST_KINDS - 1)];
            log.casts += 1;
            if reach.is_some_and(|r| gap > r) {
                log.out_of_reach += 1;
            }
            self.open = Some(Open {
                spell: a.spell,
                kind: kind.min(CAST_KINDS - 1),
                strikes,
                landed: false,
            });
        }
        let mut credited = false;
        for o in self.open.iter_mut().chain(self.trailing.iter_mut()) {
            let hit = out
                .iter()
                .any(|e| matches!(e, Out::Hit { by, spell, .. } if *by == me && *spell == o.spell));
            let stopped = state
                .notes
                .iter()
                .any(|(_, n)| matches!(n, Note::Stopped { by, spell, .. } if *by == me && *spell == o.spell));
            if hit || stopped {
                o.landed = true;
            }
            // the tick's damage goes to one cast of the spell that hit
            if hit && !credited {
                self.casts.0[o.kind].damage += dealt;
                credited = true;
            }
        }
        let out_still = |name: &str| {
            state.shots.iter().any(|s| s.owner == me && s.name == name)
                || state
                    .pending
                    .iter()
                    .any(|p| p.payload.by == me && p.payload.skill == name)
        };
        let (done, keep): (Vec<Open>, Vec<Open>) = core::mem::take(&mut self.trailing)
            .into_iter()
            .partition(|o| o.landed || !out_still(o.spell));
        self.trailing = keep;
        for o in done {
            if o.strikes && !o.landed {
                self.casts.0[o.kind].whiffs += 1;
                self.whiffed += 1;
            }
        }
    }
}

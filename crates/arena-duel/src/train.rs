//! The training environment: the trainer's learner in one corner of a fresh duel, an opponent from the league in the
//! other, one round a episode. The learner decides through a `PolicyMind` with the timing the page plays, so what
//! it learns is what it does on the page.
//!
//! Reward, per decision: +1 for the round won and -1 for the round lost (a round cut at `limit` seconds is lost too,
//! so stalling never pays); `shaping` x (dealt - taken) / 100, which the trainer anneals to 0; minus
//! `STALL` a decision once `QUIET` seconds pass with no damage either way; minus `REPEAT` when one act (other than
//! waiting) is chosen `RUN` times in a row; minus `HABIT` when the act chosen is already `SHARE` or more of the last
//! `HABITS` decisions; minus the trainer's `whiff` cost for each damaging cast that lands nothing (no hit, not guarded,
//! not turned aside), and its `far` cost for each of those that began with the foe out of its reach; plus its `variety`
//! x the share of a life the learner's actives (not the basics) took with their blows; minus its `idle` cost for a
//! decision that could cast (a cast is legal: ready and paid for) and does something else; plus its `land` for each
//! damaging cast that lands (`Pay`).
//!
//! It also counts the learner's casts by kind (`Casts`): how many, how many began out of reach (a blade, blast or dash
//! whose reach does not get to the foe's hurtboxes), how many landed nothing, and the damage they dealt. Only a skill
//! cast as its motion strikes for this: a device's or a domain's blows come later, from the field.

use crate::duel::{Duel, DuelConfig, TICK};
use crate::fighter::State;
use crate::first_mind::FirstMind;
use crate::scripted::Scripted;
use crate::seat::{BotSeat, Brain, MakeSeat, Seat, SharedDecider, policy_mind};
use crate::sense::{chosen, fill, glance_of, sense_of};
use crate::style::Style;
use arena_mind::Delayed;
use arena_mind::policy::{ACTIONS, Act, Due, OBS, PolicyMind, TAG, observe};
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
/// A move chosen again within this many seconds, with another between, is a flicker (`Pay::flicker`).
const FLICKER: f64 = 1.0;
/// A meditation left within this many seconds of choosing it gained nothing (0.4 s down, a whole second sat):
/// `Pay::short_sit`.
const SIT: f64 = 1.4;

/// The kinds of cast `Casts` counts, in `data::Kind` order.
pub const CAST_KINDS: usize = 9;

/// The learner's casts of one kind: begun, begun out of reach, landed nothing, landed nothing having begun out of
/// reach, and the damage they dealt (shares of the foe's full life, 0 to 100 a life).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CastLog {
    pub casts: u32,
    pub out_of_reach: u32,
    pub whiffs: u32,
    pub far_whiffs: u32,
    pub damage: f64,
}

/// The learner's casts by kind (`data::Kind` order), and the damage its actives' blows dealt (a skill of its loadout,
/// not one of `loadout::BASICS`; % of a life, as `CastLog::damage`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Casts {
    pub kinds: [CastLog; CAST_KINDS],
    pub active_damage: f64,
}

/// What a step pays besides the round's outcome and the fixed penalties: the damage term's weight (`shaping`), the
/// cost of a whiff (`whiff`), more for a whiff begun out of reach (`whiff_far`, on top of `whiff`), and `variety` x
/// the share of a life the learner's actives took with their blows (a skill of its loadout, not a basic: slash, dive,
/// blink), so a policy that wins with only the basics is not paid for it; `idle` taken from a decision that could
/// cast and does not (it moves, jumps, guards, meditates or waits with a skill ready and paid for), and `land` paid
/// for each damaging cast that lands, so a policy that holds its skills back is not left alone for it.
///
/// How it looks: `flicker` taken from a move that undoes the one before it within `FLICKER` seconds (duck, stand,
/// duck: an act other than a wait or a cast chosen again with another between them), `fly` from each take-off into
/// flight, `duck` from each choice to duck (a dodge that pays must pay more than it), `short_sit` from a meditation left before it could gain anything (another act chosen within `SIT` seconds
/// of sitting down: sitting down and getting up are 0.8 s it cannot act, and the extra qi starts after a whole
/// second sat), and `repeat` and `habit` scaling the fixed penalties for choosing one act again and again (`REPEAT`,
/// `HABIT`; 1 keeps them as they were, 0 drops them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pay {
    pub shaping: f32,
    pub whiff: f32,
    pub whiff_far: f32,
    pub variety: f32,
    pub idle: f32,
    pub land: f32,
    pub flicker: f32,
    pub fly: f32,
    pub duck: f32,
    pub short_sit: f32,
    pub repeat: f32,
    pub habit: f32,
}

impl Default for Pay {
    fn default() -> Pay {
        Pay {
            shaping: 0.0,
            whiff: 0.0,
            whiff_far: 0.0,
            variety: 0.0,
            idle: 0.0,
            land: 0.0,
            flicker: 0.0,
            fly: 0.0,
            duck: 0.0,
            short_sit: 0.0,
            repeat: 1.0,
            habit: 1.0,
        }
    }
}

impl Casts {
    pub fn add(&mut self, o: &Casts) {
        self.active_damage += o.active_damage;
        for (a, b) in self.kinds.iter_mut().zip(o.kinds.iter()) {
            a.casts += b.casts;
            a.out_of_reach += b.out_of_reach;
            a.whiffs += b.whiffs;
            a.far_whiffs += b.far_whiffs;
            a.damage += b.damage;
        }
    }
}

/// The learner's cast in progress: its skill, its kind's index, whether it deals damage, whether it began with the
/// foe out of its reach, and what it has done.
#[derive(Clone, Copy, Debug)]
struct Open {
    spell: &'static str,
    kind: usize,
    strikes: bool,
    far: bool,
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

/// The critic's extra inputs (`Env::privileged`): an observation from the foe's side, the clock and the rounds won.
pub const PRIVILEGED: usize = OBS + 3;

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
        let mut sense = glance_of(state, me);
        let mut wire = self.wire.borrow_mut();
        if let Some(a) = wire.answer.take() {
            self.mind.queue(a);
        }
        let (now, due) = self.mind.step(&mut sense, |s| fill(state, me, s), dt);
        if let Some(due) = due {
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
    /// The two acts before this one, with the time each was chosen, the older first (`Pay::flicker`).
    before: [Option<(usize, f64)>; 2],
    /// When it chose to meditate, while it has not chosen anything else since (`Pay::short_sit`).
    sat: Option<f64>,
    done: bool,
    /// The cast under way, the casts ended whose shots or later blows are still out, the learner's casts since the
    /// trainer last took them, and the whiffs not yet paid for.
    open: Option<Open>,
    trailing: Vec<Open>,
    casts: Casts,
    whiffed: u32,
    far_whiffed: u32,
    /// The damaging casts that landed not yet paid for.
    landed: u32,
    /// The share of a life the learner's actives took since the last step paid for it.
    active_dealt: f64,
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
            before: [None, None],
            sat: None,
            done: false,
            open: None,
            trailing: Vec::new(),
            casts: Casts::default(),
            whiffed: 0,
            far_whiffed: 0,
            landed: 0,
            active_dealt: 0.0,
        };
        env.advance();
        env
    }

    /// What the learner sees now and what it may do.
    pub fn observe(&self) -> (&[f32; OBS], &[bool; ACTIONS]) {
        (&self.due.obs, &self.due.legal)
    }

    /// What only the critic may see (the asymmetric critic, `docs/plans/2026-11-20-smarter-ai.md`, B): the duel as
    /// the foe sees it now, with no reaction delay, written as an observation (its slots, cooldowns, qi, its cast),
    /// then the round's clock over its limit and the rounds each side has won / 3. `PRIVILEGED` floats; the policy
    /// never sees them, so the bot's inputs are unchanged.
    pub fn privileged(&self) -> Vec<f32> {
        let state = self.duel.state();
        let foe = 1 - self.me;
        let sense = sense_of(state, foe);
        let mut view = Delayed::new();
        view.push(&sense);
        let (me, threats) = view.seen(sense.time, 0.0);
        let skills: Vec<_> = state.fighters[foe]
            .spells
            .iter()
            .map(|n| crate::sense::skill_of(n))
            .collect();
        let mut out = observe(&sense, &me, &threats, &skills, &[0.0; TAG]).to_vec();
        out.push((state.time / self.limit.max(1.0)) as f32);
        out.push(state.wins[self.me] as f32 / 3.0);
        out.push(state.wins[foe] as f32 / 3.0);
        out
    }

    /// Answer the learner's decision with `action` (an act it may not do is a wait), run the duel to its next
    /// decision or the round's end, and pay for it. `shaping` weighs the damage term (the trainer anneals it to 0).
    pub fn step(&mut self, action: usize, shaping: f32) -> Step {
        self.step_with(action, shaping, 0.0)
    }

    /// The learner's moment in plain words, for a judge that reads text (`words::moment`).
    pub fn moment(&self) -> String {
        crate::words::moment(self.duel.state(), self.me)
    }

    /// Each of the learner's acts now, in words (`words::acts`).
    pub fn acts(&self) -> Vec<String> {
        crate::words::acts(self.duel.state(), self.me)
    }

    /// The numbers of the learner's moment (`words::brief`).
    pub fn brief(&self) -> crate::words::Brief {
        crate::words::brief(self.duel.state(), self.me)
    }

    /// The learner's casts since this was last called, by kind.
    pub fn take_casts(&mut self) -> Casts {
        core::mem::take(&mut self.casts)
    }

    /// As `step`, with `whiff` taken from the reward for each damaging cast that landed nothing.
    pub fn step_with(&mut self, action: usize, shaping: f32, whiff: f32) -> Step {
        self.step_costs(action, shaping, whiff, 0.0)
    }

    /// As `step_with`, with `far` taken too for each of those whiffs that began with the foe out of its reach (a
    /// blade, blast or dash: `Casts`' `out_of_reach`), so a blow thrown in reach costs no more than `whiff`.
    pub fn step_costs(&mut self, action: usize, shaping: f32, whiff: f32, far: f32) -> Step {
        self.step_paid(
            action,
            &Pay {
                shaping,
                whiff,
                whiff_far: far,
                ..Pay::default()
            },
        )
    }

    /// As `step`, paid as `pay` says.
    pub fn step_paid(&mut self, action: usize, pay: &Pay) -> Step {
        let Pay {
            shaping,
            whiff,
            whiff_far: far,
            variety,
            idle,
            land,
            flicker,
            fly,
            duck,
            short_sit,
            repeat,
            habit,
        } = *pay;
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
        let could_cast = (0..ACTIONS)
            .any(|i| self.due.legal[i] && matches!(Act::from_index(i), Some(Act::Cast { .. })));
        if could_cast && !matches!(Act::from_index(act), Some(Act::Cast { .. })) {
            reward -= idle;
        }
        if act != 0 && self.last == Some(act) {
            self.run += 1;
        } else {
            self.run = 1;
        }
        self.last = Some(act);
        if act != 0 && self.run >= RUN {
            reward -= REPEAT * repeat;
        }
        let now = self.duel.state().time;
        let plain = act != 0 && !matches!(Act::from_index(act), Some(Act::Cast { .. }));
        if let [Some((older, t)), Some((prev, _))] = self.before
            && plain
            && older == act
            && prev != act
            && now - t < FLICKER
        {
            reward -= flicker;
        }
        self.before = [self.before[1], Some((act, now))];
        match Act::from_index(act) {
            Some(Act::Meditate) => {
                self.sat.get_or_insert(now);
            }
            Some(Act::Wait) => {}
            _ => {
                if self.sat.take().is_some_and(|t| now - t < SIT) {
                    reward -= short_sit;
                }
            }
        }
        if matches!(Act::from_index(act), Some(Act::FlyHigh))
            && !self.duel.state().fighters[self.me].fly
        {
            reward -= fly;
        }
        if matches!(Act::from_index(act), Some(Act::Duck | Act::DuckLong)) {
            reward -= duck;
        }
        if self.quiet >= QUIET {
            reward -= STALL;
        }
        let same = self.recent.iter().filter(|&&a| a == act).count();
        if self.recent.len() == HABITS && same as f64 >= SHARE * HABITS as f64 {
            reward -= HABIT * habit;
        }
        self.recent.push_back(act);
        if self.recent.len() > HABITS {
            self.recent.pop_front();
        }
        self.wire.borrow_mut().answer = Some(act);
        let (dealt, taken, won) = self.advance();
        reward += shaping * ((dealt - taken) / 100.0) as f32;
        reward -= whiff * core::mem::take(&mut self.whiffed) as f32;
        reward -= far * core::mem::take(&mut self.far_whiffed) as f32;
        reward += variety * (core::mem::take(&mut self.active_dealt) / 100.0) as f32;
        reward += land * core::mem::take(&mut self.landed) as f32;
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
                    Kind::Blast | Kind::Bolt | Kind::Orb | Kind::Zone | Kind::Void
                );
            // from the caster's x to the nearest edge of the foe's hurtboxes
            let x = state.fighters[me].body.x;
            let gap = crate::frames::hurtboxes(state, foe)
                .iter()
                .map(|h| (h[0] - x).max(x - h[2]).max(0.0))
                .fold(f64::INFINITY, f64::min);
            let reach = match k {
                Kind::Blast if motion => mo.range,
                _ => None,
            };
            let far = reach.is_some_and(|r| gap > r);
            let log = &mut self.casts.kinds[kind.min(CAST_KINDS - 1)];
            log.casts += 1;
            if far {
                log.out_of_reach += 1;
            }
            self.open = Some(Open {
                spell: a.spell,
                kind: kind.min(CAST_KINDS - 1),
                strikes,
                far,
                landed: false,
            });
        }
        for e in out {
            if let Out::Hit {
                by, spell, share, ..
            } = e
                && *by == me
                && !arena_calc::loadout::BASICS.contains(spell)
            {
                self.casts.active_damage += share;
                self.active_dealt += share;
            }
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
                self.casts.kinds[o.kind].damage += dealt;
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
            if o.strikes && o.landed {
                self.landed += 1;
            }
            if o.strikes && !o.landed {
                self.casts.kinds[o.kind].whiffs += 1;
                self.whiffed += 1;
                if o.far {
                    self.casts.kinds[o.kind].far_whiffs += 1;
                    self.far_whiffed += 1;
                }
            }
        }
    }
}

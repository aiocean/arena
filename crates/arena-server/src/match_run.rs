//! Fighting one match and turning it into a stream of `arena-link` frames.
//!
//! A match is the pure rules (`arena_duel`) and two seats: the house AI (`arena-bot`'s ONNX policy, run natively
//! through tract) against one of the minds the rules ship (`Pairing`), each seat wrapped in a `Recorder`. We step one
//! tick at a time, drain the recorders for the decisions and forfeits that tick produced, and hand them back as
//! frames. The caller (the publisher) paces the stepping to the wall clock and writes the frames onto a MoQ track.
//!
//! The page never runs a mind: it replays each corner's recorded decisions (`seat::Replay`) on its own copy of the
//! rules, so whatever sits in a corner here, the page needs only the rules.

use arena_bot::onnx::Onnx;
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::effects;
use arena_duel::first_mind::FirstMind;
use arena_duel::scripted::Scripted;
use arena_duel::seat::{BotSeat, Brain, Fault, Log, MakeSeat, Recorder, Recording};
use arena_duel::style::Style;
use arena_duel::view::{DuelView, view};
use arena_link::{Act, Arena, Decision, Ended, Forfeit, Snapshot, Tally};
use arena_telemetry::duel::Observer;
use std::cell::RefCell;
use std::rc::Rc;

/// The arena a match is fought in. Fixed here (not from a window), so the same seed is the same fight on every screen
/// and the server, which draws nothing, knows the arena a watcher sees. (A rule change still to measure with the
/// ladder: `docs/specs/2026-11-17-live-matches-design.md`.)
pub fn arena_config(seed: u32) -> DuelConfig {
    config_for(seed, ARENA)
}

/// The arena every live match is fought in: the one the game lays out in a 1280x720 window (`arena-game`'s
/// `Layout::default()`: walls at 22 and 298 world pixels, ground at 158, sky at 49), in the rules' pixels.
pub const ARENA: Arena = Arena {
    left: 88.0,
    right: 1192.0,
    ground: 632.0,
    sky: 196.0,
};

/// The rules' config for a match on `seed` in `arena`: what both the server and a watching page build.
pub fn config_for(seed: u32, arena: Arena) -> DuelConfig {
    DuelConfig {
        seed,
        left: arena.left,
        right: arena.right,
        ground: arena.ground,
        sky: arena.sky,
        effects: effects::all(),
    }
}

/// How many round-wins takes a match, and the tick a match is cut off at even if neither gets there.
pub const WINS_TO_TAKE: u32 = 2;
pub const MAX_TICKS: u32 = 120 * 60; // two minutes

/// Who sits in a live corner, by kind: what [`Seats`] turns into a seat. The house AI is a model run natively (ONNX
/// through tract, `arena-bot`); the minds are built into the rules. A bot module a player uploads (`arena-bot`'s host)
/// is the kind still to add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bot {
    /// The house AI, `weights/policy.onnx`, in the balanced style (as the ladder measures it).
    House,
    Brain,
    First,
    Aggressive,
    Turtle,
}

impl Bot {
    /// Its name: the corner's name on the wire and its telemetry's `bot`.
    pub fn name(self) -> &'static str {
        match self {
            Bot::House => "house",
            Bot::Brain => "brain",
            Bot::First => "first",
            Bot::Aggressive => "aggressive",
            Bot::Turtle => "turtle",
        }
    }

    /// What runs it, as its telemetry's `runtime` says.
    pub fn runtime(self) -> &'static str {
        match self {
            Bot::House => "onnx",
            _ => "builtin",
        }
    }
}

/// Who fights a live match: the house AI, in which seat, and the bot it meets. With the seed it fixes the match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pairing {
    /// The house AI's seat, 0 or 1.
    pub house_seat: usize,
    pub opponent: Bot,
}

impl Pairing {
    /// The `n`th match's pairing (counted from 1) on `seed`: the house AI against itself (owner, 2026-11-18: the page
    /// shows house against house only), its seat drawn from the seed (`house_seat`) and the looks from the seed too
    /// (`Duel::new_drawn`), so neither look keeps one seat. `n` stays for a rotation to come.
    pub fn of(_n: u64, seed: u32) -> Pairing {
        Pairing {
            house_seat: house_seat(seed),
            opponent: Bot::House,
        }
    }

    /// The bots in seat 0 and seat 1.
    pub fn bots(self) -> [Bot; 2] {
        let mut bots = [self.opponent; 2];
        bots[self.house_seat.min(1)] = Bot::House;
        bots
    }
}

/// The house AI's seat on `seed`: the top bit of the seed mixed, so it is drawn apart from the looks (the seed's
/// parity, `ladder::looks_of`).
pub fn house_seat(seed: u32) -> usize {
    (seed.wrapping_mul(0x9E37_79B1) >> 31) as usize
}

/// Makes a corner's seat by its bot's kind: the seat factory, holding what a kind loads once (the house AI's model).
pub struct Seats {
    house: Onnx,
}

impl Seats {
    /// The factory, the house AI loaded and checked.
    pub fn new() -> Result<Seats, Fault> {
        Ok(Seats {
            house: Onnx::house()?,
        })
    }

    /// A maker for `bot`'s seat. The house AI gets a fresh instance each match, so its `bot.end` (runtime "onnx")
    /// reports that match when the match is dropped.
    pub fn maker(&self, bot: Bot) -> MakeSeat {
        match bot {
            Bot::House => BotSeat::make(self.house.decider(), Some(Style::Balanced)),
            Bot::Brain => Brain::make(),
            Bot::First => FirstMind::make(),
            Bot::Aggressive => Scripted::aggressive(),
            Bot::Turtle => Scripted::turtle(),
        }
    }
}

/// `inner` wrapped in a `Recorder` once the duel makes it, its log handed out through `slot`.
fn recording(inner: MakeSeat, slot: Rc<RefCell<Option<Recording>>>) -> MakeSeat {
    Box::new(move |state, me| {
        let (rec, handle) = Recorder::new(inner(state, me));
        *slot.borrow_mut() = Some(handle);
        Box::new(rec)
    })
}

/// A match being fought: the rules, the two recorders' logs, and how far each has been published.
pub struct Match {
    duel: Duel,
    logs: [Recording; 2],
    seed: u32,
    pairing: Pairing,
    corners: [String; 2],
    /// Fighting ticks elapsed. A seat decides once per fighting tick, so this is the recorders' tick count.
    tick: u32,
    /// How many decisions of each corner's log have already been handed out as frames.
    sent: [usize; 2],
    /// Whether each corner's forfeit has been handed out.
    sent_forfeit: [bool; 2],
    ended: Option<Ended>,
    /// The duel observer, when the match is watched for telemetry (`observe`): it only reads.
    observer: Option<Observer>,
    /// Its number in the server's run and the score before it, carried by every keyframe.
    tally: Tally,
}

/// What stepping one tick produced: the new frames' payloads, in tick order, and whether the match is now over.
pub struct Tick {
    pub decisions: Vec<Decision>,
    pub forfeits: Vec<Forfeit>,
    pub ended: Option<Ended>,
}

impl Match {
    /// Start a match on `seed` for `pairing`, its seats made by `seats`.
    pub fn new(seed: u32, pairing: Pairing, seats: &Seats) -> Match {
        let bots = pairing.bots();
        let slots: [Rc<RefCell<Option<Recording>>>; 2] = Default::default();
        let duel = Duel::new_drawn(
            arena_config(seed),
            [
                recording(seats.maker(bots[0]), Rc::clone(&slots[0])),
                recording(seats.maker(bots[1]), Rc::clone(&slots[1])),
            ],
        );
        // the duel made both seats as it was built; a slot it left empty would be a seat never made
        let logs = slots.map(|s| s.borrow_mut().take().unwrap_or_default());
        Match {
            duel,
            logs,
            seed,
            pairing,
            corners: bots.map(|b| b.name().to_string()),
            tick: 0,
            sent: [0, 0],
            sent_forfeit: [false, false],
            ended: None,
            observer: None,
            tally: Tally::default(),
        }
    }

    /// Set where this match stands in the server's run (its number, the score before it): every keyframe carries it.
    pub fn set_tally(&mut self, tally: Tally) {
        self.tally = tally;
    }

    /// Who fights it.
    pub fn pairing(&self) -> Pairing {
        self.pairing
    }

    /// Each corner's decisions so far.
    pub fn decisions(&self) -> [usize; 2] {
        [
            self.logs[0].borrow().decisions.len(),
            self.logs[1].borrow().decisions.len(),
        ]
    }

    /// The match's result once over, else None.
    pub fn ended(&self) -> Option<Ended> {
        self.ended
    }

    /// Report this match as `match_id` to telemetry (`arena_telemetry::duel::Observer`): its events, spans and
    /// metrics, from now until it ends; `match.start` names each seat's bot, what runs it and the server's `build`.
    pub fn observe(&mut self, match_id: &str, build: &str) {
        let bots = self.pairing.bots();
        let corners = [self.corners[0].as_str(), self.corners[1].as_str()];
        self.observer = Some(Observer::start_seated(
            &self.duel,
            match_id,
            corners,
            bots.map(Bot::runtime),
            build,
        ));
    }

    /// Stop the match before it ends (the server shutting down): its observer reports `match.end` with `result`
    /// (`"server_stopped"`) instead of leaving the match with no end, or "abandoned". A match already over is left be.
    pub fn stop(&mut self, result: &str) {
        if let Some(o) = self.observer.take() {
            o.stop(&self.duel, result);
        }
    }

    /// Fighting ticks elapsed so far.
    pub fn tick(&self) -> u32 {
        self.tick
    }

    /// The fight as it stands, for a test or a local view.
    #[allow(dead_code)]
    pub fn view(&self) -> DuelView {
        view(self.duel.state())
    }

    /// The whole match so far, self-contained: the keyframe a group opens with.
    pub fn snapshot(&self) -> Snapshot {
        let mut decisions = Vec::new();
        let mut forfeits = Vec::new();
        for (c, log) in self.logs.iter().enumerate() {
            let log = log.borrow();
            for (t, choice) in &log.decisions {
                decisions.push(Decision {
                    tick: *t,
                    corner: c as u8,
                    act: Act::from(choice),
                });
            }
            if let Some(t) = log.forfeit {
                forfeits.push(Forfeit {
                    tick: t,
                    corner: c as u8,
                });
            }
        }
        decisions.sort_by_key(|d| (d.tick, d.corner));
        Snapshot {
            rules: arena_link::RULES_VERSION,
            seed: self.seed,
            arena: ARENA,
            corners: self.corners.clone(),
            decisions,
            forfeits,
            ended: self.ended,
            ticks: self.tick,
            tally: self.tally,
        }
    }

    /// Advance one tick; return the frames it produced. Returns `None` once the match is already over.
    pub fn step(&mut self) -> Option<Tick> {
        if self.ended.is_some() {
            return None;
        }
        match &mut self.observer {
            Some(o) => drop(o.step(&mut self.duel, TICK)),
            None => drop(self.duel.step(TICK)),
        }
        self.tick += 1;

        let mut decisions = Vec::new();
        let mut forfeits = Vec::new();
        for c in 0..2 {
            let log = self.logs[c].borrow();
            for (t, choice) in log.decisions.iter().skip(self.sent[c]) {
                decisions.push(Decision {
                    tick: *t,
                    corner: c as u8,
                    act: Act::from(choice),
                });
            }
            self.sent[c] = log.decisions.len();
            if !self.sent_forfeit[c]
                && let Some(t) = log.forfeit
            {
                forfeits.push(Forfeit {
                    tick: t,
                    corner: c as u8,
                });
                self.sent_forfeit[c] = true;
            }
        }
        decisions.sort_by_key(|d| (d.tick, d.corner));

        let v = view(self.duel.state());
        let ended = if v.wins[0] >= WINS_TO_TAKE {
            Some(Ended::Winner(0))
        } else if v.wins[1] >= WINS_TO_TAKE {
            Some(Ended::Winner(1))
        } else if self.tick >= MAX_TICKS {
            Some(Ended::Draw)
        } else {
            None
        };
        self.ended = ended;
        // take the observer only once the match is over: taken on a tick that goes on, it would drop, and a dropped
        // observer reports the match abandoned and watches nothing more
        if let Some(e) = ended
            && let Some(o) = self.observer.take()
        {
            o.end(&self.duel, matches!(e, Ended::Draw));
        }
        Some(Tick {
            decisions,
            forfeits,
            ended,
        })
    }
}

/// The score after a match on `seed` ended as `ended`: the winner's fighter (its look, drawn from the seed as the
/// rules draw it, `ladder::looks_of`) takes the match, or it is a draw.
pub fn score(tally: Tally, seed: u32, ended: Ended) -> Tally {
    tally.after(match ended {
        Ended::Winner(c) => {
            let look = arena_duel::ladder::looks_of(seed)[usize::from(c.min(1))];
            Some(usize::from(look != arena_duel::data::Look::VuongLam))
        }
        Ended::Draw => None,
    })
}

/// Split a wire snapshot back into the two corners' seat logs: what a watching page feeds its `Replay` seats.
#[allow(dead_code)]
pub fn logs_from_snapshot(snap: &Snapshot) -> [Log; 2] {
    let mut logs = [Log::default(), Log::default()];
    let mut decisions = snap.decisions.clone();
    decisions.sort_by_key(|d| (d.tick, d.corner));
    for d in decisions {
        if let Some(log) = logs.get_mut(d.corner as usize) {
            log.decisions.push((d.tick, d.act.clone().into()));
        }
    }
    for f in &snap.forfeits {
        if let Some(log) = logs.get_mut(f.corner as usize) {
            log.forfeit = Some(f.tick);
        }
    }
    logs
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_duel::seat::{MakeSeat, Replay, Seat};

    #[test]
    fn the_score_goes_to_the_winners_fighter_whatever_its_seat() {
        let t = Tally {
            number: 3,
            wins: [1, 1],
            draws: 0,
        };
        // an even seed seats Vương Lâm in corner 0, an odd one in corner 1
        assert_eq!(score(t, 4, Ended::Winner(0)).wins, [2, 1]);
        assert_eq!(score(t, 4, Ended::Winner(1)).wins, [1, 2]);
        assert_eq!(score(t, 5, Ended::Winner(1)).wins, [2, 1]);
        assert_eq!(score(t, 5, Ended::Winner(0)).wins, [1, 2]);
        let d = score(t, 5, Ended::Draw);
        assert_eq!((d.wins, d.draws, d.number), ([1, 1], 1, 3));
    }

    /// Fight a match to its end (or `ticks`), keeping the fight as it stood after each step.
    fn fight(seed: u32, pairing: Pairing, ticks: u32) -> Result<(Match, Vec<DuelView>), String> {
        let seats = Seats::new().map_err(|e| format!("the house AI did not load: {}", e.0))?;
        let mut m = Match::new(seed, pairing, &seats);
        let mut fought = Vec::new();
        for _ in 0..ticks {
            if m.step().is_none() {
                break;
            }
            fought.push(m.view());
        }
        Ok((m, fought))
    }

    /// The page's side: a duel built as `live.rs`/`rules.rs` builds one (`Duel::new_drawn`) on two `Replay` seats fed
    /// the corners' logs split from the wire snapshot, stepped tick by tick and checked against the fight the server
    /// fought.
    fn replays(seed: u32, snap: &Snapshot, fought: &[DuelView]) {
        let [log0, log1] = logs_from_snapshot(snap);
        let make0: MakeSeat = Box::new(move |_s, _m| Box::new(Replay::new(log0)) as Box<dyn Seat>);
        let make1: MakeSeat = Box::new(move |_s, _m| Box::new(Replay::new(log1)) as Box<dyn Seat>);
        let mut replay = Duel::new_drawn(config_for(snap.seed, snap.arena), [make0, make1]);
        assert_eq!(snap.seed, seed);
        for (i, want) in fought.iter().enumerate() {
            replay.step(TICK);
            assert_eq!(
                &view(replay.state()),
                want,
                "wire replay diverged at step {i}"
            );
        }
    }

    /// The server's wire snapshot (Act-encoded, split by corner, keyed by tick) replays the very fight it fought: the
    /// server-side half of P1. A page fed this snapshot draws what the server drew, the house AI in either seat and on
    /// either parity of seed (which swaps the looks).
    #[test]
    fn the_wire_snapshot_replays_the_fought_match() -> Result<(), String> {
        for (seed, house_seat, opponent) in [
            (7_654_321, 0, Bot::Brain),
            (7_654_322, 1, Bot::Brain),
            (1_234, 1, Bot::Turtle),
            (99, 0, Bot::Aggressive),
            (5, 1, Bot::House),
        ] {
            let pairing = Pairing {
                house_seat,
                opponent,
            };
            let (m, fought) = fight(seed, pairing, 1200)?;
            assert!(!fought.is_empty());
            let snap = m.snapshot();
            assert_eq!(snap.corners[house_seat], "house");
            assert_eq!(snap.corners[1 - house_seat], opponent.name());
            assert!(
                snap.decisions
                    .iter()
                    .any(|d| d.corner as usize == house_seat),
                "the house AI decided nothing"
            );
            replays(seed, &snap, &fought);
        }
        Ok(())
    }

    /// A seed, the opponent and the house AI's seat fix the match: fought twice, the same decisions and the same end.
    #[test]
    fn a_seed_and_a_pairing_fight_the_same_match_twice() -> Result<(), String> {
        let pairing = Pairing {
            house_seat: 1,
            opponent: Bot::First,
        };
        let (a, fa) = fight(424_243, pairing, MAX_TICKS)?;
        let (b, fb) = fight(424_243, pairing, MAX_TICKS)?;
        assert!(a.ended().is_some(), "the match ran out of ticks unended");
        assert_eq!(a.snapshot(), b.snapshot());
        assert_eq!(a.ended(), b.ended());
        assert_eq!(fa.last(), fb.last());
        assert!(a.decisions()[1] > 0, "the house AI decided nothing");
        Ok(())
    }

    /// Every live match is the house AI against itself, its seat from the seed, and both seats come up.
    #[test]
    fn pairings_are_house_against_house_and_draw_the_seat() {
        for n in 1..=8 {
            assert_eq!(Pairing::of(n, 5).bots(), [Bot::House, Bot::House]);
        }
        let seats: Vec<usize> = (0..64_u32).map(house_seat).collect();
        assert!(seats.contains(&0) && seats.contains(&1));
    }
}

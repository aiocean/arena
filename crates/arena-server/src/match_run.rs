//! Fighting one match and turning it into a stream of `arena-link` frames.
//!
//! A match is the pure rules (`arena_duel`) and two house brains, each seat wrapped in a `Recorder`. We step one tick
//! at a time, drain the recorders for the decisions and forfeits that tick produced, and hand them back as frames. The
//! caller (the publisher) paces the stepping to the wall clock and writes the frames onto a MoQ track.

use arena_link::{Act, Arena, Decision, Ended, Forfeit, Snapshot};
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::effects;
use arena_duel::seat::{Brain, Log, MakeSeat, Recorder, Recording};
use arena_duel::view::{DuelView, view};
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

thread_local! {
    static HANDLES: RefCell<Vec<Recording>> = const { RefCell::new(Vec::new()) };
}

fn recording_house() -> MakeSeat {
    Box::new(move |state, me| {
        let (rec, handle) = Recorder::new(Box::new(Brain::new(state, me)));
        HANDLES.with(|h| h.borrow_mut().push(handle));
        Box::new(rec)
    })
}

/// A match being fought: the rules, the two recorders' logs, and how far each has been published.
pub struct Match {
    duel: Duel,
    logs: [Recording; 2],
    seed: u32,
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
}

/// What stepping one tick produced: the new frames' payloads, in tick order, and whether the match is now over.
pub struct Tick {
    pub decisions: Vec<Decision>,
    pub forfeits: Vec<Forfeit>,
    pub ended: Option<Ended>,
}

impl Match {
    /// Start a match on `seed` between two house brains named `corners`.
    pub fn new(seed: u32, corners: [String; 2]) -> Match {
        HANDLES.with(|h| h.borrow_mut().clear());
        let duel = Duel::new_with_looks(
            arena_config(seed),
            [recording_house(), recording_house()],
            arena_duel::ladder::looks_of(seed),
        );
        let handles = HANDLES.with(|h| h.borrow().clone());
        let logs = [Rc::clone(&handles[0]), Rc::clone(&handles[1])];
        Match {
            duel,
            logs,
            seed,
            corners,
            tick: 0,
            sent: [0, 0],
            sent_forfeit: [false, false],
            ended: None,
            observer: None,
        }
    }

    /// Report this match as `match_id` to telemetry (`arena_telemetry::duel::Observer`): its events, spans and
    /// metrics, from now until it ends.
    pub fn observe(&mut self, match_id: &str) {
        let corners = [self.corners[0].as_str(), self.corners[1].as_str()];
        self.observer = Some(Observer::start(&self.duel, match_id, corners));
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

    /// The server's wire snapshot (Act-encoded, split by corner, keyed by tick) replays the very fight it fought: the
    /// server-side half of P1. A page fed this snapshot draws what the server drew.
    #[test]
    fn the_wire_snapshot_replays_the_fought_match() {
        let seed = 7_654_321;
        let mut m = Match::new(seed, ["a".into(), "b".into()]);
        let mut fought = Vec::new();
        for _ in 0..1200 {
            if m.step().is_none() {
                break;
            }
            fought.push(m.view());
        }
        assert!(!fought.is_empty());

        let snap = m.snapshot();
        let [log0, log1] = logs_from_snapshot(&snap);
        let make0: MakeSeat = Box::new(move |_s, _m| Box::new(Replay::new(log0)) as Box<dyn Seat>);
        let make1: MakeSeat = Box::new(move |_s, _m| Box::new(Replay::new(log1)) as Box<dyn Seat>);
        let mut replay = Duel::new_with_looks(
            arena_config(seed),
            [make0, make1],
            arena_duel::ladder::looks_of(seed),
        );
        for (i, want) in fought.iter().enumerate() {
            replay.step(TICK);
            assert_eq!(&view(replay.state()), want, "wire replay diverged at step {i}");
        }
    }
}

//! P1: a match replayed from its seed and the recorded decisions is the match that was fought, tick for tick.
//!
//! Two house brains fight, each seat wrapped in a `Recorder`; the whole fight's view is kept every step. Then a fresh
//! duel on the same seed is seated with two `Replay` seats built from the logs, and stepped the same way. The two
//! views must be equal at every step. This is what lets the server fight a match and the page draw it from the
//! decisions alone (`arena_link`, `docs/specs/2026-11-17-live-matches-design.md`).

use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::effects;
use arena_duel::seat::{Brain, Log, Recorder, Replay, Seat};
use arena_duel::view::{DuelView, view};
use std::cell::RefCell;
use std::rc::Rc;

fn config(seed: u32) -> DuelConfig {
    DuelConfig {
        seed,
        left: 40.0,
        right: 360.0,
        ground: 180.0,
        sky: 40.0,
        effects: effects::all(),
    }
}

thread_local! {
    /// The recorders' live log handles, in seating order, pushed as `Duel::new` makes each seat.
    static SHARED: RefCell<Vec<Rc<RefCell<Log>>>> = const { RefCell::new(Vec::new()) };
}

/// A maker that seats a recording house brain and files its live log handle in `SHARED`.
fn recording_house() -> arena_duel::seat::MakeSeat {
    Box::new(move |state, me| {
        let (rec, handle) = Recorder::new(Box::new(Brain::new(state, me)));
        SHARED.with(|s| s.borrow_mut().push(handle));
        Box::new(rec)
    })
}

/// Fight `seed` for `steps` steps of one tick, recording both corners; returns the views and the two logs.
fn fight_and_record(seed: u32, steps: usize) -> (Vec<DuelView>, [Log; 2]) {
    SHARED.with(|s| s.borrow_mut().clear());
    let mut duel = Duel::new(config(seed), [recording_house(), recording_house()]);
    let mut views = Vec::with_capacity(steps);
    for _ in 0..steps {
        duel.step(TICK);
        views.push(view(duel.state()));
    }
    let shared = SHARED.with(|s| s.borrow().clone());
    let out = [shared[0].borrow().clone(), shared[1].borrow().clone()];
    (views, out)
}

#[test]
fn a_recorded_match_replays_tick_for_tick() {
    let seed = 20_261_117;
    let steps = 1500; // 25 s at 60 ticks
    let (fought, logs) = fight_and_record(seed, steps);
    assert!(
        logs[0].decisions.len() + logs[1].decisions.len() > 0,
        "the fight recorded no decisions"
    );

    let [log0, log1] = logs;
    let make0: arena_duel::seat::MakeSeat = {
        let log = log0;
        Box::new(move |_s, _m| Box::new(Replay::new(log)) as Box<dyn Seat>)
    };
    let make1: arena_duel::seat::MakeSeat = {
        let log = log1;
        Box::new(move |_s, _m| Box::new(Replay::new(log)) as Box<dyn Seat>)
    };
    let mut duel = Duel::new(config(seed), [make0, make1]);
    for (i, want) in fought.iter().enumerate() {
        duel.step(TICK);
        let got = view(duel.state());
        assert_eq!(got, *want, "replay diverged at step {i}");
    }
}

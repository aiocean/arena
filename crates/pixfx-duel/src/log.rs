//! A fight's acts, kept and played back: `recorded` wraps a seat and logs what it chose, `replay` plays a log as two
//! seats, so a fight can be re-run without the bots that played it.

use crate::fighter::State;
use crate::seat::{Fault, MakeSeat, Seat};
use pixfx_mind::{Choice, Outcome, Posture};
use std::cell::RefCell;
use std::rc::Rc;

/// What each seat chose in a fight, by the index of its `think` call.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActLog {
    pub season: String,
    pub seed: u32,
    pub bots: [String; 2],
    pub acts: [Vec<(u32, Choice)>; 2],
}

/// `make`'s seat, logging every choice it returns into `log`.
pub fn recorded(make: MakeSeat, log: Rc<RefCell<ActLog>>) -> MakeSeat {
    Box::new(move |state, me| {
        Box::new(Recorded {
            inner: make(state, me),
            log,
            calls: 0,
        })
    })
}

/// Both seats of a fight that played `log`.
pub fn replay(log: Rc<ActLog>) -> [MakeSeat; 2] {
    [0, 1].map(|me| -> MakeSeat {
        let log = log.clone();
        Box::new(move |_, _| {
            Box::new(LogSeat {
                log,
                me,
                calls: 0,
                next: 0,
            })
        })
    })
}

struct Recorded {
    inner: Box<dyn Seat>,
    log: Rc<RefCell<ActLog>>,
    calls: u32,
}

impl Seat for Recorded {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        let call = self.calls;
        self.calls += 1;
        let choice = self.inner.think(state, me, dt)?;
        self.log.borrow_mut().acts[me].push((call, choice.clone()));
        Some(choice)
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

    fn fault(&self) -> Option<&Fault> {
        self.inner.fault()
    }
}

/// Hands back the logged choice at each logged `think` call, nothing otherwise.
struct LogSeat {
    log: Rc<ActLog>,
    me: usize,
    calls: u32,
    next: usize,
}

impl Seat for LogSeat {
    fn think(&mut self, _state: &State, _me: usize, _dt: f64) -> Option<Choice> {
        let call = self.calls;
        self.calls += 1;
        let (at, choice) = self.log.acts[self.me].get(self.next)?;
        if *at != call {
            return None;
        }
        self.next += 1;
        Some(choice.clone())
    }

    fn tell(&mut self, _outcome: &Outcome) {}

    fn reset(&mut self) {}

    fn hold(&mut self, _seconds: f64) {}
}

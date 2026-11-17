//! Pacing a match to the wall clock: fixed-rate ticks on absolute deadlines.
//!
//! Tick `i` (from 0) is due `start + i * dt`. A loop asks how long to `wait` before the next tick, sleeps that long
//! (or not at all when behind), then `begin`s it. Since each deadline is reckoned from the start, a late wake does
//! not push every later tick back: the ticks behind run back to back until the loop is on time again. If it falls
//! more than `MAX_BEHIND` behind (the process was stopped, the machine slept), it does not run a burst of that length:
//! the schedule moves up to now (a resync) and goes on from there. Pacing decides only when a tick runs, never what
//! it does, so the fight is the same whatever the clock did.

use std::time::{Duration, Instant};

/// How far behind the loop may fall and still catch up tick by tick; past it the schedule resyncs to now.
pub const MAX_BEHIND: Duration = Duration::from_millis(250);

/// The schedule of one match's ticks.
#[derive(Clone, Debug)]
pub struct Pacer {
    start: Instant,
    dt: Duration,
    /// The index of the next tick to begin.
    next: u64,
    max_behind: Duration,
    resyncs: u64,
    missed: u64,
}

/// A tick begun: its index, when it was due, and how late it began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Begun {
    pub index: u64,
    pub due: Instant,
    /// How long after its deadline the tick began.
    pub late: Duration,
    /// Whether it began a whole tick or more late: its slot passed and the loop is catching up.
    pub missed: bool,
    /// Whether the schedule moved up to now at this tick (it was more than `MAX_BEHIND` late).
    pub resynced: bool,
}

impl Pacer {
    /// A schedule whose first tick is due at `start`, one every `dt`.
    pub fn new(start: Instant, dt: Duration) -> Pacer {
        Pacer {
            start,
            dt,
            next: 0,
            max_behind: MAX_BEHIND,
            resyncs: 0,
            missed: 0,
        }
    }

    /// The same, resyncing past `max_behind` instead of `MAX_BEHIND`.
    pub fn with_max_behind(mut self, max_behind: Duration) -> Pacer {
        self.max_behind = max_behind;
        self
    }

    /// When tick `index` is due.
    pub fn due(&self, index: u64) -> Instant {
        // whole nanoseconds from the start, so no error builds up over a long match
        let nanos = self.dt.as_nanos().saturating_mul(u128::from(index));
        self.start + Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    }

    /// How long to wait at `now` before the next tick is due (zero when it is due or past).
    pub fn wait(&self, now: Instant) -> Duration {
        self.due(self.next).saturating_duration_since(now)
    }

    /// Begin the next tick at `now`.
    pub fn begin(&mut self, now: Instant) -> Begun {
        let index = self.next;
        let due = self.due(index);
        let late = now.saturating_duration_since(due);
        let missed = late >= self.dt;
        let resynced = late > self.max_behind;
        if missed {
            self.missed += 1;
        }
        if resynced {
            // this tick is now on time, and the rest follow it
            self.start += late;
            self.resyncs += 1;
        }
        self.next += 1;
        Begun {
            index,
            due,
            late,
            missed,
            resynced,
        }
    }

    /// Ticks begun so far.
    pub fn ticks(&self) -> u64 {
        self.next
    }

    /// Ticks begun a whole tick or more late.
    pub fn missed(&self) -> u64 {
        self.missed
    }

    /// Times the schedule moved up to now.
    pub fn resyncs(&self) -> u64 {
        self.resyncs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: Duration = Duration::from_micros(16_667);

    #[test]
    fn deadlines_are_absolute_so_late_wakes_do_not_add_up() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0, DT);
        // every wake 5 ms late: each tick is 5 ms late, never more
        for i in 0..600 {
            let now = p.due(i) + Duration::from_millis(5);
            let b = p.begin(now);
            assert_eq!(b.late, Duration::from_millis(5));
            assert!(!b.missed);
        }
        // tick 600 is due exactly 600 ticks after the start: no drift
        assert_eq!(p.due(600) - t0, DT * 600);
        assert_eq!(p.resyncs(), 0);
    }

    #[test]
    fn behind_it_catches_up_without_waiting() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0, DT);
        p.begin(t0);
        // the loop stalled 100 ms: the ticks behind are due already, so it runs them back to back
        let now = t0 + Duration::from_millis(100);
        let mut caught = 0;
        while p.wait(now).is_zero() {
            let b = p.begin(now);
            assert!(!b.resynced);
            caught += 1;
        }
        // ticks 1..=6 were due by 100 ms (6 * 16.667 = 100.002 is not)
        assert_eq!(caught, 5);
        assert_eq!(
            p.missed(),
            4,
            "ticks 1 to 4 began a whole tick or more late"
        );
        assert!(p.wait(now) > Duration::ZERO);
        assert_eq!(p.resyncs(), 0);
    }

    #[test]
    fn far_behind_it_resyncs_instead_of_spiralling() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0, DT);
        p.begin(t0);
        // stopped two seconds: one tick, then the schedule starts again from now
        let now = t0 + Duration::from_secs(2);
        let b = p.begin(now);
        assert!(b.resynced && b.missed);
        assert_eq!(p.resyncs(), 1);
        // the next tick is a whole tick away, not 118 ticks overdue
        assert_eq!(p.wait(now), DT);
        let b = p.begin(now + DT);
        assert_eq!(b.late, Duration::ZERO);
        assert!(!b.resynced);
    }

    #[test]
    fn on_time_it_waits_for_the_next_deadline() {
        let t0 = Instant::now();
        let mut p = Pacer::new(t0, DT);
        assert_eq!(p.wait(t0), Duration::ZERO);
        p.begin(t0 + Duration::from_millis(1));
        assert_eq!(
            p.wait(t0 + Duration::from_millis(1)),
            DT - Duration::from_millis(1)
        );
        assert_eq!(p.ticks(), 1);
    }
}

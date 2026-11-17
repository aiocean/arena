//! The observer only reads: a fight observed, with every event on, is the fight unobserved, tick for tick; and it
//! says what happened.

use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::view::view;
use arena_telemetry::duel::Observer;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// Counts events by their message, and remembers a few fields.
#[derive(Clone, Default)]
struct Count(Arc<Mutex<BTreeMap<String, u32>>>);

struct Message(String);
impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for Count {
    fn on_event(&self, e: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut m = Message(String::new());
        e.record(&mut m);
        let key = if m.0.is_empty() {
            e.metadata().target().to_string()
        } else {
            m.0
        };
        if let Ok(mut c) = self.0.lock() {
            *c.entry(key).or_insert(0) += 1;
        }
    }
}

fn config(seed: u32) -> DuelConfig {
    DuelConfig {
        seed,
        left: 0.0,
        right: 880.0,
        ground: 600.0,
        sky: 384.0,
        effects: arena_duel::effects::all(),
    }
}

#[test]
fn observing_changes_nothing_and_reports_the_fight() -> Result<(), String> {
    let count = Count::default();
    let subscriber = tracing_subscriber::registry().with(count.clone());
    let seed = 7;
    let mut plain = Duel::with_brains(config(seed));
    let mut watched = Duel::with_brains(config(seed));
    tracing::subscriber::with_default(subscriber, || -> Result<(), String> {
        let mut o = Observer::start(&watched, "test-7", ["brain", "brain"]).snapshot_every(1.0);
        for i in 0..60 * 90 {
            let a = plain.step(TICK);
            let b = o.step(&mut watched, TICK);
            if a != b || view(plain.state()) != view(watched.state()) {
                return Err(format!("the observed fight diverged at tick {i}"));
            }
            if watched.state().wins.iter().any(|w| *w >= 2) {
                break;
            }
        }
        o.end(&watched, false);
        Ok(())
    })?;
    let c = count.0.lock().map_err(|e| e.to_string())?.clone();
    for name in [
        "match.start",
        "round.start",
        "cast",
        "hit",
        "snapshot",
        "match.end",
    ] {
        if c.get(name).copied().unwrap_or(0) == 0 {
            return Err(format!("no {name} event; got {c:?}"));
        }
    }
    if c.get("match.start") != Some(&2) || c.get("match.end") != Some(&1) {
        return Err(format!("one start per seat and one end: {c:?}"));
    }
    if c.get("arena::metrics").copied().unwrap_or(0) == 0 {
        return Err(format!("no metric events: {c:?}"));
    }
    Ok(())
}

#[test]
fn with_nothing_listening_the_observer_is_off() {
    let mut duel = Duel::with_brains(config(3));
    let mut o = Observer::start(&duel, "quiet", ["a", "b"]);
    for _ in 0..120 {
        o.step(&mut duel, TICK);
    }
    o.end(&duel, true);
}

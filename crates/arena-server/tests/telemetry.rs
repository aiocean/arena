//! A live match reports its whole fight: the server's step path (`Match::observe`, then `Match::step` tick after tick)
//! keeps the duel observer to the end, so its casts, hits, snapshots and round ends reach telemetry and its
//! `match.end` carries a real result, not "abandoned".

use arena_server::match_run::Match;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// Every event's message counted, and each `match.end`'s result.
#[derive(Clone, Default)]
struct Seen {
    counts: Arc<Mutex<BTreeMap<String, u32>>>,
    results: Arc<Mutex<Vec<String>>>,
    causes: Arc<Mutex<BTreeMap<String, u32>>>,
}

#[derive(Default)]
struct Fields {
    message: String,
    result: String,
    cause: String,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "result" {
            self.result = value.to_string();
        }
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{value:?}"),
            "result" => self.result = format!("{value:?}").trim_matches('"').to_string(),
            "cause" => self.cause = format!("{value:?}").trim_matches('"').to_string(),
            _ => {}
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for Seen {
    fn on_event(&self, e: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut f = Fields::default();
        e.record(&mut f);
        if f.message == "match.end"
            && let Ok(mut r) = self.results.lock()
        {
            r.push(f.result.clone());
        }
        if f.message == "dot"
            && let Ok(mut c) = self.causes.lock()
        {
            *c.entry(f.cause.clone()).or_insert(0) += 1;
        }
        let key = if f.message.is_empty() {
            e.metadata().target().to_string()
        } else {
            f.message
        };
        if let Ok(mut c) = self.counts.lock() {
            *c.entry(key).or_insert(0) += 1;
        }
    }
}

const SEED: u32 = 3_121_955_193;

#[test]
fn a_live_match_reports_its_whole_fight() -> Result<(), String> {
    let seen = Seen::default();
    let subscriber = tracing_subscriber::registry().with(seen.clone());
    let ticks = tracing::subscriber::with_default(subscriber, || {
        // a seed whose fight has burns and bleeds
        let mut m = Match::new(SEED, ["house-a".into(), "house-b".into()]);
        m.observe("test-live");
        let mut ticks = 0;
        while let Some(t) = m.step() {
            ticks += 1;
            if t.ended.is_some() {
                break;
            }
        }
        ticks
    });
    let c = seen.counts.lock().map_err(|e| e.to_string())?.clone();
    let results = seen.results.lock().map_err(|e| e.to_string())?.clone();
    if ticks < 600 {
        return Err(format!("the match ended after only {ticks} ticks"));
    }
    let n = |name: &str| c.get(name).copied().unwrap_or(0);
    for name in ["cast", "hit", "round.end", "match.start", "match.end"] {
        if n(name) == 0 {
            return Err(format!("no {name} event in {ticks} ticks; got {c:?}"));
        }
    }
    // a snapshot of each fighter each second of fight time: the whole fight, not only its first tick
    let seconds = ticks as f64 / 60.0;
    if (n("snapshot") as f64) < 2.0 * (seconds - 1.0) {
        return Err(format!(
            "{} snapshots in {seconds:.1} s; got {c:?}",
            n("snapshot")
        ));
    }
    if n("match.end") != 1 || results.iter().any(|r| r == "abandoned" || r.is_empty()) {
        return Err(format!("one match.end with a result: {results:?}"));
    }
    // every shown tick of damage over time says what it was: a burn, a bleed, both, or a curse's strike
    let causes = seen.causes.lock().map_err(|e| e.to_string())?.clone();
    if causes.is_empty()
        || causes
            .keys()
            .any(|c| !["burn", "bleed", "burn+bleed", "curse"].contains(&c.as_str()))
    {
        return Err(format!("a dot with no known cause: {causes:?}"));
    }
    Ok(())
}

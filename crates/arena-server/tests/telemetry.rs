//! A live match reports its whole fight: the server's step path (`Match::observe`, then `Match::step` tick after tick)
//! keeps the duel observer to the end, so its casts, hits, snapshots and round ends reach telemetry and its
//! `match.end` carries a real result, not "abandoned". Its `match.start` rows name each seat's bot and what runs it,
//! and the house AI's seat reports its `bot.end` through the ONNX meter.

use arena_server::match_run::{Bot, Match, Pairing, Seats};
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
    /// Each `match.start` row's (seat, bot, runtime, build), and each `bot.end`'s (bot, runtime, decisions).
    starts: Arc<Mutex<Vec<Start>>>,
    bot_ends: Arc<Mutex<Vec<(String, String, String)>>>,
}

/// A `match.start` row: its seat, bot, runtime and build.
type Start = (String, String, String, String);

#[derive(Default)]
struct Fields {
    message: String,
    result: String,
    cause: String,
    seat: String,
    bot: String,
    runtime: String,
    build: String,
    decisions: String,
}

impl Fields {
    fn set(&mut self, name: &str, value: String) {
        match name {
            "message" => self.message = value,
            "result" => self.result = value,
            "cause" => self.cause = value,
            "seat" => self.seat = value,
            "bot" => self.bot = value,
            "runtime" => self.runtime = value,
            "build" => self.build = value,
            "decisions" => self.decisions = value,
            _ => {}
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.set(field.name(), value.to_string());
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.set(field.name(), value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.set(
            field.name(),
            format!("{value:?}").trim_matches('"').to_string(),
        );
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
        if f.message == "match.start"
            && let Ok(mut r) = self.starts.lock()
        {
            r.push((
                f.seat.clone(),
                f.bot.clone(),
                f.runtime.clone(),
                f.build.clone(),
            ));
        }
        if f.message == "bot.end"
            && let Ok(mut r) = self.bot_ends.lock()
        {
            r.push((f.bot.clone(), f.runtime.clone(), f.decisions.clone()));
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

const SEED: u32 = 6;
const PAIRING: Pairing = Pairing {
    house_seat: 1,
    opponent: Bot::Brain,
};

fn seats() -> Result<Seats, String> {
    Seats::new().map_err(|e| e.0)
}

#[test]
fn a_live_match_reports_its_whole_fight() -> Result<(), String> {
    let seen = Seen::default();
    let subscriber = tracing_subscriber::registry().with(seen.clone());
    let ticks = tracing::subscriber::with_default(subscriber, || {
        // a seed whose fight has burns and bleeds
        let mut m = Match::new(SEED, PAIRING, &seats()?);
        m.observe("test-live", "abc1234");
        let mut ticks = 0;
        while let Some(t) = m.step() {
            ticks += 1;
            if t.ended.is_some() {
                break;
            }
        }
        Ok::<_, String>(ticks)
    })?;
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
    // a snapshot of each fighter each second of fight time: the whole fight, not only its first tick. Each round's
    // clock starts again, so a round's last part second and its turn to the next take no snapshot: a second a round.
    let seconds = ticks as f64 / 60.0;
    let rounds = n("round.end").max(1) as f64;
    if (n("snapshot") as f64) < 2.0 * (seconds - rounds) {
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
    // a row a seat: the house AI on ONNX in its seat, the mind built in in the other, the build on both
    let starts = seen.starts.lock().map_err(|e| e.to_string())?.clone();
    let want = |seat: usize, bot: Bot| {
        (
            seat.to_string(),
            bot.name().to_string(),
            bot.runtime().to_string(),
            "abc1234".to_string(),
        )
    };
    let h = PAIRING.house_seat;
    if starts != [want(0, PAIRING.bots()[0]), want(1, PAIRING.bots()[1])] || starts[h].1 != "house"
    {
        return Err(format!("match.start rows: {starts:?}"));
    }
    // the house AI's instance ends with the match: one bot.end, through the ONNX meter, with its decisions
    let ends = seen.bot_ends.lock().map_err(|e| e.to_string())?.clone();
    if ends.len() != 1
        || ends[0].0 != "house"
        || ends[0].1 != "onnx"
        || ends[0].2.parse::<u64>().unwrap_or(0) == 0
    {
        return Err(format!(
            "one bot.end for the house AI, runtime onnx: {ends:?}"
        ));
    }
    Ok(())
}

/// A match stopped from outside (the server shutting down) still ends in telemetry, as `server_stopped`, once.
#[test]
fn a_stopped_match_ends_as_server_stopped() -> Result<(), String> {
    let seen = Seen::default();
    let subscriber = tracing_subscriber::registry().with(seen.clone());
    tracing::subscriber::with_default(subscriber, || {
        let mut m = Match::new(SEED, PAIRING, &seats()?);
        m.observe("test-stopped", "abc1234");
        for _ in 0..120 {
            m.step();
        }
        m.stop("server_stopped");
        // stopping twice, or dropping it after, reports nothing more
        m.stop("server_stopped");
        drop(m);
        Ok::<_, String>(())
    })?;
    let results = seen.results.lock().map_err(|e| e.to_string())?.clone();
    if results != ["server_stopped"] {
        return Err(format!("one match.end, server_stopped: {results:?}"));
    }
    Ok(())
}

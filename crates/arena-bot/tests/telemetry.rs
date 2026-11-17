//! What the hosts report of a bot: `bot.end` with its percentiles, from the ONNX path as from the wasm one.
#![cfg(any(feature = "host", feature = "onnx"))]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

/// An event's target, name (`message`) and fields, as text.
type Seen = Arc<Mutex<Vec<(String, BTreeMap<String, String>)>>>;

struct Catch(Seen);

struct Fields(BTreeMap<String, String>);

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.insert(field.name().to_string(), value.to_string());
    }
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .insert(field.name().to_string(), format!("{value:?}"));
    }
}

impl<S: tracing::Subscriber> Layer<S> for Catch {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let mut f = Fields(BTreeMap::new());
        event.record(&mut f);
        if let Ok(mut seen) = self.0.lock() {
            seen.push((event.metadata().target().to_string(), f.0));
        }
    }
}

/// The events `run` raises.
fn caught(run: impl FnOnce()) -> Vec<(String, BTreeMap<String, String>)> {
    let seen = Seen::default();
    let subscriber = tracing_subscriber::registry().with(Catch(Arc::clone(&seen)));
    tracing::subscriber::with_default(subscriber, run);
    seen.lock().map(|s| s.clone()).unwrap_or_default()
}

fn bot_end(events: &[(String, BTreeMap<String, String>)]) -> BTreeMap<String, String> {
    let ends: Vec<_> = events
        .iter()
        .filter(|(t, f)| {
            t == "arena::bot" && f.get("message").map(String::as_str) == Some("bot.end")
        })
        .collect();
    assert_eq!(ends.len(), 1, "one bot.end: {events:?}");
    ends[0].1.clone()
}

fn number(f: &BTreeMap<String, String>, k: &str) -> f64 {
    f.get(k).and_then(|v| v.parse().ok()).unwrap_or(f64::NAN)
}

#[cfg(feature = "onnx")]
#[test]
fn the_onnx_path_reports_bot_end_like_a_module() {
    use arena_bot::onnx::Onnx;
    use arena_bot::{ACTIONS, OBS};
    let house = Onnx::house();
    assert!(house.is_ok(), "the house AI loads");
    let Ok(house) = house else { return };
    let events = caught(|| {
        let bot = house.decider();
        let mut b = bot.borrow_mut();
        assert!(b.start(7).is_ok() && b.round().is_ok());
        for i in 0..50 {
            let obs = [i as f32 / 50.0; OBS];
            assert!(b.decide(&obs, &[true; ACTIONS], 0.5).is_ok());
        }
    });
    let end = bot_end(&events);
    assert_eq!(end.get("runtime").map(String::as_str), Some("onnx"));
    assert_eq!(end.get("bot").map(String::as_str), Some("house"));
    assert_eq!(number(&end, "decisions"), 50.0);
    assert_eq!(number(&end, "matches"), 1.0);
    assert_eq!(number(&end, "faults"), 0.0);
    let (p50, p95, p99, max) = (
        number(&end, "think_p50_ms"),
        number(&end, "think_p95_ms"),
        number(&end, "think_p99_ms"),
        number(&end, "think_max_ms"),
    );
    assert!(
        0.0 < p50 && p50 <= p95 && p95 <= p99 && p99 <= max,
        "{end:?}"
    );
    for k in ["fuel_mean", "fuel_p50", "fuel_p95", "fuel_max", "fuel_cap"] {
        assert!(!end.contains_key(k), "ONNX has no fuel: {k} is left out");
    }
    // each decision is a think-time metric, by runtime and bot
    let metrics = events
        .iter()
        .filter(|(t, f)| {
            t == "arena::metrics" && f.contains_key("histogram.arena_bot_think_seconds")
        })
        .filter(|(_, f)| f.get("runtime").map(String::as_str) == Some("onnx"))
        .count();
    assert_eq!(metrics, 50);
}

#[cfg(feature = "host")]
#[test]
fn a_module_reports_its_fuel_and_faults() {
    use arena_bot::host::Runner;
    use arena_bot::{ACTIONS, OBS, admit};
    use arena_duel::seat::Decider;
    // decision k loops k * 1000 times; the 5th traps
    let wat = r#"(module
  (memory (export "memory") 1)
  (func (export "arena_abi") (result i32) i32.const 3)
  (func (export "arena_input") (result i32) i32.const 1024)
  (func (export "arena_start") (param i32))
  (func (export "arena_round"))
  (func (export "arena_decide") (param $u f32) (result i32) (local $n i32)
    (i32.store (i32.const 0) (i32.add (i32.load (i32.const 0)) (i32.const 1)))
    (if (i32.eq (i32.load (i32.const 0)) (i32.const 5)) (then unreachable))
    (local.set $n (i32.mul (i32.load (i32.const 0)) (i32.const 1000)))
    (loop $l (local.set $n (i32.sub (local.get $n) (i32.const 1))) (br_if $l (local.get $n)))
    i32.const 0))"#;
    let bytes = wat::parse_str(wat).unwrap_or_default();
    let runner = admit(&bytes).ok().and_then(|a| Runner::new(&a).ok());
    assert!(runner.is_some(), "the module is admitted");
    let Some(runner) = runner else { return };
    let runner = runner.named("looper");
    let events = caught(|| {
        let bot = runner.instance();
        assert!(bot.is_ok(), "an instance");
        let Ok(mut bot) = bot else { return };
        assert!(bot.start(1).is_ok());
        for _ in 0..10 {
            let _ = bot.decide(&[0.0; OBS], &[true; ACTIONS], 0.5);
        }
    });
    let end = bot_end(&events);
    assert_eq!(end.get("runtime").map(String::as_str), Some("wasm"));
    assert_eq!(end.get("bot").map(String::as_str), Some("looper"));
    assert_eq!(number(&end, "decisions"), 10.0);
    assert_eq!(number(&end, "faults"), 1.0);
    let (p50, p95, most, cap) = (
        number(&end, "fuel_p50"),
        number(&end, "fuel_p95"),
        number(&end, "fuel_max"),
        number(&end, "fuel_cap"),
    );
    assert!(
        0.0 < p50 && p50 < p95 && p95 <= most && most < cap,
        "{end:?}"
    );
    assert_eq!(number(&end, "fuel_most"), most);
    let fault = events
        .iter()
        .find(|(_, f)| f.get("message").map(String::as_str) == Some("bot.fault"));
    assert_eq!(
        fault.and_then(|(_, f)| f.get("kind")).map(String::as_str),
        Some("trap")
    );
}

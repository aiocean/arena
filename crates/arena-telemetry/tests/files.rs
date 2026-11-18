//! What the file layer writes: a log line, a span line and metric lines, each in its service's file of the day, in
//! the documented shape, and read back by `read::scan`.

use arena_telemetry::read::{Filter, Scope, Stats, field, scan};
use arena_telemetry::{Options, layer, target, time};
use serde_json::Value;
use std::time::Duration;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;

#[test]
fn logs_spans_and_metrics_land_in_their_files() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("arena-telemetry-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let options = Options {
        metrics_every: Duration::from_secs(3600),
        ..Options::default()
    };
    let (file, guard) = layer("arena-test", &options, &dir);
    let subscriber = tracing_subscriber::registry().with(file);
    tracing::subscriber::with_default(subscriber, || {
        let span =
            tracing::info_span!(target: target::MATCH, "match", match_id = "m-1", seed = 7_u32);
        let _in = span.enter();
        tracing::event!(target: target::MATCH, Level::INFO, { match_id = "m-1", seat = 0_u32, life = 1440.0 }, "match.start");
        tracing::event!(target: target::BOT, Level::INFO, { bot = "house", runtime = "onnx", decisions = 9_u64 }, "bot.end");
        tracing::warn!(late_ms = 300_u64, "tick schedule resynced");
        // filtered out: below the export filter's info
        tracing::debug!("noise");
        for v in [0.001, 0.002, 0.004] {
            tracing::event!(target: target::METRICS, Level::INFO, {
                histogram.arena_bot_think_seconds = v, runtime = "onnx", bot = "house",
            });
        }
        tracing::event!(target: target::METRICS, Level::INFO, { monotonic_counter.arena_casts_total = 2_u64, skill = "lotus" });
        tracing::event!(target: target::METRICS, Level::INFO, { monotonic_counter.arena_casts_total = 3_u64, skill = "lotus" });
    });
    guard.flush();
    assert_eq!(guard.dropped(), 0);
    drop(guard);

    let today = time::day(time::now_us());
    for service in ["arena-test", "arena-bot"] {
        let p = dir.join(service).join(format!("{today}.jsonl"));
        if !p.exists() {
            return Err(format!("no {}", p.display()));
        }
    }
    let mut lines: Vec<Value> = Vec::new();
    scan(&dir, &Scope::default(), |_, raw| {
        if let Ok(v) = serde_json::from_str(raw) {
            lines.push(v);
        }
        true
    })
    .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    let named = |n: &str| lines.iter().find(|l| l["name"] == n).cloned();

    let start = named("match.start").ok_or(format!("no match.start: {lines:?}"))?;
    assert_eq!(start["kind"], "log");
    assert_eq!(start["service"], "arena-test");
    assert_eq!(start["level"], "INFO");
    assert_eq!(start["target"], "arena::match");
    assert_eq!(start["attrs"]["seat"], 0);
    assert_eq!(start["attrs"]["life"], 1440.0);
    let span = named("match").ok_or("no match span")?;
    assert_eq!(span["kind"], "span");
    assert_eq!(span["attrs"]["match_id"], "m-1");
    assert_eq!(
        span["trace_id"], start["trace_id"],
        "the event is in the span's trace"
    );
    assert_eq!(span["span_id"], start["span_id"]);
    assert!(span["duration_ms"].as_f64().is_some_and(|d| d >= 0.0));
    assert!(span["start"].is_string() && span["end"].is_string());

    let bot = named("bot.end").ok_or("no bot.end")?;
    assert_eq!(
        bot["service"], "arena-bot",
        "the bot host's events are filed under arena-bot"
    );
    assert_eq!(
        named("tick schedule resynced").map(|l| l["level"].clone()),
        Some("WARN".into())
    );
    assert!(named("noise").is_none());
    assert!(named("telemetry.start").is_none(), "only init writes it");

    let think = named("arena_bot_think_seconds").ok_or("no think metric")?;
    assert_eq!(think["kind"], "metric");
    assert_eq!(think["type"], "histogram");
    assert_eq!(think["count"], 3);
    assert_eq!(think["max"], 0.004);
    assert_eq!(think["attrs"]["bot"], "house");
    let mut s = Stats::default();
    s.add_line(&think, "value");
    let m = s.summary().ok_or("stats")?;
    assert!((m.p50 / 0.002 - 1.0).abs() < 0.05, "{m:?}");
    let casts = named("arena_casts_total").ok_or("no casts metric")?;
    assert_eq!(casts["type"], "counter");
    assert_eq!(casts["value"], 5.0);
    assert_eq!(field(&casts, "skill"), Some(&Value::from("lotus")));

    let f = Filter {
        kinds: vec!["metric".into()],
        ..Filter::default()
    };
    assert_eq!(lines.iter().filter(|l| f.matches(l)).count(), 2);
    Ok(())
}

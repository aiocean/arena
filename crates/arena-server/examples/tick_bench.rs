//! Where a live tick's time goes: the duel step (with the observer or without), and how late a paced loop wakes.
//!
//! ```text
//! cargo run --release -p arena-server --example tick_bench -- steps [--matches 5] [--observe]
//! cargo run --release -p arena-server --example tick_bench -- pace [--seconds 20]
//! taskpolicy -b cargo run --release -p arena-server --example tick_bench -- pace   # as a throttled background job
//! ```
//!
//! `steps` fights matches as fast as they go and prints each step's wall time; `--observe` puts the duel observer on
//! them under a subscriber that takes every event and drops it (the events are built, nothing is exported). `pace`
//! runs a match on the server's own pacer (`arena_server::pace`) and prints how late each tick starts on the wall
//! clock.

use arena_duel::duel::TICK;
use arena_server::match_run::Match;
use arena_server::pace::Pacer;
use std::time::{Duration, Instant};
use tracing_subscriber::layer::{Layer, SubscriberExt};

/// Takes every event, keeps none: the observer's cost without an exporter.
struct Sink;
impl<S: tracing::Subscriber> Layer<S> for Sink {}

fn percentiles(name: &str, mut v: Vec<f64>) {
    if v.is_empty() {
        println!("{name}: no samples");
        return;
    }
    v.sort_by(f64::total_cmp);
    let at = |q: f64| v[((v.len() - 1) as f64 * q + 0.5) as usize] * 1e3;
    println!(
        "{name}: n {}  p50 {:.3} ms  p95 {:.3} ms  p99 {:.3} ms  max {:.3} ms",
        v.len(),
        at(0.5),
        at(0.95),
        at(0.99),
        at(1.0)
    );
}

fn flag(args: &[String], name: &str) -> Option<u64> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
}

fn steps(matches: u64, observe: bool) -> Vec<f64> {
    let mut took = Vec::new();
    for n in 0..matches {
        let seed = (0x9E37_79B9_u64.wrapping_mul(n + 1) as u32) | 1;
        let mut m = Match::new(seed, ["a".into(), "b".into()]);
        if observe {
            m.observe(&format!("bench-{n}"));
        }
        loop {
            let t = Instant::now();
            let Some(tick) = m.step() else { break };
            took.push(t.elapsed().as_secs_f64());
            if tick.ended.is_some() {
                break;
            }
        }
    }
    took
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("steps") => {
            let matches = flag(&args, "--matches").unwrap_or(5);
            let observe = args.iter().any(|a| a == "--observe");
            let took = if observe {
                let sub = tracing_subscriber::registry().with(Sink);
                tracing::subscriber::with_default(sub, || steps(matches, true))
            } else {
                steps(matches, false)
            };
            percentiles(if observe { "step, observed" } else { "step" }, took);
        }
        Some("pace") => {
            let seconds = flag(&args, "--seconds").unwrap_or(20);
            let rt = match tokio::runtime::Builder::new_current_thread().enable_time().build() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("runtime: {e}");
                    return;
                }
            };
            let (late, resyncs) = rt.block_on(async move {
                let mut m = Match::new(12_345, ["a".into(), "b".into()]);
                let mut pacer = Pacer::new(Instant::now(), Duration::from_secs_f64(TICK));
                let end = Instant::now() + Duration::from_secs(seconds);
                let mut late = Vec::new();
                while Instant::now() < end {
                    let wait = pacer.wait(Instant::now());
                    if !wait.is_zero() {
                        tokio::time::sleep(wait).await;
                    }
                    let tick = pacer.begin(Instant::now());
                    late.push(tick.late.as_secs_f64());
                    match m.step() {
                        Some(t) if t.ended.is_none() => {}
                        _ => m = Match::new(m.tick() ^ 0x5555, ["a".into(), "b".into()]),
                    }
                }
                (late, pacer.resyncs())
            });
            percentiles("tick start, behind the wall clock", late);
            println!("resyncs: {resyncs}");
        }
        _ => eprintln!("usage: tick_bench steps [--matches N] [--observe] | pace [--seconds N]"),
    }
}

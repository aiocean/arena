//! The arena's match server: it fights house matches and publishes them live onto a MoQ relay, so every watching page
//! replays the same fight from its seed (`docs/specs/2026-11-17-live-matches-design.md`). It dials out to the relay, so
//! it opens no port and needs no fixed address.
//!
//! Run (the connect URL carries the relay's publish token in its path):
//!
//! ```text
//! MOQ_CONNECT="https://draft-18.cloudflare.mediaoverquic.com/<PUBLISH_JWT>" cargo run -p arena-server
//! cargo run -p arena-server -- --relay-env ops/arena/moq-relay.env    # MOQ_PUBLISH_TOKEN (and MOQ_RELAY_HOST) from a file
//! ```
//!
//! The file form keeps the token out of a service's definition: `--relay-env` reads `KEY=VALUE` lines, takes
//! `MOQ_PUBLISH_TOKEN`, and dials `https://<MOQ_RELAY_HOST>/<token>` (the host defaults to Cloudflare's draft-18 relay).
//!
//! It loops: a fresh seed, a house-vs-house match published as it is fought, then the next. Each match is one MoQ
//! broadcast (`arena_link::moq`), its track a run of postcard `arena_link::Frame`s. A new group every
//! `GROUP_TICKS` opens with a `Keyframe` (the whole match so far), so a page joining late rebuilds from the start.
//!
//! Ticks run on absolute deadlines (`arena_server::pace`). A match that ends finishes its track, so a viewer sees a
//! clean end (`arena_server::publish`). SIGTERM (launchd stopping or restarting the job) or Ctrl-C stops the loop at a
//! tick's edge: the match's track is aborted, its observer reports `match.end` as `server_stopped`, and the telemetry
//! is flushed before the process exits.
//!
//! Run by launchd, the job needs `ProcessType` `Interactive`: launchd's default for a job without one throttles its
//! timers, and a tick's wake then comes 40 ms late at the median and 200 ms at worst (`examples/tick_bench.rs`).
//!
//! Telemetry (`arena_telemetry`, `ops/observe/README.md`): with `ARENA_OTLP_ENDPOINT` set it exports each match
//! through the duel observer, and its own health: a tick's publish time, how late it runs, ticks missed and schedule
//! resyncs, the relay's subscription.

use anyhow::{Context, Result};
use arena_link::moq::{GROUP_TICKS, THROUGH_EVERY, broadcast};
use arena_link::Frame;
use arena_server::match_run::Match;
use arena_server::pace::Pacer;
use arena_server::publish::Publisher;
use std::time::{Duration, Instant};
use tokio::sync::watch;
use url::Url;

use arena_duel::duel::TICK;

/// The names the house corners fight under.
const CORNERS: [&str; 2] = ["house-thunder", "house-frost"];

/// Why the server is stopping, sent to the match loop: `None` while it runs.
type Stop = watch::Receiver<Option<&'static str>>;

// The match loop runs on the main thread (a match is not Send); the relay connection's tasks run on the workers, so
// QUIC's sends and receives never wait behind a tick, nor a tick behind them.
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let telemetry = arena_telemetry::init(
        "arena-server",
        arena_telemetry::Options {
            version: env!("CARGO_PKG_VERSION"),
            filter: "arena_server=info,moq_tokio=warn".to_string(),
            export_filter: "info,moq_tokio=warn,moq_net=warn".to_string(),
            ..Default::default()
        },
    );

    let url = connect_url()?;

    // connect::Config is non_exhaustive, so it is built from its default and then pointed at the relay
    #[allow(clippy::field_reassign_with_default)]
    let client = {
        let mut cfg = moq_tokio::connect::Config::default();
        cfg.url = Some(url.clone());
        cfg.init(Default::default()).context("building the MoQ client")?
    };

    let origin = moq_tokio::origin::spawn();
    let connection = client.with_publisher(&origin).connect(url.clone());
    // the token rides in the path, so only the host is logged
    tracing::info!(relay = url.host_str().unwrap_or("?"), "publishing to the relay");

    // the match loop stops at a tick's edge when told why: it ends its match's track and observer, then returns
    let (stop, stopped) = watch::channel(None);
    let matches = run_matches(&origin, stopped);
    tokio::pin!(matches);
    let res = tokio::select! {
        res = &mut matches => res,
        res = connection.closed() => {
            let _ = stop.send(Some("connection_closed"));
            let _ = (&mut matches).await;
            res.context("the relay connection closed")
        }
        signal = shutdown_signal() => {
            tracing::info!(signal, "stopping");
            let _ = stop.send(Some("server_stopped"));
            matches.await
        }
    };
    if let Err(e) = &res {
        tracing::error!(error = %format!("{e:#}"), "server stopped");
    }
    // send what is batched before the process goes (the guard's drop shuts the exporters down after)
    telemetry.flush();
    drop(telemetry);
    res
}

/// SIGTERM (launchd stopping or restarting the job) or Ctrl-C: the signal's name.
async fn shutdown_signal() -> &'static str {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => tokio::select! {
                _ = term.recv() => "SIGTERM",
                _ = tokio::signal::ctrl_c() => "SIGINT",
            },
            Err(e) => {
                tracing::warn!(error = %e, "no SIGTERM handler; Ctrl-C only");
                let _ = tokio::signal::ctrl_c().await;
                "SIGINT"
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
        "SIGINT"
    }
}

/// One match after another, each on a fresh seed, until told to stop.
async fn run_matches(origin: &moq_net::origin::Producer, mut stop: Stop) -> Result<()> {
    let mut n: u64 = 0;
    loop {
        if stop.borrow().is_some() {
            return Ok(());
        }
        n += 1;
        // a seed from the wall clock and the match number: unpredictable enough for a demo, and in the match id so a
        // watcher can name the fight.
        let seed = (seed_now() ^ n.wrapping_mul(0x9E37_79B9)) as u32 | 1;
        let match_id = format!("m{n}-{seed}");
        tracing::info!(match_id, seed, "new match");
        if let Err(e) = run_one(origin, &match_id, seed, &mut stop).await {
            tracing::warn!(match_id, error = %e, "match failed");
        }
    }
}

/// Fight and publish one match, paced to the wall clock, until it ends or the server stops.
async fn run_one(origin: &moq_net::origin::Producer, match_id: &str, seed: u32, stop: &mut Stop) -> Result<()> {
    let bc = origin
        .create_broadcast(broadcast(match_id))
        .context("create broadcast")?;
    let mut publisher = Publisher::new(bc)?;
    publisher
        .broadcast()
        .announce(Default::default())
        .context("announce")?;

    let corners = [CORNERS[0].to_string(), CORNERS[1].to_string()];
    let mut m = Match::new(seed, corners);
    m.observe(match_id);
    let mut watched = false;
    let mut frames: u64 = 0;
    let mut in_group: u32 = 0;
    // open the first group with the (empty) keyframe
    publisher.keyframe(&Frame::Keyframe(m.snapshot()))?;

    let start = Instant::now();
    let mut pacer = Pacer::new(start, Duration::from_secs_f64(TICK));
    loop {
        // tick n is due n ticks after the start, reckoned from the start each time, so a late wake is made up by the
        // ticks after it and never adds up; far behind, the pacer starts its schedule afresh instead of bursting
        let wait = pacer.wait(Instant::now());
        if wait.is_zero() {
            // behind: step at once, but let the relay's tasks and the stop signal in between ticks
            tokio::task::yield_now().await;
        } else {
            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                _ = stop.changed() => {}
            }
        }
        if let Some(why) = *stop.borrow() {
            // the server is stopping mid-match: the observer ends it as `why`, the track is cut off (aborted)
            m.stop(why);
            publisher.abort();
            tracing::info!(match_id, ticks = m.tick(), why, "match stopped");
            return Ok(());
        }

        let began = Instant::now();
        let at = pacer.begin(began);
        if at.missed {
            tracing::event!(target: "arena::metrics", tracing::Level::INFO, {
                monotonic_counter.arena_server_missed_ticks_total = 1_u64,
            });
        }
        if at.resynced {
            tracing::event!(target: "arena::metrics", tracing::Level::INFO, {
                monotonic_counter.arena_server_resyncs_total = 1_u64,
            });
            tracing::warn!(match_id, tick = m.tick(), late_ms = at.late.as_secs_f64() * 1e3, "tick schedule resynced");
        }
        // roll the group before stepping, so its keyframe holds everything up to now and the tick's decisions follow it
        // once, as incremental frames
        if in_group >= GROUP_TICKS {
            publisher.keyframe(&Frame::Keyframe(m.snapshot()))?;
            in_group = 0;
        }
        let Some(tick) = m.step() else { break };
        in_group += 1;
        frames += (tick.decisions.len() + tick.forfeits.len()) as u64;
        for d in tick.decisions {
            publisher.write(&Frame::Decision(d))?;
        }
        for f in tick.forfeits {
            publisher.write(&Frame::Forfeit(f))?;
        }
        if tick.ended.is_some() || m.tick().is_multiple_of(THROUGH_EVERY) {
            publisher.write(&Frame::Through(arena_link::Through {
                tick: m.tick(),
                sent_ms: now_ms(),
            }))?;
        }
        if let Some(ended) = tick.ended {
            publisher.write(&Frame::End(ended))?;
            break;
        }
        // a tick's step and publish, and how late it lands: the state at fight time n*TICK, written after the wall
        // clock's n*TICK from the start, is late by the difference
        let now = Instant::now();
        tracing::event!(target: "arena::metrics", tracing::Level::INFO, {
            histogram.arena_server_publish_seconds = (now - began).as_secs_f64(),
            histogram.arena_server_late_seconds = now.saturating_duration_since(pacer.due(at.index + 1)).as_secs_f64(),
        });
        if publisher.is_used() != watched {
            watched = !watched;
            tracing::info!(match_id, tick = m.tick(), watched, "relay subscription");
        }
    }
    let groups = publisher.groups();
    // the last group, then the track: a viewer reads to the end and sees the track end cleanly
    publisher.finish()?;
    let wall_seconds = start.elapsed().as_secs_f64();
    tracing::info!(match_id, ticks = m.tick(), groups, frames, wall_seconds, missed = pacer.missed(),
        resyncs = pacer.resyncs(), "match done");
    Ok(())
}

/// The relay's default host: Cloudflare's MoQ relay speaking draft 18.
const RELAY_HOST: &str = "draft-18.cloudflare.mediaoverquic.com";

/// Where to publish: `MOQ_CONNECT`, the first argument as a URL, or `--relay-env <file>`.
fn connect_url() -> Result<Url> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--relay-env") {
        let path = args.get(i + 1).context("--relay-env needs a file")?;
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
        let value = |key: &str| {
            text.lines()
                .filter_map(|l| l.split_once('='))
                .find(|(k, _)| k.trim() == key)
                .map(|(_, v)| v.trim().trim_matches('"').to_string())
        };
        let token = value("MOQ_PUBLISH_TOKEN").with_context(|| format!("no MOQ_PUBLISH_TOKEN in {path}"))?;
        let host = value("MOQ_RELAY_HOST").unwrap_or_else(|| RELAY_HOST.to_string());
        return format!("https://{host}/{token}")
            .parse()
            .context("the relay URL did not parse");
    }
    std::env::var("MOQ_CONNECT")
        .ok()
        .or_else(|| args.first().cloned())
        .context("set MOQ_CONNECT, pass the connect URL, or --relay-env <file>")?
        .parse()
        .context("the connect URL did not parse")
}

/// The wall clock in milliseconds since the Unix epoch, stamped on each `Through` for the page's delay reading.
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn seed_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

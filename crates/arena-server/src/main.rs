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
//! Telemetry (`arena_telemetry`, `ops/observe/README.md`): with `ARENA_OTLP_ENDPOINT` set it exports each match
//! through the duel observer, and its own health: a tick's publish time, how late it runs, the relay's subscription.

use anyhow::{Context, Result};
use arena_link::moq::{GROUP_TICKS, THROUGH_EVERY, TRACK, broadcast};
use arena_link::{Frame, encode};
use arena_server::match_run::Match;
use moq_net::Timestamp;
use std::time::Duration;
use tokio::time::{Instant, sleep_until};
use url::Url;

use arena_duel::duel::TICK;

/// The names the house corners fight under.
const CORNERS: [&str; 2] = ["house-thunder", "house-frost"];

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let _telemetry = arena_telemetry::init(
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

    let res = tokio::select! {
        res = connection.closed() => res.context("the relay connection closed"),
        res = run_matches(&origin) => res,
    };
    if let Err(e) = &res {
        tracing::error!(error = %format!("{e:#}"), "server stopped");
    }
    res
}

/// One match after another, each on a fresh seed, forever (until Ctrl-C or the connection drops).
async fn run_matches(origin: &moq_net::origin::Producer) -> Result<()> {
    let mut n: u64 = 0;
    loop {
        n += 1;
        // a seed from the wall clock and the match number: unpredictable enough for a demo, and in the match id so a
        // watcher can name the fight.
        let seed = (seed_now() ^ n.wrapping_mul(0x9E37_79B9)) as u32 | 1;
        let match_id = format!("m{n}-{seed}");
        tracing::info!(match_id, seed, "new match");
        if let Err(e) = run_one(origin, &match_id, seed).await {
            tracing::warn!(match_id, error = %e, "match failed");
        }
    }
}

/// Fight and publish one match, paced to the wall clock.
async fn run_one(origin: &moq_net::origin::Producer, match_id: &str, seed: u32) -> Result<()> {
    let bc = origin
        .create_broadcast(broadcast(match_id))
        .context("create broadcast")?;
    let track = bc.create_track(TRACK, None).context("create track")?;
    bc.announce(Default::default()).context("announce")?;

    let corners = [CORNERS[0].to_string(), CORNERS[1].to_string()];
    let mut m = Match::new(seed, corners);
    m.observe(match_id);
    let mut watched = false;
    let mut frames: u64 = 0;

    let start = Instant::now();
    let mut group_seq: u64 = 0;
    let mut in_group: u32 = 0;
    // open the first group with the (empty) keyframe
    let mut group = track.create_group(group_seq.into()).context("group")?;
    group_seq += 1;
    group
        .write_frame(Timestamp::now(), encode(&Frame::Keyframe(m.snapshot())))
        .context("keyframe")?;

    loop {
        // roll the group before stepping, so its keyframe holds everything up to now and the tick's decisions follow it
        // once, as incremental frames
        if in_group >= GROUP_TICKS {
            group.finish().ok();
            group = track.create_group(group_seq.into()).context("group")?;
            group_seq += 1;
            group
                .write_frame(Timestamp::now(), encode(&Frame::Keyframe(m.snapshot())))
                .context("keyframe")?;
            in_group = 0;
        }
        let began = Instant::now();
        let Some(tick) = m.step() else { break };
        in_group += 1;
        frames += (tick.decisions.len() + tick.forfeits.len()) as u64;
        for d in tick.decisions {
            group
                .write_frame(Timestamp::now(), encode(&Frame::Decision(d)))
                .context("decision")?;
        }
        for f in tick.forfeits {
            group
                .write_frame(Timestamp::now(), encode(&Frame::Forfeit(f)))
                .context("forfeit")?;
        }
        if tick.ended.is_some() || m.tick().is_multiple_of(THROUGH_EVERY) {
            group
                .write_frame(Timestamp::now(), encode(&Frame::Through(arena_link::Through {
                    tick: m.tick(),
                    sent_ms: now_ms(),
                })))
                .context("through")?;
        }
        if let Some(ended) = tick.ended {
            group
                .write_frame(Timestamp::now(), encode(&Frame::End(ended)))
                .context("end")?;
            break;
        }
        // a tick's step and publish, how late it is on the wall clock, and whether the relay is taking the track
        let due = start + Duration::from_secs_f64(m.tick() as f64 * TICK);
        let now = Instant::now();
        tracing::event!(target: "arena::metrics", tracing::Level::INFO, {
            histogram.arena_server_publish_seconds = (now - began).as_secs_f64(),
            histogram.arena_server_late_seconds = now.saturating_duration_since(due).as_secs_f64(),
        });
        if track.is_used() != watched {
            watched = !watched;
            tracing::info!(match_id, tick = m.tick(), watched, "relay subscription");
        }
        // pace to the wall clock: fighting tick n should land n*TICK seconds after the match began
        sleep_until(due).await;
    }
    group.finish().ok();
    bc.close();
    let wall_seconds = start.elapsed().as_secs_f64();
    tracing::info!(match_id, ticks = m.tick(), groups = group_seq, frames, wall_seconds, "match done");
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

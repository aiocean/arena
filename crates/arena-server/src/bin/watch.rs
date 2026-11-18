//! A native watcher, the page's twin on the command line: it subscribes to the relay, takes the first live match
//! announced under `arena_link::moq::NAMESPACE`, folds its postcard frames into the match (`arena_link::Follow`), and
//! when the match ends replays it from its seed and decisions alone, checking the replay reaches the same end.
//!
//! ```text
//! MOQ_CONNECT="https://draft-18.cloudflare.mediaoverquic.com/<SUBSCRIBE_JWT>" cargo run -p arena-server --bin arena-watch
//! ```
//!
//! It exits 0 when the replayed match ends as the server said, 1 otherwise.

use anyhow::{Context, Result, bail};
use arena_duel::duel::{Duel, TICK};
use arena_duel::seat::{MakeSeat, Replay, Seat};
use arena_duel::view::view;
use arena_link::moq::{JOIN_REACH_MS, NAMESPACE, TRACK};
use arena_link::{Applied, Ended, Follow, decode};
use arena_server::match_run::{WINS_TO_TAKE, config_for, logs_from_snapshot};
use url::Url;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("arena_watch=info,moq_tokio=warn")
            }),
        )
        .init();

    let url: Url = std::env::var("MOQ_CONNECT")
        .ok()
        .or_else(|| std::env::args().nth(1))
        .context(
            "set MOQ_CONNECT (or pass the connect URL) to the relay, a subscribe token in its path",
        )?
        .parse()
        .context("the connect URL did not parse")?;

    // connect::Config is non_exhaustive, so it is built from its default and then pointed at the relay
    #[allow(clippy::field_reassign_with_default)]
    let client = {
        let mut cfg = moq_tokio::connect::Config::default();
        cfg.url = Some(url.clone());
        cfg.init(Default::default())
            .context("building the MoQ client")?
    };

    let origin = moq_tokio::origin::spawn();
    let connection = client.with_subscriber(origin.clone()).connect(url.clone());

    tokio::select! {
        res = connection.closed() => {
            res.context("the relay connection closed")?;
            bail!("the relay connection closed before a match ended")
        }
        res = watch(&origin) => {
            let ok = res?;
            if !ok {
                std::process::exit(1);
            }
            Ok(())
        }
    }
}

/// Follow the first match announced, then check its replay. True when the replay ends as the server said.
async fn watch(origin: &moq_net::origin::Producer) -> Result<bool> {
    let scope = moq_net::Patterns::from(
        moq_net::Pattern::subtree(NAMESPACE)
            .map_err(|e| anyhow::anyhow!("bad namespace: {e:?}"))?,
    );
    let consumer = origin
        .consume()
        .scope("", &scope)
        .map_err(|e| anyhow::anyhow!("scope: {e}"))?;
    let mut announced = consumer.announced();

    let prefix = loop {
        let update = announced.next().await.context("no more announcements")?;
        if update.kind.is_active() {
            break update.prefix;
        }
    };
    tracing::info!(broadcast = %prefix, "watching");
    let broadcast = consumer
        .request_broadcast(&prefix)
        .await
        .map_err(|e| anyhow::anyhow!("broadcast: {e}"))?;
    let mut track = broadcast
        .track(TRACK)
        .map_err(|e| anyhow::anyhow!("track: {e}"))?
        // reaching back over the last whole group, as the page does: joined at the live edge, the relay may hand over
        // the group being written from its middle, which is dropped
        .subscribe(
            moq_net::track::Subscription::default()
                .with_max_age(std::time::Duration::from_millis(JOIN_REACH_MS)),
        )
        .await
        .map_err(|e| anyhow::anyhow!("subscribe: {e}"))?;

    let mut follow = Follow::new();
    let mut frames = 0usize;
    'groups: while let Some(mut group) = track
        .recv_group()
        .await
        .map_err(|e| anyhow::anyhow!("group: {e}"))?
    {
        while let Some(frame) = group
            .read_frame()
            .await
            .map_err(|e| anyhow::anyhow!("frame: {e}"))?
        {
            frames += 1;
            let frame = decode(&frame.payload).context("a frame did not decode")?;
            match follow.apply(frame) {
                Applied::OtherRules(v) => bail!(
                    "the match was fought on rules {v}, this build's are {}",
                    arena_link::RULES_VERSION
                ),
                Applied::Taken | Applied::Ignored => {}
            }
            if follow.snapshot().is_some_and(|s| s.ended.is_some()) {
                break 'groups;
            }
        }
    }

    let snap = follow.snapshot().context("no keyframe came")?.clone();
    let ended = snap
        .ended
        .context("the stream ended before the match did")?;
    tracing::info!(
        frames,
        seed = snap.seed,
        decisions = snap.decisions.len(),
        ?ended,
        ticks = follow.through(),
        "match received; replaying"
    );

    let [log0, log1] = logs_from_snapshot(&snap);
    let make0: MakeSeat = Box::new(move |_s, _m| Box::new(Replay::new(log0)) as Box<dyn Seat>);
    let make1: MakeSeat = Box::new(move |_s, _m| Box::new(Replay::new(log1)) as Box<dyn Seat>);
    let mut duel = Duel::new_drawn(config_for(snap.seed, snap.arena), [make0, make1]);
    // step exactly as far as the server fought, then read the end the rules reached
    let ticks = follow.through();
    for _ in 0..ticks {
        duel.step(TICK);
    }
    let v = view(duel.state());
    let replayed = if v.wins[0] >= WINS_TO_TAKE {
        Ended::Winner(0)
    } else if v.wins[1] >= WINS_TO_TAKE {
        Ended::Winner(1)
    } else {
        Ended::Draw
    };
    let ok = replayed == ended;
    tracing::info!(server = ?ended, replay = ?replayed, ok, "replay checked");
    Ok(ok)
}

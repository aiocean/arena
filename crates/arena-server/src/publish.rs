//! One match's MoQ track, from its first keyframe to a clean end.
//!
//! A `Publisher` holds the match's broadcast, its one track (`arena_link::moq::TRACK`) and the group being written.
//! `keyframe` closes the open group and starts the next with a keyframe; `write` adds a frame to it. A match that
//! ends calls `finish`: the last group, then the track, are finished, so a viewer reads to the end and sees the track
//! end cleanly, then the broadcast closes. A publisher dropped before that (the server stopping, a write failing, the
//! relay connection dropping) aborts the group and the track instead, so a viewer sees the match cut off rather than
//! a producer that vanished (moq's "track::Producer dropped without finish() or abort()").

use anyhow::{Context, Result};
use arena_link::moq::TRACK;
use arena_link::{Frame, encode};
use moq_net::{Error, Timestamp, broadcast, group, track};

/// A match's broadcast and track, being written.
pub struct Publisher {
    bc: broadcast::Producer,
    track: Option<track::Producer>,
    group: Option<group::Producer>,
    /// The next group's sequence number: also how many groups have been opened.
    next_group: u64,
}

impl Publisher {
    /// Publish on `bc`: create its track. The caller announces the broadcast once this is in place.
    pub fn new(bc: broadcast::Producer) -> Result<Publisher> {
        let track = bc.create_track(TRACK, None).context("create track")?;
        Ok(Publisher {
            bc,
            track: Some(track),
            group: None,
            next_group: 0,
        })
    }

    /// The broadcast, to announce it.
    pub fn broadcast(&self) -> &broadcast::Producer {
        &self.bc
    }

    /// Whether the relay (or any consumer) is reading the track right now.
    pub fn is_used(&self) -> bool {
        self.track.as_ref().is_some_and(track::Producer::is_used)
    }

    /// Groups opened so far.
    pub fn groups(&self) -> u64 {
        self.next_group
    }

    /// Finish the open group, if any, and open the next with `keyframe` as its first frame.
    pub fn keyframe(&mut self, keyframe: &Frame) -> Result<()> {
        if let Some(g) = self.group.take() {
            g.finish().context("finish group")?;
        }
        let track = self.track.as_ref().context("the track is closed")?;
        let mut g = track
            .create_group(self.next_group.into())
            .context("create group")?;
        self.next_group += 1;
        g.write_frame(Timestamp::now(), encode(keyframe))
            .context("keyframe")?;
        self.group = Some(g);
        Ok(())
    }

    /// Write `frame` into the open group.
    pub fn write(&mut self, frame: &Frame) -> Result<()> {
        let g = self.group.as_mut().context("no group is open")?;
        g.write_frame(Timestamp::now(), encode(frame))
            .context("write frame")?;
        Ok(())
    }

    /// The match is over: finish the last group, then the track, then close the broadcast. A viewer reads every
    /// frame and then a clean end.
    pub fn finish(mut self) -> Result<()> {
        let group = self.group.take();
        let track = self.track.take();
        let res = (|| {
            if let Some(g) = &group {
                g.finish().context("finish group")?;
            }
            if let Some(t) = &track {
                t.finish().context("finish track")?;
            }
            Ok(())
        })();
        match res {
            Ok(()) => {
                // a finished producer drops quietly, and the broadcast's announcement is withdrawn
                drop(group);
                drop(track);
                self.bc.close();
                Ok(())
            }
            Err(e) => {
                // put them back so drop aborts them
                self.group = group;
                self.track = track;
                Err(e)
            }
        }
    }

    /// Stop publishing before the match ended: abort the open group and the track (a viewer sees the match cut
    /// off), then close the broadcast. Dropping a publisher does this.
    pub fn abort(mut self) {
        self.abort_now();
    }

    fn abort_now(&mut self) {
        if let Some(g) = self.group.take() {
            let _ = g.abort(Error::Cancel);
        }
        if let Some(t) = self.track.take() {
            let _ = t.abort(Error::Cancel);
        }
        self.bc.close();
    }
}

impl Drop for Publisher {
    fn drop(&mut self) {
        if self.track.is_some() || self.group.is_some() {
            self.abort_now();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context as _;
    use arena_link::{Through, decode};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tracing::field::{Field, Visit};
    use tracing_subscriber::layer::{Context, Layer, SubscriberExt};

    /// Counts moq's warnings about a producer dropped without finish or abort.
    #[derive(Clone, Default)]
    struct DropWarnings(Arc<AtomicUsize>);

    struct Message(String);
    impl Visit for Message {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.0 = format!("{value:?}");
            }
        }
    }

    impl<S: tracing::Subscriber> Layer<S> for DropWarnings {
        fn on_event(&self, e: &tracing::Event<'_>, _: Context<'_, S>) {
            let mut m = Message(String::new());
            e.record(&mut m);
            if m.0.contains("dropped without finish") {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn through(tick: u32) -> Frame {
        Frame::Through(Through { tick, sent_ms: 0 })
    }

    /// Read every group and frame a subscriber gets, then how the track ended: `Ok` clean, `Err` aborted.
    async fn drain(mut sub: track::Subscriber) -> (Vec<Vec<Frame>>, Result<(), Error>) {
        let mut groups = Vec::new();
        loop {
            match sub.recv_group().await {
                Ok(Some(mut g)) => {
                    let mut frames = Vec::new();
                    while let Ok(Some(f)) = g.read_frame().await {
                        if let Ok(frame) = decode(&f.payload) {
                            frames.push(frame);
                        }
                    }
                    groups.push(frames);
                }
                Ok(None) => return (groups, Ok(())),
                Err(e) => return (groups, Err(e)),
            }
        }
    }

    fn run<T>(f: impl std::future::Future<Output = T>) -> anyhow::Result<T> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()?;
        Ok(rt.block_on(f))
    }

    fn reader(p: &Publisher) -> anyhow::Result<track::Subscriber> {
        Ok(p.track.as_ref().context("track")?.subscribe(None))
    }

    #[test]
    fn a_finished_match_ends_its_track_cleanly() -> anyhow::Result<()> {
        let warns = DropWarnings::default();
        let sub = tracing_subscriber::registry().with(warns.clone());
        let _g = tracing::subscriber::set_default(sub);
        let (groups, end) = run(async {
            let mut p = Publisher::new(broadcast::Info::new().produce())?;
            let reader = reader(&p)?;
            p.keyframe(&through(0))?;
            p.write(&through(1))?;
            p.keyframe(&through(2))?;
            p.write(&through(3))?;
            anyhow::ensure!(p.groups() == 2, "two groups");
            p.finish()?;
            Ok(drain(reader).await)
        })??;
        anyhow::ensure!(end.is_ok(), "a finished track ends cleanly: {end:?}");
        // the default subscription is at the live edge: the last group, read whole
        anyhow::ensure!(
            groups.last().map(Vec::len) == Some(2),
            "the last group whole: {groups:?}"
        );
        anyhow::ensure!(warns.0.load(Ordering::Relaxed) == 0, "no drop warning");
        Ok(())
    }

    #[test]
    fn a_dropped_publisher_aborts_its_track_without_a_warning() -> anyhow::Result<()> {
        let warns = DropWarnings::default();
        let sub = tracing_subscriber::registry().with(warns.clone());
        let _g = tracing::subscriber::set_default(sub);
        let end = run(async {
            let mut p = Publisher::new(broadcast::Info::new().produce())?;
            let reader = reader(&p)?;
            p.keyframe(&through(0))?;
            p.write(&through(1))?;
            // the server stopping mid-match
            drop(p);
            anyhow::Ok(drain(reader).await.1)
        })??;
        anyhow::ensure!(
            end.is_err(),
            "a cut-off track ends with an error, not a clean end"
        );
        anyhow::ensure!(warns.0.load(Ordering::Relaxed) == 0, "no drop warning");
        Ok(())
    }

    #[test]
    fn a_track_never_written_ends_quietly() -> anyhow::Result<()> {
        let warns = DropWarnings::default();
        let sub = tracing_subscriber::registry().with(warns.clone());
        let _g = tracing::subscriber::set_default(sub);
        Publisher::new(broadcast::Info::new().produce())?.finish()?;
        Publisher::new(broadcast::Info::new().produce())?.abort();
        anyhow::ensure!(warns.0.load(Ordering::Relaxed) == 0, "no drop warning");
        Ok(())
    }

    /// What the old loop did, for contrast: the track dropped unfinished, which moq warns of.
    #[test]
    fn an_unfinished_track_dropped_bare_warns() -> anyhow::Result<()> {
        let warns = DropWarnings::default();
        let sub = tracing_subscriber::registry().with(warns.clone());
        let _g = tracing::subscriber::set_default(sub);
        let bc = broadcast::Info::new().produce();
        let track = bc.create_track(TRACK, None)?;
        let _reader = track.subscribe(None);
        let g = track.create_group(0u64.into())?;
        g.finish()?;
        drop(track);
        anyhow::ensure!(
            warns.0.load(Ordering::Relaxed) >= 1,
            "moq warns of a bare drop"
        );
        Ok(())
    }
}

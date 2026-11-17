//! Watching the matches the server fights, live (`docs/specs/2026-11-17-live-matches-design.md`).
//!
//! The server publishes each match onto a MoQ relay as `arena_link` frames: a keyframe (the seed, the arena, the corners
//! and every decision so far), then each decision as it is made, and a `Through` mark every tenth of a second. The page
//! subscribes over WebTransport (`net`, the browser only) and hands each frame here; the game seats two `Replay` seats on
//! logs that grow as the decisions come, and steps its own copy of the rules only as far as the marks say has come
//! (`rules`). The fight on the page is the server's, tick for tick.
//!
//! The folding of frames into a match ([`Watching`]) is plain code, tested natively; only the subscribing is the
//! browser's.

use arena_duel::seat::{Log, Recording};
use arena_link::{Applied, Arena, Ended, Follow, Frame};
use bevy::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// What the relay connection hands the game, in order.
#[derive(Debug)]
pub enum Event {
    /// The match being watched is over or gone: the next frames are another match's.
    NewMatch,
    /// A frame of the match being watched.
    Frame(Frame),
    /// How the connection stands, for the status line.
    Status(String),
    /// The relay session is up (`true`) or lost (`false`).
    Connected(bool),
}

/// A match as the page holds it: what the rules are built on, and the two corners' logs, growing.
pub struct LiveMatch {
    /// Bumped for each new match, so the fight starts over on it.
    pub id: u32,
    pub seed: u32,
    pub arena: Arena,
    pub names: [String; 2],
    /// The logs the `Replay` seats read; they grow as decisions come.
    pub logs: [Recording; 2],
    /// Ticks of the rules the page may step: every frame for them has come.
    pub through: u32,
    pub ended: Option<Ended>,
}

/// Frames folded into the match they make, and the corners' seat logs kept in step with it.
#[derive(Default)]
pub struct Watching {
    follow: Follow,
    current: Option<LiveMatch>,
    next_id: u32,
}

impl Watching {
    pub fn current(&self) -> Option<&LiveMatch> {
        self.current.as_ref()
    }

    /// Forget the match: the next keyframe starts a new one.
    pub fn new_match(&mut self) {
        self.follow = Follow::new();
        self.current = None;
    }

    /// Take in a frame. `Err` when the match was fought on rules other than this build's (the page must reload).
    pub fn frame(&mut self, frame: Frame) -> Result<(), String> {
        if self.follow.apply(frame) == Applied::OtherRules {
            return Err("the arena runs newer rules: reload the page".into());
        }
        let Some(snap) = self.follow.snapshot() else {
            return Ok(());
        };
        let same = self
            .current
            .as_ref()
            .is_some_and(|m| m.seed == snap.seed && m.names == snap.corners);
        if !same {
            self.next_id += 1;
            self.current = Some(LiveMatch {
                id: self.next_id,
                seed: snap.seed,
                arena: snap.arena,
                names: snap.corners.clone(),
                logs: [
                    Rc::new(RefCell::new(Log::default())),
                    Rc::new(RefCell::new(Log::default())),
                ],
                through: 0,
                ended: None,
            });
        }
        let Some(m) = self.current.as_mut() else {
            return Ok(());
        };
        // each corner's log takes the decisions it does not hold yet, in tick order
        for (c, log) in m.logs.iter().enumerate() {
            let mut log = log.borrow_mut();
            let held = log.decisions.len();
            let mut theirs = snap.decisions.iter().filter(|d| d.corner as usize == c);
            for d in theirs.by_ref().skip(held) {
                log.decisions.push((d.tick, d.act.clone().into()));
            }
            if log.forfeit.is_none() {
                log.forfeit = snap
                    .forfeits
                    .iter()
                    .find(|f| f.corner as usize == c)
                    .map(|f| f.tick);
            }
        }
        m.through = self.follow.through();
        m.ended = snap.ended;
        Ok(())
    }
}

/// The page's live state: whether it is watching (a player seating a bot turns it off), the match, the status line.
#[derive(Default)]
pub struct Live {
    pub on: bool,
    pub watching: Watching,
    pub status: String,
    /// Set when the page must reload (another rules build).
    pub stale: Option<String>,
    /// Whether a relay session is up, and how many frames came over it since the page loaded: what the page shows a
    /// smoke test (`window.__arena.live`, `crates/arena-smoke`).
    pub connected: bool,
    pub frames: u64,
}

thread_local! {
    /// What the connection has received since the last frame, for `pull` to fold in.
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
}

/// Queue an event from the connection (it runs on the page's one thread, outside the schedule).
pub fn push(e: Event) {
    EVENTS.with_borrow_mut(|q| q.push(e));
}

pub struct LivePlugin;

impl Plugin for LivePlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(Live::default())
            .add_systems(Startup, start)
            .add_systems(
                Update,
                (pull, fall_back, publish)
                    .chain()
                    .in_set(crate::Frame::Pace),
            );
    }
}

fn start(start: Res<crate::Start>, mut live: NonSendMut<Live>) {
    let Some(url) = &start.live else {
        return;
    };
    live.on = true;
    live.status = "Connecting to the arena".into();
    #[cfg(target_arch = "wasm32")]
    net::watch(url.clone());
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
        live.status = "Live matches are watched in the browser".into();
    }
}

/// How long the page waits for a live match before it fights on its own instead.
const WAIT_SECONDS: f32 = 8.0;

/// No match on the relay (the server is down, the browser cannot reach it): the page seats the house bot in both
/// corners and fights on its own, as it does without a relay.
fn fall_back(
    time: Res<Time>,
    mut live: NonSendMut<Live>,
    mut seat: MessageWriter<crate::bots::Seat>,
) {
    if !live.on || live.watching.current().is_some() || time.elapsed_secs() < WAIT_SECONDS {
        return;
    }
    live.on = false;
    seat.write(crate::bots::Seat {
        corners: [crate::bots::Corner::House, crate::bots::Corner::House],
        seed: crate::bots::any_seed(),
    });
}

/// Fold what the connection received into the match.
fn pull(mut live: NonSendMut<Live>) {
    let events: Vec<Event> = EVENTS.with_borrow_mut(std::mem::take);
    for e in events {
        match e {
            Event::NewMatch => live.watching.new_match(),
            Event::Status(s) => live.status = s,
            Event::Connected(up) => live.connected = up,
            Event::Frame(f) => {
                live.frames += 1;
                if let Err(e) = live.watching.frame(f) {
                    live.stale = Some(e);
                }
            }
        }
    }
}

/// How watching stands: on, connected, frames, the ticks through, the match's id.
type Standing = (bool, bool, u64, u32, u32);

/// How watching stands, on the page as `window.__arena.live` (`on`, `connected`, `frames`, `through`, `match`), so a
/// smoke test can tell the relay is reached and the match moves. It is set only when it changes; nothing reads it.
fn publish(live: NonSend<Live>) {
    let now: Standing = (
        live.on,
        live.connected,
        live.frames,
        live.watching.current().map_or(0, |m| m.through),
        live.watching.current().map_or(0, |m| m.id),
    );
    #[cfg(target_arch = "wasm32")]
    {
        use std::cell::Cell;
        thread_local! {
            static SHOWN: Cell<Option<Standing>> = const { Cell::new(None) };
        }
        if SHOWN.get() == Some(now) {
            return;
        }
        SHOWN.set(Some(now));
        net::show(now);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = now;
}

/// The subscription, in the browser: the relay's WebTransport through moq-dev's `moq-net`, the frames to `push`.
#[cfg(target_arch = "wasm32")]
mod net {
    use super::{Event, push};
    use arena_link::decode;
    use arena_link::moq::{NAMESPACE, TRACK};
    use wasm_bindgen_futures::spawn_local;

    /// Set `window.__arena.live` to how watching stands.
    pub fn show((on, connected, frames, through, id): super::Standing) {
        use js_sys::{Object, Reflect};
        use wasm_bindgen::JsValue;
        let Some(window) = web_sys::window() else {
            return;
        };
        let live = Object::new();
        let set = |k: &str, v: JsValue| {
            let _ = Reflect::set(&live, &JsValue::from_str(k), &v);
        };
        set("on", on.into());
        set("connected", connected.into());
        set("frames", (frames as f64).into());
        set("through", through.into());
        set("match", id.into());
        let arena = Object::new();
        let _ = Reflect::set(&arena, &JsValue::from_str("live"), &live);
        let _ = Reflect::set(&window, &JsValue::from_str("__arena"), &arena);
    }

    /// Watch the matches on the relay at `url` (a subscribe token in its path), one after another, until the page goes.
    pub fn watch(url: String) {
        spawn_local(async move {
            loop {
                let ended = session(&url).await;
                push(Event::Connected(false));
                if let Err(e) = ended {
                    push(Event::Status(format!("Lost the arena ({e}); reconnecting")));
                }
                // a short wait before dialling again
                sleep_ms(2000).await;
            }
        });
    }

    async fn sleep_ms(ms: i32) {
        let p = js_sys::Promise::new(&mut |resolve, _| {
            if let Some(w) = web_sys::window() {
                let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
            }
        });
        let _ = wasm_bindgen_futures::JsFuture::from(p).await;
    }

    /// One session: dial, then follow match after match.
    async fn session(url: &str) -> Result<(), String> {
        let url = url::Url::parse(url).map_err(|e| e.to_string())?;
        let alpns: Vec<&str> = moq_net::ALPNS.to_vec();
        let transport = web_transport_wasm::ClientBuilder::new()
            .with_protocols(alpns)
            .with_system_roots()
            .connect(url)
            .await
            .map_err(|e| format!("{e:?}"))?;
        let (origin, origin_driver) = moq_net::origin::Producer::new(Default::default());
        let client = moq_net::Client::new().with_subscriber(origin.clone());
        let (session, driver) = client
            .connect(moq_net::time::Instant::now(), transport)
            .await
            .map_err(|e| e.to_string())?;
        spawn_local(async move {
            let _ = moq_net::time::run(driver).await;
        });
        spawn_local(async move {
            let _ = moq_net::time::run(origin_driver).await;
        });
        push(Event::Connected(true));
        push(Event::Status("Waiting for a match".into()));

        let scope = moq_net::Pattern::subtree(NAMESPACE)
            .map(moq_net::Patterns::from)
            .map_err(|e| format!("{e:?}"))?;
        let consumer = origin
            .consume()
            .scope("", &scope)
            .map_err(|e| e.to_string())?;
        let mut announced = consumer.announced();
        loop {
            let Some(update) = announced.next().await else {
                break;
            };
            if !update.kind.is_active() {
                continue;
            }
            push(Event::NewMatch);
            push(Event::Status("Live".into()));
            if let Err(e) = follow(&consumer, &update.prefix).await {
                push(Event::Status(format!("A match was lost ({e})")));
            }
        }
        drop(session);
        Err("the relay stopped announcing".into())
    }

    /// Read one match's frames until it ends.
    async fn follow(
        consumer: &moq_net::origin::Consumer,
        prefix: &moq_net::PathOwned,
    ) -> Result<(), String> {
        let broadcast = consumer
            .request_broadcast(prefix)
            .await
            .map_err(|e| e.to_string())?;
        let mut track = broadcast
            .track(TRACK)
            .map_err(|e| e.to_string())?
            .subscribe(None)
            .await
            .map_err(|e| e.to_string())?;
        while let Some(mut group) = track.recv_group().await.map_err(|e| e.to_string())? {
            while let Some(frame) = group.read_frame().await.map_err(|e| e.to_string())? {
                let Ok(frame) = decode(&frame.payload) else {
                    continue;
                };
                let end = matches!(frame, arena_link::Frame::End(_));
                push(Event::Frame(frame));
                if end {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_link::{Act, Decision, RULES_VERSION, Snapshot};

    const ARENA: Arena = Arena {
        left: 88.0,
        right: 1192.0,
        ground: 632.0,
        sky: 196.0,
    };

    fn d(tick: u32, corner: u8) -> Decision {
        Decision {
            tick,
            corner,
            act: Act::Jump,
        }
    }

    #[test]
    fn frames_grow_the_corners_logs_and_a_new_seed_is_a_new_match() -> Result<(), String> {
        let mut w = Watching::default();
        let mut key = Snapshot::new(RULES_VERSION, 5, ARENA, ["a".into(), "b".into()]);
        key.decisions = vec![d(0, 0), d(2, 1)];
        key.ticks = 30;
        w.frame(Frame::Keyframe(key.clone()))?;
        let id = w.current().map(|m| m.id);
        w.frame(Frame::Decision(d(4, 0)))?;
        w.frame(Frame::Through(arena_link::Through {
            tick: 42,
            sent_ms: 0,
        }))?;
        // the group's next keyframe repeats what is held: nothing is taken twice
        key.decisions.push(d(4, 0));
        key.ticks = 60;
        w.frame(Frame::Keyframe(key))?;
        let m = w.current().ok_or("no match")?;
        assert_eq!(Some(m.id), id, "the same match kept its id");
        assert_eq!(m.logs[0].borrow().decisions.len(), 2);
        assert_eq!(m.logs[1].borrow().decisions.len(), 1);
        assert_eq!(m.through, 60);

        w.new_match();
        let other = Snapshot::new(RULES_VERSION, 6, ARENA, ["a".into(), "b".into()]);
        w.frame(Frame::Keyframe(other))?;
        let m = w.current().ok_or("no match")?;
        assert_ne!(Some(m.id), id, "a new seed is a new match");
        assert!(m.logs[0].borrow().decisions.is_empty());
        Ok(())
    }

    #[test]
    fn other_rules_ask_for_a_reload() {
        let mut w = Watching::default();
        let key = Snapshot::new(RULES_VERSION + 1, 5, ARENA, ["a".into(), "b".into()]);
        assert!(w.frame(Frame::Keyframe(key)).is_err());
    }
}

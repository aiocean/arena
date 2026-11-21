//! The live-match link: a match being fought on the server, carried to every watching page.
//!
//! The rules are deterministic from a seed (`arena_duel`), so a match is its **seed and each corner's decisions**: the
//! page runs its own copy of the rules over the same decisions and draws the same fight. Nothing of the fight's state
//! crosses the link. The server publishes, pages subscribe; the carrier is MoQ over WebTransport (a Cloudflare relay),
//! so the server opens no port and needs no fixed address (`docs/specs/2026-11-17-live-matches-design.md`).
//!
//! This crate is the pure heart of that: the message types and their postcard encoding, shared by the server
//! (`arena-server`, native) and the page (`arena-game`'s browser driver). It does no I/O and builds for wasm and
//! native alike. The MoQ wiring (which namespace, which track, how a group is rolled) lives at each end; the layout it
//! agrees on is [`moq`].
//!
//! # On the wire
//!
//! A match is one MoQ track. Its frames are postcard-encoded [`Frame`]s. Every group begins with a [`Frame::Keyframe`]:
//! a self-contained [`Snapshot`] of the whole match so far, so a page that joins mid-match (or at the next group) can
//! rebuild from the start and then follow the [`Frame::Decision`]s that come after. The server rolls a new group
//! every [`moq::GROUP_TICKS`] ticks, each starting with a fresh keyframe, so the cached group a late joiner receives
//! stays bounded.
//!
//! Postcard is not self-describing, so the types here obey ADR-0095's rules (plain externally tagged enums, no
//! `flatten`, no `skip_serializing_if`, no internally tagged enum). The round-trip tests pin it.

mod act;
pub mod moq;

pub use act::Act;

use serde::{Deserialize, Serialize};

/// The rules build, and the frames' shape, this link speaks for (2: `Through` carries the server's clock; 3: bodies
/// sized to the HD figures, strikes ahead of the striker; 4: hurtboxes and hitboxes frame by frame; 5: no ultimate,
/// the top tier priced, one Tín Niệm skill a loadout; 6: a blade's active window, the blade never hurt, hurtboxes
/// inside the cloth; 7: the fighters' turns swap who goes first every tick, the policy's tag reads its look, not its
/// seat, and looks are drawn apart from seats; 8: bot ABI 2, 305 inputs and 27 acts, the ultimate's places gone;
/// 9: the arm hurt to the wrist, the blade from the hilt, tracked on the hand; 10: a recovery drawn over the rules'
/// recover, as the wind-up is over its windup; 11: a lying or falling fighter struck by a bolt only where it lies, a
/// blade's retry gone with its window or its broken act; 12: bot ABI 3, 1741 inputs: the casts' frames, the slots'
/// builds and supports, both fighters' effects, the units, the foe's slots; 13: the balance pass, a bleed the same on a
/// moving bearer, every bare cast under 40% of a life and its shield; 14: every skill plays its own drawn effect from
/// pvfx; 15: Lôi Bạo Trễ, a stopped shot stays as its caster's mine; 16: every hit test on parry
/// (`arena_calc::collide`): touching is not overlapping, a bolt a capsule swept in f64, fields and blasts striking the
/// hurtboxes and units' bodies; 17: a bolt as thick as its drawn body; 18: the keyframe carries the server's [`Tally`];
/// 19: spells only, no melee: the blade, the dive and every striking dash gone, the basics a qi bolt and a close qi
/// burst; 20: no basic that deals damage, the blink the one basic, the qi bolt and burst actives of the pool; 21:
/// Đạo Dung Thân gone, its keystone flag kept and always off; 22: a damaging blow pushes harder the harder it hits, two
/// bodies kept apart on their hurtboxes and a blink never landing inside the foe; 23: casts slower and riskier,
/// meditation a skill in phases; 24: qi regen 8/s, a jump or a take-off 8 qi, flight 8/s; 25: range bands, a skill thrown at the foe begun only from its reach's distance, and bot ABI
/// 4, 1889 inputs: each slot's band and why it cannot be cast, how long buffs, domains, fields and auras last; 27:
/// bot ABI 5, 2055 inputs and 29 acts: its last 3 acts and the foe's last 4 starts, each with its age, and a long duck
/// and a long guard; 28:
/// bot ABI 6, 2299 inputs: its last 8 acts and the foe's last 8 starts, and when it last saw the foe start each kind of
/// skill). A
/// page whose rules differ steps a different fight from
/// the same decisions, so it says so and stops (`Applied::OtherRules`).
/// Bump this with any change to the rules or to a bot's inputs or acts, in the same change (`docs/rules.md`,
/// `docs/bot.md`), as the goldens are re-pinned.
pub const RULES_VERSION: u32 = 28;

/// A decision a corner made at a fighting tick. `tick` counts fighting ticks from the match's first (each seat decides
/// once per fighting tick), so the page, stepping the same deterministic rules, lines its replay up with it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub tick: u32,
    pub corner: u8,
    pub act: Act,
}

/// How a match (or a round) ended.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Ended {
    /// The corner that won.
    Winner(u8),
    /// Neither fighter down when the match was cut (a disconnect, a shutdown).
    Draw,
}

/// A corner that gave up a round from `tick` on: a bot module that trapped or ran out of fuel. The page's replay seat
/// forfeits from the same tick so its rules end the round where the server's did.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Forfeit {
    pub tick: u32,
    pub corner: u8,
}

/// The arena a match is fought in, in the rules' pixels: its walls, its ground and how high a fighter may fly. The
/// page builds its copy of the rules on this, not on its window, so the fight is the server's on any screen.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arena {
    pub left: f64,
    pub right: f64,
    pub ground: f64,
    pub sky: f64,
}

/// Where a match stands in the server's run, as its keyframe tells a watcher: the match's number since the server
/// started (from 1), and how the matches before it went, by fighter (not by seat: the looks are drawn from each seed),
/// Vương Lâm's wins then Thất Thải Đạo Nhân's, and the draws. A page that joins late reads the score from here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub number: u32,
    pub wins: [u32; 2],
    pub draws: u32,
}

impl Tally {
    /// The score once one more match is over: won by fighter `winner` (0 Vương Lâm, 1 Thất Thải Đạo Nhân), or drawn.
    pub fn after(self, winner: Option<usize>) -> Tally {
        let mut t = self;
        match winner {
            Some(w) => t.wins[w.min(1)] += 1,
            None => t.draws += 1,
        }
        t
    }
}

/// The whole match so far, self-contained: a group's keyframe, and the first thing a watcher reads. From this and the
/// decisions after it, the page rebuilds the fight exactly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The rules build the match was fought on. A page checks it against [`RULES_VERSION`].
    pub rules: u32,
    /// The seed the rules start from.
    pub seed: u32,
    /// The arena it is fought in.
    pub arena: Arena,
    /// The two corners' names (the bots seated), Vương Lâm's then Thất Thải's.
    pub corners: [String; 2],
    /// Every decision made so far, in tick order.
    pub decisions: Vec<Decision>,
    /// Every forfeit so far.
    pub forfeits: Vec<Forfeit>,
    /// Set once the match is over.
    pub ended: Option<Ended>,
    /// How many ticks of the rules (fought or between rounds) the match had run when this was taken. A page that joins on
    /// this keyframe steps its own copy of the rules this far without drawing, then plays on live.
    pub ticks: u32,
    /// The match's number and the score of the matches before it.
    pub tally: Tally,
}

impl Snapshot {
    /// A fresh match's keyframe: its seed, its corners, no decisions yet.
    pub fn new(rules: u32, seed: u32, arena: Arena, corners: [String; 2]) -> Snapshot {
        Snapshot {
            rules,
            seed,
            arena,
            corners,
            decisions: Vec::new(),
            forfeits: Vec::new(),
            ended: None,
            ticks: 0,
            tally: Tally::default(),
        }
    }
}

/// How far a match has been sent, and when the server sent the mark (its wall clock, milliseconds since the Unix
/// epoch): a page subtracts it from its own clock for the link's one-way delay (two machines' clocks, so give or take
/// their difference).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Through {
    pub tick: u32,
    pub sent_ms: u64,
}

/// One frame on a match's track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Frame {
    /// A group's first frame: the match so far, decodable on its own.
    Keyframe(Snapshot),
    /// A decision made since the last frame.
    Decision(Decision),
    /// A corner gave up a round.
    Forfeit(Forfeit),
    /// Everything for every tick of the rules up to this one has been sent. A tick with no decision is a real "nothing",
    /// but a page cannot tell it from a decision not yet come, so it steps the fight only as far as this says.
    Through(Through),
    /// The match ended.
    End(Ended),
}

/// Encode a frame to postcard bytes (one MoQ object).
pub fn encode(frame: &Frame) -> Vec<u8> {
    // postcard's std serializer never fails for these owned types; a failure can only be an allocator giving out, which
    // is not ours to handle, so an empty vec stands in (it decodes to an error the reader already handles).
    postcard::to_stdvec(frame).unwrap_or_default()
}

/// Decode a frame from postcard bytes.
pub fn decode(bytes: &[u8]) -> Result<Frame, postcard::Error> {
    postcard::from_bytes(bytes)
}

/// A watcher's view of a match, built frame by frame: the latest keyframe, then the decisions, forfeits and end that
/// follow it. A keyframe replaces everything (it holds the whole match so far); a decision already held is not taken
/// twice. What a page and the `arena-watch` checker both run over the frames they receive.
#[derive(Clone, Debug, Default)]
pub struct Follow {
    snapshot: Option<Snapshot>,
    through: u32,
}

/// What a frame did to a [`Follow`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// It moved the match on.
    Taken,
    /// It came before any keyframe, or repeated what is held: nothing changed.
    Ignored,
    /// The keyframe was fought on other rules than this build's (the server's `RULES_VERSION` given): the page must
    /// reload, or the server is behind it.
    OtherRules(u32),
}

impl Follow {
    pub fn new() -> Follow {
        Follow::default()
    }

    /// The match so far, once a keyframe has come.
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    /// How many ticks of the rules a page may step: every frame for them has come.
    pub fn through(&self) -> u32 {
        self.through
    }

    /// Take in one frame.
    pub fn apply(&mut self, frame: Frame) -> Applied {
        match frame {
            Frame::Keyframe(s) => {
                if s.rules != RULES_VERSION {
                    return Applied::OtherRules(s.rules);
                }
                self.through = self.through.max(s.ticks);
                self.snapshot = Some(s);
                Applied::Taken
            }
            Frame::Through(t) => match &self.snapshot {
                Some(_) if t.tick > self.through => {
                    self.through = t.tick;
                    Applied::Taken
                }
                _ => Applied::Ignored,
            },
            Frame::Decision(d) => match &mut self.snapshot {
                Some(s)
                    if !s
                        .decisions
                        .iter()
                        .rev()
                        .take(8)
                        .any(|h| h.tick == d.tick && h.corner == d.corner) =>
                {
                    s.decisions.push(d);
                    Applied::Taken
                }
                _ => Applied::Ignored,
            },
            Frame::Forfeit(f) => match &mut self.snapshot {
                Some(s) if !s.forfeits.contains(&f) => {
                    s.forfeits.push(f);
                    Applied::Taken
                }
                _ => Applied::Ignored,
            },
            Frame::End(e) => match &mut self.snapshot {
                Some(s) if s.ended.is_none() => {
                    s.ended = Some(e);
                    Applied::Taken
                }
                _ => Applied::Ignored,
            },
        }
    }
}

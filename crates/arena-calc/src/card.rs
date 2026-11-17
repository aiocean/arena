//! Reading each other (rules 8.4, research 8 and 10): lá bài một lần (a skill with a fixed number of uses a match,
//! never refilled between rounds) and Lộ bài (showing it without casting, to scare a guard or an escape out of the
//! foe); the "Một chưởng" feint (a fake that makes a guard spend its cooldown); and the bookkeeping Nhử Đòn reads (the
//! target used a guard just now). Pure and deterministic: no dice.

use crate::cultivator::Cultivator;
use crate::tag::{Tag, Tags};

/// The numbers of Lộ bài.
pub struct CardRules {
    /// Seconds a show takes (it costs no use).
    pub bluff_windup: f64,
    /// Seconds after the show in which a foe's guard or escape is caught.
    pub bluff_window: f64,
    /// Shows a card a round that still scare; later ones are empty.
    pub bluff_shows: u8,
    /// A caught skill's cooldown is multiplied by this.
    pub bluff_cooldown: f64,
    /// Killing intent the bluffer gains when the foe is caught.
    pub bluff_intent: f64,
}
pub const CARD: CardRules = CardRules {
    bluff_windup: 0.4,
    bluff_window: 1.5,
    bluff_shows: 2,
    bluff_cooldown: 2.0,
    bluff_intent: 10.0,
};

/// A one-use card in a slot: its uses left this match and the shows made this round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Card {
    /// Uses left this match.
    pub uses: u8,
    /// Whether it has Lộ bài.
    pub bluff: bool,
    /// Shows made this round.
    pub shown: u8,
}

impl Card {
    /// A card of `uses` a match.
    pub fn new(uses: u8, bluff: bool) -> Card {
        Card {
            uses,
            bluff,
            shown: 0,
        }
    }

    /// The card a skill with `uses` a match makes (`gem::Gem::uses`, `bluff`); none for an ordinary skill.
    pub fn of(uses: Option<u8>, bluff: bool) -> Option<Card> {
        uses.map(|n| Card::new(n, bluff))
    }

    /// Any use left.
    pub fn usable(&self) -> bool {
        self.uses > 0
    }

    /// Casts it for real: spends a use. Returns whether one was left.
    pub fn spend(&mut self) -> bool {
        if self.uses == 0 {
            return false;
        }
        self.uses -= 1;
        true
    }

    /// A new round: the shows come back, the uses never do.
    pub fn new_round(&mut self) {
        self.shown = 0;
    }

    /// Starts a show (Lộ bài) of the card in `slot`: no use spent. None when the card has no Lộ bài or no use left.
    /// A show past [`CARD`]`.bluff_shows` this round still plays, but scares no one.
    pub fn show(&mut self, slot: usize) -> Option<Bluff> {
        if !self.bluff || self.uses == 0 {
            return None;
        }
        self.shown = self.shown.saturating_add(1);
        Some(Bluff {
            slot,
            windup: CARD.bluff_windup,
            window: CARD.bluff_window,
            live: self.shown <= CARD.bluff_shows,
        })
    }
}

/// A one-use card cannot be fused with another skill (rules 10): it would copy the card. Takes each skill's uses a
/// match.
pub fn fusable(a: Option<u8>, b: Option<u8>) -> bool {
    a.is_none() && b.is_none()
}

/// A show under way: its windup, then the window in which the foe's guard or escape is caught.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Bluff {
    /// The slot of the card shown.
    pub slot: usize,
    /// Seconds of windup left; shown at 0.
    pub windup: f64,
    /// Seconds of the window left once shown.
    pub window: f64,
    /// Whether it can still catch the foe.
    pub live: bool,
}

/// A foe caught by a show: the caught skill's cooldown is multiplied by `cooldown`, the bluffer gains `intent`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Caught {
    pub cooldown: f64,
    pub intent: f64,
}

impl Caught {
    /// The bluffer's gain.
    pub fn reward(&self, bluffer: &mut Cultivator) {
        bluffer.gain_intent(self.intent);
    }
}

impl Bluff {
    /// Shown: past the windup.
    pub fn shown(&self) -> bool {
        self.windup <= 0.0
    }

    /// Time passes. The moment it is shown, a foe under a hard control cannot answer it, so it scares no one (rules
    /// 10). Returns whether it is still under way.
    pub fn tick(&mut self, dt: f64, foe_controlled: bool) -> bool {
        if self.windup > 0.0 {
            self.windup -= dt;
            if self.windup <= 0.0 {
                self.windup = 0.0;
                if foe_controlled {
                    self.live = false;
                }
            }
            return true;
        }
        self.window -= dt;
        if self.window <= 0.0 {
            self.window = 0.0;
            self.live = false;
        }
        self.window > 0.0
    }

    /// The foe used a skill tagged `tags`. A guard or an escape inside the window is caught, once; standing still or
    /// attacking does nothing.
    pub fn answer(&mut self, tags: Tags) -> Option<Caught> {
        if !self.live || !self.shown() || self.window <= 0.0 {
            return None;
        }
        if !tags.any(&[Tag::Guard, Tag::Movement]) {
            return None;
        }
        self.live = false;
        Some(Caught {
            cooldown: CARD.bluff_cooldown,
            intent: CARD.bluff_intent,
        })
    }

    /// Broken off (hit in the windup, or a real cast).
    pub fn cancel(&mut self) {
        self.live = false;
        self.window = 0.0;
    }
}

/// The numbers of the "Một chưởng" feint: a giant palm with no power behind it.
pub struct FeintRules {
    pub windup: f64,
    /// Seconds the palm hangs, during which a guard raised against it is wasted.
    pub life: f64,
}
pub const FEINT: FeintRules = FeintRules {
    windup: 0.3,
    life: 0.8,
};

/// A "Một chưởng" feint on the field: it deals nothing; a guard raised against it spends its cooldown and stops
/// nothing (TN299).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Feint {
    /// Seconds it still hangs.
    pub left: f64,
}

impl Default for Feint {
    fn default() -> Self {
        Feint::new()
    }
}

impl Feint {
    /// A feint just shown.
    pub fn new() -> Feint {
        Feint { left: FEINT.life }
    }

    /// Time passes. Returns whether it still hangs.
    pub fn tick(&mut self, dt: f64) -> bool {
        self.left = (self.left - dt).max(0.0);
        self.left > 0.0
    }

    /// The target used a skill tagged `tags`. A guard against the hanging feint is wasted: the caller puts it on its
    /// cooldown and drops it. The feint is spent either way it is answered by a guard. Returns whether a guard was
    /// wasted.
    pub fn meets(&mut self, tags: Tags) -> bool {
        if self.left <= 0.0 || !tags.has(Tag::Guard) {
            return false;
        }
        self.left = 0.0;
        true
    }
}

/// The numbers of Nhử Đòn (bait): a control lasts `control` times as long on a target that used a guard within the
/// last `window` seconds.
pub struct BaitRules {
    pub window: f64,
    pub control: f64,
}
pub const BAIT: BaitRules = BaitRules {
    window: 1.0,
    control: 2.0,
};

/// What Nhử Đòn reads on a fighter: seconds since it last used a guard (Hộ thể).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct GuardWatch {
    pub since: Option<f64>,
}

impl GuardWatch {
    /// The fighter used a skill tagged `tags`.
    pub fn on_skill(&mut self, tags: Tags) {
        if tags.has(Tag::Guard) {
            self.since = Some(0.0);
        }
    }

    /// Time passes.
    pub fn tick(&mut self, dt: f64) {
        if let Some(t) = &mut self.since {
            *t += dt;
        }
    }

    /// Used a guard within the last `window` seconds.
    pub fn guarded_within(&self, window: f64) -> bool {
        self.since.is_some_and(|t| t <= window)
    }

    /// A new round forgets.
    pub fn new_round(&mut self) {
        self.since = None;
    }
}

/// A control of `seconds` with Nhử Đòn on a target watched by `watch`: doubled when the target just guarded. Before
/// grit (`Cultivator::control`).
pub fn bait(seconds: f64, watch: &GuardWatch) -> f64 {
    if watch.guarded_within(BAIT.window) {
        seconds * BAIT.control
    } else {
        seconds
    }
}

//! How a skill reaches its target in the simulator, read from its tags (design 3.2 and 3.3). These are the
//! simulator's own choices, made to measure balance before the real fight exists; the real fight (the Bevy port)
//! decides for itself. Every length is in T, a fighter's height.

use arena_calc::device;
use arena_calc::gem::{self, Gem};
use arena_calc::tag::{Tag, Tags};

/// Reach of a skill tagged Near (design 7: under 1 T).
pub const REACH_NEAR: f64 = 1.0;
/// Reach of a skill tagged Mid (design 7: 1 to 4 T), and of a skill with no reach tag.
pub const REACH_MID: f64 = 4.0;
/// Reach of a skill tagged Far (design 7: over 4 T): most of the field.
pub const REACH_FAR: f64 = 9.0;
/// Reach of a skill tagged Whole: the whole field.
pub const REACH_WHOLE: f64 = crate::FIELD;

/// A projectile's speed in T a second, before the caster's `ProjectileSpeed`.
pub const PROJECTILE_SPEED: f64 = 14.0;
/// An area's radius in T around where it lands, before the caster's `Area` (`play::radius`), to a body's edge: a
/// fighter (`FIGHTER_RADIUS`) is struck with its centre within 1.25 T.
pub const AREA_RADIUS: f64 = 0.75;
/// A nova's radius in T around its caster, before `Area`, to a body's edge (a fighter's centre within 1.6 T).
pub const NOVA_RADIUS: f64 = 1.1;
/// Extra seconds a Delayed area (a seal, a meteor) waits after its windup before it lands.
pub const DELAYED: f64 = 0.4;
/// How far a dash carries its caster, in T. It strikes the target if the target stands within it.
pub const DASH_LENGTH: f64 = 4.0;
/// How far a blink or another movement skill carries its caster, in T.
pub const BLINK_LENGTH: f64 = 3.5;
/// A mine's blast radius, and a trap's when its skill is not an area, before `Area` (T).
pub const DEVICE_RADIUS: f64 = 1.0;
/// A minion: its run speed (T a second), its reach, and seconds between its blows. Each blow is the summoning
/// skill's built blow times `device::Spawn::damage`. A minion expires after `device::MINION_LIFETIME`.
pub const MINION_SPEED: f64 = 3.0;
pub const MINION_REACH: f64 = 1.0;
pub const MINION_EVERY: f64 = 1.6;
/// Bodies as circles (`play::Target`): a fighter's centre height and radius, a minion's and a totem's, and a trap's
/// or a mine's (on the floor, under a flying shot).
pub const FIGHTER_Y: f64 = 0.5;
pub const FIGHTER_RADIUS: f64 = 0.5;
pub const UNIT_Y: f64 = 0.4;
pub const UNIT_RADIUS: f64 = 0.4;
pub const FLOOR_Y: f64 = 0.1;
pub const FLOOR_RADIUS: f64 = 0.2;
/// Seconds between the blows of a skill that lands more than once (`gem::Effect::Hits`, repeats, extra
/// projectiles).
pub const PULSE: f64 = 0.15;
/// A guard skill's ward: blows it erases, and seconds it lasts, before the caster's `SkillDuration`.
pub const WARD_CHARGES: u8 = 1;
pub const WARD_SECONDS: f64 = 2.0;
/// A stance, blessing or other buff skill with no data of its own in the pool: seconds it lasts (before
/// `SkillDuration`), the damage more a stance gives, and the damage taken less a blessing gives.
pub const BUFF_SECONDS: f64 = 8.0;
pub const STANCE_MORE: f64 = 0.25;
pub const BLESSING_TAKEN: f64 = -0.2;

/// How a skill reaches the foe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delivery {
    /// Melee, Strike, Slam, Line and Cone: hits if the foe stands within reach when the blow comes out.
    Melee,
    /// Nova: hits if the foe stands within the radius around the caster.
    Nova,
    /// Flies at a speed from the caster toward where the foe stands; hits when it reaches the foe, unless dodged.
    Projectile,
    /// Area: lands where the foe stood when the cast began, after the windup (and `DELAYED` more when Delayed);
    /// strikes every enemy body still within the radius.
    Area { delayed: bool },
    /// Trap, Mine, Totem or Minion (its own tag, or a support's `becomes`): the cast makes things on the field
    /// (`arena_calc::device`), which strike as the skill would without that tag (`SkillSlot::inner`).
    Device(device::Kind),
    /// Instant: always hits.
    Instant,
    /// Carries the caster `DASH_LENGTH` toward the foe, through it; hits if the foe stood within the dash.
    Dash,
    /// Single, Channel, Chain, and anything not above: hits the nearest enemy body in front within reach.
    Targeted,
    /// Movement: repositions the caster (toward or away from the foe, as the brain asks).
    Movement,
    /// Guard: a ward that erases the next blows for a moment.
    Guard,
    /// Recovery: heals its `gem::Effect::Heal` share of full life.
    Heal,
    /// An intent (aura), lit through `arena_calc::aura`.
    Aura,
    /// A domain, raised through `arena_calc::domain`; one that deals damage also strikes the whole field.
    Domain { damaging: bool },
    /// A curse with no damage of its own: laid if the foe stands within reach.
    Curse,
    /// Stance, blessing, trigger or field body skills with no data of their own: a timed buff (`STANCE_MORE`,
    /// `BLESSING_TAKEN`).
    Buff,
}

impl Delivery {
    /// The delivery of `g` cast with `tags` (its own and its supports' `becomes`).
    pub fn of(g: &Gem, tags: Tags) -> Delivery {
        if gem::aura_of(g).is_some() {
            return Delivery::Aura;
        }
        if gem::domain_of(g).is_some() {
            return Delivery::Domain {
                damaging: g.deals_damage(),
            };
        }
        if tags.has(Tag::Guard) {
            return Delivery::Guard;
        }
        if tags.has(Tag::Recovery) {
            return Delivery::Heal;
        }
        if tags.has(Tag::Movement) {
            return Delivery::Movement;
        }
        if !g.deals_damage() {
            return if tags.has(Tag::Curse) {
                Delivery::Curse
            } else {
                Delivery::Buff
            };
        }
        if let Some(k) = device_kind(tags) {
            return Delivery::Device(k);
        }
        if tags.has(Tag::Dash) {
            Delivery::Dash
        } else if tags.has(Tag::Instant) {
            Delivery::Instant
        } else if tags.has(Tag::Projectile) {
            Delivery::Projectile
        } else if tags.has(Tag::Nova) {
            Delivery::Nova
        } else if tags.has(Tag::Area) {
            Delivery::Area {
                delayed: tags.has(Tag::Delayed),
            }
        } else if tags.any(&[Tag::Melee, Tag::Strike, Tag::Slam, Tag::Line, Tag::Cone]) {
            Delivery::Melee
        } else {
            Delivery::Targeted
        }
    }

    /// Whether it deals damage when it lands (the others act on the caster or lay something).
    pub fn strikes(self) -> bool {
        matches!(
            self,
            Delivery::Melee
                | Delivery::Nova
                | Delivery::Projectile
                | Delivery::Area { .. }
                | Delivery::Instant
                | Delivery::Dash
                | Delivery::Targeted
                | Delivery::Domain { damaging: true }
                | Delivery::Device(_)
        )
    }

    /// The farthest the foe may stand for a cast to be worth starting: the reach for blows, the dash's length
    /// for a dash, the field for what does not aim at the foe and for a minion (it walks). `area` is the radius
    /// factor (`play::radius` of 1).
    pub fn range(self, tags: Tags, area: f64) -> f64 {
        match self {
            Delivery::Device(device::Kind::Minion) => f64::INFINITY,
            Delivery::Device(device::Kind::Totem) => reach(tags),
            Delivery::Device(_) => reach(tags) + DEVICE_RADIUS * area,
            Delivery::Dash => DASH_LENGTH.max(reach(tags)),
            Delivery::Nova => NOVA_RADIUS * area,
            Delivery::Area { .. } => reach(tags) + AREA_RADIUS * area * 0.5,
            Delivery::Domain { .. }
            | Delivery::Movement
            | Delivery::Guard
            | Delivery::Heal
            | Delivery::Aura
            | Delivery::Buff => f64::INFINITY,
            Delivery::Melee
            | Delivery::Projectile
            | Delivery::Instant
            | Delivery::Targeted
            | Delivery::Curse => reach(tags),
        }
    }
}

/// The device tag a skill carries, in `device::spawn`'s order (trap, mine, totem, minion).
pub fn device_kind(tags: Tags) -> Option<device::Kind> {
    if tags.has(Tag::Trap) {
        Some(device::Kind::Trap)
    } else if tags.has(Tag::Mine) {
        Some(device::Kind::Mine)
    } else if tags.has(Tag::Totem) {
        Some(device::Kind::Totem)
    } else if tags.has(Tag::Minion) {
        Some(device::Kind::Minion)
    } else {
        None
    }
}

/// `tags` without the device tags: how a device's own blow travels.
pub fn without_device(tags: Tags) -> Tags {
    [Tag::Trap, Tag::Mine, Tag::Totem, Tag::Minion]
        .into_iter()
        .fold(tags, Tags::without)
}

/// The reach of a skill of `tags`, by its reach tag (Mid when it has none).
pub fn reach(tags: Tags) -> f64 {
    if tags.has(Tag::Near) {
        REACH_NEAR
    } else if tags.has(Tag::Far) {
        REACH_FAR
    } else if tags.has(Tag::Whole) {
        REACH_WHOLE
    } else {
        REACH_MID
    }
}

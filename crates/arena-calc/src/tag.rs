//! A skill's tags (design, section 3): its nature, form, reach, delivery, role, dao and kinds of damage. Tags decide
//! which supports fit, what guard, evasion and shields stop, and which "increased" applies. A bot reads tags, never
//! names.

use crate::element::Element;

macro_rules! tags {
    ($($group:ident: $($tag:ident = $name:literal),+;)+) => {
        /// One tag. The groups are in [`Group`].
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum Tag {
            $($($tag,)+)+
        }

        /// The group a tag belongs to.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Group {
            $($group,)+
        }

        impl Tag {
            pub const ALL: &'static [Tag] = &[$($(Tag::$tag,)+)+];

            pub fn name(self) -> &'static str {
                match self {
                    $($(Tag::$tag => $name,)+)+
                }
            }

            pub fn group(self) -> Group {
                match self {
                    $($(Tag::$tag => Group::$group,)+)+
                }
            }
        }
    };
}

tags! {
    // Đòn, Pháp, Thân: what a skill's base comes from.
    Nature:
        Attack = "attack", Spell = "spell", Body = "body";
    // Hình thức, from the novels (research, 2.1).
    Form:
        Finger = "finger", Palm = "palm", Fist = "fist", Sword = "sword", Seal = "seal", Banner = "banner",
        Formation = "formation", Restriction = "restriction", Talisman = "talisman", Escape = "escape",
        Transform = "transform", Sound = "sound", Illusion = "illusion", Spirit = "spirit", Treasure = "treasure",
        Intent = "intent", Domain = "domain", Tribulation = "tribulation";
    // Tầm.
    Reach:
        Near = "near", Mid = "mid", Far = "far", Whole = "whole";
    // Cách đưa đòn, as PoE's gem tags (design, 3.2).
    Delivery:
        Single = "single", Line = "line", Cone = "cone", Area = "area", Projectile = "projectile",
        Instant = "instant", Channel = "channel", Charged = "charged", Delayed = "delayed", Chain = "chain",
        Melee = "melee", Strike = "strike", Slam = "slam", Nova = "nova", Dash = "dash", Trap = "trap",
        Mine = "mine", Totem = "totem", Minion = "minion";
    // Vai trò (design, 3.3).
    Role:
        Aura = "aura", Blessing = "blessing", Curse = "curse", Warcry = "warcry", Herald = "herald",
        Movement = "movement", Guard = "guard", Recovery = "recovery", Stance = "stance", Trigger = "trigger",
        Vaal = "vaal", Control = "control", Field = "field";
    // Đạo (research, 4), and the marks the daos read.
    Dao:
        LifeDeath = "lifedeath", Karma = "karma", TrueFalse = "truefalse", Time = "time", Space = "space",
        Bond = "bond", Fate = "fate", Slaughter = "slaughter", Hex = "hex", Ban = "ban",
        Life = "life", Death = "death", Immortal = "immortal", Ancient = "ancient", Blood = "blood",
        Insect = "insect";
    // Loại sát thương.
    Damage:
        Force = "force", Metal = "metal", Wood = "wood", Water = "water", Fire = "fire", Earth = "earth",
        Thunder = "thunder", Light = "light", Yin = "yin", Soul = "soul";
}

impl Tag {
    /// The tag of a kind of damage.
    pub fn of(e: Element) -> Tag {
        match e {
            Element::Force => Tag::Force,
            Element::Metal => Tag::Metal,
            Element::Wood => Tag::Wood,
            Element::Water => Tag::Water,
            Element::Fire => Tag::Fire,
            Element::Earth => Tag::Earth,
            Element::Thunder => Tag::Thunder,
            Element::Light => Tag::Light,
            Element::Yin => Tag::Yin,
            Element::Soul => Tag::Soul,
        }
    }

    fn bit(self) -> u128 {
        1u128 << (self as u32)
    }
}

/// A set of tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Tags(pub u128);

impl Tags {
    pub const NONE: Tags = Tags(0);

    pub fn of(tags: &[Tag]) -> Tags {
        Tags(tags.iter().fold(0, |m, t| m | t.bit()))
    }

    pub fn has(self, t: Tag) -> bool {
        self.0 & t.bit() != 0
    }

    pub fn any(self, tags: &[Tag]) -> bool {
        tags.iter().any(|&t| self.has(t))
    }

    pub fn all(self, tags: &[Tag]) -> bool {
        tags.iter().all(|&t| self.has(t))
    }

    pub fn with(self, t: Tag) -> Tags {
        Tags(self.0 | t.bit())
    }

    pub fn without(self, t: Tag) -> Tags {
        Tags(self.0 & !t.bit())
    }

    pub fn iter(self) -> impl Iterator<Item = Tag> {
        Tag::ALL.iter().copied().filter(move |&t| self.has(t))
    }

    /// The tags as a one-hot row in [`Tag::ALL`]'s order, for a bot.
    pub fn one_hot(self) -> Vec<f64> {
        Tag::ALL
            .iter()
            .map(|&t| if self.has(t) { 1.0 } else { 0.0 })
            .collect()
    }

    /// A blow evasion can dodge: an attack or a projectile, never an instant.
    pub fn evadable(self) -> bool {
        !self.has(Tag::Instant) && (self.has(Tag::Attack) || self.has(Tag::Projectile))
    }

    /// A blow a guard can stop: melee, projectiles, dashes (never areas, instants or fields).
    pub fn guardable(self) -> bool {
        self.any(&[Tag::Melee, Tag::Projectile, Tag::Dash])
            && !self.any(&[Tag::Area, Tag::Instant, Tag::Domain])
    }
}

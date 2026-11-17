//! Items a fighter wears (design, section 17.4): a weapon, armour, jewellery and a bonded treasure, each with a base
//! and a list of modifiers. Only the base exists for now: no item is made or rolled yet, and a fighter with nothing
//! worn fights bare-handed ([`Gear::default`]), which is exactly the fight as it is without items.

use crate::cultivator::BASE;
use crate::element::{Element, N, Parts};
use crate::stat::Mod;

/// Where an item is worn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum ItemSlot {
    Weapon,
    Offhand,
    Helmet,
    Body,
    Gloves,
    Boots,
    Belt,
    Amulet,
    RingLeft,
    RingRight,
    /// Pháp bảo bản mệnh: a bonded treasure.
    Treasure,
}

impl ItemSlot {
    pub const ALL: [ItemSlot; 11] = [
        ItemSlot::Weapon,
        ItemSlot::Offhand,
        ItemSlot::Helmet,
        ItemSlot::Body,
        ItemSlot::Gloves,
        ItemSlot::Boots,
        ItemSlot::Belt,
        ItemSlot::Amulet,
        ItemSlot::RingLeft,
        ItemSlot::RingRight,
        ItemSlot::Treasure,
    ];
}

/// A weapon's base: the damage an attack starts from, by kind; attacks a second; its crit chance; its reach in T.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct WeaponBase {
    pub damage: Parts,
    pub speed: f64,
    pub crit: f64,
    pub reach: f64,
}

impl WeaponBase {
    /// Bare hands: the fighter's own might as force, one attack a second, the base crit, a body's reach.
    pub fn bare() -> WeaponBase {
        let mut damage = [0.0; N];
        damage[Element::Force.index()] = BASE.might;
        WeaponBase {
            damage,
            speed: 1.0,
            crit: BASE.crit,
            reach: 1.0,
        }
    }
}

/// An armour piece's base defences.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ArmourBase {
    pub armour: f64,
    pub evasion: f64,
    pub shield: f64,
}

/// What an item is before its modifiers.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ItemBase {
    Weapon(WeaponBase),
    Armour(ArmourBase),
    /// Jewellery and treasures: modifiers only.
    Trinket,
}

/// One item: its name, slot, base and modifiers (implicit and rolled alike, in order).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Item {
    pub name: String,
    pub slot: ItemSlot,
    pub base: ItemBase,
    pub mods: Vec<Mod>,
}

/// Everything a fighter wears, at most one item a slot.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Gear {
    pub items: Vec<Item>,
}

impl Gear {
    /// Wears an item, taking off whatever was in its slot.
    pub fn wear(&mut self, item: Item) {
        self.items.retain(|i| i.slot != item.slot);
        self.items.push(item);
    }

    pub fn in_slot(&self, slot: ItemSlot) -> Option<&Item> {
        self.items.iter().find(|i| i.slot == slot)
    }

    /// The weapon's base, bare hands without one.
    pub fn weapon(&self) -> WeaponBase {
        match self.in_slot(ItemSlot::Weapon).map(|i| i.base) {
            Some(ItemBase::Weapon(w)) => w,
            _ => WeaponBase::bare(),
        }
    }

    /// The armour pieces' base defences, summed.
    pub fn armour(&self) -> ArmourBase {
        self.items
            .iter()
            .fold(ArmourBase::default(), |a, i| match i.base {
                ItemBase::Armour(b) => ArmourBase {
                    armour: a.armour + b.armour,
                    evasion: a.evasion + b.evasion,
                    shield: a.shield + b.shield,
                },
                _ => a,
            })
    }

    /// Every modifier on every item, slot by slot.
    pub fn mods(&self) -> impl Iterator<Item = &Mod> {
        ItemSlot::ALL
            .iter()
            .filter_map(|&s| self.in_slot(s))
            .flat_map(|i| i.mods.iter())
    }
}

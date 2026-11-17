//! A fighter's loadout (`arena_calc::loadout::Loadout`) made into the skills it casts: the three basics, the eight
//! actives with their supports, and any linked skill a trigger casts that the loadout does not carry.
//! Each skill keeps its stored uses (`arena_calc::charge::Uses`), how often it was cast this round, a one-use card's
//! uses left, and how it does what it does in the duel (`Way`). Its sheet holds the loadout's keystones; no gear, no
//! passive tree (design 17.4).

use crate::data::{SKILL_LEVEL, Spell, motion};
use arena_calc::build::{Situation, Skill, build_skill};
use arena_calc::charge::Uses;
use arena_calc::cultivator::Cultivator;
use arena_calc::device;
use arena_calc::event::Trigger;
use arena_calc::gem::{self, Gem, Resource};
use arena_calc::keystone::Keystone;
use arena_calc::loadout::{BASICS, Loadout};
use arena_calc::sheet::Sheet;
use arena_calc::support::{self, Behaviour, Support};
use arena_calc::tag::{Tag, Tags};

/// Where a skill sits in a fighter's kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// One of the three basics (`loadout::BASICS`), always at hand.
    Basic,
    /// One of the loadout's actives, by its index in `Loadout::actives`: the bar draws from these.
    Active(usize),
    /// A skill a trigger casts that the loadout does not carry (Liên Hoàn Suy Yếu's linked attack).
    Linked,
}

/// How a skill does what it does, read from its tags (design 3) before the motion it borrows: an intent, a domain,
/// a curse or a buff with no blow of its own, or a trap, mine, totem or minion (its own tag or a support's
/// `becomes`); anything else moves as its motion (`data::Spell::kind`) does: a blow, a guard, a heal, a blink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Way {
    Aura,
    /// A domain; one that deals damage strikes as its motion too.
    Domain {
        damaging: bool,
    },
    /// A curse with no damage of its own: laid on the foe within the motion's reach.
    Curse,
    /// A stance, blessing or other body skill with no data of its own: a timed buff on the caster.
    Buff,
    /// It makes things on the field (`crate::unit`), which strike as its motion does.
    Device(device::Kind),
    /// As its motion does.
    Motion,
}

impl Way {
    /// The way of `g` cast with `tags` (its own and its supports' `becomes`).
    pub fn of(g: &Gem, tags: Tags) -> Way {
        if gem::aura_of(g).is_some() {
            return Way::Aura;
        }
        if gem::domain_of(g).is_some() {
            return Way::Domain {
                damaging: g.deals_damage(),
            };
        }
        if tags.any(&[Tag::Guard, Tag::Recovery, Tag::Movement]) {
            return Way::Motion;
        }
        if !g.deals_damage() {
            return if tags.has(Tag::Curse) {
                Way::Curse
            } else {
                Way::Buff
            };
        }
        match device_kind(tags) {
            Some(k) => Way::Device(k),
            None => Way::Motion,
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

/// One skill of a fighter's kit.
#[derive(Clone, Debug, PartialEq)]
pub struct KitSkill {
    pub gem: &'static Gem,
    /// The skill as cast (fused with its partner under Dung), at `SKILL_LEVEL`.
    pub skill: Skill,
    pub supports: Vec<&'static Support>,
    pub place: Place,
    /// Cast by its support's trigger, never by hand (`support::Behaviour::Triggered`).
    pub triggered: bool,
    /// Its tags with its kinds of damage and its supports' `becomes`.
    pub tags: Tags,
    pub way: Way,
    /// How it moves and looks: the duel's skill for its id (`data::spell`).
    pub motion: &'static Spell,
    /// Its stored cooldown uses.
    pub uses: Uses,
    /// Casts this round (`Situation::use_count` reads it plus one).
    pub count: u32,
    /// Uses left this match for a one-use card (`Gem::uses`).
    pub card: Option<u8>,
    /// Its effect is in the duel's registry: a skill whose effect is missing is never cast.
    pub usable: bool,
}

impl KitSkill {
    /// `g` with `supports` (fused with `fuse` under Dung) at `place`, built once for its timings and uses.
    pub fn new(
        g: &'static Gem,
        place: Place,
        supports: Vec<&'static Support>,
        fuse: Option<&str>,
        c: &Cultivator,
        sheet: &Sheet,
    ) -> Option<KitSkill> {
        let motion = crate::data::spell(g.id)?;
        motion_of(g)?;
        let mut skill = Skill::of(g, SKILL_LEVEL, c.might);
        if let Some(partner) = fuse.and_then(gem::gem) {
            skill = support::fuse(&skill, &Skill::of(partner, SKILL_LEVEL, c.might));
        }
        let own = skill.all_tags();
        let tags = supports
            .iter()
            .filter(|s| s.fits(own))
            .flat_map(|s| s.mods.becomes.iter().copied())
            .fold(own, Tags::with);
        // Kênh Dẫn Pháp casts while channelling; the duel has no channel, so that skill is cast by hand
        let triggered = place == Place::Linked
            || supports.iter().any(|s| {
                s.behaviours.contains(&Behaviour::Triggered)
                    && !s
                        .behaviours
                        .iter()
                        .any(|b| matches!(b, Behaviour::CastWhileChannelling { .. }))
            });
        let built = build_skill(&skill, &supports, c, sheet, &Situation::default());
        Some(KitSkill {
            gem: g,
            skill,
            supports,
            place,
            triggered,
            tags,
            way: Way::of(g, tags),
            motion,
            uses: Uses::of(&built),
            count: 0,
            card: g.uses,
            usable: true,
        })
    }

    /// Its costs before a cast: the skill's, times its fitting supports' cost multipliers (what a mind is told it
    /// can pay; the cast pays what it builds to).
    pub fn costs(&self) -> Vec<(Resource, f64)> {
        let own = self.skill.all_tags();
        let factor: f64 = self
            .supports
            .iter()
            .filter(|s| s.fits(own))
            .map(|s| s.cost)
            .product();
        self.skill
            .costs
            .iter()
            .map(|&(r, v)| (r, v * factor))
            .collect()
    }

    /// The qi it costs before a cast.
    pub fn qi(&self) -> f64 {
        self.costs()
            .iter()
            .filter(|c| c.0 == Resource::Qi)
            .map(|c| c.1)
            .sum()
    }

    /// A fresh round: every use stored, none spent.
    pub fn new_round(&mut self) {
        self.uses.stored = self.uses.max;
        self.uses.recharge_left = 0.0;
        self.count = 0;
    }
}

/// The motion gem `g` borrows (`Gem::spell`).
pub fn motion_of(g: &Gem) -> Option<&'static Spell> {
    motion(g.spell)
}

/// A fighter's skills and its supports' triggers, aimed at the skills they hang on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kit {
    pub skills: Vec<KitSkill>,
    pub triggers: Vec<Trigger>,
}

impl Kit {
    /// The kit of `loadout` for a fighter of cultivator `c` wearing `sheet`: the basics, then the actives. Skills the pool or the duel does not know are left out.
    pub fn of(loadout: &Loadout, c: &Cultivator, sheet: &Sheet) -> Kit {
        let mut kit = Kit::default();
        let add = |kit: &mut Kit, g: &'static Gem, place, supports: Vec<&'static Support>, fuse| {
            if let Some(k) = KitSkill::new(g, place, supports, fuse, c, sheet) {
                for s in &k.supports {
                    if let Some(t) = s.trigger_for(&k.skill) {
                        kit.triggers.push(t);
                    }
                }
                kit.skills.push(k);
            }
        };
        for id in BASICS {
            if let Some(g) = gem::gem(id) {
                add(&mut kit, g, Place::Basic, Vec::new(), None);
            }
        }
        for (i, slot) in loadout.actives.iter().enumerate() {
            if let Some(g) = gem::gem(&slot.skill) {
                let supports = slot
                    .supports
                    .iter()
                    .filter_map(|s| support::support(s))
                    .collect();
                add(
                    &mut kit,
                    g,
                    Place::Active(i),
                    supports,
                    slot.fuse.as_deref(),
                );
            }
        }
        kit
    }

    /// The skill called `id`.
    pub fn get(&self, id: &str) -> Option<&KitSkill> {
        self.skills.iter().find(|k| k.gem.id == id)
    }

    /// Where the skill called `id` sits in the kit.
    pub fn index(&self, id: &str) -> Option<usize> {
        self.skills.iter().position(|k| k.gem.id == id)
    }

    /// Adds a skill a trigger casts that the kit does not carry (a linked attack), unsupported; returns where it sits.
    pub fn link(&mut self, id: &str, c: &Cultivator, sheet: &Sheet) -> Option<usize> {
        if let Some(i) = self.index(id) {
            return Some(i);
        }
        let g = gem::gem(id)?;
        let k = KitSkill::new(g, Place::Linked, Vec::new(), None, c, sheet)?;
        self.skills.push(k);
        Some(self.skills.len() - 1)
    }
}

/// A fighter's damage-system state fresh for a round: the base, the loadout's root, the sheet's lasting stats and the
/// keystones' start.
pub fn cultivator_of(loadout: &Loadout, sheet: &Sheet) -> Cultivator {
    let mut c = Cultivator::new();
    if let Some(r) = loadout.root {
        c = c.with_root(r);
    }
    sheet.apply(&mut c);
    for k in &loadout.keystones {
        k.apply(&mut c);
    }
    c
}

/// Whether `loadout` holds keystone `k` (Cực Cảnh of any kind counts for any).
pub fn holds(loadout: &Loadout, k: Keystone) -> bool {
    loadout.keystones.iter().any(|o| o.same(k))
}

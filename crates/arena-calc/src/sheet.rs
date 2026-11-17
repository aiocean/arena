//! A fighter's character sheet (design, section 17.4): the permanent modifiers it brings to a fight (items worn,
//! passive nodes allocated, keystones) and the stats they make, plus the timed effects it carries in the moment.
//! Every source of a modifier meets here, so a blow or a defence gathers from one place ([`Sheet::table`]).
//!
//! With no items, no tree and no keystones the sheet is empty and every stat is the base (`cultivator::BASE`).

use crate::cultivator::{BASE, Cultivator};
use crate::item::Gear;
use crate::keystone::Keystone;
use crate::passive::Tree;
use crate::stat::{Ctx, Mod, Stat, Table};

/// What a fighter brings: what it wears, the tree and the nodes it allocated, the keystones its loadout takes, and
/// modifiers from anywhere else that last the whole fight (a season's rule).
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Sheet {
    pub gear: Gear,
    pub tree: Tree,
    pub allocated: Vec<u32>,
    /// The loadout's keystones (đạo tâm); the tree's allocated keystones join them ([`Sheet::keystones`]).
    pub keystones: Vec<Keystone>,
    pub extra: Vec<Mod>,
}

/// The empty sheet: bare hands, no tree, no keystone. What a caller with no sheet uses.
pub static EMPTY: Sheet = Sheet {
    gear: Gear { items: Vec::new() },
    tree: Tree {
        nodes: Vec::new(),
        starts: Vec::new(),
    },
    allocated: Vec::new(),
    keystones: Vec::new(),
    extra: Vec::new(),
};

impl Sheet {
    /// A sheet with these keystones and nothing else.
    pub fn with_keystones(keystones: &[Keystone]) -> Sheet {
        Sheet {
            keystones: keystones.to_vec(),
            ..Sheet::default()
        }
    }

    /// Every keystone it holds: the loadout's, then the tree's allocated ones.
    pub fn keystones(&self) -> Vec<Keystone> {
        let mut out = self.keystones.clone();
        out.extend(self.tree.keystones(&self.allocated));
        out
    }

    /// Every permanent modifier: items slot by slot, then passive nodes, then the keystones' lasting ones
    /// ([`Keystone::lasting`]), then the rest.
    pub fn mods(&self) -> impl Iterator<Item = &Mod> {
        self.gear
            .mods()
            .chain(self.tree.mods(&self.allocated))
            .chain(
                self.keystones()
                    .into_iter()
                    .flat_map(|k| k.lasting().iter()),
            )
            .chain(self.extra.iter())
    }

    /// Gathers the permanent modifiers, `ctx.me`'s timed effects and the modifiers of its state (stars, ailments:
    /// `Cultivator::state_mods`) in `ctx`, plus any `more` a caller adds (a skill's own supports).
    pub fn table<'a>(&'a self, ctx: &Ctx<'a>, more: impl IntoIterator<Item = &'a Mod>) -> Table {
        let state = ctx.me.state_mods();
        let more: Vec<&Mod> = more.into_iter().collect();
        Table::gather(
            self.mods()
                .chain(ctx.me.effect_mods())
                .chain(state.iter())
                .chain(more.iter().copied()),
            ctx,
        )
    }

    /// Sets `c`'s bars and might from the modifiers that hold with no target and no skill: full life, shield,
    /// sense and qi, and might. Armour and evasion become the base with the items' base defences. Called once as a fight
    /// begins; the bars start full.
    ///
    /// Armour, evasion, resistances, damage taken and crit are not set here: every blow reads them from the
    /// gathered table (`hit::resolve`, step 8; `build`), so a conditional modifier holds only while it holds and
    /// nothing counts twice.
    pub fn apply(&self, c: &mut Cultivator) {
        let t = Table::gather(self.mods(), &Ctx::alone(c));
        let armour = self.gear.armour();
        c.life_max = t.get(Stat::Life, BASE.life);
        c.shield_max = t.get(Stat::Shield, BASE.shield + armour.shield);
        c.sense_max = t.get(Stat::Sense, BASE.sense);
        c.qi_max = t.get(Stat::QiMax, BASE.qi).max(0.0);
        c.might = t.get(Stat::Might, BASE.might);
        c.armour = BASE.armour + armour.armour;
        c.evasion = BASE.evasion + armour.evasion;
        c.life = c.life_max;
        c.shield = c.shield_max;
        c.sense = c.sense_max;
        c.qi = c.qi_max;
    }
}

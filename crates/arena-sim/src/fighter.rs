//! A fighter in the simulator: its loadout made into castable skills (the three basics, the eight actives, the
//! ultimate), its sheet (the loadout's keystones; no gear, no tree), the triggers its supports carry, and its place
//! and timers on the field. Its damage-system state (`arena_calc::cultivator::Cultivator`) lives beside it in the
//! fight, so the event queue can take both fighters' as one slice.

use arena_calc::aura::Auras;
use arena_calc::build::{Situation, Skill, build_skill};
use arena_calc::charge::Uses;
use arena_calc::cultivator::Cultivator;
use arena_calc::curse::Curses;
use arena_calc::event::Trigger;
use arena_calc::gem::{self, Gem};
use arena_calc::keystone::Keystone;
use arena_calc::loadout::{BASICS, Loadout};
use arena_calc::sheet::Sheet;
use arena_calc::support::{self, Behaviour, Support};
use arena_calc::tag::{Tag, Tags};

use crate::SKILL_LEVEL;
use crate::delivery::Delivery;
use crate::report::{Place, SideReport, SkillLine};

/// One castable skill of a fighter.
#[derive(Clone, Debug)]
pub struct SkillSlot {
    pub gem: &'static Gem,
    /// The skill as this fighter casts it (fused with its partner under Dung).
    pub skill: Skill,
    pub supports: Vec<&'static Support>,
    pub place: Place,
    /// Cast by its support's trigger, never by hand (`support::Behaviour::Triggered`).
    pub triggered: bool,
    /// Its tags with its supports' `becomes`, its delivery and how far the foe may stand for a cast to be worth it.
    pub tags: Tags,
    pub delivery: Delivery,
    /// How its blows travel without its device tag: a totem's, a trap's or a mine's own blow.
    pub inner: Delivery,
    pub range: f64,
    /// Its stored cooldown uses (`charge::Uses`: one, plus `ExtraCooldownUse`).
    pub charges: Uses,
    /// Casts this round (`Situation::use_count` reads it plus one).
    pub uses: u32,
    /// Uses left this match for a one-use card (`Gem::uses`).
    pub card: Option<u8>,
    /// The brain's last estimate of its damage against the foe.
    pub estimate: f64,
}

/// What a fighter is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    Idle,
    /// Gathering a cast of `slot`: seconds `left`, seconds gone, where it aims (the foe's place at the cast), and
    /// for a movement skill, toward the foe (+1) or away (-1).
    Windup {
        slot: usize,
        left: f64,
        gone: f64,
        aim: f64,
        dir: f64,
    },
    /// After the blow, before it can act again.
    Recover { left: f64 },
}

/// A fighter's side of the fight besides its cultivator.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub loadout: Loadout,
    pub sheet: Sheet,
    pub slots: Vec<SkillSlot>,
    /// Its supports' triggers, aimed at the skills they hang on.
    pub triggers: Vec<Trigger>,
    pub x: f64,
    pub act: Act,
    /// Seconds it cannot act or move (stun, knockdown, hold, jolt, a blow's stagger); seconds it cannot move.
    pub stun: f64,
    pub root: f64,
    pub moving: bool,
    pub rage: u8,
    /// A guard skill's ward: blows it erases, and seconds left.
    pub ward: u8,
    pub ward_left: f64,
    pub auras: Auras,
    /// The curses on this fighter.
    pub curses: Curses,
    /// Which of its slots laid the curse of each kind on the foe (`curse::Kind::index`), for the report.
    pub curse_slots: Vec<(usize, usize)>,
    /// Which of its slots last laid a burn and a bleed on the foe, for the report.
    pub last_burn: Option<usize>,
    pub last_bleed: Option<usize>,
    /// Sát Ý stacks this round; spare lives used (Tam Mệnh, Ngọn Đèn Hồn) this match; Bất Khuất used this round;
    /// Tâm Pháp's qi-paid spells toward the next rage point.
    pub fury: u8,
    pub lives_used: u8,
    pub unyielding_used: bool,
    pub attuned: u8,
    /// What it did lately, for `stat::Counts`: (time, kill, hit, qi spent, kinds dealt).
    pub history: Vec<(f64, u32, u32, f64, u16)>,
    pub report: SideReport,
}

impl Fighter {
    /// A fighter from a legal `loadout`, at seat `seat`, and its cultivator: the root, the sheet's bars, the
    /// keystones' start. Skills the pool does not know are left out.
    pub fn new(loadout: &Loadout, seat: usize) -> (Fighter, Cultivator) {
        let sheet = Sheet::with_keystones(&loadout.keystones);
        let mut c = Cultivator::new();
        if let Some(r) = loadout.root {
            c = c.with_root(r);
        }
        sheet.apply(&mut c);
        for k in &loadout.keystones {
            k.apply(&mut c);
        }
        let might = c.might;

        let mut slots = Vec::new();
        let mut triggers = Vec::new();
        let mut add = |g: &'static Gem, place: Place, supports: Vec<&'static Support>, fuse: Option<&str>| {
            let mut skill = Skill::of(g, SKILL_LEVEL, might);
            if let Some(partner) = fuse.and_then(gem::gem) {
                skill = support::fuse(&skill, &Skill::of(partner, SKILL_LEVEL, might));
            }
            let own = skill.all_tags();
            let tags = supports
                .iter()
                .filter(|s| s.fits(own))
                .flat_map(|s| s.mods.becomes.iter().copied())
                .fold(own, Tags::with);
            for s in &supports {
                if let Some(t) = s.trigger_for(&skill) {
                    triggers.push(t);
                }
            }
            // Kênh Dẫn Pháp casts while channelling; the simulator has no channel, so that skill is cast by hand
            let triggered = supports.iter().any(|s| {
                s.behaviours.contains(&Behaviour::Triggered)
                    && !s
                        .behaviours
                        .iter()
                        .any(|b| matches!(b, Behaviour::CastWhileChannelling { .. }))
            });
            let delivery = Delivery::of(g, tags);
            let built = build_skill(&skill, &supports, &c, &sheet, &Situation::default());
            let area = arena_calc::play::area_factor(&built.table);
            slots.push(SkillSlot {
                gem: g,
                skill,
                supports,
                place,
                triggered,
                tags,
                delivery,
                inner: Delivery::of(g, crate::delivery::without_device(tags)),
                range: delivery.range(tags, libm::sqrt(area)),
                charges: Uses::of(&built),
                uses: 0,
                card: g.uses,
                estimate: 0.0,
            });
        };
        for id in BASICS {
            if let Some(g) = gem::gem(id) {
                add(g, Place::Basic, Vec::new(), None);
            }
        }
        for (i, slot) in loadout.actives.iter().enumerate() {
            if let Some(g) = gem::gem(&slot.skill) {
                let supports = slot
                    .supports
                    .iter()
                    .filter_map(|s| support::support(s))
                    .collect();
                add(g, Place::Active(i), supports, slot.fuse.as_deref());
            }
        }
        if let Some(g) = gem::gem(&loadout.ultimate) {
            add(g, Place::Ultimate, Vec::new(), None);
        }
        let report = SideReport {
            skills: slots
                .iter()
                .map(|s| SkillLine::new(s.gem.id, s.place))
                .collect(),
            ..SideReport::default()
        };
        let witch = loadout.keystones.contains(&Keystone::Witch);
        let f = Fighter {
            loadout: loadout.clone(),
            sheet,
            slots,
            triggers,
            x: crate::START[seat.min(1)],
            act: Act::Idle,
            stun: 0.0,
            root: 0.0,
            moving: false,
            rage: 0,
            ward: 0,
            ward_left: 0.0,
            auras: Auras::default(),
            curses: Curses::new(seat as u32, witch),
            curse_slots: Vec::new(),
            last_burn: None,
            last_bleed: None,
            fury: 0,
            lives_used: 0,
            unyielding_used: false,
            attuned: 0,
            history: Vec::new(),
            report,
        };
        (f, c)
    }

    /// Whether it holds keystone `k` (Cực Cảnh of any kind counts for any).
    pub fn has(&self, k: Keystone) -> bool {
        self.loadout.keystones.iter().any(|o| o.same(k))
    }

    /// Whether it may start a cast or move: idle and not stunned.
    pub fn free(&self) -> bool {
        self.act == Act::Idle && self.stun <= 0.0
    }

    /// Whether it is gathering a cast.
    pub fn in_windup(&self) -> bool {
        matches!(self.act, Act::Windup { .. })
    }

    /// Whether it is gathering its ultimate's first `ULTIMATE_OPENING` seconds (blows pass through).
    pub fn in_opening(&self) -> bool {
        match self.act {
            Act::Windup { slot, gone, .. } => {
                gone < crate::ULTIMATE_OPENING
                    && self
                        .slots
                        .get(slot)
                        .is_some_and(|s| s.place == Place::Ultimate)
            }
            _ => false,
        }
    }

    /// Its kit is fresh for a new round: cooldowns, uses, rage, timers and its place.
    pub fn new_round(&mut self, seat: usize) {
        self.x = crate::START[seat.min(1)];
        self.act = Act::Idle;
        self.stun = 0.0;
        self.root = 0.0;
        self.moving = false;
        self.rage = 0;
        self.ward = 0;
        self.ward_left = 0.0;
        self.auras = Auras::default();
        self.curses.on.clear();
        self.curses.boons.clear();
        self.curse_slots.clear();
        self.last_burn = None;
        self.last_bleed = None;
        self.fury = 0;
        self.unyielding_used = false;
        self.attuned = 0;
        self.history.clear();
        for s in &mut self.slots {
            s.charges.stored = s.charges.max;
            s.charges.recharge_left = 0.0;
            s.uses = 0;
        }
    }

    /// Whether skill `tags` is a spell a silenced fighter cannot cast.
    pub fn is_spell(tags: Tags) -> bool {
        tags.has(Tag::Spell)
    }
}

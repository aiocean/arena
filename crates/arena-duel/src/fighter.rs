//! A fighter of the duel and the duel's whole state.

use crate::body::{Arena, Body};
use crate::data::{Look, body_spec};
use crate::kit::{Kit, Place, cultivator_of};
use crate::out::{Note, Out};
use crate::shot::Shot;
use crate::strike::Pending;
use crate::style::Style;
use crate::unit::{Group, Unit};
use arena_calc::aura::Auras;
use arena_calc::cultivator::Cultivator;
use arena_calc::curse::Curses;
use arena_calc::domain;
use arena_calc::event::Queue;
use arena_calc::keystone::Keystone;
use arena_calc::loadout::Loadout;
use arena_calc::sheet::Sheet;
use arena_calc::stat::{Counts, RECENT};
use arena_mind::{Outcome, Rng};

/// Where a cast is: gathering before it lands, or recovering after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Phase {
    Windup,
    Recover,
}

/// A cast under way: its skill, phase and time in it, its windup and recover as built at the cast (supports and
/// keystones change them), where it aims, the rune circle it stands in, and whether its blow was guarded (`blocked`:
/// an unsafe skill then recovers longer, `FRAMES.blocked_recover`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Act {
    pub spell: &'static str,
    pub phase: Phase,
    pub t: f64,
    pub windup: f64,
    pub recover: f64,
    pub target: Option<f64>,
    pub circle: Option<u32>,
    pub blocked: bool,
}

/// A slot of the rotating bar: counting down to a draw, or holding a skill of the loadout (bar tiers, traits, luck
/// and evolving pairs are off while the damage system is balanced: design 2).
#[derive(Clone, Debug, PartialEq)]
pub enum Slot {
    Empty { left: f64, full: f64 },
    Held { spell: &'static str, idle: f64 },
}

impl Slot {
    /// The skill it holds.
    pub fn spell(&self) -> Option<&'static str> {
        match self {
            Slot::Held { spell, .. } => Some(spell),
            Slot::Empty { .. } => None,
        }
    }
}

/// One of the two fighters. Its bars and resources are its cultivator's (`c`); `hp` and `qi` read them as shares.
#[derive(Clone, Debug, PartialEq)]
pub struct Fighter {
    pub index: usize,
    /// How it is drawn and coloured: nothing in the rules reads it.
    pub look: Look,
    pub loadout: Loadout,
    /// The loadout's keystones (no gear, no tree).
    pub sheet: Sheet,
    /// Its damage-system state: life, shield, sense, qi and every resource, ailments, grit, effects.
    pub c: Cultivator,
    pub kit: Kit,
    /// Every skill its mind may name, the kit's ids in order (basics, actives, ultimate).
    pub spells: Vec<&'static str>,
    /// The actives its bar draws from: those cast by hand.
    pub pool: Vec<&'static str>,
    pub bar: Vec<Slot>,
    pub act: Option<Act>,
    pub guard: f64,
    pub medit: bool,
    pub fly: bool,
    pub hover: f64,
    pub rage: f64,
    /// Blinking locked (Không Khóa, Chặn Lui).
    pub sealed: f64,
    /// Rooted (Triền full): no walking, no blinking.
    pub root: f64,
    /// No guard skill (Mê Dục).
    pub no_guard: f64,
    /// A ward's charges, and the time it has left.
    pub charges: f64,
    pub shield_t: f64,
    /// A barrier's time left, the skill that raised it, and its effect.
    pub barrier: f64,
    pub barrier_spell: Option<&'static str>,
    pub barrier_key: Option<u32>,
    pub rang: f64,
    pub dodged: f64,
    /// A blow it cast is still playing out (a bolt in flight, a seeker, a field, a spray, an area's strikes, a skill's
    /// repeats): until it is over, it casts no other skill that binds (`bar::ready`, `Gem::binds`).
    pub bound: bool,
    /// The way it walks: -1, 0 or 1.
    pub intent: f64,
    /// Walking this tick (bleeds run twice, Thừa Thắng's target moving).
    pub moving: bool,
    pub auras: Auras,
    /// The curses on this fighter.
    pub curses: Curses,
    /// Sát Ý stacks this round; spare lives used this match (Tam Mệnh, Ngọn Đèn Hồn); Bất Khuất used this round;
    /// Tâm Pháp's qi-paid spells toward the next rage point.
    pub fury: u8,
    pub lives_used: u8,
    pub unyielding_used: bool,
    pub attuned: u8,
    /// What it did lately, for `stat::Counts`: (time, kills, hits, qi spent, kinds dealt).
    pub history: Vec<(f64, u32, u32, f64, u16)>,
    /// Damage over time taken since it was last shown, its share of life, and the seconds since (`strike::DOT_SHOWN`).
    pub dots: (f64, f64, f64),
    pub body: Body,
    /// How it fights, drawn once a fight (its seat reads it; the rules do not).
    pub style: Style,
}

impl Fighter {
    /// Fighter `index` with `loadout`, fresh, looking as its seat does.
    pub fn new(index: usize, loadout: Loadout) -> Fighter {
        let sheet = Sheet::with_keystones(&loadout.keystones);
        let c = cultivator_of(&loadout, &sheet);
        let kit = Kit::of(&loadout, &c, &sheet);
        let spells = kit.skills.iter().map(|k| k.gem.id).collect();
        let pool = kit
            .skills
            .iter()
            .filter(|k| matches!(k.place, Place::Active(_)) && !k.triggered)
            .map(|k| k.gem.id)
            .collect();
        let witch = loadout.keystones.contains(&Keystone::Witch);
        Fighter {
            index,
            look: Look::of_seat(index),
            loadout,
            sheet,
            c,
            kit,
            spells,
            pool,
            bar: Vec::new(),
            act: None,
            guard: 0.0,
            medit: false,
            fly: false,
            hover: 0.0,
            rage: 0.0,
            sealed: 0.0,
            root: 0.0,
            no_guard: 0.0,
            charges: 0.0,
            shield_t: 0.0,
            barrier: 0.0,
            barrier_spell: None,
            barrier_key: None,
            rang: 0.0,
            dodged: 0.0,
            bound: false,
            intent: 0.0,
            moving: false,
            auras: Auras::default(),
            curses: Curses::new(index as u32, witch),
            fury: 0,
            lives_used: 0,
            unyielding_used: false,
            attuned: 0,
            history: Vec::new(),
            dots: (0.0, 0.0, 0.0),
            body: Body::with_spec(0.0, body_spec()),
            style: Style::Balanced,
        }
    }

    /// The round's fresh state: a fresh cultivator, every use stored, nothing on it; the loadout, kit, look, body,
    /// style and the match's spare lives carry over (the round start handles the body and the bar).
    pub fn refresh(&mut self) {
        self.c = cultivator_of(&self.loadout, &self.sheet);
        for k in &mut self.kit.skills {
            k.new_round();
        }
        let witch = self.loadout.keystones.contains(&Keystone::Witch);
        self.act = None;
        self.guard = 0.0;
        self.medit = false;
        self.fly = false;
        self.hover = 0.0;
        self.rage = 0.0;
        self.sealed = 0.0;
        self.root = 0.0;
        self.no_guard = 0.0;
        self.charges = 0.0;
        self.shield_t = 0.0;
        self.barrier = 0.0;
        self.barrier_spell = None;
        self.barrier_key = None;
        self.rang = 0.0;
        self.dodged = 0.0;
        self.bound = false;
        self.intent = 0.0;
        self.moving = false;
        self.auras = Auras::default();
        self.curses = Curses::new(self.index as u32, witch);
        self.fury = 0;
        self.unyielding_used = false;
        self.attuned = 0;
        self.history.clear();
        self.dots = (0.0, 0.0, 0.0);
    }

    /// Life as a share of full life, 0 to 100.
    pub fn hp(&self) -> f64 {
        share(self.c.life, self.c.life_max)
    }

    /// Qi as a share of full qi, 0 to 100.
    pub fn qi(&self) -> f64 {
        share(self.c.qi, self.c.qi_max)
    }

    /// Whether it still stands.
    pub fn alive(&self) -> bool {
        self.c.life > 0.0
    }

    /// The height of its chest above the ground, where its events show.
    pub fn chest(&self) -> f64 {
        self.body.air + self.body.spec.chest
    }

    /// Whether it holds keystone `k`.
    pub fn has(&self, k: Keystone) -> bool {
        crate::kit::holds(&self.loadout, k)
    }

    /// Silenced: its soul lost (thất hồn), only attacks answer.
    pub fn silenced(&self) -> bool {
        self.c.soul_lost > 0.0
    }

    /// What it did lately (`stat::Counts`, over `RECENT`), at `now`.
    pub fn counts(&self, now: f64) -> Counts {
        let mut c = Counts::default();
        for &(t, k, h, q, kinds) in &self.history {
            if now - t <= RECENT {
                c.kills += k;
                c.hits += h;
                c.qi_spent += q;
                c.kinds |= kinds;
            }
        }
        c
    }
}

/// `v` of `max` as a share, 0 to 100.
fn share(v: f64, max: f64) -> f64 {
    if max > 0.0 {
        (v / max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

/// What the rules tell a fighter's mind; the duel hands them over in order before that mind next thinks.
#[derive(Clone, Debug, PartialEq)]
pub enum Told {
    Outcome(Outcome),
    Reset,
}

/// The duel's whole state: the arena, the dice, the clock, the rounds, the fighters, the things on the field, the
/// damage system's event queue and domains, and what this step emitted.
#[derive(Clone, Debug, PartialEq, bevy_ecs::resource::Resource)]
pub struct State {
    pub seed: u32,
    pub rng: Rng,
    pub left: f64,
    pub right: f64,
    pub ground: f64,
    /// How high above the ground a fighter may fly.
    pub top: f64,
    pub time: f64,
    /// Ticks run.
    pub ticks: u64,
    /// Time left over from the last step, short of a whole tick.
    pub carry: f64,
    pub round: u32,
    pub wins: [u32; 2],
    pub winner: Option<usize>,
    pub pause: f64,
    /// The last effect key handed out.
    pub key: u32,
    /// Spells in flight, in the order they were cast.
    pub shots: Vec<Shot>,
    /// The last shot id handed out.
    pub shot_id: u32,
    pub fighters: [Fighter; 2],
    /// Traps, mines, totems and minions standing, in id order: the tick's working copy of the world's `Unit`
    /// entities (`crate::unit`).
    pub units: Vec<Unit>,
    /// The last unit id handed out.
    pub unit_id: u32,
    /// Each (owner, skill)'s placed things (`device::Placed`: caps, ages, arming, springing, detonating).
    pub groups: Vec<Group>,
    /// Blows due later: a skill's repeats, a stored force (Lưu Lực).
    pub pending: Vec<Pending>,
    /// The damage system's events and triggers.
    pub queue: Queue,
    /// The domains standing.
    pub field: domain::Field,
    pub out: Vec<Out>,
    /// What this step noted for a measure of the duel, each beside how many of `out` came before it.
    pub notes: Vec<(usize, Note)>,
    /// The effects in the registry: a skill whose effect is missing is never cast.
    pub effects: Vec<String>,
    /// The arena shows its lamps and night.
    pub decor: bool,
    /// What the rules told each mind since it last thought.
    pub told: Vec<(usize, Told)>,
}

impl State {
    pub fn new(
        seed: u32,
        left: f64,
        right: f64,
        ground: f64,
        top: f64,
        effects: Vec<String>,
        fighters: [Fighter; 2],
    ) -> State {
        State {
            seed,
            rng: Rng::new(seed),
            left,
            right,
            ground,
            top,
            time: 0.0,
            ticks: 0,
            carry: 0.0,
            round: 0,
            wins: [0, 0],
            winner: None,
            pause: 0.0,
            key: 0,
            shots: Vec::new(),
            shot_id: 0,
            fighters,
            units: Vec::new(),
            unit_id: 0,
            groups: Vec::new(),
            pending: Vec::new(),
            queue: Queue::new(),
            field: domain::Field::default(),
            out: Vec::new(),
            notes: Vec::new(),
            effects,
            decor: false,
            told: Vec::new(),
        }
    }

    /// Whether effect `fx` is in the registry.
    pub fn has(&self, fx: &str) -> bool {
        self.effects.iter().any(|e| e == fx)
    }

    /// The arena the bodies move in.
    pub fn arena(&self) -> Arena {
        Arena {
            left: self.left,
            right: self.right,
            top: self.top,
        }
    }

    pub fn emit(&mut self, out: Out) {
        self.out.push(out);
    }

    /// Note something for a measure of the duel.
    pub fn note(&mut self, note: Note) {
        self.notes.push((self.out.len(), note));
    }

    /// Tell fighter `me`'s mind something.
    pub fn tell(&mut self, me: usize, told: Told) {
        self.told.push((me, told));
    }

    /// The field's width in T (the damage system's unit).
    pub fn width_t(&self) -> f64 {
        (self.right - self.left) / crate::data::T
    }

    /// An x in the arena's pixels, in T from the left wall.
    pub fn to_t(&self, x: f64) -> f64 {
        (x - self.left) / crate::data::T
    }

    /// An x in T from the left wall, in the arena's pixels.
    pub fn from_t(&self, x: f64) -> f64 {
        self.left + x * crate::data::T
    }

    /// The distance between the fighters, in T.
    pub fn distance_t(&self) -> f64 {
        (self.fighters[0].body.x - self.fighters[1].body.x).abs() / crate::data::T
    }
}

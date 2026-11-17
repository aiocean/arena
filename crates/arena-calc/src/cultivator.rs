//! A fighter as the damage system sees it (design, 1 and rules, 1 and 8): the three living bars (life, energy shield,
//! sense), the resources (qi, lifespan, killing intent, souls, star points, rejection, death qi), defences,
//! ailments, the fusion window and grit. Every fighter starts from the same [`BASE`]; a loadout changes the rest.

use crate::ailment::{AIL, Ailment, Ailments, Control};
use crate::dot::{DotTick, Ticked, mitigation};
use crate::element::{Element, FUSE, N, Parts, is_root};
use crate::sheet::{EMPTY, Sheet};
use crate::stat::{Ctx, Mod, Recent, Source, Stat, Table};

/// The base every fighter starts from: no class, no school.
pub struct Base {
    pub life: f64,
    pub shield: f64,
    /// Shield regen a second, after `shield_delay` s unhit; a broken shield waits `shield_broken` s more.
    pub shield_regen: f64,
    pub shield_delay: f64,
    pub shield_broken: f64,
    pub sense: f64,
    pub sense_regen: f64,
    /// Thất hồn: silenced this long when the sense runs out.
    pub soulless: f64,
    pub qi: f64,
    pub qi_regen: f64,
    pub meditate_regen: f64,
    pub lifespan: f64,
    /// Out of lifespan, "old": damage, speed and recovery fall by this share.
    pub old: f64,
    pub might: f64,
    pub armour: f64,
    pub resist_cap: f64,
    pub resist_floor: f64,
    pub ailment_resist_cap: f64,
    pub crit: f64,
    pub crit_multi: f64,
    pub evasion: f64,
    /// Armour's reduction at most.
    pub armour_cap: f64,
    /// Armour's formula: `H / (H + armour_k × D)`.
    pub armour_k: f64,
}
pub const BASE: Base = Base {
    life: 3000.0,
    shield: 600.0,
    shield_regen: 60.0,
    shield_delay: 2.0,
    shield_broken: 1.0,
    sense: 100.0,
    sense_regen: 4.0,
    soulless: 1.5,
    qi: 100.0,
    qi_regen: 3.0,
    meditate_regen: 16.0,
    lifespan: 100.0,
    old: 0.2,
    might: 400.0,
    armour: 600.0,
    resist_cap: 0.75,
    resist_floor: -1.0,
    ailment_resist_cap: 0.6,
    crit: 0.05,
    crit_multi: 1.5,
    evasion: 0.05,
    armour_cap: 0.9,
    armour_k: 5.0,
};

/// Killing intent (sát khí), souls (hồn), star points (tinh điểm), rejection (bài xích), death qi (tử khí) and grit
/// (Kiên): their caps and rates.
pub struct Res {
    pub intent_max: f64,
    pub intent_hit: f64,
    pub intent_kill: f64,
    pub intent_fall: f64,
    /// Above this, each point adds 1% damage.
    pub intent_edge: f64,
    pub intent_per: f64,
    /// Full for `demon_after` s: Tâm ma, `demon` of full life a second until below `demon_until`.
    pub demon_after: f64,
    pub demon: f64,
    pub demon_until: f64,
    pub souls_max: f64,
    /// A soul for each this much soul damage dealt.
    pub souls_per_damage: f64,
    pub stars_max: f64,
    pub star_time: f64,
    /// Thần: damage taken down and resistances up a star; Yêu: speed a star; Ma: crit chance and multi, soul a star.
    pub star_body: f64,
    pub star_speed: f64,
    pub star_crit: f64,
    pub star_crit_multi: f64,
    pub star_soul: f64,
    /// A blow over this share of full life earns a body star.
    pub star_body_hit: f64,
    /// Shattering stars: power a star, or life a star.
    pub shatter_power: f64,
    pub shatter_heal: f64,
    pub rejection_max: f64,
    pub rejection_step: f64,
    pub rejection_window: f64,
    pub rejection_fall: f64,
    pub rejection_blast: f64,
    /// Below this share of life a life-death cultivator pays life costs in death qi.
    pub death_below: f64,
    pub death_max: f64,
    pub grit_max: f64,
    pub grit_fall: f64,
    pub grit_factor: f64,
    /// A target of a higher intent rank frees itself in this share of a control.
    pub grit_intent: f64,
}
pub const RES: Res = Res {
    intent_max: 100.0,
    intent_hit: 4.0,
    intent_kill: 10.0,
    intent_fall: 2.0,
    intent_edge: 50.0,
    intent_per: 0.01,
    demon_after: 3.0,
    demon: 0.01,
    demon_until: 80.0,
    souls_max: 30.0,
    souls_per_damage: 100.0,
    stars_max: 3.0,
    star_time: 10.0,
    star_body: 0.04,
    star_speed: 0.05,
    star_crit: 0.03,
    star_crit_multi: 0.1,
    star_soul: 0.05,
    star_body_hit: 0.08,
    shatter_power: 0.25,
    shatter_heal: 0.08,
    rejection_max: 100.0,
    rejection_step: 20.0,
    rejection_window: 2.0,
    rejection_fall: 10.0,
    rejection_blast: 0.15,
    death_below: 0.1,
    death_max: 50.0,
    grit_max: 3.0,
    grit_fall: 2.0,
    grit_factor: 0.5,
    grit_intent: 0.5,
};

/// The three kinds of star (Cổ tộc: mi tâm, mắt phải, mắt trái), as PoE's endurance, frenzy and power charges.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Star {
    /// Thần tinh: from heavy blows taken.
    Body,
    /// Yêu tinh: from dodges and blinks through a blow.
    Speed,
    /// Ma tinh: from crits.
    Mind,
}

/// A fighter's damage-system state.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Cultivator {
    pub life: f64,
    pub life_max: f64,
    pub shield: f64,
    pub shield_max: f64,
    /// Seconds until the shield regrows.
    pub shield_wait: f64,
    pub sense: f64,
    pub sense_max: f64,
    /// Sense held by intents (auras).
    pub sense_reserved: f64,
    pub qi: f64,
    pub qi_max: f64,
    pub lifespan: f64,
    /// Never old, whatever the lifespan (Nguyền Bẩm Sinh).
    pub ageless: bool,
    pub intent: f64,
    /// Seconds the intent has stood full.
    pub intent_full: f64,
    pub demon: bool,
    pub souls: f64,
    /// Star counts and the seconds they have left, by [`Star`].
    pub stars: [f64; 3],
    pub star_time: [f64; 3],
    pub rejection: f64,
    /// Seconds since a cast tagged immortal (`Some(true)`) or ancient (`Some(false)`).
    pub last_school: Option<(bool, f64)>,
    pub death_qi: f64,
    pub grit: f64,
    pub grit_wait: f64,
    pub might: f64,
    pub armour: f64,
    pub resist: Parts,
    pub ailment_resist: [f64; crate::ailment::AILMENTS],
    pub crit: f64,
    pub crit_multi: f64,
    pub evasion: f64,
    /// Bản mệnh.
    pub root: Option<Element>,
    /// Immune to one element (Hệ Đại Thành).
    pub immune: Option<Element>,
    /// The overcome factor against this fighter's root (1.25; 1.5 under Hệ Đại Thành).
    pub root_weak: f64,
    /// No soul (puppets, fire spirits): soul damage is lost.
    pub soulless_body: bool,
    /// Damage taken scaled by this (body stars, keystones).
    pub taken: f64,
    pub ailments: Ailments,
    /// The fusion window: each part landed and the seconds it has left.
    pub window: Vec<(Element, f64, f64)>,
    /// Ngạt: no qi regen, slow cooldowns.
    pub choke: f64,
    /// Full wither: healing hurts.
    pub rot: f64,
    /// Wind: qi regen stopped.
    pub wind_lock: f64,
    /// Thất hồn: silenced, intents out.
    pub soul_lost: f64,
    pub meditating: bool,
    /// Seconds since each thing that "recently" conditions read.
    pub recent: Recent,
    /// Timed effects on this fighter (blessings, intents, curses, domain rules, ailments' own modifiers): each a
    /// list of modifiers and the seconds it has left (`None` while it is held).
    pub effects: Vec<Active>,
}

/// A timed effect on a fighter: its modifiers, where they come from, and the seconds it has left (`None` until it is
/// put out, as an intent).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Active {
    pub source: Source,
    pub mods: Vec<Mod>,
    pub left: Option<f64>,
}

impl Default for Cultivator {
    fn default() -> Self {
        Cultivator::new()
    }
}

impl Cultivator {
    /// A fighter at the base, full.
    pub fn new() -> Cultivator {
        Cultivator {
            life: BASE.life,
            life_max: BASE.life,
            shield: BASE.shield,
            shield_max: BASE.shield,
            shield_wait: 0.0,
            sense: BASE.sense,
            sense_max: BASE.sense,
            sense_reserved: 0.0,
            qi: BASE.qi,
            qi_max: BASE.qi,
            lifespan: BASE.lifespan,
            ageless: false,
            intent: 0.0,
            intent_full: 0.0,
            demon: false,
            souls: 0.0,
            stars: [0.0; 3],
            star_time: [0.0; 3],
            rejection: 0.0,
            last_school: None,
            death_qi: 0.0,
            grit: 0.0,
            grit_wait: 0.0,
            might: BASE.might,
            armour: BASE.armour,
            resist: [0.0; N],
            ailment_resist: [0.0; crate::ailment::AILMENTS],
            crit: BASE.crit,
            crit_multi: BASE.crit_multi,
            evasion: BASE.evasion,
            root: None,
            immune: None,
            root_weak: crate::element::ROOT.overcomes,
            soulless_body: false,
            taken: 1.0,
            ailments: Ailments::default(),
            window: Vec::new(),
            choke: 0.0,
            rot: 0.0,
            wind_lock: 0.0,
            soul_lost: 0.0,
            meditating: false,
            recent: Recent::default(),
            effects: Vec::new(),
        }
    }

    /// Takes a root (bản mệnh); a root that is not a phase, light or yin is ignored.
    pub fn with_root(mut self, root: Element) -> Cultivator {
        if is_root(root) {
            self.root = Some(root);
        }
        self
    }

    /// Out of lifespan.
    pub fn old(&self) -> bool {
        !self.ageless && self.lifespan <= 0.0
    }

    /// The damage this fighter's own blows deal more: killing intent above its edge, minus age.
    pub fn power(&self) -> f64 {
        let intent = (self.intent - RES.intent_edge).max(0.0) * RES.intent_per;
        (1.0 + intent) * if self.old() { 1.0 - BASE.old } else { 1.0 }
    }

    /// The modifiers this fighter's stars give ([`Source::Star`]), from their counts now. Thần (body): damage taken
    /// less and resistances up; Yêu (speed): speed; Ma (mind): crit chance and multiplier, and soul damage.
    pub fn star_mods(&self) -> Vec<Mod> {
        let mut out = Vec::new();
        let body = self.stars[Star::Body as usize];
        if body > 0.0 {
            let src = Source::Star(Star::Body);
            out.push(Mod::more(Stat::Taken, -body * RES.star_body, src));
            for e in Element::ALL.into_iter().skip(1) {
                out.push(Mod::flat(Stat::Resist(e), body * RES.star_body, src));
            }
        }
        let speed = self.stars[Star::Speed as usize];
        if speed > 0.0 {
            out.push(Mod::inc(
                Stat::Speed,
                speed * RES.star_speed,
                Source::Star(Star::Speed),
            ));
        }
        let mind = self.stars[Star::Mind as usize];
        if mind > 0.0 {
            let src = Source::Star(Star::Mind);
            out.push(Mod::flat(Stat::CritChance, mind * RES.star_crit, src));
            out.push(Mod::flat(Stat::CritMulti, mind * RES.star_crit_multi, src));
            out.push(Mod::inc(
                Stat::DamageOf(Element::Soul),
                mind * RES.star_soul,
                src,
            ));
        }
        out
    }

    /// The modifiers the ailments on this fighter give it ([`Source::Ailment`]), from their stacks now: jolt raises
    /// all damage taken, soak thunder taken and cuts qi regen, wither yin taken and cuts healing, sunder cuts armour,
    /// bind slows the run, dread lengthens windups. What an ailment deals over time is not here (the tick).
    pub fn ailment_mods(&self) -> Vec<Mod> {
        let mut out = Vec::new();
        let n = |a| self.ailments.stacks(a);
        let src = Source::Ailment;
        if n(Ailment::Jolt) > 0.0 {
            out.push(Mod::more(
                Stat::Taken,
                n(Ailment::Jolt) * AIL.jolt_taken,
                src(Ailment::Jolt),
            ));
        }
        if n(Ailment::Soak) > 0.0 {
            out.push(Mod::more(
                Stat::TakenOf(Element::Thunder),
                n(Ailment::Soak) * AIL.soak_thunder,
                src(Ailment::Soak),
            ));
            out.push(Mod::inc(
                Stat::QiRegen,
                -n(Ailment::Soak) * AIL.soak_qi,
                src(Ailment::Soak),
            ));
        }
        if n(Ailment::Wither) > 0.0 {
            out.push(Mod::more(
                Stat::TakenOf(Element::Yin),
                n(Ailment::Wither) * AIL.wither_yin,
                src(Ailment::Wither),
            ));
            out.push(Mod::inc(
                Stat::Healing,
                -n(Ailment::Wither) * AIL.wither_heal,
                src(Ailment::Wither),
            ));
        }
        if n(Ailment::Sunder) > 0.0 {
            out.push(Mod::more(
                Stat::Armour,
                -n(Ailment::Sunder) * AIL.sunder,
                src(Ailment::Sunder),
            ));
        }
        if n(Ailment::Bind) > 0.0 {
            out.push(Mod::inc(
                Stat::MoveSpeed,
                -n(Ailment::Bind) * AIL.bind_slow,
                src(Ailment::Bind),
            ));
        }
        if n(Ailment::Dread) > 0.0 {
            out.push(Mod::inc(
                Stat::Windup,
                n(Ailment::Dread) * AIL.dread_windup,
                src(Ailment::Dread),
            ));
        }
        out
    }

    /// Every modifier this fighter's state gives it now: its stars, then its ailments. `sheet::Sheet::table`
    /// gathers these with the timed [`effects`](Cultivator::effects).
    pub fn state_mods(&self) -> Vec<Mod> {
        let mut out = self.star_mods();
        out.extend(self.ailment_mods());
        out
    }

    /// Its own state and timed effects gathered with no target and no skill.
    fn own_table(&self) -> Table {
        let state = self.state_mods();
        Table::gather(self.effect_mods().chain(state.iter()), &Ctx::alone(self))
    }

    /// The resistance to `e` after the cap, with body stars (its [`star_mods`](Cultivator::star_mods)).
    pub fn resist_of(&self, e: Element) -> f64 {
        let state = self.star_mods();
        let t = Table::gather(state.iter(), &Ctx::alone(self));
        t.get(Stat::Resist(e), self.resist[e.index()])
            .clamp(BASE.resist_floor, BASE.resist_cap)
    }

    /// Crit chance and multiplier with mind stars (its [`star_mods`](Cultivator::star_mods)).
    pub fn crit_of(&self) -> (f64, f64) {
        let state = self.star_mods();
        let t = Table::gather(state.iter(), &Ctx::alone(self));
        (
            t.get(Stat::CritChance, self.crit),
            t.get(Stat::CritMulti, self.crit_multi),
        )
    }

    /// The share of damage taken with body stars (its [`star_mods`](Cultivator::star_mods)); a blow reads the
    /// whole defensive table instead (`hit::resolve`, step 8).
    pub fn taken_of(&self) -> f64 {
        let state = self.star_mods();
        let t = Table::gather(state.iter(), &Ctx::alone(self));
        self.taken * t.get(Stat::Taken, 1.0)
    }

    /// Damage in the fusion window for `e`.
    pub fn in_window(&self, e: Element) -> f64 {
        self.window.iter().filter(|w| w.0 == e).map(|w| w.1).sum()
    }

    /// Spends all of `e` in the fusion window.
    pub fn spend_window(&mut self, e: Element) {
        self.window.retain(|w| w.0 != e);
    }

    /// Puts `parts` in the fusion window.
    pub fn fill_window(&mut self, parts: &Parts) {
        for e in Element::ALL {
            let v = parts[e.index()];
            if v > 0.0 {
                self.window.push((e, v, FUSE.window));
            }
        }
    }

    /// A hard control of `seconds`, shortened by grit and, against a higher intent rank, halved; grit rises.
    pub fn control(&mut self, seconds: f64, higher_intent: bool) -> f64 {
        let mut t = seconds * libm::pow(RES.grit_factor, self.grit);
        if higher_intent {
            t *= RES.grit_intent;
        }
        self.grit = (self.grit + 1.0).min(RES.grit_max);
        self.grit_wait = RES.grit_fall;
        t
    }

    /// Life lost, never below 0; returns what was lost.
    pub fn lose_life(&mut self, amount: f64) -> f64 {
        let lost = amount.max(0.0).min(self.life);
        self.life -= lost;
        lost
    }

    /// Damage that the shield takes first, the rest to life. Returns (to shield, to life).
    pub fn hurt(&mut self, amount: f64) -> (f64, f64) {
        let amount = amount.max(0.0);
        let to_shield = amount.min(self.shield);
        self.shield -= to_shield;
        self.shield_wait = BASE.shield_delay;
        if to_shield > 0.0 && self.shield <= 0.0 {
            self.shield_wait += BASE.shield_broken;
        }
        let to_life = self.lose_life(amount - to_shield);
        (to_shield, to_life)
    }

    /// Breaks the shield at once (full sunder).
    pub fn break_shield(&mut self) {
        self.shield = 0.0;
        self.shield_wait = BASE.shield_delay + BASE.shield_broken;
    }

    /// Soul damage: the sense first, the rest at half to life. Returns (to sense, to life). Running the sense out
    /// costs the soul (thất hồn) and every intent.
    pub fn hurt_soul(&mut self, amount: f64) -> (f64, f64) {
        if self.soulless_body {
            return (0.0, 0.0);
        }
        let to_sense = amount.max(0.0).min(self.sense);
        self.sense -= to_sense;
        let rest = amount.max(0.0) - to_sense;
        if self.sense <= 0.0 && to_sense > 0.0 {
            self.soul_lost = BASE.soulless;
            self.sense_reserved = 0.0;
        }
        let to_life = self.lose_life(rest * 0.5);
        (to_sense, to_life)
    }

    /// Heals `amount`, scaled by the healing its state and effects give (wither cuts it); under full wither (rot) it
    /// hurts instead. Returns the life change.
    pub fn heal(&mut self, amount: f64) -> f64 {
        let cut = self.own_table().get(Stat::Healing, 1.0);
        let amount = amount * cut.max(0.0) * if self.old() { 1.0 - BASE.old } else { 1.0 };
        if self.rot > 0.0 {
            return -self.lose_life(amount);
        }
        let gain = amount.min(self.life_max - self.life).max(0.0);
        self.life += gain;
        gain
    }

    /// Pays life for a cost, never the last point. Returns what was paid.
    pub fn pay_life(&mut self, amount: f64) -> f64 {
        let paid = amount.max(0.0).min((self.life - 1.0).max(0.0));
        self.life -= paid;
        paid
    }

    /// Adds a star of kind `s`, up to the cap; renews all of that kind.
    pub fn add_star(&mut self, s: Star, cap: f64) {
        let i = s as usize;
        self.stars[i] = (self.stars[i] + 1.0).min(cap);
        self.star_time[i] = RES.star_time;
    }

    /// Shatters every star of kind `s`; returns how many.
    pub fn shatter(&mut self, s: Star) -> f64 {
        let i = s as usize;
        let n = self.stars[i];
        self.stars[i] = 0.0;
        self.star_time[i] = 0.0;
        n
    }

    /// Killing intent gained.
    pub fn gain_intent(&mut self, n: f64) {
        self.intent = (self.intent + n).clamp(0.0, RES.intent_max);
    }

    /// A cast tagged immortal (`true`) or ancient (`false`): mixing the two within the window builds rejection;
    /// returns true when it bursts (the caller deals `rejection_blast` of full life to this fighter).
    pub fn cast_school(&mut self, immortal: bool) -> bool {
        if let Some((last, t)) = self.last_school
            && last != immortal
            && t <= RES.rejection_window
        {
            self.rejection += RES.rejection_step;
        }
        self.last_school = Some((immortal, 0.0));
        if self.rejection >= RES.rejection_max {
            self.rejection = 0.0;
            return true;
        }
        false
    }

    /// A cast while burning with hollow fire lays a copy of the strongest burn at half its damage
    /// (`ailment::Ailments::hollow_cast`).
    pub fn on_cast(&mut self) {
        self.ailments.hollow_cast();
    }

    /// [`tick_with`](Cultivator::tick_with) for a fighter that wears nothing (the empty sheet; its timed effects and
    /// state still count). Returns the life lost to damage over time and Tâm ma.
    pub fn tick(&mut self, dt: f64, moving: bool) -> f64 {
        self.tick_with(dt, moving, &EMPTY).lost
    }

    /// The table this fighter's own rates and its defence against damage over time are read from: its `sheet`, its
    /// timed effects and its state, with no target and no skill.
    pub fn bearer_table(&self, sheet: &Sheet, moving: bool) -> Table {
        let ctx = Ctx {
            moving,
            ..Ctx::alone(self)
        };
        sheet.table(&ctx, [])
    }

    /// Time passes for a fighter wearing `sheet`: damage over time, regen, timers. Every rate is read from its
    /// table as the tick begins ([`bearer_table`](Cultivator::bearer_table)):
    /// - burns (only the strongest) and bleeds (all, twice while `moving`) through [`mitigation`]: a burn straight to
    ///   life, a bleed the shield first;
    /// - Tâm ma, a share of full life a second;
    /// - shield regen `ShieldRegen` (base `BASE.shield_regen`) once the wait is over; life regen `LifeRegen` (base 0,
    ///   a flat amount a second) as healing, so wither cuts it and rot turns it; qi regen `QiRegen` (base
    ///   `BASE.qi_regen`, `BASE.meditate_regen` meditating), which soak, Ly Phong and pressing intents cut through
    ///   their modifiers; nothing while choked or wind-locked.
    pub fn tick_with(&mut self, dt: f64, moving: bool, sheet: &Sheet) -> Ticked {
        let t = self.bearer_table(sheet, moving);
        let mut out = Ticked::default();
        for a in [Ailment::Burn, Ailment::Bleed] {
            let Some(v) = self.ailments.dots(a) else {
                continue;
            };
            let kind = a.element();
            let raw = v.damage(dt, moving);
            if raw <= 0.0 {
                continue;
            }
            let amount = raw * mitigation(self, &t, kind);
            let (to_shield, to_life) = if a == Ailment::Burn {
                (0.0, self.lose_life(amount))
            } else {
                self.hurt(amount)
            };
            if to_shield + to_life > 0.0 {
                out.lost += to_life;
                out.dots.push(DotTick {
                    ailment: a,
                    kind,
                    to_shield,
                    to_life,
                });
            }
        }
        if self.demon {
            out.demon = self.lose_life(RES.demon * self.life_max * dt);
            out.lost += out.demon;
        }
        self.ailments.tick(dt);

        self.shield_wait = (self.shield_wait - dt).max(0.0);
        if self.shield_wait <= 0.0 {
            let regen = t.get(Stat::ShieldRegen, BASE.shield_regen).max(0.0);
            self.shield = (self.shield + regen * dt).min(self.shield_max);
        }
        let life_regen = t.get(Stat::LifeRegen, 0.0);
        if life_regen > 0.0 {
            self.heal(life_regen * dt);
        }
        self.soul_lost = (self.soul_lost - dt).max(0.0);
        let sense_cap = (self.sense_max - self.sense_reserved).max(0.0);
        self.sense = (self.sense + BASE.sense_regen * dt).min(sense_cap);

        self.choke = (self.choke - dt).max(0.0);
        self.rot = (self.rot - dt).max(0.0);
        self.wind_lock = (self.wind_lock - dt).max(0.0);
        if self.choke <= 0.0 && self.wind_lock <= 0.0 {
            let base = if self.meditating {
                BASE.meditate_regen
            } else {
                BASE.qi_regen
            };
            let rate = t.get(Stat::QiRegen, base).max(0.0);
            self.qi = (self.qi + rate * dt).min(self.qi_max);
        }

        for w in &mut self.window {
            w.2 -= dt;
        }
        self.window.retain(|w| w.2 > 0.0);

        self.grit_wait -= dt;
        if self.grit_wait <= 0.0 && self.grit > 0.0 {
            self.grit = (self.grit - 1.0).max(0.0);
            self.grit_wait = RES.grit_fall;
        }

        for i in 0..3 {
            self.star_time[i] -= dt;
            if self.star_time[i] <= 0.0 {
                self.stars[i] = 0.0;
                self.star_time[i] = 0.0;
            }
        }

        if self.intent >= RES.intent_max {
            self.intent_full += dt;
            if self.intent_full >= RES.demon_after {
                self.demon = true;
            }
        } else {
            self.intent_full = 0.0;
        }
        if self.demon && self.intent < RES.demon_until {
            self.demon = false;
        }

        self.rejection = (self.rejection - RES.rejection_fall * dt).max(0.0);
        self.recent.tick(dt);
        for e in &mut self.effects {
            if let Some(t) = &mut e.left {
                *t -= dt;
            }
        }
        self.effects.retain(|e| e.left.is_none_or(|t| t > 0.0));
        if let Some((_, t)) = &mut self.last_school {
            *t += dt;
        }
        out
    }

    /// Every modifier of the effects on this fighter.
    pub fn effect_mods(&self) -> impl Iterator<Item = &Mod> {
        self.effects.iter().flat_map(|e| e.mods.iter())
    }

    /// Puts on a timed effect, replacing one from the same source.
    pub fn put_on(&mut self, a: Active) {
        self.effects.retain(|e| e.source != a.source);
        self.effects.push(a);
    }

    /// Takes off every effect from `source`.
    pub fn take_off(&mut self, source: Source) {
        self.effects.retain(|e| e.source != source);
    }

    /// The control a full ailment lays, if any, through grit.
    pub fn full_control(&mut self, a: Ailment, higher_intent: bool) -> Option<(Control, f64)> {
        self.full_control_for(a, higher_intent, 1.0)
    }

    /// [`full_control`](Cultivator::full_control) lasting `scale` times as long before grit.
    pub fn full_control_for(
        &mut self,
        a: Ailment,
        higher_intent: bool,
        scale: f64,
    ) -> Option<(Control, f64)> {
        let d = a.data();
        let c = d.control?;
        Some((c, self.control(d.seconds * scale.max(0.0), higher_intent)))
    }
}

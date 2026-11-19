//! The observation: the fighter, the foe it sees, the nearest threats and its slots, then the frame data of both
//! casts, what each slot builds to, what lies on both fighters, the things on the field and the foe's slots, as
//! `OBS` floats in a frame where the foe is ahead (positive x), every value clamped to [-4, 4] and anything not
//! finite read as 0. Times are in seconds unless a scale is given. ABI 3 only appended to ABI 2's 305: those keep
//! their indices and meaning, except that a slot's cost and damage are what it builds to now.
//!
//! Layout, in order (index ranges are of the whole observation; `BLOCKS` lists the blocks):
//! - me (`ME` = 17, 0..17): distance to the wall behind and ahead over the arena's width, air over the ceiling,
//!   facing the foe, hp / 100, qi / 100, flying, hover over the ceiling, vx toward the foe / 200, can act, silenced,
//!   sealed, barrier, shield (0..14); winding up, recovering, seconds left in that phase (14..17, zero when it uses
//!   no skill);
//! - foe (`FOE` = 37, 17..54): distance over the width, air over the ceiling, flying, hp / 100, qi / 100 (17..22),
//!   what it does (idle, moving toward me, moving away, guarding, meditating, flying, 22..28), winding up,
//!   recovering (28..30), the kind of skill it uses (12, 30..42), how far into its phase as a share of it (42),
//!   seconds left in the phase (43), the phase's span (44), the skill's reach / 100 (45) and damage / 50 (46)
//!   (28..47 zero when it uses no skill), out for, open in, frozen, shocked, barrier, the share of its skills ready,
//!   seeker out (mine) (47..54);
//! - threats (`THREATS` x `THREAT` = 4 x 12, 54..102), nearest first, zero when absent: present, shape
//!   (projectile, area, seeker, field, barrage), distance ahead / 100, height / 50, speed toward me / 200, lands
//!   in, size / 50, damage / 50;
//! - slots (`SLOTS` x `SLOT` = 6 x 33, 102..300), zero when empty, each: present, ready, kind (12), cost / 50,
//!   damage x power / 50 (both as built now, `SlotX`, when the game says), reach / 100, windup, recover,
//!   radius / 50 (+0..+20); then what it inflicts (`Inflicts`, +20..+33): freeze s, stun s, silence s / 2,
//!   defence-down s / 5, defence lost (share), burn s / 3, burn's share of attack a second x 5, chill stacks a hit
//!   adds now (x `Sense::stacks`) over the most it holds, slow per chill stack x 4, shock stacks a hit adds now over
//!   the most it holds, hold per shock twitch x 4, share of attack per shock twitch x 10, pull / 100;
//! - the game's tag (`TAG` = 5, 300..305);
//! - my cast, then the foe's (`ACT` = 11 each, 305..316 and 316..327), zero when not casting (the foe's also until
//!   it is seen acting): acting, in its active window, seconds to the window, the window's length, seconds until
//!   punishable (its recovery begins), the recovery's span, a hitbox, the hitbox's near and far x ahead of me / 100,
//!   its bottom and top over the ground / 50 (`ActFrames`);
//! - slot builds (`SLOTS` x `SLOTX` = 6 x 148, 327..1215), zero for an empty slot, each (`SlotX`): cooldown left,
//!   cooldown / 10, stored uses / 4, most uses / 4, windup, recover, the recovery a guarded blow adds, the active
//!   window's start and length, damage / 50, qi / 50, extra projectiles / 4, strikes / 4, area factor, pierce / 4,
//!   chain / 4, fork / 4, inert supports / 3 (+0..+18); each ailment a blow lays over its full (+18..+29); the
//!   supports it carries, one flag each (+29..+148);
//! - effects (2 x `EFFECT` = 2 x 113, 1215..1441), on me then on the foe (`Effects`): each ailment's stacks over its
//!   full, full / 10, longest time left / 10, a burn's or bleed's strongest damage a second over full life x 10
//!   (+0..+44); stun, root, down, shocked, soul lost, sealed, no guard, choke, rot, seconds (+44..+53); each curse's
//!   time left / 10 (+53..+65); auras held (+65..+80); buffs / 3, the longest's time left / 10, their sizes summed
//!   (+80..+83); energy shield share, ward charges / 3, barrier s (+83..+86); keystones (+86..+106); root one-hot
//!   (+106..+113);
//! - units (`UNITS` x `UNIT` = 8 x 12, 1441..1537), nearest first, zero when absent: present, mine, kind (trap,
//!   mine, totem, minion), distance ahead / 100, height over my feet / 50, life share, time left / 10 (40 s for
//!   one that lasts), armed, seconds to its next blow;
//! - the foe's slots (`SLOTS` x `FOE_SLOT` = 6 x 34, 1537..1741), zero when empty: present, ready, cooldown left,
//!   stored uses / 4, kind (12), windup, recover, the active window's start and length, damage / 50, qi / 50,
//!   reach / 100, each ailment a blow lays over its full (11);
//!
//! ABI 4 appends to ABI 3's 1741, which keep their indices and meaning (a slot's `ready` now also says the foe
//! stands inside its band):
//! - why (`SLOTS` x `WHY` = 6 x 9, 1741..1795), zero when empty: has a band, its min / 100, max / 100 (zero for
//!   none: castable at any distance), the foe in band, then why it cannot be cast now, one flag each: cooling down,
//!   short of qi, too near, too far, barred (silenced, sealed, a curse, can't act...);
//! - the foe's bands (`SLOTS` x `FOE_BAND` = 6 x 4, 1795..1819), zero when empty: has a band, min / 100, max / 100,
//!   I stand in it;
//! - buffs (`BUFFS` x `BUFF` = 4 x 5, 1819..1839): its own timed buffs, soonest to end first: present, stance,
//!   blessing, other, time left / 10;
//! - in play (`DOMAINS` + `FIELDS` + `AURAS_IN_PLAY` = 2 + 4 + 4, x `IN_PLAY` = 5, 1839..1889): the domains and
//!   auras its own first, the fields nearest first, each: present, mine, time left / 10 (4 for one held as long as
//!   its holder keeps it), seconds until it works, where it stands ahead of me / 100 (0 for an aura).

use super::{SLOTS, ahead};
use crate::sense::{
    AILMENTS, AURAS, ActFrames, ActPhase, Band, BuffKind, CURSES, Doing, Effects, Foe, InPlay,
    KEYSTONES, ROOTS, SUPPORTS, Sense, SlotX, Threat, ThreatShape, UnitKind,
};
use crate::skill::{Skill, SkillKind};

const ME: usize = 17;
const FOE: usize = 37;
const THREATS: usize = 4;
const THREAT: usize = 12;
const SLOT: usize = 33;
/// The game's tag: who the fighter is (the duel: its look, 2, and its style, 3).
pub const TAG: usize = 5;
/// A cast's frame data.
const ACT: usize = 11;
/// A slot's build: 18 numbers, an ailment's share each, a support's flag each.
const SLOTX: usize = 18 + AILMENTS + SUPPORTS;
/// What lies on one fighter: four numbers an ailment, 9 controls, the curses, the auras, 3 for buffs, 3 for shields,
/// the keystones, the roots.
const EFFECT: usize = 4 * AILMENTS + 9 + CURSES + AURAS + 3 + 3 + KEYSTONES + ROOTS;
/// The things on the field it sees, nearest first.
pub const UNITS: usize = 8;
const UNIT: usize = 12;
/// One of the foe's slots: 23 numbers and an ailment's share each.
const FOE_SLOT: usize = 23 + AILMENTS;
/// A band: has one, min, max (ABI 4).
const BAND: usize = 3;
/// Why one of its slots can or cannot be cast: its band, in band, cooling, short of qi, too near, too far, barred.
const WHY: usize = BAND + 6;
/// One of the foe's slots' band and whether I stand in it.
const FOE_BAND: usize = BAND + 1;
/// Its own timed buffs it sees, soonest to end first: present, stance, blessing, other, time left / 10.
pub const BUFFS: usize = 4;
const BUFF: usize = 5;
/// The domains (one a side), fields and auras in play it sees: present, mine, time left / 10 (`LASTS` for one held),
/// seconds until it works, where it stands ahead of me / 100 (0 for an aura).
pub const DOMAINS: usize = 2;
pub const FIELDS: usize = 4;
pub const AURAS_IN_PLAY: usize = 4;
const IN_PLAY: usize = 5;

/// Every block in order, as `docs/bot.md` names it, with its length.
pub const BLOCKS: [(&str, usize); 15] = [
    ("Me", ME),
    ("Foe", FOE),
    ("Threats", THREATS * THREAT),
    ("Slots", SLOTS * SLOT),
    ("Tag", TAG),
    ("My cast", ACT),
    ("Foe's cast", ACT),
    ("Slot builds", SLOTS * SLOTX),
    ("Effects", 2 * EFFECT),
    ("Units", UNITS * UNIT),
    ("Foe's slots", SLOTS * FOE_SLOT),
    ("Why", SLOTS * WHY),
    ("Foe's bands", SLOTS * FOE_BAND),
    ("Buffs", BUFFS * BUFF),
    ("In play", (DOMAINS + FIELDS + AURAS_IN_PLAY) * IN_PLAY),
];

pub const OBS: usize = total(&BLOCKS);

const fn total(blocks: &[(&str, usize)]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < blocks.len() {
        n += blocks[i].1;
        i += 1;
    }
    n
}

/// The slots a skill's kind takes, one-hot: three reserved (where the blade, the dive and the dash were before the arena
/// went spells only; never set, so a bot's inputs keep their shape), then [`KINDS`].
const RESERVED_KINDS: usize = 3;
const KINDS: [SkillKind; 9] = [
    SkillKind::Blast,
    SkillKind::Bolt,
    SkillKind::Seeker,
    SkillKind::Area,
    SkillKind::Field,
    SkillKind::Heal,
    SkillKind::Ward,
    SkillKind::Barrier,
    SkillKind::Blink,
];

/// Writes floats in order, each clamped.
struct Out {
    v: [f32; OBS],
    at: usize,
}

impl Out {
    fn put(&mut self, x: f64) {
        let x = if x.is_finite() {
            x.clamp(-4.0, 4.0)
        } else {
            0.0
        };
        if let Some(slot) = self.v.get_mut(self.at) {
            *slot = x as f32;
        }
        self.at += 1;
    }
    fn flag(&mut self, b: bool) {
        self.put(if b { 1.0 } else { 0.0 });
    }
    fn kind(&mut self, k: Option<SkillKind>) {
        self.skip(RESERVED_KINDS);
        for c in KINDS {
            self.flag(k == Some(c));
        }
    }
    fn skip(&mut self, n: usize) {
        self.at += n;
    }
    /// A band (`BAND`): has one, its min / 100 and max / 100; zero for none.
    fn band(&mut self, b: Option<Band>) {
        match b {
            Some(b) => {
                self.flag(true);
                self.put(b.min / 100.0);
                self.put(b.max / 100.0);
            }
            None => self.skip(BAND),
        }
    }
}

/// What the policy sees: `sense` for itself and its slots, `foe` and `threats` as it sees them (late, through
/// `Delayed`), `skills` as handed to the mind, and the game's `tag`.
pub fn observe(
    sense: &Sense,
    foe: &Foe,
    threats: &[Threat],
    skills: &[Skill],
    tag: &[f32; TAG],
) -> [f32; OBS] {
    let me = &sense.me;
    let a = &sense.arena;
    let dir = ahead(me, foe);
    let width = (a.right - a.left).max(1.0);
    let top = a.top.max(1.0);
    let mut o = Out {
        v: [0.0; OBS],
        at: 0,
    };

    let (behind, front) = if dir > 0.0 {
        (me.x - a.left, a.right - me.x)
    } else {
        (a.right - me.x, me.x - a.left)
    };
    o.put(behind / width);
    o.put(front / width);
    o.put(me.air / top);
    o.flag(me.face * dir > 0.0);
    o.put(me.hp / 100.0);
    o.put(me.qi / 100.0);
    o.flag(me.flying);
    o.put(me.hover / top);
    o.put(me.vx * dir / 200.0);
    o.flag(me.can_act);
    o.flag(me.silenced);
    o.flag(me.sealed);
    o.flag(me.barrier);
    o.flag(me.shield);
    match &me.act {
        Some(a) => {
            o.flag(a.phase == ActPhase::Windup);
            o.flag(a.phase == ActPhase::Recover);
            o.put(span_of(a.phase, a.windup, a.recover) - a.t);
        }
        None => o.skip(3),
    }

    o.put((foe.x - me.x) * dir / width);
    o.put(foe.air / top);
    o.flag(foe.flying);
    o.put(foe.hp / 100.0);
    o.put(foe.qi / 100.0);
    let (moving, toward) = match foe.doing {
        Doing::Move { dir: d } => (true, d * dir < 0.0),
        _ => (false, false),
    };
    o.flag(foe.doing == Doing::Idle);
    o.flag(moving && toward);
    o.flag(moving && !toward);
    o.flag(foe.doing == Doing::Guard);
    o.flag(foe.doing == Doing::Meditate);
    o.flag(foe.doing == Doing::Fly);
    match &foe.doing {
        Doing::Act {
            kind,
            phase,
            t,
            windup,
            recover,
            reach,
            damage,
        } => {
            o.flag(*phase == ActPhase::Windup);
            o.flag(*phase == ActPhase::Recover);
            o.kind(Some(*kind));
            let span = span_of(*phase, *windup, *recover);
            o.put(t / span.max(1e-3));
            o.put(span - t);
            o.put(span);
            o.put(reach / 100.0);
            o.put(damage / 50.0);
        }
        _ => o.skip(19),
    }
    o.put(foe.out_for);
    o.put(foe.open_in);
    o.put(foe.frozen);
    o.put(foe.shocked);
    o.flag(foe.barrier);
    let ready = foe.ready.iter().filter(|&&r| r).count() as f64 / (foe.ready.len().max(1) as f64);
    o.put(ready);
    o.flag(me.seeker_out);

    let mut near: Vec<(f64, &Threat)> = threats.iter().map(|t| (distance(t, me.x), t)).collect();
    near.sort_by(|x, y| x.0.total_cmp(&y.0));
    for i in 0..THREATS {
        let Some((_, t)) = near.get(i) else {
            o.skip(THREAT);
            continue;
        };
        o.flag(true);
        let (shape, x, y, vx, lands, size) = match t.shape {
            ThreatShape::Projectile { x, y, vx } => (0, x, y, vx, 0.0, 0.0),
            ThreatShape::Area {
                x,
                radius,
                reach,
                lands_in,
                ..
            } => (1, x, 0.0, 0.0, lands_in, radius.max(reach)),
            ThreatShape::Seeker { x, y, radius } => (2, x, y, 0.0, 0.0, radius),
            ThreatShape::Field { x, reach } => (3, x, 0.0, 0.0, 0.0, reach),
            ThreatShape::Barrage { lands_in } => (4, me.x, 0.0, 0.0, lands_in, 0.0),
        };
        for s in 0..5 {
            o.flag(s == shape);
        }
        o.put((x - me.x) * dir / 100.0);
        o.put(y / 50.0);
        o.put(-vx * dir / 200.0);
        o.put(lands);
        o.put(size / 50.0);
        o.put(t.damage / 50.0);
    }

    for i in 0..SLOTS {
        let skill = sense
            .slots
            .get(i)
            .copied()
            .flatten()
            .and_then(|k| skills.get(k).map(|s| (k, s)));
        let Some((k, s)) = skill else {
            o.skip(SLOT);
            continue;
        };
        o.flag(true);
        o.flag(sense.ready.get(k).copied().unwrap_or(false));
        o.kind(Some(s.kind));
        let power = sense.power.get(k).copied().unwrap_or(1.0);
        match sense.slotx.get(i).copied().flatten() {
            Some(x) => {
                o.put(x.qi / 50.0);
                o.put(x.damage * power / 50.0);
            }
            None => {
                o.put(s.cost / 50.0);
                o.put(s.damage * power / 50.0);
            }
        }
        o.put(s.reach / 100.0);
        o.put(s.windup);
        o.put(s.recover);
        o.put(s.radius / 50.0);
        let i = &s.inflicts;
        let stacks = sense.stacks.get(k).copied().unwrap_or(1.0);
        o.put(i.freeze);
        o.put(i.stun);
        o.put(i.silence / 2.0);
        o.put(i.def_down / 5.0);
        o.put(i.def_loss);
        o.put(i.burn / 3.0);
        o.put(i.burn_rate * 5.0);
        o.put(i.chill * stacks / i.chill_max.max(1.0));
        o.put(i.slow * 4.0);
        o.put(i.shock * stacks / i.shock_max.max(1.0));
        o.put(i.shock_hold * 4.0);
        o.put(i.shock_rate * 10.0);
        o.put(i.pull / 100.0);
    }

    for &t in tag {
        o.put(f64::from(t));
    }

    o.act(sense.me_act.as_ref(), me.x, dir);
    // the foe's cast is seen as its doing is: not while the delay still hides it
    let seen = match (&foe.doing, &sense.foe.doing) {
        (
            Doing::Act { kind, phase, .. },
            Doing::Act {
                kind: k, phase: p, ..
            },
        ) => kind == k && phase == p,
        _ => false,
    };
    o.act(sense.foe_act.as_ref().filter(|_| seen), me.x, dir);

    for x in &sense.slotx {
        match x {
            Some(x) => o.slotx(x),
            None => o.skip(SLOTX),
        }
    }

    for e in &sense.effects {
        o.effects(e);
    }

    let mut units: Vec<_> = sense.units.iter().collect();
    units.sort_by(|a, b| libm::fabs(a.x - me.x).total_cmp(&libm::fabs(b.x - me.x)));
    for i in 0..UNITS {
        let Some(u) = units.get(i) else {
            o.skip(UNIT);
            continue;
        };
        o.flag(true);
        o.flag(u.mine);
        for k in [
            UnitKind::Trap,
            UnitKind::Mine,
            UnitKind::Totem,
            UnitKind::Minion,
        ] {
            o.flag(u.kind == k);
        }
        o.put((u.x - me.x) * dir / 100.0);
        o.put((u.y - me.air) / 50.0);
        o.put(u.hp);
        o.put(u.left.unwrap_or(LASTS).min(LASTS) / 10.0);
        o.flag(u.armed);
        o.put(u.next);
    }

    for f in &sense.foe_slots {
        let Some(f) = f else {
            o.skip(FOE_SLOT);
            continue;
        };
        let x = &f.built;
        o.flag(true);
        o.flag(f.ready);
        o.put(x.cooldown_left);
        o.put(x.stored / 4.0);
        o.kind(Some(f.kind));
        o.put(x.windup);
        o.put(x.recover);
        o.put(x.window_start);
        o.put(x.window);
        o.put(x.damage / 50.0);
        o.put(x.qi / 50.0);
        o.put(f.reach / 100.0);
        for &v in &x.inflict {
            o.put(v);
        }
    }

    // ABI 4: the bands and why a slot cannot be cast, then how long what is in play lasts
    for w in &sense.why {
        let Some(w) = w else {
            o.skip(WHY);
            continue;
        };
        o.band(w.band);
        o.flag(w.in_band);
        o.flag(w.cooling);
        o.flag(w.short);
        o.flag(w.too_near);
        o.flag(w.too_far);
        o.flag(w.barred);
    }
    for f in &sense.foe_slots {
        let Some(f) = f else {
            o.skip(FOE_BAND);
            continue;
        };
        o.band(f.band);
        o.flag(f.in_band);
    }
    let l = &sense.lasting;
    let mut buffs = l.buffs.clone();
    buffs.sort_by(|a, b| a.left.total_cmp(&b.left));
    for i in 0..BUFFS {
        let Some(b) = buffs.get(i) else {
            o.skip(BUFF);
            continue;
        };
        o.flag(true);
        for k in [BuffKind::Stance, BuffKind::Blessing, BuffKind::Other] {
            o.flag(b.kind == k);
        }
        o.put(b.left / 10.0);
    }
    // a domain or an aura: its own first; a field: nearest first
    let mine_first = |v: &[InPlay]| {
        let mut v = v.to_vec();
        v.sort_by_key(|p| !p.mine);
        v
    };
    let mut fields = l.fields.clone();
    fields.sort_by(|a, b| {
        let d = |p: &InPlay| p.x.map_or(f64::INFINITY, |x| (x - me.x).abs());
        d(a).total_cmp(&d(b))
    });
    for (v, n) in [
        (mine_first(&l.domains), DOMAINS),
        (fields, FIELDS),
        (mine_first(&l.auras), AURAS_IN_PLAY),
    ] {
        for i in 0..n {
            let Some(p) = v.get(i) else {
                o.skip(IN_PLAY);
                continue;
            };
            o.flag(true);
            o.flag(p.mine);
            o.put(p.left.unwrap_or(LASTS).min(LASTS) / 10.0);
            o.put(p.works_in);
            o.put(p.x.map_or(0.0, |x| (x - me.x) * dir / 100.0));
        }
    }
    o.v
}

/// The seconds a unit that never falls apart reads as.
const LASTS: f64 = 40.0;

impl Out {
    /// A cast's frame data (`ACT`), its hitbox seen from `x` with the foe on side `dir`.
    fn act(&mut self, a: Option<&ActFrames>, x: f64, dir: f64) {
        let Some(a) = a else {
            self.skip(ACT);
            return;
        };
        self.flag(true);
        self.flag(a.in_window);
        self.put(a.to_window);
        self.put(a.window);
        self.put(a.to_punish);
        self.put(a.recover);
        match a.hitbox {
            Some([x0, y0, x1, y1]) => {
                let (n, f) = ((x0 - x) * dir, (x1 - x) * dir);
                self.flag(true);
                self.put(n.min(f) / 100.0);
                self.put(n.max(f) / 100.0);
                self.put(y0 / 50.0);
                self.put(y1 / 50.0);
            }
            None => self.skip(5),
        }
    }

    /// A slot's build (`SLOTX`).
    fn slotx(&mut self, x: &SlotX) {
        self.put(x.cooldown_left);
        self.put(x.cooldown / 10.0);
        self.put(x.stored / 4.0);
        self.put(x.max_uses / 4.0);
        self.put(x.windup);
        self.put(x.recover);
        self.put(x.blocked_extra);
        self.put(x.window_start);
        self.put(x.window);
        self.put(x.damage / 50.0);
        self.put(x.qi / 50.0);
        self.put(x.projectiles / 4.0);
        self.put(x.strikes / 4.0);
        self.put(x.area);
        self.put(x.pierce / 4.0);
        self.put(x.chain / 4.0);
        self.put(x.fork / 4.0);
        self.put(x.inert / 3.0);
        for &v in &x.inflict {
            self.put(v);
        }
        for i in 0..SUPPORTS {
            self.flag(x.supports >> i & 1 == 1);
        }
    }

    /// What lies on one fighter (`EFFECT`).
    fn effects(&mut self, e: &Effects) {
        for a in &e.ailments {
            self.put(if a.cap > 0.0 { a.stacks / a.cap } else { 0.0 });
            self.put(a.cap / 10.0);
            self.put(a.left / 10.0);
            self.put(a.strength * 10.0);
        }
        for c in [
            e.stun,
            e.root,
            e.down,
            e.shocked,
            e.soul_lost,
            e.sealed,
            e.no_guard,
            e.choke,
            e.rot,
        ] {
            self.put(c);
        }
        for &c in &e.curses {
            self.put(c / 10.0);
        }
        for &a in &e.auras {
            self.flag(a);
        }
        self.put(e.buffs / 3.0);
        self.put(e.buff_left / 10.0);
        self.put(e.buff_size);
        self.put(e.shield);
        self.put(e.ward / 3.0);
        self.put(e.barrier);
        for &k in &e.keystones {
            self.flag(k);
        }
        for r in 0..ROOTS {
            self.flag(e.element == Some(r));
        }
    }
}

/// Seconds phase `phase` of a skill lasts.
fn span_of(phase: ActPhase, windup: f64, recover: f64) -> f64 {
    if phase == ActPhase::Windup {
        windup
    } else {
        recover
    }
}

/// How far a threat is from `x`: what "nearest" sorts by (a barrage is upon the fighter already).
fn distance(t: &Threat, x: f64) -> f64 {
    match t.shape {
        ThreatShape::Projectile { x: tx, .. }
        | ThreatShape::Area { x: tx, .. }
        | ThreatShape::Seeker { x: tx, .. }
        | ThreatShape::Field { x: tx, .. } => libm::fabs(tx - x),
        ThreatShape::Barrage { .. } => 0.0,
    }
}

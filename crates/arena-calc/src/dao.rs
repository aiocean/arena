//! The daos (đạo): mechanisms that change the rules rather than kinds of damage (research, 4; rules, 6.1 to 6.8).
//! Sinh tử (life marks and death qi), Nhân quả (karmic threads, Khí Nhân Quả, the Tử Ấn tether), Chân giả
//! (illusory blows), Tuế nguyệt (slow, the rewind buffer, the tree watcher), Không gian (blink lock, rifts), Duyên
//! (bond threads and their cutting), Mệnh (fate cuts, lifespan drain) and Sát lục (killing intent). Pure state and
//! functions: no dice inside, positions are the caller's, and every blow a dao makes comes back as a [`Hit`] for
//! [`crate::hit::resolve`]. Curses (6.9) are in [`crate::curse`].

use std::collections::VecDeque;

use crate::cultivator::{Cultivator, RES};
use crate::element::Element;
use crate::gem::Resource;
use crate::grade::{Grade, Rank};
use crate::hit::{Hit, ILLUSION_TRUE_PEN};
use crate::tag::{Tag, Tags};

/// An entity the fight names: a fighter (its seat, 0 or 1), a minion, a shield, a treasure, a zone.
pub type Id = u32;

/// Every dao's numbers (rules, 6).
pub struct Dao {
    /// Sinh tử: a life mark lasts this long, in seconds.
    pub mark_time: f64,
    /// Sinh tử: the healing a mark stores, at most this share of the marked target's full life.
    pub mark_cap: f64,
    /// Sinh tử, Kích Tử: a Death blow on a marked target adds yin of this many times the stored healing.
    pub death_trigger: f64,
    /// Sinh tử, Luân hồi: sowing Life within this many seconds of a Death trigger on the same target rewinds it.
    pub rebirth_window: f64,
    /// Sinh tử, Luân hồi: how far back the rewind goes, in seconds.
    pub rebirth_back: f64,
    /// Sinh tử, Tử khí: death qi points per whole full life, both paid (for a life cost) and filled (by yin dealt).
    pub death_qi_per_life: f64,
    /// Nhân quả: the share of a blow on one end that echoes to the other, when the skill does not say.
    pub echo: f64,
    /// Nhân quả, Khí Nhân Quả: a broken shield sends back this share of the blow that broke it.
    pub rebound: f64,
    /// Nhân quả, Tử Ấn Tổ Quyết: the tether's longest stretch in T, when the skill does not say.
    pub tether_length: f64,
    /// Tử Ấn: the tether shows itself once every this many seconds...
    pub tether_every: f64,
    /// ...for this many seconds, at the end of each cycle (the first opening comes 2.5 s after the curse).
    pub tether_open: f64,
    /// Tử Ấn: the curser dying costs the cursed this share of its current life.
    pub tether_death: f64,
    /// Chân giả: a target that answers an illusion within this many seconds makes it true.
    pub believe_window: f64,
    /// Chân giả: two fighters' Chân giả skills active within this many seconds of each other clash...
    pub clash_window: f64,
    /// ...and each takes this share of its full life as soul damage.
    pub clash: f64,
    /// Tuế nguyệt: samples the rewind buffer keeps per fighter (2 s at 60 ticks a second).
    pub history: usize,
    /// Tuế nguyệt: samples a second.
    pub rate: f64,
    /// Tuế nguyệt, Người Xem Cây: standing still this long makes the keystone's bearer immune to slow and rewind.
    pub tree_still: f64,
    /// Không gian: a rift's radius in T...
    pub rift_radius: f64,
    /// ...its life in seconds...
    pub rift_time: f64,
    /// ...and the share of full life a blink ending inside it deals as force, armour ignored.
    pub rift: f64,
    /// Duyên: cutting a fighter with no outside thing drains this much lifespan instead.
    pub bond_drain: f64,
    /// Mệnh: a fate cut that finds no extra life and no buff drains this much lifespan.
    pub fate_drain: f64,
}
pub const DAO: Dao = Dao {
    mark_time: 5.0,
    mark_cap: 0.3,
    death_trigger: 2.5,
    rebirth_window: 1.0,
    rebirth_back: 1.0,
    death_qi_per_life: 100.0,
    echo: 0.3,
    rebound: 0.5,
    tether_length: 4.0,
    tether_every: 3.0,
    tether_open: 0.5,
    tether_death: 0.5,
    believe_window: 3.0,
    clash_window: 0.5,
    clash: 0.1,
    history: 120,
    rate: 60.0,
    tree_still: 0.5,
    rift_radius: 1.0,
    rift_time: 1.0,
    rift: 0.2,
    bond_drain: 10.0,
    fate_drain: 10.0,
};

/// A blow of `amount` of one kind that cannot be dodged and never crits (a dao's own damage: a trigger, a rebound,
/// a rift, a clash).
pub fn dao_hit(element: Element, amount: f64) -> Hit {
    let mut h = Hit::split(amount.max(0.0), &[(element, 1.0)]);
    h.instant = true;
    h.crit = 0.0;
    h
}

/// `hit` scaled by `share`: its base and added damage, no crit, no dodge, no leech, no counter. Echoes and rebounds
/// keep the blow's kinds and rank.
pub fn scaled(hit: &Hit, share: f64) -> Hit {
    let mut h = hit.clone();
    let k = share.max(0.0);
    for i in 0..h.base.len() {
        h.base[i] *= k;
        h.added[i] *= k;
    }
    h.crit = 0.0;
    h.evadable = false;
    h.leech = 0.0;
    h.frame = crate::hit::Frame::Plain;
    h
}

// ---------------------------------------------------------------------------------------------------------------
// Sinh tử
// ---------------------------------------------------------------------------------------------------------------

/// Dấu Sinh: the healing a Life skill sowed on a target, waiting for a Death blow.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LifeMark {
    /// H, the healing stored (the skill's heal, not what the full bar took).
    pub stored: f64,
    /// Seconds left.
    pub left: f64,
}

/// What sowing Life did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sow {
    /// The life the target gained (negative under full wither).
    pub healed: f64,
    /// The mark's healing after this sowing.
    pub stored: f64,
    /// Luân hồi: the target was struck by Death within the window; the caller rewinds it with [`rebirth`].
    pub rebirth: bool,
}

/// Sinh tử on one target: its mark, and the seconds since a Death trigger on it (for Luân hồi).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LifeDeath {
    pub mark: Option<LifeMark>,
    pub since_death: Option<f64>,
}

impl LifeDeath {
    /// A Life skill heals `target` by `heal` and marks it: one mark a target, a new sowing adds to it and renews it,
    /// up to [`Dao::mark_cap`] of the target's full life. Sowing on oneself is allowed, and the opponent's Death
    /// triggers that mark too.
    pub fn sow(&mut self, target: &mut Cultivator, heal: f64) -> Sow {
        let heal = heal.max(0.0);
        let healed = target.heal(heal);
        let cap = DAO.mark_cap * target.life_max;
        let stored = (self.mark.map_or(0.0, |m| m.stored) + heal).min(cap);
        self.mark = Some(LifeMark {
            stored,
            left: DAO.mark_time,
        });
        let rebirth = self.since_death.is_some_and(|t| t <= DAO.rebirth_window);
        if rebirth {
            self.since_death = None;
        }
        Sow {
            healed,
            stored,
            rebirth,
        }
    }

    /// Kích Tử: a blow tagged Death on this target sets off its mark: the returned blow is yin of
    /// [`Dao::death_trigger`] × H, and the mark goes. `None` with no mark.
    pub fn trigger(&mut self) -> Option<Hit> {
        let m = self.mark.take()?;
        self.since_death = Some(0.0);
        Some(dao_hit(Element::Yin, DAO.death_trigger * m.stored))
    }

    /// Time passes: the mark runs out, the Luân hồi window closes.
    pub fn tick(&mut self, dt: f64) {
        if let Some(m) = &mut self.mark {
            m.left -= dt;
            if m.left <= 0.0 {
                self.mark = None;
            }
        }
        if let Some(t) = &mut self.since_death {
            *t += dt;
            if *t > DAO.rebirth_window {
                self.since_death = None;
            }
        }
    }
}

/// A skill tagged Life sows a mark.
pub fn sows(tags: Tags) -> bool {
    tags.has(Tag::Life)
}

/// A skill tagged Death sets a mark off.
pub fn triggers(tags: Tags) -> bool {
    tags.has(Tag::Death)
}

/// Cạn sinh hóa tử: a life-death cultivator below [`RES`]`.death_below` of its full life pays life costs in death qi.
pub fn death_open(c: &Cultivator) -> bool {
    c.life < RES.death_below * c.life_max
}

/// How a life cost was paid.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Paid {
    /// In life (never the last point).
    Life(f64),
    /// In death qi points.
    DeathQi(f64),
    /// Not enough death qi: the skill cannot be cast.
    Short,
}

/// Pays a life cost of `cost`: in life, or for a `life_death` cultivator with the death qi bar open, in death qi
/// ([`Dao::death_qi_per_life`] points per whole full life of cost).
pub fn pay_life_cost(c: &mut Cultivator, life_death: bool, cost: f64) -> Paid {
    if life_death && death_open(c) {
        let need = cost.max(0.0) / c.life_max * DAO.death_qi_per_life;
        if c.death_qi + 1e-9 < need {
            return Paid::Short;
        }
        c.death_qi = (c.death_qi - need).max(0.0);
        return Paid::DeathQi(need);
    }
    Paid::Life(c.pay_life(cost))
}

/// Yin damage a `life_death` cultivator dealt fills its death qi while the bar is open, up to
/// [`RES`]`.death_max`. Returns the points gained.
pub fn fill_death_qi(c: &mut Cultivator, life_death: bool, yin_dealt: f64) -> f64 {
    if !life_death || !death_open(c) {
        return 0.0;
    }
    let before = c.death_qi;
    c.death_qi =
        (c.death_qi + yin_dealt.max(0.0) / c.life_max * DAO.death_qi_per_life).min(RES.death_max);
    c.death_qi - before
}

// ---------------------------------------------------------------------------------------------------------------
// Nhân quả
// ---------------------------------------------------------------------------------------------------------------

/// Sợi nhân quả: a karmic thread between two entities (two fighters, a fighter and a minion, a fighter and a
/// shield). A blow on one end echoes to the other.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Thread {
    pub a: Id,
    pub b: Id,
    /// The share that echoes ([`Dao::echo`] unless the skill says).
    pub share: f64,
    /// The rank of the skill that tied it.
    pub rank: Rank,
    /// Seconds left.
    pub left: f64,
}

impl Thread {
    /// A thread of the default share.
    pub fn new(a: Id, b: Id, rank: Rank, seconds: f64) -> Thread {
        Thread {
            a,
            b,
            share: DAO.echo,
            rank,
            left: seconds,
        }
    }

    /// The other end from `struck`, if `struck` is an end.
    pub fn other(&self, struck: Id) -> Option<Id> {
        if struck == self.a {
            Some(self.b)
        } else if struck == self.b {
            Some(self.a)
        } else {
            None
        }
    }

    /// A blow on `struck` echoes to the other end: the same kinds and rank, `share` of it, never dodged and never a
    /// crit. The caller never echoes an echo (two threads would ring forever).
    pub fn echo(&self, struck: Id, hit: &Hit) -> Option<(Id, Hit)> {
        if self.left <= 0.0 {
            return None;
        }
        let to = self.other(struck)?;
        Some((to, scaled(hit, self.share)))
    }

    /// Time passes; false once the thread is gone.
    pub fn tick(&mut self, dt: f64) -> bool {
        self.left -= dt;
        self.left > 0.0
    }
}

/// Khí Nhân Quả: when a shield of a fighter who bears it breaks to `blow`, a blow of [`Dao::rebound`] of it goes
/// back from that fighter at whoever dealt `blow`. `None` when the shield held or the fighter lacks Khí Nhân Quả.
pub fn rebound(broke: bool, karma_qi: bool, blow: &Hit) -> Option<Hit> {
    (broke && karma_qi).then(|| scaled(blow, DAO.rebound))
}

/// Tử Ấn Tổ Quyết's tether: the cursed fighter cannot stray more than `length` T from the curser. Every
/// [`Dao::tether_every`] s it shows for [`Dao::tether_open`] s, and a sword of its rank or more cuts it then.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tether {
    /// The curser.
    pub by: Id,
    /// The cursed.
    pub on: Id,
    /// The longest stretch, in T.
    pub length: f64,
    pub rank: Rank,
    /// Seconds since it was tied.
    pub clock: f64,
}

impl Tether {
    /// A tether of the default length.
    pub fn new(by: Id, on: Id, rank: Rank) -> Tether {
        Tether {
            by,
            on,
            length: DAO.tether_length,
            rank,
            clock: 0.0,
        }
    }

    /// The thread shows: the last [`Dao::tether_open`] s of each [`Dao::tether_every`] s cycle.
    pub fn exposed(&self) -> bool {
        let t = libm::fmod(self.clock.max(0.0), DAO.tether_every);
        t >= DAO.tether_every - DAO.tether_open
    }

    /// Where the cursed may stand: pulled back onto the tether's reach around the curser when it stretched past it.
    pub fn hold(&self, curser: (f64, f64), cursed: (f64, f64)) -> (f64, f64) {
        let (dx, dy) = (cursed.0 - curser.0, cursed.1 - curser.1);
        let d = libm::hypot(dx, dy);
        if d <= self.length || d <= 0.0 {
            return cursed;
        }
        let k = self.length / d;
        (curser.0 + dx * k, curser.1 + dy * k)
    }

    /// A blow cuts the tether: a sword of a rank not below the tether's, while it shows. A restriction has no rank
    /// and cannot cut it.
    pub fn cut_by(&self, tags: Tags, blow: Grade) -> bool {
        self.exposed() && tags.has(Tag::Sword) && matches!(blow, Grade::Ranked(b) if b >= self.rank)
    }

    /// The curser died: the cursed loses [`Dao::tether_death`] of its current life (not all of it: killing at once
    /// is too strong in a duel). Returns what it lost.
    pub fn curser_died(cursed: &mut Cultivator) -> f64 {
        let v = cursed.life * DAO.tether_death;
        cursed.lose_life(v)
    }

    /// Time passes.
    pub fn tick(&mut self, dt: f64) {
        self.clock += dt;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Chân giả
// ---------------------------------------------------------------------------------------------------------------

/// Đòn ảo: an illusory blow in flight. It deals [`crate::hit::ILLUSION`] of its damage and lays nothing, unless
/// the target answers it in time and makes it true.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Illusion {
    /// Seconds since it appeared.
    pub age: f64,
    /// Ảo hóa thật.
    pub believed: bool,
    /// It has landed (or is gone): answers no longer change it.
    pub landed: bool,
}

impl Illusion {
    pub fn new() -> Illusion {
        Illusion::default()
    }

    /// The target cast a skill: a Guard, a Movement or its own top-rank skill within [`Dao::believe_window`] s,
    /// while the blow still flies, makes it true. Returns whether it is true now. A target that moved out of its
    /// path still dodges it: true or not, a dodge is a dodge.
    pub fn answer(&mut self, tags: Tags, top_rank: bool) -> bool {
        if !self.landed
            && self.age <= DAO.believe_window
            && (top_rank || tags.any(&[Tag::Guard, Tag::Movement]))
        {
            self.believed = true;
        }
        self.believed
    }

    /// Shapes the blow it carries: illusory unless believed; a believed one passes [`ILLUSION_TRUE_PEN`] of armour.
    pub fn shape(&self, hit: &mut Hit) {
        hit.illusory = !self.believed;
        if self.believed {
            hit.armour_pen = hit.armour_pen.max(ILLUSION_TRUE_PEN);
        }
    }

    /// Time passes.
    pub fn tick(&mut self, dt: f64) {
        self.age += dt;
    }
}

/// A viewer sees an illusion for what it is: under glare (Chiếu), or with a Chân giả skill of its own active. The
/// bot reads this as the blow's `ảo` flag.
pub fn sees_through(viewer: &Cultivator, own_true_false: bool) -> bool {
    own_true_false || viewer.ailments.stacks(crate::ailment::Ailment::Glare) > 0.0
}

/// Two fighters' Chân giả skills, active at `a` and `b` (seconds on the fight's clock), clash.
pub fn clash(a: f64, b: f64) -> bool {
    (a - b).abs() <= DAO.clash_window
}

/// The soul damage each side of a clash takes: [`Dao::clash`] of its own full life.
pub fn clash_hit(c: &Cultivator) -> Hit {
    dao_hit(Element::Soul, DAO.clash * c.life_max)
}

// ---------------------------------------------------------------------------------------------------------------
// Tuế nguyệt
// ---------------------------------------------------------------------------------------------------------------

/// The speed factor under a slow of `s` (0..1): velocities and skill clocks are multiplied, ticks still run. A tree
/// watcher standing still is immune.
pub fn slowed(s: f64, tree_immune: bool) -> f64 {
    if tree_immune {
        1.0
    } else {
        1.0 - s.clamp(0.0, 1.0)
    }
}

/// Every Time skill costs lifespan.
pub fn needs_lifespan(tags: Tags) -> bool {
    tags.has(Tag::Time)
}

/// A fighter's state at one tick, for rewinding.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sample {
    pub x: f64,
    pub y: f64,
    pub life: f64,
    pub shield: f64,
}

/// The rewind buffer: a fighter's last [`Dao::history`] samples, newest last.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct History {
    pub samples: VecDeque<Sample>,
}

impl History {
    pub fn new() -> History {
        History::default()
    }

    /// Records this tick's sample, dropping the oldest past [`Dao::history`].
    pub fn record(&mut self, s: Sample) {
        self.samples.push_back(s);
        while self.samples.len() > DAO.history {
            self.samples.pop_front();
        }
    }

    /// The sample `t` seconds ago (the oldest kept when `t` reaches past the buffer); `None` when empty.
    pub fn back(&self, t: f64) -> Option<Sample> {
        let n = self.samples.len();
        if n == 0 {
            return None;
        }
        let k = libm::floor(t.max(0.0) * DAO.rate + 0.5) as usize;
        self.samples.get(n - 1 - k.min(n - 1)).copied()
    }
}

/// A skill in its windup, as a rewind sees it.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Windup {
    /// Tự bạo.
    pub self_destruct: bool,
    /// Seconds since the cast began (when its costs were paid).
    pub age: f64,
    /// What it cost.
    pub paid: Vec<(Resource, f64)>,
}

/// What a rewind did.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rewound {
    /// Where the fighter goes back to (the caller moves the body; a restriction cage still holds it).
    pub at: Sample,
    /// The windup was cancelled.
    pub cancelled: bool,
    /// What was given back (a self-destruct's costs).
    pub refunded: Vec<(Resource, f64)>,
}

/// Gives `paid` back to `c`, each resource up to its cap.
pub fn refund(c: &mut Cultivator, paid: &[(Resource, f64)]) {
    for &(r, v) in paid {
        let v = v.max(0.0);
        match r {
            Resource::Qi => c.qi = (c.qi + v).min(c.qi_max),
            Resource::Life => c.life = (c.life + v).min(c.life_max),
            Resource::Lifespan => c.lifespan += v,
            Resource::Intent => c.gain_intent(v),
            Resource::Souls => c.souls = (c.souls + v).min(RES.souls_max),
            Resource::Sense => c.sense = (c.sense + v).min(c.sense_max),
            Resource::DeathQi => c.death_qi = (c.death_qi + v).min(RES.death_max),
            Resource::Star(s) => {
                let i = s as usize;
                c.stars[i] = (c.stars[i] + v).min(RES.stars_max);
                if c.stars[i] > 0.0 {
                    c.star_time[i] = RES.star_time;
                }
            }
        }
    }
}

/// Quay lui: `target` goes back `t` seconds: position, life and energy shield from the buffer; spent resources stay
/// spent, except a self-destruct caught in its windup, which is cancelled and its costs given back (the life part
/// only when the cast is older than `t`, since the rewound life already holds it). `None` for a tree watcher
/// standing still, or an empty buffer.
pub fn rewind(
    target: &mut Cultivator,
    history: &History,
    t: f64,
    windup: Option<&Windup>,
    tree_immune: bool,
) -> Option<Rewound> {
    if tree_immune {
        return None;
    }
    let at = history.back(t)?;
    target.life = at.life.clamp(0.0, target.life_max);
    target.shield = at.shield.clamp(0.0, target.shield_max);
    let mut out = Rewound {
        at,
        cancelled: false,
        refunded: Vec::new(),
    };
    if let Some(w) = windup.filter(|w| w.self_destruct) {
        out.cancelled = true;
        out.refunded = w
            .paid
            .iter()
            .copied()
            .filter(|&(r, _)| !(r == Resource::Life && w.age <= t))
            .collect();
        refund(target, &out.refunded);
    }
    Some(out)
}

/// Luân hồi: the target goes back [`Dao::rebirth_back`] s in position and life (not shield), and whatever it was
/// winding up is cancelled, with nothing given back.
pub fn rebirth(target: &mut Cultivator, history: &History, winding: bool) -> Option<Rewound> {
    let at = history.back(DAO.rebirth_back)?;
    target.life = at.life.clamp(0.0, target.life_max);
    Some(Rewound {
        at,
        cancelled: winding,
        refunded: Vec::new(),
    })
}

/// Người Xem Cây: how long a fighter has stood still (velocity 0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stillness {
    pub still: f64,
}

impl Stillness {
    /// Time passes; any motion starts the count again.
    pub fn tick(&mut self, dt: f64, moving: bool) {
        self.still = if moving { 0.0 } else { self.still + dt };
    }

    /// Immune to slow and rewind: the keystone's bearer, still for [`Dao::tree_still`] s.
    pub fn immune(&self, keystone: bool) -> bool {
        keystone && self.still >= DAO.tree_still
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Không gian
// ---------------------------------------------------------------------------------------------------------------

/// Khóa thuấn di: no Movement skill while it lasts, except one tagged Time (Quỷ Sát moves through time).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlinkLock {
    pub left: f64,
}

impl BlinkLock {
    /// Locks for `seconds`, keeping the longer of the two.
    pub fn lock(&mut self, seconds: f64) {
        self.left = self.left.max(seconds);
    }

    /// A skill of `tags` may be used.
    pub fn allows(&self, tags: Tags) -> bool {
        self.left <= 0.0 || !tags.has(Tag::Movement) || tags.has(Tag::Time)
    }

    /// Time passes.
    pub fn tick(&mut self, dt: f64) {
        self.left = (self.left - dt).max(0.0);
    }
}

/// A blink passes every blow flying at it during its windup, except an instant (Thuấn).
pub fn blink_passes(blow: Tags) -> bool {
    !blow.has(Tag::Instant)
}

/// Khe nứt: a rift of [`Dao::rift_radius`] T for [`Dao::rift_time`] s. A blink that ends inside it tears the
/// blinker; walking through does nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rift {
    pub x: f64,
    pub y: f64,
    pub left: f64,
}

impl Rift {
    pub fn new(x: f64, y: f64) -> Rift {
        Rift {
            x,
            y,
            left: DAO.rift_time,
        }
    }

    /// A move ending at (`x`, `y`) is caught: a blink, inside, while the rift lasts.
    pub fn catches(&self, x: f64, y: f64, blink: bool) -> bool {
        blink
            && self.left > 0.0
            && crate::collide::overlap(
                &crate::collide::Shape::circle(x, y, 0.0),
                &crate::collide::Shape::circle(self.x, self.y, DAO.rift_radius),
            )
    }

    /// The tear: [`Dao::rift`] of the blinker's full life as force, through all armour.
    pub fn tear(blinker: &Cultivator) -> Hit {
        let mut h = dao_hit(Element::Force, DAO.rift * blinker.life_max);
        h.armour_pen = 1.0;
        h
    }

    /// Time passes; false once it closes.
    pub fn tick(&mut self, dt: f64) -> bool {
        self.left -= dt;
        self.left > 0.0
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Duyên
// ---------------------------------------------------------------------------------------------------------------

/// A thing a fighter's bond threads reach. The order is the cut order when a skill does not pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Tie {
    /// A buff from outside (a domain, the field, an ally): any buff the fighter's own skills did not make.
    Outside,
    /// A treasure (pháp bảo) set out.
    Treasure,
    /// A minion.
    Minion,
    /// A zone the fighter laid (not in the rules' order: cut last).
    Zone,
}

/// One bond thread.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bond {
    pub tie: Tie,
    pub id: Id,
    /// Cut: the thing is out until the round ends.
    pub cut: bool,
}

/// What a bond cut took.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum BondCut {
    /// This thing is out until the round ends.
    Thing(Tie, Id),
    /// No thing to cut: this much lifespan drained.
    Lifespan(f64),
}

/// A fighter's bond threads. Its own (life, cultivation) are not threads and cannot be cut.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bonds {
    pub threads: Vec<Bond>,
}

impl Bonds {
    pub fn new() -> Bonds {
        Bonds::default()
    }

    /// A thing of the fighter's comes into the world. Returns false when it was cut this round: it stays out.
    pub fn tie(&mut self, tie: Tie, id: Id) -> bool {
        if let Some(b) = self.threads.iter().find(|b| b.id == id) {
            return !b.cut;
        }
        self.threads.push(Bond {
            tie,
            id,
            cut: false,
        });
        true
    }

    /// A thing ended by itself; a cut one stays on record until the round ends.
    pub fn untie(&mut self, id: Id) {
        self.threads.retain(|b| b.id != id || b.cut);
    }

    /// The thing is out (its thread was cut this round).
    pub fn is_cut(&self, id: Id) -> bool {
        self.threads.iter().any(|b| b.id == id && b.cut)
    }

    /// The threads still whole.
    pub fn live(&self) -> impl Iterator<Item = &Bond> {
        self.threads.iter().filter(|b| !b.cut)
    }

    /// Trảm duyên on this fighter (`owner`): cuts `pick` when it is a live thread, otherwise the first live one in
    /// [`Tie`] order (outside buffs, treasures, minions, zones; the oldest first). With no thread, drains
    /// [`Dao::bond_drain`] lifespan from the owner instead.
    pub fn cut(&mut self, pick: Option<Id>, owner: &mut Cultivator) -> BondCut {
        let picked = pick.and_then(|id| self.threads.iter().position(|b| b.id == id && !b.cut));
        let first = || {
            self.threads
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.cut)
                .min_by_key(|&(i, b)| (b.tie, i))
                .map(|(i, _)| i)
        };
        match picked.or_else(first) {
            Some(i) => {
                let b = &mut self.threads[i];
                b.cut = true;
                BondCut::Thing(b.tie, b.id)
            }
            None => BondCut::Lifespan(drain(owner, DAO.bond_drain)),
        }
    }

    /// The round ends: every thread, cut or whole, is forgotten.
    pub fn new_round(&mut self) {
        self.threads.clear();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Mệnh
// ---------------------------------------------------------------------------------------------------------------

/// Lifespan lost, never below 0; returns what was lost.
fn drain(c: &mut Cultivator, amount: f64) -> f64 {
    let v = amount.max(0.0).min(c.lifespan.max(0.0));
    c.lifespan -= v;
    v
}

/// Only a skill tagged Fate or Time drains an opponent's lifespan.
pub fn drains_lifespan(tags: Tags) -> bool {
    tags.any(&[Tag::Fate, Tag::Time])
}

/// A skill of `tags` drains `amount` of `target`'s lifespan, when it may. Returns what was drained.
pub fn drain_lifespan(tags: Tags, target: &mut Cultivator, amount: f64) -> f64 {
    if drains_lifespan(tags) {
        drain(target, amount)
    } else {
        0.0
    }
}

/// A buff as a fate cut sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Held {
    pub grade: Grade,
    /// Hư bản nguyên: only Faith removes it, and a fate cut passes it by.
    pub hollow: bool,
}

/// What a fate cut took.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum FateCut {
    /// An extra life (Tam Mệnh, Ngọn Đèn Hồn).
    ExtraLife,
    /// The buff at this index of the list given (the caller removes it).
    Buff(usize),
    /// This much lifespan.
    Lifespan(f64),
}

/// Chém mệnh cách: takes, in order, an extra life; else the highest-ranked buff that is neither hollow nor a
/// restriction (the first of equals); else [`Dao::fate_drain`] lifespan.
pub fn cut_fate(extra_lives: &mut u8, buffs: &[Held], target: &mut Cultivator) -> FateCut {
    if *extra_lives > 0 {
        *extra_lives -= 1;
        return FateCut::ExtraLife;
    }
    let best = buffs
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.hollow)
        .filter_map(|(i, b)| b.grade.rank().map(|r| (i, r)))
        .fold(None::<(usize, Rank)>, |best, (i, r)| match best {
            Some((_, br)) if br >= r => best,
            _ => Some((i, r)),
        });
    match best {
        Some((i, _)) => FateCut::Buff(i),
        None => FateCut::Lifespan(drain(target, DAO.fate_drain)),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Sát lục
// ---------------------------------------------------------------------------------------------------------------

/// A blow landed: killing intent rises by [`RES`]`.intent_hit`.
pub fn intent_on_hit(c: &mut Cultivator) {
    c.gain_intent(RES.intent_hit);
}

/// A minion was struck down: killing intent rises by [`RES`]`.intent_kill`.
pub fn intent_on_kill(c: &mut Cultivator) {
    c.gain_intent(RES.intent_kill);
}

/// Time passes without striking: killing intent falls by [`RES`]`.intent_fall` a second. (Its edge, its power and
/// Tâm ma are in [`Cultivator::power`] and [`Cultivator::tick`].)
pub fn intent_tick(c: &mut Cultivator, dt: f64, striking: bool) {
    if !striking {
        c.gain_intent(-RES.intent_fall * dt);
    }
}

//! Bí quyết: the supports a player hangs on each skill of a loadout (design, 8; research, 11; rules, 2.2). A
//! support is data: the tags it needs, its stat modifiers ([`Support::stats`], with its cost and situational ones
//! made at the cast by [`Support::mods`]), what it does that is not a stat ([`Mods`]), and the behaviours the fight
//! reads. `build` gathers its modifiers with the striker's sheet and reads only the table. A support whose tags do
//! not fit sits inert (rules, 2.2.1); `loadout::check` refuses it outright.
//!
//! Conversion runs only forward in the element order (Force → Metal → Wood → Water → Fire → Earth → Thunder → Light →
//! Yin → Soul). A support that would need a backward conversion (Âm Hàn: Fire and Thunder to Water) instead rewrites
//! the base split at step 1 ([`Mods::rewrite`]), before step 2's added damage converts.
//!
//! Supports the engine's conditions make possible (PoE's): Thời Gian Kéo Dài (ailment duration), Độc Tâm (damage
//! over time multiplier), Hỏa Ngục Thiêu Đốt (burn more, hits less), Bạo Phát (more on a cursed target), Thừa Thắng
//! (more on a target bleeding hard) and Thị Huyết (more just after a kill).
//!
//! Then PoE's own supports, each named after its analogue in a comment on its row (design, 8, the second table):
//! added damage of every kind; focus and destruction; projectiles (pierce, fork, chain, speed, more near or far);
//! melee (every-nth-use, impale, culling, maim, a multistrike); spells (echo, unleash, arcane surge, efficacy,
//! inspiration); damage over time and ailments (burning, immolate, swift and deadly ailments, cruelty, bloodlust,
//! withering touch, void manipulation, combustion, overcharge, and Thủy Phệ, a water hypothermia that soaks and never
//! chills); crits and stars; recovery; curses and intents (hextouch, blasphemy, enlighten, generosity); traps, mines,
//! totems and minions; the trigger supports, which aim an `event::Trigger` at the skill they are hung on
//! ([`Support::trigger_for`]); and Huyết Tế (lifetap). They speak the richer modifiers (`stat::Per`, the new
//! conditions and stats); what only the fight can do is a [`Behaviour`], and the field-bound stats
//! (pierce, area, minions, devices...) are read by `projectile`, `play`, `motion`, `device` and `charge`.

use crate::ailment::Ailment;
use crate::build::{Situation, Skill};
use crate::cultivator::Star;
use crate::dot::BURN_TIME;
use crate::element::{Element, N};
use crate::event::{CAST_ON_CRIT, CAST_ON_LOW_LIFE, CAST_ON_SHIELD_BREAK, CAST_WHEN_HURT, Trigger};
use crate::hit::Debuff;
use crate::stat::{Condition, Happened, Mod, Per, Source, Stat};
use crate::tag::{Group, Tag, Tags};

/// What a support does that is not a stat modifier (its stat modifiers are [`Support::stats`]): rewrites, added
/// damage as a share of the skill's base, more that grows with the cast's situation, and the flags and costs the
/// builder reads. All zero for none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mods {
    /// Step 1: moves `(from, to, share)` of the base (and of added damage) from one kind to another, in any
    /// direction. This is how a support that would convert backward works (Âm Hàn).
    pub rewrite: &'static [(Element, Element, f64)],
    /// Step 2: added damage of a kind, as a share of the skill's base (made a flat [`Stat::Added`] at the cast)...
    pub added: &'static [(Element, f64)],
    /// ...while this holds (Hỏa Thiêu Thân: on a burning target).
    pub added_when: Condition,
    /// Step 4: more for each turn a charged skill was held (Cửu Chuyển), up to [`TURNS_MAX`].
    pub more_per_turn: f64,
    /// Step 4: more for each T a projectile has flown (Dịch Linh), up to `travel_max`.
    pub more_per_t: f64,
    pub travel_max: f64,
    /// Evasion cannot dodge it (Chắc Trúng).
    pub unevadable: bool,
    /// The burn tier it lays at least (0 red .. 3 black), and hollow fire.
    pub fire_tier: Option<u8>,
    pub hollow_fire: bool,
    /// One rank step, at most Rule (Quy Tắc).
    pub rank_step: bool,
    /// Life paid at the cast, a share of full life (each 1% adds `build::BLOOD_POWER` more).
    pub blood: f64,
    /// Controls the blow lays last this many times as long when the target just used a guard skill (Nhử Đòn).
    pub bait_control: f64,
    /// Tags the skill takes on (Bẫy Trận makes it a trap): its modifiers are gathered with them and the blow carries
    /// them (`build`).
    pub becomes: &'static [Tag],
    /// A timed effect the blow puts on its target (`hit::Debuff`: Phế Cước's maim, Nhiên Thiêu's fire resistance).
    pub debuff: Option<Debuff>,
    /// Damage over time this much more at most, by the hit's size (Tàn Khốc; `hit::CRUELTY_LIFE`).
    pub cruelty: f64,
}

impl Mods {
    /// Nothing.
    pub const NONE: Mods = Mods {
        rewrite: &[],
        added: &[],
        added_when: Condition::Always,
        more_per_turn: 0.0,
        more_per_t: 0.0,
        travel_max: 0.0,
        unevadable: false,
        fire_tier: None,
        hollow_fire: false,
        rank_step: false,
        blood: 0.0,
        bait_control: 1.0,
        becomes: &[],
        debuff: None,
        cruelty: 0.0,
    };
}

/// Something a support makes the fight do that the calculator does not: the fight reads these.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", tag = "type"))]
pub enum Behaviour {
    /// Cửu Chuyển: held to turn 1..9 times; blocked by the right skill, the shot changes owner (bounces back).
    TurnsOnBlock,
    /// Dung: fused with another skill of the loadout ([`fuse`]).
    Fuse,
    /// Dung: each cast has this chance to blow back this share of the skill's damage on the caster.
    Rejection { chance: f64, share: f64 },
    /// Hóa Ma: a foe's minion dying under it has this chance to blow up on its owner.
    MinionsBurst { chance: f64 },
    /// Huyết Luyện: the treasure breaking costs this share of full life.
    TreasureBreak { life: f64 },
    /// Phun Máu: the blow passes one layer of defence (a ward, a barrier).
    PierceLayer,
    /// Súc Địa: leaps to the target, then strikes.
    LeapToTarget,
    /// Dịch Linh: the shot grows with distance and can be called back.
    Recall,
    /// Lôi Bạo Trễ: a blocked or swallowed shot stays in what stopped it as a mine, set off on command.
    MineOnBlock,
    /// Nhử Đòn: controls last longer if the target just guarded (the factor is in [`Mods::bait_control`]).
    Bait,
    /// Liên Hoàn Suy Yếu: when this guard breaks, cast an attack of the loadout dealing what the guard absorbed.
    CastOnGuardBreak,
    /// Phản Ấn: the guard absorbs one projectile and returns it as this skill's kind.
    AbsorbAndReturn,
    /// Khí Nhân Quả: when the shield or formation breaks, a copy of the blow that broke it flies back.
    ReflectBreaker,
    /// Tự Bạo: the thing it makes explodes when destroyed, for this share of its life.
    SelfDestruct { share: f64 },
    /// Thu Hồn: anything that dies under it makes it this much more, for the round.
    GrowOnKill { more: f64 },
    /// Thôn Cấm: a foe's trap, totem or zone inside it is unmade and charges the skill.
    DevourField,
    /// Sao Chép: learns a weaker copy (this share) of the foe's last skill.
    CopyLast { share: f64 },
    /// Mê Dục: on hit, the target cannot use a guard skill for this long.
    NoGuard { seconds: f64 },
    /// Trọng: drops a flyer.
    Grounds,
    /// Tuế Nguyệt: projectiles in the zone slow by this share.
    SlowShots { share: f64 },
    /// Chặn Lui: on hit, no escape for this long.
    NoEscape { seconds: f64 },
    /// Không Khóa: on hit, no blink for this long.
    NoBlink { seconds: f64 },
    /// Hồn Bạo: sacrifices a minion to explode for this share of its life left.
    SacrificeMinion { share: f64 },
    /// Nạp Hồn: soul damage dealt earns souls (one each `RES.souls_per_damage`).
    SoulsFromSoul,
    /// Đoạt Tu Vi: drains this share of what lands from the target's qi.
    DrainQi { share: f64 },
    /// Lưu Lực (PoE's Impale): this share of the force a blow deals strikes again, as force, `delay` seconds later.
    StoredForce { share: f64, delay: f64 },
    /// Sát Sinh Bổ Huyết: a kill heals this share of full life.
    LifeOnKill { share: f64 },
    /// Chú Ấn Kích (PoE's Hextouch): the curse is laid by the bearer's blows that land, not cast.
    CurseOnHit,
    /// Chú Vực (PoE's Blasphemy): the curse stands as an aura around the bearer, holding sense (`build::Built::
    /// reserve`), on every foe inside.
    CurseAura,
    /// Khảng Khái (PoE's Generosity): the intent works on allies, never on its holder.
    NotOnSelf,
    /// Ảo Linh (PoE's Summon Phantasm): each blow that lands has this chance to raise a phantasm minion.
    SummonPhantasm { chance: f64 },
    /// The skill is cast by its support's trigger (`Support::trigger_for`), never by hand.
    Triggered,
    /// Kênh Dẫn Pháp (PoE's Cast while Channelling): cast every this many seconds while the bearer channels.
    CastWhileChannelling { every: f64 },
    /// Phong Ấn (PoE's Unleash): a seal gathers every `seconds` while it is ready, up to `max`; a cast spends them
    /// all, a repeat each.
    Seals { max: u8, seconds: f64 },
    /// Bẫy Trận, Địa Lôi Trận, Pháp Đàn: the skill is thrown as a trap, laid as a mine or raised on a totem (the
    /// tag), which casts it.
    Device { kind: Tag },
}

/// A support (bí quyết).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Support {
    /// Its code name.
    pub id: &'static str,
    /// Its Vietnamese name.
    pub name: &'static str,
    /// The skill must have every one of these tags, at least one of `any_of` (when not empty), and none of
    /// `none_of`. Tags include the skill's kinds of damage (`Gem::all_tags`).
    pub all_of: &'static [Tag],
    pub any_of: &'static [Tag],
    pub none_of: &'static [Tag],
    /// Its cost multiplier: a skill's costs times the product of its supports' (rules, 2.2.3).
    pub cost: f64,
    /// Loadout points, 1 to 3.
    pub points: u32,
    /// Its stat modifiers ([`Source::Support`] its id): damage more and less, conversion, crit, penetration, leech,
    /// ailments, projectiles and repeats, windup. Its cost multiplier joins them at the cast ([`Support::mods`]).
    pub stats: &'static [Mod],
    pub mods: Mods,
    pub behaviours: &'static [Behaviour],
    /// The event it is cast on (`event::Trigger`), for a trigger support; [`Support::trigger_for`] aims it at the
    /// skill it is hung on.
    pub trigger: Option<Trigger>,
    /// It converts or rewrites kinds (Cực Cảnh forbids it).
    pub conversion: bool,
    /// Supports it cannot share a skill with (rules, 2.2.4); the relation is read both ways.
    pub excludes: &'static [&'static str],
    /// Where it comes from: `TN<chapter>` or `CM<chapter>`, or "arena".
    pub source: &'static str,
}

impl Support {
    /// A blank support to build the table from.
    pub const NONE: Support = Support {
        id: "",
        name: "",
        all_of: &[],
        any_of: &[],
        none_of: &[],
        cost: 1.0,
        points: 1,
        stats: &[],
        mods: Mods::NONE,
        behaviours: &[],
        trigger: None,
        conversion: false,
        excludes: &[],
        source: "arena",
    };

    /// Whether it fits a skill of `tags`.
    pub fn fits(&self, tags: Tags) -> bool {
        tags.all(self.all_of)
            && (self.any_of.is_empty() || tags.any(self.any_of))
            && !tags.any(self.none_of)
    }

    /// The kinds it adds, converts or rewrites to.
    pub fn kinds(&self) -> Vec<Element> {
        let m = &self.mods;
        let mut out: Vec<Element> = m
            .rewrite
            .iter()
            .map(|r| r.1)
            .chain(m.added.iter().map(|a| a.0))
            .chain(self.stats.iter().filter_map(|x| match x.stat {
                Stat::Added(e) | Stat::Convert(_, e) | Stat::Extra(_, e) => Some(e),
                _ => None,
            }))
            .collect();
        if m.fire_tier.is_some() {
            out.push(Element::Fire);
        }
        out.sort();
        out.dedup();
        out
    }

    /// Every modifier it gives a cast of `skill` in `sit`: its [`stats`](Support::stats), its cost multiplier as a
    /// [`Stat::Cost`] more, its added damage as flat [`Stat::Added`] (the share of the base, over the skill's
    /// effectiveness, so step 2 gives the share back), and the more a held turn or a flown T earns.
    pub fn mods(&self, skill: &Skill, sit: &Situation) -> Vec<Mod> {
        let src = Source::Support(self.id);
        let m = &self.mods;
        let mut out = self.stats.to_vec();
        if self.cost != 1.0 {
            out.push(Mod::more(Stat::Cost, self.cost - 1.0, src));
        }
        if skill.effectiveness > 0.0 {
            for &(e, share) in m.added {
                out.push(
                    Mod::flat(
                        Stat::Added(e),
                        share * skill.base / skill.effectiveness,
                        src,
                    )
                    .when(m.added_when),
                );
            }
        }
        if m.more_per_turn != 0.0 {
            for _ in 0..sit.turns.min(TURNS_MAX) {
                out.push(Mod::more(Stat::Damage, m.more_per_turn, src));
            }
        }
        let flown = (sit.travelled.max(0.0) * m.more_per_t).min(m.travel_max);
        if flown > 0.0 {
            out.push(Mod::more(Stat::Damage, flown, src));
        }
        out
    }

    /// Its trigger aimed at `skill`: the event casts the skill it is hung on, at its level. `None` for a support
    /// that is not a trigger.
    pub fn trigger_for(&self, skill: &Skill) -> Option<Trigger> {
        self.trigger.map(|t| t.casting(skill.id, skill.level))
    }

    /// Whether it is the fusing support (Dung).
    pub fn is_fuse(&self) -> bool {
        self.behaviours.contains(&Behaviour::Fuse)
    }
}

/// Whether two supports exclude each other on one skill.
pub fn excluded(a: &Support, b: &Support) -> bool {
    a.excludes.contains(&b.id) || b.excludes.contains(&a.id)
}

/// Thanh Trần: more.
pub const FOCUS_MORE: f64 = 0.25;
/// Liên Châu: extra projectiles, and each one's more (less).
pub const MULTISHOT: u8 = 2;
pub const MULTISHOT_MORE: f64 = -0.3;
/// Tàn Ảnh Tốc Thần: extra strikes, and each one's more (less).
pub const AFTERIMAGE: u8 = 1;
pub const AFTERIMAGE_MORE: f64 = -0.25;
/// Cửu Chuyển: more a turn held, and the turns at most.
pub const TURN_MORE: f64 = 0.15;
pub const TURNS_MAX: u8 = 9;
/// Cửu Biến: more, and the windup factor.
pub const CHANGES_MORE: f64 = 0.8;
pub const CHANGES_WINDUP: f64 = 2.5;
/// Hóa Ma: the chance a dying foe's minion blows up.
pub const DEMON_BURST: f64 = 0.25;
/// Âm Hàn: the share of fire and thunder rewritten to water at step 1.
pub const COLD_SHARE: f64 = 0.5;
/// Huyết Luyện: life paid a point of qi, a share of full life (all its qi is paid in life: [`Stat::CostAsLife`] at
/// `build::LIFE_PER_QI`); life lost when the treasure breaks.
pub const BLOOD_FORGE_QI: f64 = crate::build::LIFE_PER_QI;
pub const BLOOD_FORGE_BREAK: f64 = 0.1;
/// Phun Máu: life paid, a share of full life.
pub const BLOOD_SPIT: f64 = 0.1;
/// Dịch Linh: more each T flown, at most.
pub const SHIFT_MORE_PER_T: f64 = 0.1;
pub const SHIFT_MORE_MAX: f64 = 0.6;
/// Nhử Đòn: controls on a target that just guarded.
pub const BAIT_CONTROL: f64 = 2.0;
/// Chắc Trúng: windup factor.
pub const SURE_WINDUP: f64 = 1.4;
/// Tự Bạo: the explosion's share of the thing's life.
pub const SELF_DESTRUCT: f64 = 0.5;
/// Thu Hồn: more a kill, for the round.
pub const HARVEST_MORE: f64 = 0.1;
/// Sao Chép: the copy's strength.
pub const COPY_SHARE: f64 = 0.5;
/// Mê Dục: no guard skills for this long.
pub const ALLURE_SECONDS: f64 = 2.0;
/// Trọng: earth added, a share of the base, and the windup factor.
pub const HEAVY_ADDED: f64 = 0.15;
pub const HEAVY_WINDUP: f64 = 1.2;
/// Hư Hỏa: the burn tier (purple).
pub const HOLLOW_TIER: u8 = 2;
/// Tuế Nguyệt: projectiles in the zone slow by this.
pub const YEARS_SLOW: f64 = 0.4;
/// Chặn Lui and Không Khóa: seconds.
pub const NO_ESCAPE: f64 = 1.0;
pub const NO_BLINK: f64 = 2.0;
/// Hồn Bạo: the explosion's share of the minion's life left.
pub const SOUL_BURST: f64 = 1.0;
/// Đoạt Tu Vi: life drunk and qi drained, shares of what lands.
pub const DRAIN_LEECH: f64 = 0.05;
pub const DRAIN_QI: f64 = 0.02;
/// The five phase conversions: force converted.
pub const PHASE_SHARE: f64 = 0.5;
/// Xuyên Giáp, Phá Kháng.
pub const PIERCE_ARMOUR: f64 = 0.3;
pub const BREAK_RESIST: f64 = 0.2;
/// Ác Tính: ailments more, damage less.
pub const MALIGNANT_AILMENT: f64 = 0.5;
pub const MALIGNANT_MORE: f64 = -0.15;
/// Chí Mạng: crit chance and multiplier added.
pub const CRITICAL: f64 = 0.1;
pub const CRITICAL_MULTI: f64 = 0.3;

/// Dung: the fused skill's damage, a share of both bases.
pub const FUSE_SHARE: f64 = 0.6;
/// Dung: each cast's chance of rejection, and the share of the skill's damage it blows back.
pub const FUSE_REJECTION: f64 = 0.15;
pub const FUSE_BACKLASH: f64 = 0.2;

/// Thời Gian Kéo Dài: ailment duration increased.
pub const LINGER_DURATION: f64 = 0.5;
/// Độc Tâm: damage-over-time multiplier added.
pub const VENOM_DOT_MULTI: f64 = 0.3;
/// Hỏa Ngục Thiêu Đốt: burn more, and hit damage less.
pub const INFERNO_BURN: f64 = 0.5;
pub const INFERNO_HIT: f64 = -0.25;
/// Bạo Phát: more against a cursed target, for each curse it carries, at most.
pub const OUTBURST_MORE: f64 = 0.3;
pub const OUTBURST_MAX: f64 = 0.6;
/// Thừa Thắng: more against a target bleeding at least this many stacks.
pub const PRESS_MORE: f64 = 0.35;
pub const PRESS_BLEED: f64 = 5.0;
/// Thị Huyết: more within this many seconds of a kill.
pub const BLOODTHIRST_MORE: f64 = 0.25;
pub const BLOODTHIRST_SECONDS: f64 = 4.0;

// ---- the PoE supports (design, 8: the second table) ----

/// Phụ Hỏa, Phụ Thủy... (PoE's Added Fire Damage and kin): added damage of a kind, a share of the skill's base.
pub const ADDED_SHARE: f64 = 0.25;
/// Ngũ Hành Ngưng (Elemental Focus): more, and the ailments it no longer lays.
pub const ELEMENTAL_FOCUS_MORE: f64 = 0.35;
/// Khống Diệt (Controlled Destruction): spell more; crit chance reduced (an increase of −1 is none at all).
pub const CONTROLLED_MORE: f64 = 0.3;
pub const CONTROLLED_CRIT: f64 = -1.0;
/// Tụ Lực (Concentrated Effect): area less, damage more.
pub const CONCENTRATED_AREA: f64 = -0.3;
pub const CONCENTRATED_MORE: f64 = 0.35;
/// Khoách Vực (Increased Area of Effect): area increased.
pub const WIDE_AREA: f64 = 0.4;
/// Thuần Lực (Brutality): force more; every other kind deals none.
pub const BRUTALITY_MORE: f64 = 0.4;
/// Xuyên Thấu (Pierce): projectiles pierce this many, and more.
pub const PIERCE_COUNT: f64 = 2.0;
pub const PIERCE_MORE: f64 = 0.1;
/// Phân Nhánh (Fork): forks, and less.
pub const FORK_COUNT: f64 = 1.0;
pub const FORK_MORE: f64 = -0.1;
/// Liên Kích (Chain): chains, and less.
pub const CHAIN_COUNT: f64 = 2.0;
pub const CHAIN_MORE: f64 = -0.25;
/// Tật Tiễn (Faster Projectiles): speed increased, and more.
pub const FASTER_SHOTS: f64 = 0.5;
pub const FASTER_SHOTS_MORE: f64 = 0.1;
/// Hoãn Tiễn (Slower Projectiles): speed reduced, and more.
pub const SLOWER_SHOTS: f64 = -0.3;
pub const SLOWER_SHOTS_MORE: f64 = 0.2;
/// Tiểu Liên Châu (Lesser Multiple Projectiles): extra projectiles, and less.
pub const LESSER_MULTISHOT: f64 = 1.0;
pub const LESSER_MULTISHOT_MORE: f64 = -0.15;
/// Cận Xạ (Point Blank): more within `POINT_BLANK_NEAR` T, less beyond `POINT_BLANK_FAR`.
pub const POINT_BLANK_MORE: f64 = 0.3;
pub const POINT_BLANK_NEAR: f64 = 2.0;
pub const POINT_BLANK_LESS: f64 = -0.2;
pub const POINT_BLANK_FAR: f64 = 6.0;
/// Viễn Xạ (Far Shot): more each T of distance, at most.
pub const FAR_SHOT_PER_T: f64 = 0.05;
pub const FAR_SHOT_MAX: f64 = 0.3;
/// Cương Lực (Melee Physical Damage): force more, speed less.
pub const MELEE_FORCE_MORE: f64 = 0.35;
pub const MELEE_FORCE_SPEED: f64 = -0.1;
/// Áp Sát (Close Combat): more within this many T.
pub const CLOSE_MORE: f64 = 0.3;
pub const CLOSE_NEAR: f64 = 1.5;
/// Tàn Nhẫn (Ruthless): every `RUTHLESS_EVERY`th use, more, and daze more.
pub const RUTHLESS_EVERY: u32 = 3;
pub const RUTHLESS_MORE: f64 = 0.9;
pub const RUTHLESS_DAZE: f64 = 0.5;
/// Chiến Quyền (Fist of War): every `FIST_EVERY`th use, more and area increased.
pub const FIST_EVERY: u32 = 4;
pub const FIST_MORE: f64 = 0.6;
pub const FIST_AREA: f64 = 0.5;
/// Lưu Lực (Impale): the share of force stored, struck again after this many seconds.
pub const STORED_FORCE: f64 = 0.3;
pub const STORED_DELAY: f64 = 1.0;
/// Trảm Tận (Culling Strike): a target left at or under this share of full life dies.
pub const CULLING: f64 = 0.1;
/// Phế Cước (Maim): force more; the target runs slower and takes more force, for this long.
pub const MAIM_MORE: f64 = 0.1;
pub const MAIM_SLOW: f64 = -0.3;
pub const MAIM_TAKEN: f64 = 0.1;
pub const MAIM_SECONDS: f64 = 4.0;
/// Tốc Kích (Faster Attacks), Tốc Chú (Faster Casting): speed increased.
pub const FASTER_ATTACKS: f64 = 0.3;
pub const FASTER_CASTING: f64 = 0.3;
/// Tam Liên Ảnh (a multistrike): extra strikes, less, and speed more.
pub const TRIPLE_STRIKES: f64 = 2.0;
pub const TRIPLE_MORE: f64 = -0.35;
pub const TRIPLE_SPEED: f64 = 0.2;
/// Hồi Âm (Spell Echo): a repeat, cast speed more, damage less.
pub const ECHO_REPEATS: f64 = 1.0;
pub const ECHO_SPEED: f64 = 0.5;
pub const ECHO_MORE: f64 = -0.1;
/// Linh Triều (Arcane Surge): once this much qi was spent recently, spell more and speed increased.
pub const SURGE_QI: f64 = 30.0;
pub const SURGE_MORE: f64 = 0.15;
pub const SURGE_SPEED: f64 = 0.1;
/// Hiệu Lực (Efficacy): more, damage over time more, the skill's effects shorter.
pub const EFFICACY_MORE: f64 = 0.15;
pub const EFFICACY_DOT: f64 = 0.2;
pub const EFFICACY_DURATION: f64 = -0.2;
/// Linh Cảm (Inspiration): its cost multiplier (cheaper), and crit chance increased each mind star, at most.
pub const INSPIRATION_COST: f64 = 0.75;
pub const INSPIRATION_CRIT: f64 = 0.25;
pub const INSPIRATION_MAX: f64 = 0.75;
/// Liệt Diễm (Burning Damage): burn more.
pub const BURNING_MORE: f64 = 0.45;
/// Hỏa Thiêu Thân (Immolate): fire added on a burning target, a share of the base.
pub const IMMOLATE_ADDED: f64 = 0.3;
/// Tốc Độc (Swift Affliction): damage over time more; ailments and effects shorter.
pub const SWIFT_DOT: f64 = 0.35;
pub const SWIFT_DURATION: f64 = -0.35;
/// Độc Ngấm (Deadly Ailments): damage over time more, hits less.
pub const DEADLY_DOT: f64 = 0.45;
pub const DEADLY_HIT: f64 = -0.2;
/// Tàn Khốc (Cruelty): damage over time more at most, by the hit's size.
pub const CRUELTY_MORE: f64 = 0.4;
/// Khát Huyết (Bloodlust): more each bleed on the target, at most.
pub const BLOODLUST_PER: f64 = 0.08;
pub const BLOODLUST_MAX: f64 = 0.48;
/// Tịch Thủ (Withering Touch): wither stacks each blow lays.
pub const WITHER_ON_HIT: f64 = 1.0;
/// Hư Không (Void Manipulation): yin more, the five phases less.
pub const VOID_MORE: f64 = 0.25;
pub const VOID_LESS: f64 = -0.25;
/// Nhiên Thiêu (Combustion): fire more; a burn it lays lowers the target's fire resistance, for a burn's time.
pub const COMBUSTION_MORE: f64 = 0.1;
pub const COMBUSTION_RESIST: f64 = -0.15;
/// Tăng Ý (Intensify): more and area less for each blow landed recently (its intensity), at most.
pub const INTENSIFY_MORE: f64 = 0.08;
pub const INTENSIFY_AREA: f64 = -0.05;
pub const INTENSIFY_MAX: f64 = 3.0;
/// Phong Ấn (Unleash): seals at most, a seal every this many seconds, and less.
pub const UNLEASH_SEALS: u8 = 2;
pub const UNLEASH_SECONDS: f64 = 1.5;
pub const UNLEASH_MORE: f64 = -0.25;
/// Quá Tải (Overcharge): jolt more, thunder more.
pub const OVERCHARGE_JOLT: f64 = 0.5;
pub const OVERCHARGE_MORE: f64 = 0.1;
/// Thủy Phệ (the water Hypothermia: soak, never chill): more on a soaked target, soak effect increased.
pub const SOAKED_MORE: f64 = 0.25;
pub const SOAKED_EFFECT: f64 = 0.3;
/// Bạo Thương (Increased Critical Damage): crit multiplier added.
pub const CRIT_DAMAGE: f64 = 0.5;
/// Ma Tinh Dẫn (Power Charge on Critical): the chance of a mind star on a crit, and crit chance increased.
pub const POWER_ON_CRIT: f64 = 0.5;
pub const POWER_CRIT: f64 = 0.2;
/// Thần Tinh Dẫn (Endurance Charge on Melee Stun): the chance of a body star on a hard control, and daze more.
pub const ENDURANCE_ON_CONTROL: f64 = 1.0;
pub const ENDURANCE_DAZE: f64 = 0.25;
/// Yêu Tinh Dẫn (Frenzy Charge on Hit): the chance of a speed star on a blow against a foe with a soul.
pub const FRENZY_ON_HIT: f64 = 0.25;
/// Tam Tài (Trinity): more each kind of damage dealt recently, at most.
pub const TRINITY_PER: f64 = 0.05;
pub const TRINITY_MAX: f64 = 0.3;
/// Hồi Huyết (Life Gain on Hit): life a blow.
pub const LIFE_ON_HIT: f64 = 20.0;
/// Hấp Khí (Mana Leech): qi drunk, a share of what lands.
pub const QI_LEECH: f64 = 0.02;
/// Sát Sinh Bổ Huyết (Life Gain on Kill): a share of full life a kill.
pub const LIFE_ON_KILL: f64 = 0.05;
/// Chú Ấn Kích (Hextouch): curse effect less.
pub const HEXTOUCH_EFFECT: f64 = -0.2;
/// Chú Vực (Blasphemy): sense held while the curse aura stands.
pub const BLASPHEMY_SENSE: f64 = 30.0;
/// Khai Ngộ (Enlighten): sense held less.
pub const ENLIGHTEN: f64 = -0.3;
/// Khảng Khái (Generosity): intent effect more.
pub const GENEROSITY: f64 = 0.3;
/// Bẫy Trận, Địa Lôi Trận (Trap, Mine): less, and the devices laid at once.
pub const TRAP_MORE: f64 = -0.1;
pub const MINE_MORE: f64 = -0.1;
pub const DEVICES: f64 = 1.0;
/// Pháp Đàn (Spell Totem): less, and the totems standing at once.
pub const TOTEM_MORE: f64 = -0.3;
/// Ngự Linh Lực, Ngự Linh Thể (Minion Damage, Minion Life): more.
pub const MINION_DAMAGE: f64 = 0.35;
pub const MINION_LIFE: f64 = 0.4;
/// Ảo Linh (Summon Phantasm): the chance a blow raises a phantasm.
pub const PHANTASM: f64 = 0.25;
/// Kênh Dẫn Pháp (Cast while Channelling): seconds between casts.
pub const CHANNEL_CAST: f64 = 0.5;
/// Huyết Tế (Lifetap): the share of qi paid in life, and more.
pub const LIFETAP_SHARE: f64 = 1.0;
pub const LIFETAP_MORE: f64 = 0.15;

/// A support's modifiers come from it.
const fn by(id: &'static str) -> Source {
    Source::Support(id)
}

const fn demonize(e: Element) -> Mod {
    Mod::flat(Stat::Convert(e, Element::Yin), 1.0, by("demonize"))
}

/// Hóa Ma: every kind before yin converts to it.
static DEMONIZE: [Mod; 8] = [
    demonize(Element::Force),
    demonize(Element::Metal),
    demonize(Element::Wood),
    demonize(Element::Water),
    demonize(Element::Fire),
    demonize(Element::Earth),
    demonize(Element::Thunder),
    demonize(Element::Light),
];

const fn break_resist(e: Element) -> Mod {
    Mod::flat(Stat::Penetration(e), BREAK_RESIST, by("breakresist"))
}

/// Phá Kháng: penetration of every kind but force (force meets armour, not resistance).
static BREAK_RESIST_MODS: [Mod; N - 1] = [
    break_resist(Element::Metal),
    break_resist(Element::Wood),
    break_resist(Element::Water),
    break_resist(Element::Fire),
    break_resist(Element::Earth),
    break_resist(Element::Thunder),
    break_resist(Element::Light),
    break_resist(Element::Yin),
    break_resist(Element::Soul),
];

const fn brutal(e: Element) -> Mod {
    Mod::more(Stat::DamageOf(e), -1.0, by("brutality"))
}

/// Thuần Lực: force more, every other kind none (a part converted from force has been both, so it deals none).
static BRUTALITY_MODS: [Mod; N] = [
    Mod::more(
        Stat::DamageOf(Element::Force),
        BRUTALITY_MORE,
        by("brutality"),
    ),
    brutal(Element::Metal),
    brutal(Element::Wood),
    brutal(Element::Water),
    brutal(Element::Fire),
    brutal(Element::Earth),
    brutal(Element::Thunder),
    brutal(Element::Light),
    brutal(Element::Yin),
    brutal(Element::Soul),
];

const fn void_less(e: Element) -> Mod {
    Mod::more(Stat::DamageOf(e), VOID_LESS, by("voidmanipulation"))
}

/// Hư Không: yin more, the five phases less.
static VOID_MODS: [Mod; 6] = [
    Mod::more(
        Stat::DamageOf(Element::Yin),
        VOID_MORE,
        by("voidmanipulation"),
    ),
    void_less(Element::Metal),
    void_less(Element::Wood),
    void_less(Element::Water),
    void_less(Element::Fire),
    void_less(Element::Earth),
];

/// Phế Cước's maim on the target.
static MAIM_MODS: [Mod; 2] = [
    Mod::inc(Stat::MoveSpeed, MAIM_SLOW, by("maim")),
    Mod::more(Stat::TakenOf(Element::Force), MAIM_TAKEN, by("maim")),
];

/// Nhiên Thiêu's mark on a burning target.
static COMBUSTION_MODS: [Mod; 1] = [Mod::flat(
    Stat::Resist(Element::Fire),
    COMBUSTION_RESIST,
    by("combustion"),
)];

/// Phụ Hỏa and its kin: `added` of the skill's base, on any skill that deals damage.
const fn added(id: &'static str, name: &'static str, added: &'static [(Element, f64)]) -> Support {
    Support {
        id,
        name,
        any_of: &[Tag::Attack, Tag::Spell],
        cost: 1.2,
        points: 1,
        mods: Mods {
            added,
            ..Mods::NONE
        },
        ..Support::NONE
    }
}

/// What a trap, a mine or a totem cannot carry: what must be in the hand (melee, dashes, channels), what is
/// already a device or a minion, and what does not strike.
const DEVICE_NONE: &[Tag] = &[
    Tag::Melee,
    Tag::Dash,
    Tag::Channel,
    Tag::Trap,
    Tag::Mine,
    Tag::Totem,
    Tag::Minion,
    Tag::Aura,
    Tag::Movement,
    Tag::Guard,
    Tag::Field,
    Tag::Domain,
];
/// What a triggered spell cannot be: channelled, a device, a minion, an intent.
const TRIGGER_NONE: &[Tag] = &[
    Tag::Channel,
    Tag::Trap,
    Tag::Mine,
    Tag::Totem,
    Tag::Minion,
    Tag::Aura,
];

/// Every support.
pub const SUPPORTS: &[Support] = &[
    Support {
        id: "focus",
        name: "Thanh Trần",
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(Stat::Damage, FOCUS_MORE, by("focus"))],
        source: "CM1",
        ..Support::NONE
    },
    Support {
        id: "multishot",
        name: "Liên Châu",
        any_of: &[Tag::Projectile],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, MULTISHOT_MORE, by("multishot")),
            Mod::flat(Stat::Projectiles, MULTISHOT as f64, by("multishot")),
        ],
        source: "CM1",
        ..Support::NONE
    },
    Support {
        id: "afterimage",
        name: "Tàn Ảnh Tốc Thần",
        all_of: &[Tag::Attack],
        any_of: &[Tag::Melee, Tag::Strike, Tag::Slam],
        cost: 1.5,
        points: 3,
        stats: &[
            Mod::more(Stat::Damage, AFTERIMAGE_MORE, by("afterimage")),
            Mod::flat(Stat::Repeats, AFTERIMAGE as f64, by("afterimage")),
        ],
        excludes: &["nineturns"],
        source: "TN8",
        ..Support::NONE
    },
    Support {
        id: "nineturns",
        name: "Cửu Chuyển",
        any_of: &[Tag::Charged],
        cost: 1.3,
        points: 3,
        mods: Mods {
            more_per_turn: TURN_MORE,
            ..Mods::NONE
        },
        behaviours: &[Behaviour::TurnsOnBlock],
        excludes: &["afterimage"],
        source: "TN499",
        ..Support::NONE
    },
    Support {
        id: "ninechanges",
        name: "Cửu Biến",
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, CHANGES_MORE, by("ninechanges")),
            Mod::more(Stat::Windup, CHANGES_WINDUP - 1.0, by("ninechanges")),
        ],
        source: "CM",
        ..Support::NONE
    },
    Support {
        id: "rule",
        name: "Quy Tắc",
        any_of: &[Tag::Sword, Tag::Palm],
        cost: 1.4,
        points: 3,
        mods: Mods {
            rank_step: true,
            ..Mods::NONE
        },
        source: "TN",
        ..Support::NONE
    },
    Support {
        id: "fuse",
        name: "Dung",
        points: 3,
        behaviours: &[
            Behaviour::Fuse,
            Behaviour::Rejection {
                chance: FUSE_REJECTION,
                share: FUSE_BACKLASH,
            },
        ],
        source: "TN1322",
        ..Support::NONE
    },
    Support {
        id: "demonize",
        name: "Hóa Ma",
        cost: 1.2,
        points: 2,
        stats: &DEMONIZE,
        behaviours: &[Behaviour::MinionsBurst {
            chance: DEMON_BURST,
        }],
        conversion: true,
        excludes: &["coldqi"],
        source: "TN3",
        ..Support::NONE
    },
    Support {
        id: "coldqi",
        name: "Âm Hàn",
        any_of: &[Tag::Fire, Tag::Thunder],
        cost: 1.1,
        points: 1,
        mods: Mods {
            rewrite: &[
                (Element::Fire, Element::Water, COLD_SHARE),
                (Element::Thunder, Element::Water, COLD_SHARE),
            ],
            ..Mods::NONE
        },
        conversion: true,
        excludes: &["demonize"],
        source: "TN104",
        ..Support::NONE
    },
    Support {
        id: "bloodforge",
        name: "Huyết Luyện",
        any_of: &[Tag::Treasure],
        points: 2,
        stats: &[Mod::flat(Stat::CostAsLife, 1.0, by("bloodforge"))],
        behaviours: &[Behaviour::TreasureBreak {
            life: BLOOD_FORGE_BREAK,
        }],
        excludes: &["bloodspit"],
        source: "TN1",
        ..Support::NONE
    },
    Support {
        id: "bloodspit",
        name: "Phun Máu",
        points: 1,
        mods: Mods {
            blood: BLOOD_SPIT,
            ..Mods::NONE
        },
        behaviours: &[Behaviour::PierceLayer],
        excludes: &["bloodforge"],
        source: "TN1",
        ..Support::NONE
    },
    Support {
        id: "leap",
        name: "Súc Địa",
        any_of: &[Tag::Near, Tag::Melee, Tag::Strike, Tag::Slam],
        none_of: &[Tag::Charged],
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::LeapToTarget],
        source: "TN",
        ..Support::NONE
    },
    Support {
        id: "spiritshift",
        name: "Dịch Linh",
        any_of: &[Tag::Projectile],
        cost: 1.2,
        points: 2,
        mods: Mods {
            more_per_t: SHIFT_MORE_PER_T,
            travel_max: SHIFT_MORE_MAX,
            ..Mods::NONE
        },
        behaviours: &[Behaviour::Recall],
        source: "TN5",
        ..Support::NONE
    },
    Support {
        id: "delayedthunder",
        name: "Lôi Bạo Trễ",
        any_of: &[Tag::Projectile],
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::MineOnBlock],
        source: "TN740",
        ..Support::NONE
    },
    Support {
        id: "bait",
        name: "Nhử Đòn",
        any_of: &[Tag::Control],
        cost: 1.2,
        points: 1,
        mods: Mods {
            bait_control: BAIT_CONTROL,
            ..Mods::NONE
        },
        behaviours: &[Behaviour::Bait],
        source: "TN226",
        ..Support::NONE
    },
    Support {
        id: "surehit",
        name: "Chắc Trúng",
        cost: 1.2,
        points: 1,
        stats: &[Mod::more(Stat::Windup, SURE_WINDUP - 1.0, by("surehit"))],
        mods: Mods {
            unevadable: true,
            ..Mods::NONE
        },
        source: "CM5",
        ..Support::NONE
    },
    Support {
        id: "chainweaken",
        name: "Liên Hoàn Suy Yếu",
        any_of: &[Tag::Guard],
        cost: 1.3,
        points: 2,
        behaviours: &[Behaviour::CastOnGuardBreak],
        trigger: Some(CAST_ON_SHIELD_BREAK),
        source: "CM6",
        ..Support::NONE
    },
    Support {
        id: "returnseal",
        name: "Phản Ấn",
        any_of: &[Tag::Guard],
        cost: 1.3,
        points: 2,
        behaviours: &[Behaviour::AbsorbAndReturn],
        source: "TN511",
        ..Support::NONE
    },
    Support {
        id: "karmaqi",
        name: "Khí Nhân Quả",
        any_of: &[Tag::Guard, Tag::Formation],
        cost: 1.3,
        points: 2,
        behaviours: &[Behaviour::ReflectBreaker],
        source: "TN7",
        ..Support::NONE
    },
    Support {
        id: "selfdestruct",
        name: "Tự Bạo",
        any_of: &[Tag::Spirit, Tag::Minion, Tag::Treasure],
        cost: 1.1,
        points: 1,
        behaviours: &[Behaviour::SelfDestruct {
            share: SELF_DESTRUCT,
        }],
        source: "TN, CM",
        ..Support::NONE
    },
    Support {
        id: "soulharvest",
        name: "Thu Hồn",
        any_of: &[Tag::Seal, Tag::Banner],
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::GrowOnKill { more: HARVEST_MORE }],
        source: "TN4",
        ..Support::NONE
    },
    Support {
        id: "devourban",
        name: "Thôn Cấm",
        any_of: &[Tag::Banner, Tag::Formation],
        cost: 1.3,
        points: 2,
        behaviours: &[Behaviour::DevourField],
        source: "TN5",
        ..Support::NONE
    },
    Support {
        id: "copy",
        name: "Sao Chép",
        any_of: &[Tag::Transform, Tag::Spirit, Tag::Soul],
        cost: 1.5,
        points: 3,
        behaviours: &[Behaviour::CopyLast { share: COPY_SHARE }],
        source: "TN4, CM5",
        ..Support::NONE
    },
    Support {
        id: "allure",
        name: "Mê Dục",
        any_of: &[Tag::Projectile],
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::NoGuard {
            seconds: ALLURE_SECONDS,
        }],
        source: "CM2",
        ..Support::NONE
    },
    Support {
        id: "heavy",
        name: "Trọng",
        cost: 1.1,
        points: 1,
        stats: &[Mod::more(Stat::Windup, HEAVY_WINDUP - 1.0, by("heavy"))],
        mods: Mods {
            added: &[(Element::Earth, HEAVY_ADDED)],
            ..Mods::NONE
        },
        behaviours: &[Behaviour::Grounds],
        source: "TN1812",
        ..Support::NONE
    },
    Support {
        id: "hollowfire",
        name: "Hư Hỏa",
        any_of: &[Tag::Fire],
        cost: 1.2,
        points: 2,
        mods: Mods {
            fire_tier: Some(HOLLOW_TIER),
            hollow_fire: true,
            ..Mods::NONE
        },
        source: "TN1280",
        ..Support::NONE
    },
    Support {
        id: "years",
        name: "Tuế Nguyệt",
        any_of: &[Tag::Area, Tag::Aura],
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::SlowShots { share: YEARS_SLOW }],
        source: "TN267",
        ..Support::NONE
    },
    Support {
        id: "noretreat",
        name: "Chặn Lui",
        any_of: &[Tag::Seal, Tag::Palm],
        cost: 1.2,
        points: 1,
        behaviours: &[Behaviour::NoEscape { seconds: NO_ESCAPE }],
        source: "TN8",
        ..Support::NONE
    },
    Support {
        id: "spacelock",
        name: "Không Khóa",
        cost: 1.2,
        points: 1,
        behaviours: &[Behaviour::NoBlink { seconds: NO_BLINK }],
        source: "TN3",
        ..Support::NONE
    },
    Support {
        id: "soulburst",
        name: "Hồn Bạo",
        any_of: &[Tag::Spirit, Tag::Minion],
        cost: 1.1,
        points: 1,
        behaviours: &[Behaviour::SacrificeMinion { share: SOUL_BURST }],
        source: "TN913",
        ..Support::NONE
    },
    Support {
        id: "soulfeed",
        name: "Nạp Hồn",
        cost: 1.1,
        points: 1,
        behaviours: &[Behaviour::SoulsFromSoul],
        source: "TN",
        ..Support::NONE
    },
    Support {
        id: "drain",
        name: "Đoạt Tu Vi",
        cost: 1.2,
        points: 2,
        stats: &[Mod::flat(Stat::Leech, DRAIN_LEECH, by("drain"))],
        behaviours: &[Behaviour::DrainQi { share: DRAIN_QI }],
        source: "CM6",
        ..Support::NONE
    },
    Support {
        id: "fireforge",
        name: "Hỏa Luyện",
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(
            Stat::Convert(Element::Force, Element::Fire),
            PHASE_SHARE,
            by("fireforge"),
        )],
        conversion: true,
        ..Support::NONE
    },
    Support {
        id: "metalwind",
        name: "Kim Phong",
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(
            Stat::Convert(Element::Force, Element::Metal),
            PHASE_SHARE,
            by("metalwind"),
        )],
        conversion: true,
        ..Support::NONE
    },
    Support {
        id: "waterintent",
        name: "Thủy Ý",
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(
            Stat::Convert(Element::Force, Element::Water),
            PHASE_SHARE,
            by("waterintent"),
        )],
        conversion: true,
        ..Support::NONE
    },
    Support {
        id: "thunderlead",
        name: "Lôi Dẫn",
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(
            Stat::Convert(Element::Force, Element::Thunder),
            PHASE_SHARE,
            by("thunderlead"),
        )],
        conversion: true,
        ..Support::NONE
    },
    Support {
        id: "woodgrowth",
        name: "Mộc Sinh",
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(
            Stat::Convert(Element::Force, Element::Wood),
            PHASE_SHARE,
            by("woodgrowth"),
        )],
        conversion: true,
        ..Support::NONE
    },
    Support {
        id: "pierce",
        name: "Xuyên Giáp",
        any_of: &[Tag::Attack],
        cost: 1.2,
        points: 2,
        stats: &[Mod::flat(Stat::ArmourPen, PIERCE_ARMOUR, by("pierce"))],
        ..Support::NONE
    },
    Support {
        id: "breakresist",
        name: "Phá Kháng",
        any_of: &[Tag::Spell],
        cost: 1.2,
        points: 2,
        stats: &BREAK_RESIST_MODS,
        ..Support::NONE
    },
    Support {
        id: "malignant",
        name: "Ác Tính",
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::inc(Stat::AilmentEffect, MALIGNANT_AILMENT, by("malignant")),
            Mod::more(Stat::HitDamage, MALIGNANT_MORE, by("malignant")),
        ],
        ..Support::NONE
    },
    Support {
        id: "critical",
        name: "Chí Mạng",
        none_of: &[Tag::Instant],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::flat(Stat::CritChance, CRITICAL, by("critical")),
            Mod::flat(Stat::CritMulti, CRITICAL_MULTI, by("critical")),
        ],
        ..Support::NONE
    },
    Support {
        id: "lingering",
        name: "Thời Gian Kéo Dài",
        cost: 1.1,
        points: 1,
        stats: &[Mod::inc(
            Stat::AilmentDuration,
            LINGER_DURATION,
            by("lingering"),
        )],
        ..Support::NONE
    },
    Support {
        id: "venomheart",
        name: "Độc Tâm",
        any_of: &[Tag::Force, Tag::Fire, Tag::Yin],
        cost: 1.2,
        points: 2,
        stats: &[Mod::flat(Stat::DotMulti, VENOM_DOT_MULTI, by("venomheart"))],
        ..Support::NONE
    },
    Support {
        id: "inferno",
        name: "Hỏa Ngục Thiêu Đốt",
        any_of: &[Tag::Fire],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::more(
                Stat::AilmentDamage(Ailment::Burn),
                INFERNO_BURN,
                by("inferno"),
            ),
            Mod::more(Stat::HitDamage, INFERNO_HIT, by("inferno")),
        ],
        ..Support::NONE
    },
    Support {
        id: "outburst",
        name: "Bạo Phát",
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(Stat::Damage, OUTBURST_MORE, by("outburst"))
            .when(Condition::TargetCursed)
            .per(Per::Curses)
            .max(OUTBURST_MAX)],
        ..Support::NONE
    },
    Support {
        id: "pressadvantage",
        name: "Thừa Thắng",
        all_of: &[Tag::Attack],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(Stat::Damage, PRESS_MORE, by("pressadvantage"))
            .when(Condition::TargetHas(Ailment::Bleed, PRESS_BLEED))],
        ..Support::NONE
    },
    Support {
        id: "bloodthirst",
        name: "Thị Huyết",
        cost: 1.1,
        points: 1,
        stats: &[Mod::more(Stat::Damage, BLOODTHIRST_MORE, by("bloodthirst"))
            .when(Condition::Recently(Happened::Kill, BLOODTHIRST_SECONDS))],
        ..Support::NONE
    },
    // ---- added damage (PoE's Added Fire, Cold, Lightning, Chaos... Damage) ----
    // PoE's Added Fire Damage.
    added("addedfire", "Phụ Hỏa", &[(Element::Fire, ADDED_SHARE)]),
    // PoE's Added Cold Damage, as water (it soaks; nothing chills).
    added("addedwater", "Phụ Thủy", &[(Element::Water, ADDED_SHARE)]),
    // PoE's Added Lightning Damage.
    added(
        "addedthunder",
        "Phụ Lôi",
        &[(Element::Thunder, ADDED_SHARE)],
    ),
    // PoE's Added Chaos Damage.
    added("addedyin", "Phụ Âm", &[(Element::Yin, ADDED_SHARE)]),
    // The arena's metal, as PoE's added damage.
    added("addedmetal", "Phụ Kim", &[(Element::Metal, ADDED_SHARE)]),
    // The arena's wood, as PoE's added damage.
    added("addedwood", "Phụ Mộc", &[(Element::Wood, ADDED_SHARE)]),
    // The arena's earth, as PoE's added damage.
    added("addedearth", "Phụ Thổ", &[(Element::Earth, ADDED_SHARE)]),
    // The arena's light, as PoE's added damage.
    added("addedlight", "Phụ Quang", &[(Element::Light, ADDED_SHARE)]),
    // The arena's soul, as PoE's added damage.
    added("addedsoul", "Phụ Hồn", &[(Element::Soul, ADDED_SHARE)]),
    // ---- focus and destruction ----
    // PoE's Elemental Focus: more, and the blow lays no ailment.
    Support {
        id: "elementalfocus",
        name: "Ngũ Hành Ngưng",
        any_of: &[Tag::Metal, Tag::Wood, Tag::Water, Tag::Fire, Tag::Earth],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, ELEMENTAL_FOCUS_MORE, by("elementalfocus")),
            Mod::more(Stat::AilmentEffect, -1.0, by("elementalfocus")),
        ],
        ..Support::NONE
    },
    // PoE's Controlled Destruction: spell more, no crit.
    Support {
        id: "controlleddestruction",
        name: "Khống Diệt",
        all_of: &[Tag::Spell],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, CONTROLLED_MORE, by("controlleddestruction")),
            Mod::inc(
                Stat::CritChance,
                CONTROLLED_CRIT,
                by("controlleddestruction"),
            ),
        ],
        ..Support::NONE
    },
    // PoE's Concentrated Effect: area less, damage more.
    Support {
        id: "concentrated",
        name: "Tụ Lực",
        any_of: &[Tag::Area, Tag::Nova, Tag::Cone],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(Stat::Area, CONCENTRATED_AREA, by("concentrated")),
            Mod::more(Stat::Damage, CONCENTRATED_MORE, by("concentrated")),
        ],
        excludes: &["widearea"],
        ..Support::NONE
    },
    // PoE's Increased Area of Effect.
    Support {
        id: "widearea",
        name: "Khoách Vực",
        any_of: &[Tag::Area, Tag::Nova, Tag::Cone, Tag::Aura, Tag::Field],
        cost: 1.2,
        points: 1,
        stats: &[Mod::inc(Stat::Area, WIDE_AREA, by("widearea"))],
        excludes: &["concentrated"],
        ..Support::NONE
    },
    // PoE's Brutality: force more, and nothing but force.
    Support {
        id: "brutality",
        name: "Thuần Lực",
        all_of: &[Tag::Force],
        cost: 1.3,
        points: 2,
        stats: &BRUTALITY_MODS,
        ..Support::NONE
    },
    // ---- projectiles ----
    // PoE's Pierce.
    Support {
        id: "piercing",
        name: "Xuyên Thấu",
        any_of: &[Tag::Projectile],
        cost: 1.15,
        points: 1,
        stats: &[
            Mod::flat(Stat::Pierce, PIERCE_COUNT, by("piercing")),
            Mod::more(Stat::Damage, PIERCE_MORE, by("piercing")),
        ],
        ..Support::NONE
    },
    // PoE's Fork.
    Support {
        id: "fork",
        name: "Phân Nhánh",
        any_of: &[Tag::Projectile],
        cost: 1.2,
        points: 1,
        stats: &[
            Mod::flat(Stat::Fork, FORK_COUNT, by("fork")),
            Mod::more(Stat::Damage, FORK_MORE, by("fork")),
        ],
        ..Support::NONE
    },
    // PoE's Chain.
    Support {
        id: "chain",
        name: "Liên Kích",
        any_of: &[Tag::Projectile, Tag::Chain],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::flat(Stat::Chain, CHAIN_COUNT, by("chain")),
            Mod::more(Stat::Damage, CHAIN_MORE, by("chain")),
        ],
        ..Support::NONE
    },
    // PoE's Faster Projectiles.
    Support {
        id: "fasterprojectiles",
        name: "Tật Tiễn",
        any_of: &[Tag::Projectile],
        cost: 1.15,
        points: 1,
        stats: &[
            Mod::inc(Stat::ProjectileSpeed, FASTER_SHOTS, by("fasterprojectiles")),
            Mod::more(Stat::Damage, FASTER_SHOTS_MORE, by("fasterprojectiles")),
        ],
        excludes: &["slowerprojectiles"],
        ..Support::NONE
    },
    // PoE's Slower Projectiles.
    Support {
        id: "slowerprojectiles",
        name: "Hoãn Tiễn",
        any_of: &[Tag::Projectile],
        cost: 1.15,
        points: 1,
        stats: &[
            Mod::inc(Stat::ProjectileSpeed, SLOWER_SHOTS, by("slowerprojectiles")),
            Mod::more(Stat::Damage, SLOWER_SHOTS_MORE, by("slowerprojectiles")),
        ],
        excludes: &["fasterprojectiles"],
        ..Support::NONE
    },
    // PoE's Lesser Multiple Projectiles (it does not stack with Liên Châu).
    Support {
        id: "lessermultiple",
        name: "Tiểu Liên Châu",
        any_of: &[Tag::Projectile],
        cost: 1.2,
        points: 1,
        stats: &[
            Mod::flat(Stat::Projectiles, LESSER_MULTISHOT, by("lessermultiple")),
            Mod::more(Stat::Damage, LESSER_MULTISHOT_MORE, by("lessermultiple")),
        ],
        excludes: &["multishot"],
        ..Support::NONE
    },
    // PoE's Point Blank: more near, less far.
    Support {
        id: "pointblank",
        name: "Cận Xạ",
        any_of: &[Tag::Projectile],
        cost: 1.1,
        points: 1,
        stats: &[
            Mod::more(Stat::Damage, POINT_BLANK_MORE, by("pointblank"))
                .when(Condition::Near(POINT_BLANK_NEAR)),
            Mod::more(Stat::Damage, POINT_BLANK_LESS, by("pointblank"))
                .when(Condition::Far(POINT_BLANK_FAR)),
        ],
        excludes: &["farshot"],
        ..Support::NONE
    },
    // PoE's Far Shot: more with each T of distance.
    Support {
        id: "farshot",
        name: "Viễn Xạ",
        any_of: &[Tag::Projectile],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(Stat::Damage, FAR_SHOT_PER_T, by("farshot"))
            .per(Per::Distance(1.0))
            .max(FAR_SHOT_MAX)],
        excludes: &["pointblank"],
        ..Support::NONE
    },
    // ---- melee ----
    // PoE's Melee Physical Damage.
    Support {
        id: "meleephysical",
        name: "Cương Lực",
        all_of: &[Tag::Attack],
        any_of: &[Tag::Melee, Tag::Strike, Tag::Slam],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(
                Stat::DamageOf(Element::Force),
                MELEE_FORCE_MORE,
                by("meleephysical"),
            ),
            Mod::more(Stat::Speed, MELEE_FORCE_SPEED, by("meleephysical")),
        ],
        ..Support::NONE
    },
    // PoE's Close Combat: more close up.
    Support {
        id: "closecombat",
        name: "Áp Sát",
        all_of: &[Tag::Attack],
        any_of: &[Tag::Melee, Tag::Strike, Tag::Slam, Tag::Dash],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(Stat::Damage, CLOSE_MORE, by("closecombat"))
            .when(Condition::Near(CLOSE_NEAR))],
        ..Support::NONE
    },
    // PoE's Ruthless: every third blow far more, and it shakes harder.
    Support {
        id: "ruthless",
        name: "Tàn Nhẫn",
        all_of: &[Tag::Attack],
        any_of: &[Tag::Melee, Tag::Strike, Tag::Slam],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, RUTHLESS_MORE, by("ruthless"))
                .when(Condition::EveryNth(RUTHLESS_EVERY)),
            Mod::more(
                Stat::AilmentEffectOf(Ailment::Daze),
                RUTHLESS_DAZE,
                by("ruthless"),
            )
            .when(Condition::EveryNth(RUTHLESS_EVERY)),
        ],
        ..Support::NONE
    },
    // PoE's Fist of War: every fourth slam far more, and wider.
    Support {
        id: "fistofwar",
        name: "Chiến Quyền",
        all_of: &[Tag::Attack],
        any_of: &[Tag::Slam, Tag::Fist],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, FIST_MORE, by("fistofwar"))
                .when(Condition::EveryNth(FIST_EVERY)),
            Mod::inc(Stat::Area, FIST_AREA, by("fistofwar")).when(Condition::EveryNth(FIST_EVERY)),
        ],
        ..Support::NONE
    },
    // PoE's Impale: a share of the force dealt strikes again later (the fight).
    Support {
        id: "impale",
        name: "Lưu Lực",
        all_of: &[Tag::Attack, Tag::Force],
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::StoredForce {
            share: STORED_FORCE,
            delay: STORED_DELAY,
        }],
        ..Support::NONE
    },
    // PoE's Culling Strike.
    Support {
        id: "culling",
        name: "Trảm Tận",
        any_of: &[Tag::Attack, Tag::Spell],
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(Stat::Execute, CULLING, by("culling"))],
        ..Support::NONE
    },
    // PoE's Maim: force more; the target runs slower and takes more force.
    Support {
        id: "maim",
        name: "Phế Cước",
        all_of: &[Tag::Attack],
        cost: 1.1,
        points: 1,
        stats: &[Mod::more(
            Stat::DamageOf(Element::Force),
            MAIM_MORE,
            by("maim"),
        )],
        mods: Mods {
            debuff: Some(Debuff {
                source: by("maim"),
                on: None,
                mods: &MAIM_MODS,
                seconds: MAIM_SECONDS,
            }),
            ..Mods::NONE
        },
        ..Support::NONE
    },
    // PoE's Faster Attacks.
    Support {
        id: "fasterattacks",
        name: "Tốc Kích",
        all_of: &[Tag::Attack],
        cost: 1.15,
        points: 1,
        stats: &[Mod::inc(Stat::Speed, FASTER_ATTACKS, by("fasterattacks"))],
        ..Support::NONE
    },
    // A PoE Multistrike: two more strikes, each less, quicker.
    Support {
        id: "triplestrike",
        name: "Tam Liên Ảnh",
        all_of: &[Tag::Attack],
        any_of: &[Tag::Melee, Tag::Strike, Tag::Slam],
        cost: 1.6,
        points: 3,
        stats: &[
            Mod::flat(Stat::Repeats, TRIPLE_STRIKES, by("triplestrike")),
            Mod::more(Stat::Damage, TRIPLE_MORE, by("triplestrike")),
            Mod::more(Stat::Speed, TRIPLE_SPEED, by("triplestrike")),
        ],
        excludes: &["afterimage", "nineturns"],
        ..Support::NONE
    },
    // ---- spells ----
    // PoE's Faster Casting.
    Support {
        id: "fastercasting",
        name: "Tốc Chú",
        all_of: &[Tag::Spell],
        cost: 1.15,
        points: 1,
        stats: &[Mod::inc(Stat::Speed, FASTER_CASTING, by("fastercasting"))],
        ..Support::NONE
    },
    // PoE's Spell Echo: cast twice, quicker, each less.
    Support {
        id: "spellecho",
        name: "Hồi Âm",
        all_of: &[Tag::Spell],
        none_of: &[Tag::Channel, Tag::Instant, Tag::Charged],
        cost: 1.4,
        points: 3,
        stats: &[
            Mod::flat(Stat::Repeats, ECHO_REPEATS, by("spellecho")),
            Mod::more(Stat::Speed, ECHO_SPEED, by("spellecho")),
            Mod::more(Stat::Damage, ECHO_MORE, by("spellecho")),
        ],
        excludes: &["unleash"],
        ..Support::NONE
    },
    // PoE's Arcane Surge: after spending qi, spell more and quicker.
    Support {
        id: "arcanesurge",
        name: "Linh Triều",
        all_of: &[Tag::Spell],
        cost: 1.1,
        points: 1,
        stats: &[
            Mod::more(Stat::Damage, SURGE_MORE, by("arcanesurge"))
                .when(Condition::QiSpent(SURGE_QI)),
            Mod::inc(Stat::Speed, SURGE_SPEED, by("arcanesurge"))
                .when(Condition::QiSpent(SURGE_QI)),
        ],
        ..Support::NONE
    },
    // PoE's Efficacy: more, damage over time more, shorter effects.
    Support {
        id: "efficacy",
        name: "Hiệu Lực",
        all_of: &[Tag::Spell],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, EFFICACY_MORE, by("efficacy")),
            Mod::more(Stat::DotDamage, EFFICACY_DOT, by("efficacy")),
            Mod::more(Stat::SkillDuration, EFFICACY_DURATION, by("efficacy")),
        ],
        ..Support::NONE
    },
    // PoE's Inspiration: cheaper, and crit chance per mind star (its inspiration charges).
    Support {
        id: "inspiration",
        name: "Linh Cảm",
        all_of: &[Tag::Spell],
        none_of: &[Tag::Instant],
        cost: INSPIRATION_COST,
        points: 1,
        stats: &[
            Mod::inc(Stat::CritChance, INSPIRATION_CRIT, by("inspiration"))
                .per(Per::Stars(Star::Mind))
                .max(INSPIRATION_MAX),
        ],
        ..Support::NONE
    },
    // ---- damage over time and ailments ----
    // PoE's Burning Damage.
    Support {
        id: "burningdamage",
        name: "Liệt Diễm",
        any_of: &[Tag::Fire],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(
            Stat::AilmentDamage(Ailment::Burn),
            BURNING_MORE,
            by("burningdamage"),
        )],
        ..Support::NONE
    },
    // PoE's Immolate: fire added against a burning target.
    Support {
        id: "immolate",
        name: "Hỏa Thiêu Thân",
        all_of: &[Tag::Attack],
        cost: 1.2,
        points: 2,
        mods: Mods {
            added: &[(Element::Fire, IMMOLATE_ADDED)],
            added_when: Condition::TargetHas(Ailment::Burn, 1.0),
            ..Mods::NONE
        },
        ..Support::NONE
    },
    // PoE's Swift Affliction: damage over time more, everything shorter.
    Support {
        id: "swiftaffliction",
        name: "Tốc Độc",
        any_of: &[Tag::Force, Tag::Fire, Tag::Yin],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::more(Stat::DotDamage, SWIFT_DOT, by("swiftaffliction")),
            Mod::more(Stat::AilmentDuration, SWIFT_DURATION, by("swiftaffliction")),
            Mod::more(Stat::SkillDuration, SWIFT_DURATION, by("swiftaffliction")),
        ],
        ..Support::NONE
    },
    // PoE's Deadly Ailments: damage over time more, hits less.
    Support {
        id: "deadlyailments",
        name: "Độc Ngấm",
        any_of: &[Tag::Force, Tag::Fire],
        cost: 1.3,
        points: 2,
        stats: &[
            Mod::more(Stat::DotDamage, DEADLY_DOT, by("deadlyailments")),
            Mod::more(Stat::HitDamage, DEADLY_HIT, by("deadlyailments")),
        ],
        ..Support::NONE
    },
    // PoE's Cruelty: the bigger the hit, the harder its burns and bleeds.
    Support {
        id: "cruelty",
        name: "Tàn Khốc",
        any_of: &[Tag::Force, Tag::Fire],
        cost: 1.2,
        points: 2,
        mods: Mods {
            cruelty: CRUELTY_MORE,
            ..Mods::NONE
        },
        ..Support::NONE
    },
    // PoE's Bloodlust: more for each bleed on the target.
    Support {
        id: "bloodlust",
        name: "Khát Huyết",
        all_of: &[Tag::Attack],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(Stat::Damage, BLOODLUST_PER, by("bloodlust"))
            .per(Per::Stacks(Ailment::Bleed))
            .max(BLOODLUST_MAX)],
        ..Support::NONE
    },
    // PoE's Withering Touch: each blow lays wither.
    Support {
        id: "witheringtouch",
        name: "Tịch Thủ",
        all_of: &[Tag::Attack],
        cost: 1.2,
        points: 2,
        stats: &[Mod::flat(
            Stat::Inflict(Ailment::Wither),
            WITHER_ON_HIT,
            by("witheringtouch"),
        )],
        ..Support::NONE
    },
    // PoE's Void Manipulation: yin more, the five phases less.
    Support {
        id: "voidmanipulation",
        name: "Hư Không",
        any_of: &[Tag::Yin],
        cost: 1.2,
        points: 2,
        stats: &VOID_MODS,
        ..Support::NONE
    },
    // PoE's Combustion: fire more; its burns lower the target's fire resistance.
    Support {
        id: "combustion",
        name: "Nhiên Thiêu",
        any_of: &[Tag::Fire],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(
            Stat::DamageOf(Element::Fire),
            COMBUSTION_MORE,
            by("combustion"),
        )],
        mods: Mods {
            debuff: Some(Debuff {
                source: by("combustion"),
                on: Some(Ailment::Burn),
                mods: &COMBUSTION_MODS,
                seconds: BURN_TIME,
            }),
            ..Mods::NONE
        },
        ..Support::NONE
    },
    // PoE's Intensify: each blow landed lately makes the area tighter and harder.
    Support {
        id: "intensify",
        name: "Tăng Ý",
        all_of: &[Tag::Spell],
        any_of: &[Tag::Area, Tag::Nova],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, INTENSIFY_MORE, by("intensify"))
                .per(Per::Hits)
                .max(INTENSIFY_MORE * INTENSIFY_MAX),
            Mod::more(Stat::Area, INTENSIFY_AREA, by("intensify"))
                .per(Per::Hits)
                .max(INTENSIFY_AREA * INTENSIFY_MAX),
        ],
        ..Support::NONE
    },
    // PoE's Unleash: seals gather, a cast spends them as repeats, each less.
    Support {
        id: "unleash",
        name: "Phong Ấn",
        all_of: &[Tag::Spell],
        none_of: &[Tag::Channel, Tag::Instant],
        cost: 1.3,
        points: 2,
        stats: &[Mod::more(Stat::Damage, UNLEASH_MORE, by("unleash"))],
        behaviours: &[Behaviour::Seals {
            max: UNLEASH_SEALS,
            seconds: UNLEASH_SECONDS,
        }],
        excludes: &["spellecho"],
        ..Support::NONE
    },
    // PoE's Overcharge: jolt (shock) harder.
    Support {
        id: "overcharge",
        name: "Quá Tải",
        any_of: &[Tag::Thunder],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::more(
                Stat::AilmentEffectOf(Ailment::Jolt),
                OVERCHARGE_JOLT,
                by("overcharge"),
            ),
            Mod::more(
                Stat::DamageOf(Element::Thunder),
                OVERCHARGE_MORE,
                by("overcharge"),
            ),
        ],
        ..Support::NONE
    },
    // PoE's Hypothermia as water: more on a soaked target, soak laid harder. It soaks; nothing chills.
    Support {
        id: "soaked",
        name: "Thủy Phệ",
        any_of: &[Tag::Water],
        cost: 1.2,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, SOAKED_MORE, by("soaked"))
                .when(Condition::TargetHas(Ailment::Soak, 1.0)),
            Mod::inc(
                Stat::AilmentEffectOf(Ailment::Soak),
                SOAKED_EFFECT,
                by("soaked"),
            ),
        ],
        ..Support::NONE
    },
    // ---- crits and stars ----
    // PoE's Increased Critical Damage.
    Support {
        id: "critdamage",
        name: "Bạo Thương",
        none_of: &[Tag::Instant],
        cost: 1.2,
        points: 2,
        stats: &[Mod::flat(Stat::CritMulti, CRIT_DAMAGE, by("critdamage"))],
        ..Support::NONE
    },
    // PoE's Power Charge on Critical: a mind star on a crit.
    Support {
        id: "powercharge",
        name: "Ma Tinh Dẫn",
        none_of: &[Tag::Instant],
        cost: 1.1,
        points: 1,
        stats: &[
            Mod::flat(
                Stat::StarOn(Happened::Crit, Star::Mind),
                POWER_ON_CRIT,
                by("powercharge"),
            ),
            Mod::inc(Stat::CritChance, POWER_CRIT, by("powercharge")),
        ],
        ..Support::NONE
    },
    // PoE's Endurance Charge on Melee Stun: a body star when the blow lays a hard control.
    Support {
        id: "endurancecharge",
        name: "Thần Tinh Dẫn",
        all_of: &[Tag::Attack],
        any_of: &[Tag::Melee, Tag::Strike, Tag::Slam],
        cost: 1.1,
        points: 1,
        stats: &[
            Mod::flat(
                Stat::StarOn(Happened::LaidControl, Star::Body),
                ENDURANCE_ON_CONTROL,
                by("endurancecharge"),
            ),
            Mod::more(
                Stat::AilmentEffectOf(Ailment::Daze),
                ENDURANCE_DAZE,
                by("endurancecharge"),
            ),
        ],
        ..Support::NONE
    },
    // PoE's Frenzy Charge on Hit vs a rare or unique: a speed star on a blow against a foe with a soul.
    Support {
        id: "frenzycharge",
        name: "Yêu Tinh Dẫn",
        all_of: &[Tag::Attack],
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(
            Stat::StarOn(Happened::Hit, Star::Speed),
            FRENZY_ON_HIT,
            by("frenzycharge"),
        )
        .when(Condition::TargetHasSoul)],
        ..Support::NONE
    },
    // PoE's Trinity: more for each kind of damage dealt lately.
    Support {
        id: "trinity",
        name: "Tam Tài",
        any_of: &[Tag::Attack, Tag::Spell],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(Stat::Damage, TRINITY_PER, by("trinity"))
            .per(Per::KindsDealt)
            .max(TRINITY_MAX)],
        ..Support::NONE
    },
    // ---- recovery ----
    // PoE's Life Gain on Hit.
    Support {
        id: "lifeonhit",
        name: "Hồi Huyết",
        any_of: &[Tag::Attack, Tag::Spell],
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(Stat::LifeOnHit, LIFE_ON_HIT, by("lifeonhit"))],
        ..Support::NONE
    },
    // PoE's Mana Leech, as qi.
    Support {
        id: "qileech",
        name: "Hấp Khí",
        any_of: &[Tag::Attack, Tag::Spell],
        cost: 1.1,
        points: 1,
        stats: &[Mod::flat(Stat::QiLeech, QI_LEECH, by("qileech"))],
        ..Support::NONE
    },
    // PoE's Life Gain on Kill (the fight heals).
    Support {
        id: "lifeonkill",
        name: "Sát Sinh Bổ Huyết",
        any_of: &[Tag::Attack, Tag::Spell],
        cost: 1.1,
        points: 1,
        behaviours: &[Behaviour::LifeOnKill {
            share: LIFE_ON_KILL,
        }],
        ..Support::NONE
    },
    // ---- curses and intents ----
    // PoE's Hextouch: the curse is laid by blows, a little weaker.
    Support {
        id: "hextouch",
        name: "Chú Ấn Kích",
        all_of: &[Tag::Curse],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(
            Stat::CurseEffect,
            HEXTOUCH_EFFECT,
            by("hextouch"),
        )],
        behaviours: &[Behaviour::CurseOnHit],
        excludes: &["blasphemy"],
        ..Support::NONE
    },
    // PoE's Blasphemy: the curse as an aura around its bearer, holding sense.
    Support {
        id: "blasphemy",
        name: "Chú Vực",
        all_of: &[Tag::Curse],
        points: 2,
        stats: &[Mod::flat(Stat::Reserved, BLASPHEMY_SENSE, by("blasphemy"))],
        behaviours: &[Behaviour::CurseAura],
        excludes: &["hextouch"],
        ..Support::NONE
    },
    // PoE's Enlighten: less sense held.
    Support {
        id: "enlighten",
        name: "Khai Ngộ",
        any_of: &[Tag::Aura, Tag::Curse],
        points: 2,
        stats: &[Mod::more(Stat::Reservation, ENLIGHTEN, by("enlighten"))],
        ..Support::NONE
    },
    // PoE's Generosity: a stronger intent that spares its holder.
    Support {
        id: "generosity",
        name: "Khảng Khái",
        any_of: &[Tag::Aura],
        points: 2,
        stats: &[Mod::more(Stat::AuraEffect, GENEROSITY, by("generosity"))],
        behaviours: &[Behaviour::NotOnSelf],
        ..Support::NONE
    },
    // ---- devices and minions ----
    // PoE's Trap.
    Support {
        id: "trap",
        name: "Bẫy Trận",
        any_of: &[Tag::Spell, Tag::Attack],
        none_of: DEVICE_NONE,
        cost: 1.1,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, TRAP_MORE, by("trap")),
            Mod::flat(Stat::TrapCount, DEVICES, by("trap")),
        ],
        mods: Mods {
            becomes: &[Tag::Trap],
            ..Mods::NONE
        },
        behaviours: &[Behaviour::Device { kind: Tag::Trap }],
        excludes: &[
            "mine",
            "totem",
            "critcast",
            "hurtcast",
            "lowlifecast",
            "channelcast",
        ],
        ..Support::NONE
    },
    // PoE's Blastchain Mine (a mine).
    Support {
        id: "mine",
        name: "Địa Lôi Trận",
        any_of: &[Tag::Spell, Tag::Attack],
        none_of: DEVICE_NONE,
        cost: 1.1,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, MINE_MORE, by("mine")),
            Mod::flat(Stat::MineCount, DEVICES, by("mine")),
        ],
        mods: Mods {
            becomes: &[Tag::Mine],
            ..Mods::NONE
        },
        behaviours: &[Behaviour::Device { kind: Tag::Mine }],
        excludes: &[
            "trap",
            "totem",
            "critcast",
            "hurtcast",
            "lowlifecast",
            "channelcast",
        ],
        ..Support::NONE
    },
    // PoE's Spell Totem.
    Support {
        id: "totem",
        name: "Pháp Đàn",
        any_of: &[Tag::Spell],
        none_of: DEVICE_NONE,
        cost: 1.1,
        points: 2,
        stats: &[
            Mod::more(Stat::Damage, TOTEM_MORE, by("totem")),
            Mod::flat(Stat::TotemCount, DEVICES, by("totem")),
        ],
        mods: Mods {
            becomes: &[Tag::Totem],
            ..Mods::NONE
        },
        behaviours: &[Behaviour::Device { kind: Tag::Totem }],
        excludes: &[
            "trap",
            "mine",
            "critcast",
            "hurtcast",
            "lowlifecast",
            "channelcast",
            "leap",
        ],
        ..Support::NONE
    },
    // PoE's Minion Damage.
    Support {
        id: "miniondamage",
        name: "Ngự Linh Lực",
        any_of: &[Tag::Minion],
        cost: 1.2,
        points: 2,
        stats: &[Mod::more(
            Stat::MinionDamage,
            MINION_DAMAGE,
            by("miniondamage"),
        )],
        ..Support::NONE
    },
    // PoE's Minion Life.
    Support {
        id: "minionlife",
        name: "Ngự Linh Thể",
        any_of: &[Tag::Minion],
        cost: 1.2,
        points: 1,
        stats: &[Mod::more(Stat::MinionLife, MINION_LIFE, by("minionlife"))],
        ..Support::NONE
    },
    // PoE's Summon Phantasm.
    Support {
        id: "phantasm",
        name: "Ảo Linh",
        all_of: &[Tag::Spell],
        none_of: &[Tag::Minion, Tag::Channel],
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::SummonPhantasm { chance: PHANTASM }],
        ..Support::NONE
    },
    // ---- triggers (`event`) ----
    // PoE's Cast On Critical Strike: an attack's crit casts the spell.
    Support {
        id: "critcast",
        name: "Bạo Kích Dẫn Pháp",
        all_of: &[Tag::Spell],
        none_of: TRIGGER_NONE,
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::Triggered],
        trigger: Some(CAST_ON_CRIT),
        excludes: &[
            "hurtcast",
            "lowlifecast",
            "channelcast",
            "trap",
            "mine",
            "totem",
        ],
        ..Support::NONE
    },
    // PoE's Cast when Damage Taken.
    Support {
        id: "hurtcast",
        name: "Thụ Thương Phản Chú",
        all_of: &[Tag::Spell],
        none_of: TRIGGER_NONE,
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::Triggered],
        trigger: Some(CAST_WHEN_HURT),
        excludes: &[
            "critcast",
            "lowlifecast",
            "channelcast",
            "trap",
            "mine",
            "totem",
        ],
        ..Support::NONE
    },
    // PoE's Cast on Death, at death's door: life falling under the low line casts the spell.
    Support {
        id: "lowlifecast",
        name: "Lâm Nguy Phản Kích",
        all_of: &[Tag::Spell],
        none_of: TRIGGER_NONE,
        cost: 1.2,
        points: 2,
        behaviours: &[Behaviour::Triggered],
        trigger: Some(CAST_ON_LOW_LIFE),
        excludes: &[
            "critcast",
            "hurtcast",
            "channelcast",
            "trap",
            "mine",
            "totem",
        ],
        ..Support::NONE
    },
    // PoE's Cast while Channelling (the fight casts it on a timer).
    Support {
        id: "channelcast",
        name: "Kênh Dẫn Pháp",
        all_of: &[Tag::Spell],
        none_of: TRIGGER_NONE,
        cost: 1.2,
        points: 2,
        behaviours: &[
            Behaviour::Triggered,
            Behaviour::CastWhileChannelling {
                every: CHANNEL_CAST,
            },
        ],
        excludes: &[
            "critcast",
            "hurtcast",
            "lowlifecast",
            "trap",
            "mine",
            "totem",
        ],
        ..Support::NONE
    },
    // ---- blood ----
    // PoE's Lifetap: the qi is paid in life, for more.
    Support {
        id: "lifetap",
        name: "Huyết Tế",
        any_of: &[Tag::Attack, Tag::Spell],
        points: 1,
        stats: &[
            Mod::flat(Stat::CostAsLife, LIFETAP_SHARE, by("lifetap")),
            Mod::more(Stat::Damage, LIFETAP_MORE, by("lifetap")),
        ],
        excludes: &["bloodforge", "bloodspit"],
        ..Support::NONE
    },
];

/// The support named `id`.
pub fn support(id: &str) -> Option<&'static Support> {
    SUPPORTS.iter().find(|s| s.id == id)
}

/// Dung (rules, 2.2.5): two skills of one loadout made one. The fused skill takes the nature, form, reach, delivery,
/// windup and recover of `second`; the kinds, roles, daos, rank, force ailment and effects of `first`; damage
/// [`FUSE_SHARE`] of both bases (each at its own base: an attack's from might, a spell's from its level); both costs;
/// the longer cooldown.
pub fn fuse(first: &Skill, second: &Skill) -> Skill {
    let shape = [Group::Nature, Group::Form, Group::Reach, Group::Delivery];
    let tags = second
        .tags
        .iter()
        .filter(|t| shape.contains(&t.group()))
        .chain(
            first
                .tags
                .iter()
                .filter(|t| matches!(t.group(), Group::Role | Group::Dao)),
        )
        .fold(Tags::NONE, Tags::with);
    let mut costs = first.costs.clone();
    for &(r, v) in &second.costs {
        match costs.iter_mut().find(|c| c.0 == r) {
            Some(c) => c.1 += v,
            None => costs.push((r, v)),
        }
    }
    Skill {
        id: first.id,
        tags,
        split: first.split.clone(),
        base: FUSE_SHARE * (first.base + second.base),
        effectiveness: second.effectiveness,
        level: first.level,
        rank: first.rank,
        restriction: first.restriction,
        costs,
        cooldown: first.cooldown.max(second.cooldown),
        windup: second.windup,
        recover: second.recover,
        force: first.force,
        effects: first.effects,
        basic: false,
        fused: Some(second.id),
    }
}

/// Dung's rejection: a cast whose `roll` (from the fight's dice, in [0, 1)) falls under [`FUSE_REJECTION`] blows
/// [`FUSE_BACKLASH`] of the skill's `damage` back on the caster. Returns that damage, or 0.
pub fn rejection(roll: f64, damage: f64) -> f64 {
    if roll < FUSE_REJECTION {
        FUSE_BACKLASH * damage.max(0.0)
    } else {
        0.0
    }
}

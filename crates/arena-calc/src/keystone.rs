//! Đạo tâm: the keystones a loadout takes, 0 to 2 (design, 2 and 9; research, 7). Each one has a gain and a cost.
//! What it does to a number is a modifier: lasting ones live on the fighter's sheet ([`Keystone::lasting`], gathered
//! by every blow it strikes or takes), situational ones join a blow at the cast ([`Keystone::strike_mods`]). What
//! is not a number stays a rule: the start of a fight ([`Keystone::apply`]), rewrites and resistance passed on a
//! blow it strikes ([`Keystone::on_strike`]) or takes ([`Keystone::on_taken`]), a spared death
//! ([`Keystone::on_death`]), and the supports and skills it forbids (read by `loadout::check`). No keystone belongs
//! to a class; none freezes.

use crate::build::{Situation, Skill, rewrite};
use crate::cultivator::Cultivator;
use crate::element::{Element, N};
use crate::grade::{Grade, Rank};
use crate::hit::Hit;
use crate::stat::{Condition, Mod, ModKind, Source, Stat};
use crate::support::Support;
use crate::tag::{Tag, Tags};

/// A keystone. Cực Cảnh carries the kind it was taken for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Keystone {
    /// Cực Cảnh: one kind more; no conversion, no fusing.
    Extreme(Element),
    /// Âm Hàn Linh Lực: every blow water at step 1; thunder taken half water; no support of another kind.
    ColdQi,
    /// Cổ Thần Quyết: no qi cap (and drinks qi spilled on the field); no recovery skills.
    AncientGod,
    /// Sát Lục Tiên Quyết: each blow taken lays a brand that is armour; too many brands, the heart demon; rank-3
    /// blows pierce several layers at once.
    SlaughterImmortal,
    /// Tiên Trì: every spell more; no auras.
    ImmortalPool,
    /// Tóc Đen: every blow ignores resistance and armour (slaughter), immune to dispel; stats down, no auras or
    /// foreign buffs.
    BlackHair,
    /// Thái Cổ Lôi Long: thunder taken becomes qi, own thunder ignores thunder resistance, a share of the foe's thunder
    /// shots turn back; one kind besides thunder in the whole loadout.
    ThunderDragon,
    /// Hệ Đại Thành: immune to the root's kind, a share of the foe's blows of it subdued; what overcomes the root
    /// strikes harder.
    Mastery,
    /// Tam Mệnh: two spare lives, each used costs damage; never refilled.
    ThreeLives,
    /// Ngọn Đèn Hồn: soul lamps, each a spare life or a burst when snuffed; out of lamps loses the match.
    SoulLamps,
    /// Hai Mươi Năm Khổ Luyện: rank-1 skills more; no skill of rank 3 or more.
    TwentyYears,
    /// Tâm Xám: immune to emotions; no emotion buffs.
    GreyHeart,
    /// Người Xem Cây: standing still, immune to slows, rewinds and age, and time skills on it cost double lifespan;
    /// moving, time blows strike harder.
    TreeWatcher,
    /// Nguyền Bẩm Sinh: never old, time skills on it curse their caster back; no time skills of its own.
    BornCursed,
    /// Đạo Dung Thân: each skill used charges a vortex into the next basic; basics slower, each guard layer it meets
    /// eats a vortex.
    DaoBody,
    /// Phá Rồi Lập: clones broken quickly raise the realm for a while; a clone lost is lost for the round.
    BreakRebuild,
    /// Vu Chú: two curses on the foe; each curse fills a bar that curses its owner when full.
    Witch,
    /// Niềm Tin Tất Thắng: fist chains more by the season's win streak; one loss clears it.
    Faith,
    /// Bất Khuất (the old `unyielding`): once a round a killing blow leaves 1 life; less full life.
    Unyielding,
    /// Sát Ý (the old `fury`): each blow landed more, for the round; half the energy shield.
    Fury,
    /// Tâm Pháp (the old `attuned`): every two qi-paid spells give a rage point; attacks give none.
    Attuned,
}

/// Cực Cảnh: the kind's more, and the life share under which its instants execute an equal or lower rank.
pub const EXTREME_MORE: f64 = 0.6;
pub const EXTREME_EXECUTE: f64 = 0.15;
/// Âm Hàn Linh Lực: the share of thunder taken that turns water.
pub const COLD_TAKEN: f64 = 0.5;
/// Cổ Thần Quyết: the qi bar's size (no cap in practice).
pub const ANCIENT_QI_MAX: f64 = 10_000.0;
/// Sát Lục Tiên Quyết: armour a brand, the brands past which the heart demon wakes, and the armour rank-3 blows pass.
pub const SLAUGHTER_BRAND_ARMOUR: f64 = 5.0;
pub const SLAUGHTER_BRANDS_MAX: u32 = 1000;
pub const SLAUGHTER_PIERCE: f64 = 0.3;
/// Tiên Trì: spells' more.
pub const POOL_MORE: f64 = 0.4;
/// Tóc Đen: what is left of life, shield, might and armour.
pub const BLACK_HAIR_STATS: f64 = 0.7;
/// Thái Cổ Lôi Long: qi a point of thunder taken, the share of foe thunder shots that turn, kinds besides thunder.
pub const DRAGON_QI: f64 = 0.05;
pub const DRAGON_HIJACK: f64 = 0.3;
pub const DRAGON_KINDS: usize = 1;
/// Hệ Đại Thành: the overcome factor against the root, and the share of the foe's blows of the root's kind subdued.
pub const MASTERY_WEAK: f64 = 1.5;
pub const MASTERY_SUBDUE: f64 = 0.2;
/// Tam Mệnh: spare lives, life on revival, less damage a life used.
pub const THREE_LIVES: u8 = 2;
pub const THREE_LIVES_REVIVE: f64 = 0.3;
pub const THREE_LIVES_LESS: f64 = 0.1;
/// Ngọn Đèn Hồn: lamps, life on revival, and a snuffed lamp's more on the next blow.
pub const SOUL_LAMPS: u8 = 3;
pub const SOUL_LAMP_REVIVE: f64 = 0.3;
pub const SOUL_LAMP_SNUFF: f64 = 0.5;
/// Hai Mươi Năm Khổ Luyện: rank-1 skills' more.
pub const TWENTY_YEARS_MORE: f64 = 1.0;
/// Người Xem Cây: time blows' more while moving, and the lifespan factor of time skills on it while still.
pub const TREE_TIME_TAKEN: f64 = 0.15;
pub const TREE_LIFESPAN: f64 = 2.0;
/// Đạo Dung Thân: more a vortex, vortices at most, and the basics' windup factor.
pub const VORTEX_MORE: f64 = 0.1;
pub const VORTEX_MAX: u8 = 9;
pub const VORTEX_WINDUP: f64 = 1.2;
/// Phá Rồi Lập: the window clones must break in, and the realm buff's length.
pub const REBUILD_WINDOW: f64 = 5.0;
pub const REBUILD_BUFF: f64 = 10.0;
/// Vu Chú: curses on the foe, the bar a curse cast, and the bar's size.
pub const WITCH_CURSES: u8 = 2;
pub const WITCH_STEP: f64 = 25.0;
pub const WITCH_MAX: f64 = 100.0;
/// Niềm Tin Tất Thắng: more a win in the streak, wins counted at most.
pub const FAITH_MORE: f64 = 0.05;
pub const FAITH_MAX: u8 = 10;
/// Bất Khuất: what is left of full life.
pub const UNYIELDING_LIFE: f64 = 0.85;
/// Sát Ý: more a blow landed, stacks at most, and what is left of the energy shield.
pub const FURY_MORE: f64 = 0.04;
pub const FURY_STACKS: u8 = 5;
pub const FURY_SHIELD: f64 = 0.5;
/// Tâm Pháp: qi-paid spells a rage point.
pub const ATTUNED_SPELLS: u8 = 2;

const fn extreme(e: Element) -> [Mod; 1] {
    [Mod::more(
        Stat::DamageOf(e),
        EXTREME_MORE,
        Source::Keystone("extreme"),
    )]
}

/// Cực Cảnh's more, one row a kind (in [`Element::ALL`]'s order).
static EXTREME: [[Mod; 1]; N] = [
    extreme(Element::Force),
    extreme(Element::Metal),
    extreme(Element::Wood),
    extreme(Element::Water),
    extreme(Element::Fire),
    extreme(Element::Earth),
    extreme(Element::Thunder),
    extreme(Element::Light),
    extreme(Element::Yin),
    extreme(Element::Soul),
];

/// Tiên Trì: spells more.
static POOL: [Mod; 1] =
    [Mod::more(Stat::Damage, POOL_MORE, Source::Keystone("immortalpool")).needs(&[Tag::Spell])];

/// Tóc Đen: every blow passes armour; life, shield, might and armour less.
static BLACK_HAIR: [Mod; 5] = {
    let src = Source::Keystone("blackhair");
    let less = BLACK_HAIR_STATS - 1.0;
    [
        Mod::new(Stat::ArmourPen, ModKind::Override, 1.0, src),
        Mod::more(Stat::Life, less, src),
        Mod::more(Stat::Shield, less, src),
        Mod::more(Stat::Might, less, src),
        Mod::more(Stat::Armour, less, src),
    ]
};

/// Người Xem Cây: time blows taken while moving strike harder.
static TREE_WATCHER: [Mod; 1] = [Mod::more(
    Stat::Taken,
    TREE_TIME_TAKEN,
    Source::Keystone("treewatcher"),
)
.needs(&[Tag::Time])
.when(Condition::Moving)];

/// Bất Khuất: less full life.
static UNYIELDING: [Mod; 1] = [Mod::more(
    Stat::Life,
    UNYIELDING_LIFE - 1.0,
    Source::Keystone("unyielding"),
)];

/// Sát Ý: less energy shield.
static FURY: [Mod; 1] = [Mod::more(
    Stat::Shield,
    FURY_SHIELD - 1.0,
    Source::Keystone("fury"),
)];

impl Keystone {
    /// Every keystone; Cực Cảnh stands for each kind (taken here as force).
    pub const ALL: [Keystone; 21] = [
        Keystone::Extreme(Element::Force),
        Keystone::ColdQi,
        Keystone::AncientGod,
        Keystone::SlaughterImmortal,
        Keystone::ImmortalPool,
        Keystone::BlackHair,
        Keystone::ThunderDragon,
        Keystone::Mastery,
        Keystone::ThreeLives,
        Keystone::SoulLamps,
        Keystone::TwentyYears,
        Keystone::GreyHeart,
        Keystone::TreeWatcher,
        Keystone::BornCursed,
        Keystone::DaoBody,
        Keystone::BreakRebuild,
        Keystone::Witch,
        Keystone::Faith,
        Keystone::Unyielding,
        Keystone::Fury,
        Keystone::Attuned,
    ];

    /// Its code name.
    pub fn id(self) -> &'static str {
        self.data().0
    }

    /// Its Vietnamese name.
    pub fn name(self) -> &'static str {
        self.data().1
    }

    /// What it gives, in words.
    pub fn gain(self) -> &'static str {
        self.data().2
    }

    /// What it costs, in words.
    pub fn cost(self) -> &'static str {
        self.data().3
    }

    /// Where it comes from.
    pub fn source(self) -> &'static str {
        self.data().4
    }

    fn data(
        self,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    ) {
        match self {
            Keystone::Extreme(_) => (
                "extreme",
                "Cực Cảnh",
                "its kind 60% more; its instants execute an equal or lower rank under 15% life",
                "no conversion support, no Dung",
                "TN91, TN125",
            ),
            Keystone::ColdQi => (
                "coldqi",
                "Âm Hàn Linh Lực",
                "every blow water at step 1; half of thunder taken turns water",
                "no support of another kind",
                "TN104",
            ),
            Keystone::AncientGod => (
                "ancientgod",
                "Cổ Thần Quyết",
                "no qi cap; drinks qi spilled on the field",
                "no recovery skills",
                "TN1",
            ),
            Keystone::SlaughterImmortal => (
                "slaughterimmortal",
                "Sát Lục Tiên Quyết",
                "each blow taken lays a brand worth armour",
                "past 1000 brands the heart demon; rank-3 blows pass 30% armour",
                "TN575",
            ),
            Keystone::ImmortalPool => (
                "immortalpool",
                "Tiên Trì",
                "every spell 40% more",
                "no auras, no dao-will contests",
                "TN879",
            ),
            Keystone::BlackHair => (
                "blackhair",
                "Tóc Đen",
                "every blow passes resistance and armour; no dispel takes its buffs",
                "life, shield, might and armour 30% less; no auras or blessings",
                "TN8",
            ),
            Keystone::ThunderDragon => (
                "thunderdragon",
                "Thái Cổ Lôi Long",
                "thunder taken becomes qi; own thunder passes thunder resistance; 30% of foe thunder shots turn",
                "one kind besides thunder in the loadout",
                "TN702, TN873",
            ),
            Keystone::Mastery => (
                "mastery",
                "Hệ Đại Thành",
                "immune to the root's kind; 20% of the foe's blows of it subdued",
                "what overcomes the root strikes x1.5 (not x1.25); needs a root",
                "TN1657",
            ),
            Keystone::ThreeLives => (
                "threelives",
                "Tam Mệnh",
                "two spare lives: rise at 30% life, karma cut",
                "never refilled; each life used 10% less damage",
                "TN7, TN8",
            ),
            Keystone::SoulLamps => (
                "soullamps",
                "Ngọn Đèn Hồn",
                "three lamps: each a spare life, or snuffed for 50% more on the next blow",
                "lamps can be snuffed from afar; out of lamps loses the match",
                "CM5",
            ),
            Keystone::TwentyYears => (
                "twentyyears",
                "Hai Mươi Năm Khổ Luyện",
                "rank-1 skills 100% more",
                "no skill of rank 3 or more",
                "TN1",
            ),
            Keystone::GreyHeart => (
                "greyheart",
                "Tâm Xám",
                "immune to emotions",
                "no emotion buffs",
                "CM5",
            ),
            Keystone::TreeWatcher => (
                "treewatcher",
                "Người Xem Cây",
                "standing still: immune to slows, rewinds and age; time skills on it cost double lifespan",
                "moving: time blows 15% more",
                "CM1339",
            ),
            Keystone::BornCursed => (
                "borncursed",
                "Nguyền Bẩm Sinh",
                "never old; time skills on it curse their caster",
                "no time skills",
                "CM1051",
            ),
            Keystone::DaoBody => (
                "daobody",
                "Đạo Dung Thân",
                "each skill used charges a vortex into the next basic, 10% more each, 9 at most",
                "basics 20% slower; each guard layer eats a vortex",
                "TN1919",
            ),
            Keystone::BreakRebuild => (
                "breakrebuild",
                "Phá Rồi Lập",
                "clones broken within 5 s raise the realm for 10 s",
                "a clone lost is lost for the round",
                "CM6",
            ),
            Keystone::Witch => (
                "witch",
                "Vu Chú",
                "two curses on the foe",
                "each curse fills the Tam Hoang bar; full, its owner takes the last curse",
                "CM1261",
            ),
            Keystone::Faith => (
                "faith",
                "Niềm Tin Tất Thắng",
                "fist skills 5% more a win in the season's streak, 10 at most",
                "one loss clears the streak",
                "TN544",
            ),
            Keystone::Unyielding => (
                "unyielding",
                "Bất Khuất",
                "once a round a killing blow leaves 1 life",
                "15% less full life",
                "arena",
            ),
            Keystone::Fury => (
                "fury",
                "Sát Ý",
                "each blow landed 4% more for the round, 5 stacks",
                "half the energy shield",
                "arena",
            ),
            Keystone::Attuned => (
                "attuned",
                "Tâm Pháp",
                "every two qi-paid spells give a rage point",
                "attacks give no rage",
                "arena",
            ),
        }
    }

    /// The same keystone, whatever kind a Cực Cảnh was taken for.
    pub fn same(self, other: Keystone) -> bool {
        core::mem::discriminant(&self) == core::mem::discriminant(&other)
    }

    /// It needs a root (Hệ Đại Thành).
    pub fn needs_root(self) -> bool {
        self == Keystone::Mastery
    }

    /// What it does to the fighter at the start of a fight that is not a stat: Cổ Thần Quyết's qi bar, Hệ Đại
    /// Thành's immunity and harsher bane, Nguyền Bẩm Sinh's agelessness. Set the root first. Its stats (less life,
    /// less shield...) are [`Keystone::lasting`] modifiers, which `sheet::Sheet::apply` bakes into the bars.
    pub fn apply(self, c: &mut Cultivator) {
        match self {
            Keystone::AncientGod => {
                c.qi_max = ANCIENT_QI_MAX;
            }
            Keystone::Mastery => {
                if let Some(r) = c.root {
                    c.immune = Some(r);
                    c.root_weak = MASTERY_WEAK;
                }
            }
            Keystone::BornCursed => c.ageless = true,
            _ => {}
        }
    }

    /// Its modifiers that hold the whole fight, whatever happens ([`Source::Keystone`]): they live on the sheet
    /// (`sheet::Sheet::mods`), so a blow it strikes and a blow it takes both gather them.
    pub fn lasting(self) -> &'static [Mod] {
        match self {
            Keystone::Extreme(e) => &EXTREME[e.index()],
            Keystone::ImmortalPool => &POOL,
            Keystone::BlackHair => &BLACK_HAIR,
            Keystone::TreeWatcher => &TREE_WATCHER,
            Keystone::Unyielding => &UNYIELDING,
            Keystone::Fury => &FURY,
            _ => &[],
        }
    }

    /// Its modifiers on one blow, from what the fight knows at the cast: Tam Mệnh's lives used, Ngọn Đèn Hồn's
    /// snuffed lamp, Hai Mươi Năm's rank-1 skill (`grade`, after supports), Đạo Dung Thân's vortices and slower
    /// basics, Niềm Tin Tất Thắng's streak, Sát Ý's stacks.
    pub fn strike_mods(self, skill: &Skill, grade: Grade, sit: &Situation) -> Vec<Mod> {
        let src = Source::Keystone(self.id());
        let more = |v: f64| Mod::more(Stat::Damage, v, src);
        match self {
            Keystone::ThreeLives if sit.lives_used > 0 => {
                vec![more(
                    -THREE_LIVES_LESS * f64::from(sit.lives_used.min(THREE_LIVES)),
                )]
            }
            Keystone::SoulLamps if sit.snuffed => vec![more(SOUL_LAMP_SNUFF)],
            Keystone::TwentyYears if grade == Grade::Ranked(Rank::Art) => {
                vec![more(TWENTY_YEARS_MORE)]
            }
            Keystone::DaoBody if skill.basic => {
                let mut out = vec![Mod::more(Stat::Windup, VORTEX_WINDUP - 1.0, src)];
                if sit.vortices > 0 {
                    out.push(more(VORTEX_MORE * f64::from(sit.vortices.min(VORTEX_MAX))));
                }
                out
            }
            Keystone::Faith if skill.tags.has(Tag::Fist) && sit.streak > 0 => {
                vec![more(FAITH_MORE * f64::from(sit.streak.min(FAITH_MAX)))]
            }
            Keystone::Fury if sit.fury > 0 => {
                vec![more(FURY_MORE * f64::from(sit.fury.min(FURY_STACKS)))]
            }
            _ => Vec::new(),
        }
    }

    /// What it does to a blow its fighter strikes that is not a stat: Âm Hàn Linh Lực rewrites every kind to water
    /// at step 1; Tóc Đen and Thái Cổ Lôi Long pass the target's resistance (penetration up to it, read from
    /// `sit.target`).
    pub fn on_strike(self, hit: &mut Hit, sit: &Situation) {
        match self {
            Keystone::ColdQi => {
                for e in Element::ALL {
                    if e != Element::Water {
                        rewrite(hit, e, Element::Water, 1.0);
                    }
                }
            }
            Keystone::BlackHair => {
                if let Some(t) = sit.target {
                    for e in Element::ALL.into_iter().skip(1) {
                        let i = e.index();
                        hit.pen[i] = hit.pen[i].max(t.resist_of(e).max(0.0));
                    }
                }
            }
            Keystone::ThunderDragon => {
                if let Some(t) = sit.target {
                    let i = Element::Thunder.index();
                    hit.pen[i] = hit.pen[i].max(t.resist_of(Element::Thunder).max(0.0));
                }
            }
            _ => {}
        }
    }

    /// What it does to a blow its fighter takes that is not a stat, before [`resolve`](crate::hit::resolve): Âm Hàn
    /// turns thunder to water, Thái Cổ Lôi Long drinks thunder as qi, Sát Lục Tiên Quyết lets rank-3 blows pass
    /// armour. Returns the qi the fighter gains at once. (Người Xem Cây's harder time blows are a lasting
    /// modifier: `Taken` for time skills while moving.)
    pub fn on_taken(self, hit: &mut Hit) -> f64 {
        match self {
            Keystone::ColdQi => {
                rewrite(hit, Element::Thunder, Element::Water, COLD_TAKEN);
                0.0
            }
            Keystone::ThunderDragon => {
                let i = Element::Thunder.index();
                let drunk = hit.base[i] + hit.added[i] * hit.effectiveness;
                hit.base[i] = 0.0;
                hit.added[i] = 0.0;
                drunk * DRAGON_QI
            }
            Keystone::SlaughterImmortal => {
                if let Grade::Ranked(r) = hit.grade
                    && r >= Rank::Rule
                {
                    hit.armour_pen = (hit.armour_pen + SLAUGHTER_PIERCE).min(1.0);
                }
                0.0
            }
            _ => 0.0,
        }
    }

    /// The fighter's life ran out; `used` is how many times this keystone has spared it already (this round for Bất
    /// Khuất, this match for the others). Returns true when it spares the fighter, whose life it sets.
    pub fn on_death(self, c: &mut Cultivator, used: u8) -> bool {
        let rise = match self {
            Keystone::Unyielding if used == 0 => 1.0,
            Keystone::ThreeLives if used < THREE_LIVES => THREE_LIVES_REVIVE * c.life_max,
            Keystone::SoulLamps if used < SOUL_LAMPS => SOUL_LAMP_REVIVE * c.life_max,
            _ => return false,
        };
        c.life = rise.min(c.life_max);
        true
    }

    /// Armour from `brands` (Sát Lục Tiên Quyết), and whether they woke the heart demon.
    pub fn brands(brands: u32) -> (f64, bool) {
        (
            f64::from(brands) * SLAUGHTER_BRAND_ARMOUR,
            brands > SLAUGHTER_BRANDS_MAX,
        )
    }

    /// It forbids `s` on any skill.
    pub fn forbids_support(self, s: &Support) -> bool {
        match self {
            Keystone::Extreme(_) => s.conversion || s.is_fuse(),
            Keystone::ColdQi => s.kinds().iter().any(|&k| k != Element::Water),
            _ => false,
        }
    }

    /// It forbids a skill of `tags` whose rank, after its supports, is `rank`.
    pub fn forbids_skill(self, tags: Tags, rank: Rank) -> bool {
        match self {
            Keystone::AncientGod => tags.has(Tag::Recovery),
            Keystone::ImmortalPool => tags.has(Tag::Aura),
            Keystone::BlackHair => tags.any(&[Tag::Aura, Tag::Blessing]),
            Keystone::TwentyYears => rank >= Rank::Rule,
            Keystone::BornCursed => tags.has(Tag::Time),
            _ => false,
        }
    }

    /// Whether the loadout's kinds of damage (`kinds[i]`: some skill deals kind `i`) are allowed: Thái Cổ Lôi Long
    /// allows one kind besides thunder.
    pub fn kinds_allowed(self, kinds: &[bool; N]) -> bool {
        match self {
            Keystone::ThunderDragon => {
                Element::ALL
                    .iter()
                    .filter(|&&e| e != Element::Thunder && kinds[e.index()])
                    .count()
                    <= DRAGON_KINDS
            }
            _ => true,
        }
    }
}

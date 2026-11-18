//! Supports, keystones, the loadout and building a blow from a skill (`docs/specs/2026-11-17-damage-system-design.md`,
//! sections 2, 8, 9 and 10; `docs/specs/2026-11-17-skill-system-rules.md`, 2.2). The pool here is local, so these
//! pass whatever `GEMS` holds.

use arena_calc::ailment::{Ailment, ForceAilment};
use arena_calc::build::{BLOOD_POWER, Built, Situation, Skill, build, build_skill};
use arena_calc::cultivator::{BASE, Cultivator};
use arena_calc::element::{Element, N};
use arena_calc::gem::{GEMS, Gem, Resource, Tier};
use arena_calc::grade::{Grade, Rank};
use arena_calc::hit::{Defence, Hit, Outcome, Rolls, resolve, resolve_with, spell_base};
use arena_calc::keystone::{self as ks, Keystone};
use arena_calc::loadout::{
    ACTIVES, BUDGET, KEYSTONE_POINTS, Loadout, LoadoutError, Slot, check, check_in, points_in,
    random, random_in,
};
use arena_calc::sheet::{EMPTY, Sheet};
use arena_calc::stat::{Ctx, Mod, Source, Stat};
use arena_calc::support::{
    self as sp, Behaviour, SUPPORTS, Support, excluded, fuse, rejection, support,
};
use arena_calc::tag::{Tag, Tags};

const NO_CRIT: Rolls = Rolls {
    evade: 1.0,
    crit: 1.0,
    ..Rolls::NONE
};

const QI: &[(Resource, f64)] = &[(Resource::Qi, 20.0)];

const fn gem(
    id: &'static str,
    tags: &'static [Tag],
    split: &'static [(Element, f64)],
    base: f64,
    tier: Tier,
) -> Gem {
    Gem {
        id,
        name: id,
        tags,
        split,
        rank: Rank::Art,
        restriction: false,
        base,
        costs: QI,
        cooldown: 4.0,
        windup: 0.5,
        recover: 0.4,
        uses: None,
        bluff: false,
        force: ForceAilment::Bleed,
        tier,
        spell: "bolt",
        source: "arena",
        effects: &[],
    }
}

const FORCE: &[(Element, f64)] = &[(Element::Force, 1.0)];

const POOL: &[Gem] = &[
    gem(
        "palm",
        &[Tag::Attack, Tag::Palm, Tag::Melee, Tag::Slam],
        FORCE,
        1.0,
        Tier::C,
    ),
    gem(
        "sword",
        &[Tag::Attack, Tag::Sword, Tag::Melee, Tag::Strike],
        FORCE,
        1.2,
        Tier::B,
    ),
    gem(
        "nineturn",
        &[Tag::Attack, Tag::Melee, Tag::Slam, Tag::Charged],
        FORCE,
        1.5,
        Tier::A,
    ),
    gem(
        "bolt",
        &[Tag::Spell, Tag::Projectile],
        &[(Element::Thunder, 1.0)],
        100.0,
        Tier::C,
    ),
    gem(
        "spark",
        &[Tag::Spell, Tag::Projectile],
        &[(Element::Fire, 1.0)],
        100.0,
        Tier::C,
    ),
    gem(
        "finger",
        &[Tag::Spell, Tag::Projectile, Tag::Finger],
        &[(Element::Metal, 1.0)],
        120.0,
        Tier::B,
    ),
    gem(
        "thirdeye",
        &[Tag::Spell, Tag::Instant],
        &[(Element::Light, 1.0)],
        150.0,
        Tier::B,
    ),
    gem(
        "quake",
        &[Tag::Spell, Tag::Area],
        &[(Element::Earth, 1.0)],
        140.0,
        Tier::C,
    ),
    gem(
        "jet",
        &[Tag::Spell, Tag::Line],
        &[(Element::Water, 1.0)],
        110.0,
        Tier::C,
    ),
    gem(
        "seal",
        &[Tag::Spell, Tag::Seal, Tag::Delayed],
        &[(Element::Thunder, 1.0)],
        130.0,
        Tier::C,
    ),
    gem(
        "bodyfix",
        &[Tag::Spell, Tag::Instant, Tag::Control],
        &[(Element::Soul, 1.0)],
        60.0,
        Tier::B,
    ),
    gem("ward", &[Tag::Body, Tag::Guard], &[], 0.0, Tier::C),
    gem(
        "bell",
        &[Tag::Body, Tag::Guard, Tag::Treasure],
        &[],
        0.0,
        Tier::C,
    ),
    gem("heal", &[Tag::Body, Tag::Recovery], &[], 0.0, Tier::C),
    gem("aura", &[Tag::Body, Tag::Aura], &[], 0.0, Tier::C),
    gem(
        "flow",
        &[Tag::Body, Tag::Recovery, Tag::Time],
        &[],
        0.0,
        Tier::C,
    ),
    gem(
        "qiburst",
        &[Tag::Attack, Tag::Strike, Tag::Melee],
        FORCE,
        1.0,
        Tier::C,
    ),
    gem(
        "qibolt",
        &[Tag::Attack, Tag::Projectile],
        FORCE,
        1.0,
        Tier::C,
    ),
    gem("blink", &[Tag::Body, Tag::Movement], &[], 0.0, Tier::C),
    gem(
        "tribulation",
        &[Tag::Spell, Tag::Area],
        &[(Element::Thunder, 1.0)],
        400.0,
        Tier::S,
    ),
    gem(
        "daofusion",
        &[Tag::Attack, Tag::Nova, Tag::Aura],
        FORCE,
        3.0,
        Tier::S,
    ),
];

fn pool(id: &str) -> &'static Gem {
    POOL.iter().find(|g| g.id == id).unwrap_or(&POOL[0])
}

fn sup(id: &str) -> &'static Support {
    support(id).unwrap_or(&Support::NONE)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

fn bare() -> Cultivator {
    let mut c = Cultivator::new();
    c.armour = 0.0;
    c.shield = 0.0;
    c.shield_max = 0.0;
    c
}

fn made(id: &str, supports: &[&str], keystones: &[Keystone]) -> Built {
    let s: Vec<&Support> = supports.iter().map(|i| sup(i)).collect();
    build(
        pool(id),
        1,
        &s,
        &Cultivator::new(),
        &Sheet::with_keystones(keystones),
        &Situation::default(),
    )
}

fn land(hit: &Hit) -> Outcome {
    resolve(hit, None, &mut bare(), NO_CRIT)
}

fn slot(skill: &str, supports: &[&str]) -> Slot {
    Slot {
        skill: skill.to_string(),
        supports: supports.iter().map(|s| s.to_string()).collect(),
        fuse: None,
    }
}

/// A legal loadout of eight C and B skills: 2×4 + 6×3 = 26 points.
fn legal() -> Loadout {
    Loadout {
        actives: [
            "palm", "sword", "bolt", "spark", "finger", "quake", "jet", "ward",
        ]
        .iter()
        .map(|s| slot(s, &[]))
        .collect(),
        keystones: Vec::new(),
        root: None,
    }
}

fn errors(l: &Loadout) -> Vec<LoadoutError> {
    check_in(l, POOL).err().unwrap_or_default()
}

fn has(l: &Loadout, f: impl Fn(&LoadoutError) -> bool) -> bool {
    errors(l).iter().any(f)
}

// ---- the table ----

/// The PoE supports (design, 8, the second table), by their Vietnamese names.
const POE_NAMES: [&str; 72] = [
    "Phụ Hỏa",
    "Phụ Thủy",
    "Phụ Lôi",
    "Phụ Âm",
    "Phụ Kim",
    "Phụ Mộc",
    "Phụ Thổ",
    "Phụ Quang",
    "Phụ Hồn",
    "Ngũ Hành Ngưng",
    "Khống Diệt",
    "Tụ Lực",
    "Khoách Vực",
    "Thuần Lực",
    "Xuyên Thấu",
    "Phân Nhánh",
    "Liên Kích",
    "Tật Tiễn",
    "Hoãn Tiễn",
    "Tiểu Liên Châu",
    "Cận Xạ",
    "Viễn Xạ",
    "Cương Lực",
    "Áp Sát",
    "Tàn Nhẫn",
    "Chiến Quyền",
    "Lưu Lực",
    "Trảm Tận",
    "Phế Cước",
    "Tốc Kích",
    "Tam Liên Ảnh",
    "Tốc Chú",
    "Hồi Âm",
    "Linh Triều",
    "Hiệu Lực",
    "Linh Cảm",
    "Liệt Diễm",
    "Hỏa Thiêu Thân",
    "Tốc Độc",
    "Độc Ngấm",
    "Tàn Khốc",
    "Khát Huyết",
    "Tịch Thủ",
    "Hư Không",
    "Nhiên Thiêu",
    "Tăng Ý",
    "Phong Ấn",
    "Quá Tải",
    "Thủy Phệ",
    "Bạo Thương",
    "Ma Tinh Dẫn",
    "Thần Tinh Dẫn",
    "Yêu Tinh Dẫn",
    "Tam Tài",
    "Hồi Huyết",
    "Hấp Khí",
    "Sát Sinh Bổ Huyết",
    "Chú Ấn Kích",
    "Chú Vực",
    "Khai Ngộ",
    "Khảng Khái",
    "Bẫy Trận",
    "Địa Lôi Trận",
    "Pháp Đàn",
    "Ngự Linh Lực",
    "Ngự Linh Thể",
    "Ảo Linh",
    "Bạo Kích Dẫn Pháp",
    "Thụ Thương Phản Chú",
    "Lâm Nguy Phản Kích",
    "Kênh Dẫn Pháp",
    "Huyết Tế",
];

#[test]
fn every_support_of_the_research_and_the_design_is_there_once() {
    let names = [
        "Thanh Trần",
        "Liên Châu",
        "Tàn Ảnh Tốc Thần",
        "Cửu Chuyển",
        "Cửu Biến",
        "Quy Tắc",
        "Dung",
        "Hóa Ma",
        "Âm Hàn",
        "Huyết Luyện",
        "Phun Máu",
        "Súc Địa",
        "Dịch Linh",
        "Lôi Bạo Trễ",
        "Nhử Đòn",
        "Chắc Trúng",
        "Liên Hoàn Suy Yếu",
        "Phản Ấn",
        "Khí Nhân Quả",
        "Tự Bạo",
        "Thu Hồn",
        "Thôn Cấm",
        "Sao Chép",
        "Mê Dục",
        "Trọng",
        "Hư Hỏa",
        "Tuế Nguyệt",
        "Chặn Lui",
        "Không Khóa",
        "Hồn Bạo",
        "Nạp Hồn",
        "Đoạt Tu Vi",
        "Hỏa Luyện",
        "Kim Phong",
        "Thủy Ý",
        "Lôi Dẫn",
        "Mộc Sinh",
        "Xuyên Giáp",
        "Phá Kháng",
        "Ác Tính",
        "Chí Mạng",
        "Thời Gian Kéo Dài",
        "Độc Tâm",
        "Hỏa Ngục Thiêu Đốt",
        "Bạo Phát",
        "Thừa Thắng",
        "Thị Huyết",
    ];
    for n in names.iter().chain(&POE_NAMES) {
        assert_eq!(SUPPORTS.iter().filter(|s| s.name == *n).count(), 1, "{n}");
    }
    assert_eq!(SUPPORTS.len(), names.len() + POE_NAMES.len());
    for (i, s) in SUPPORTS.iter().enumerate() {
        assert!(SUPPORTS[..i].iter().all(|o| o.id != s.id), "{} twice", s.id);
        assert!((1..=3).contains(&s.points), "{}", s.id);
        assert!(s.cost > 0.0 && s.cost <= 2.0, "{}", s.id);
        assert!(
            s.id.is_ascii() && !s.id.is_empty() && !s.name.is_empty(),
            "{}",
            s.id
        );
        for m in s.stats {
            if let Stat::Convert(from, to) = m.stat {
                assert!(to > from, "{} converts backward", s.id);
                assert!(m.value > 0.0 && m.value <= 1.0);
            }
        }
        for x in s.excludes {
            assert!(support(x).is_some(), "{} excludes unknown {x}", s.id);
        }
        let none_of_freeze = ["freeze", "frost", "chill", "ice", "băng"];
        assert!(
            none_of_freeze
                .iter()
                .all(|w| !s.id.contains(w) && !s.name.to_lowercase().contains(w))
        );
    }
}

// ---- exclusions (rules, 2.2.4) ----

#[test]
fn demonize_and_cold_exclude_each_other() {
    assert!(excluded(sup("demonize"), sup("coldqi")));
    let mut l = legal();
    l.actives[3] = slot("spark", &["demonize", "coldqi"]);
    assert!(has(&l, |e| matches!(e, LoadoutError::Excluded { .. })));
}

#[test]
fn nine_turns_and_afterimage_exclude_each_other() {
    assert!(excluded(sup("afterimage"), sup("nineturns")));
    let mut l = legal();
    l.actives[0] = slot("nineturn", &["nineturns", "afterimage"]);
    let e = errors(&l);
    assert!(
        e.iter().any(|e| matches!(e, LoadoutError::Excluded { .. })),
        "{e:?}"
    );
    assert!(
        !e.iter()
            .any(|e| matches!(e, LoadoutError::SupportTags { .. })),
        "{e:?}"
    );
}

#[test]
fn blood_forge_and_blood_spit_exclude_each_other() {
    assert!(excluded(sup("bloodspit"), sup("bloodforge")));
    let mut l = legal();
    l.actives[7] = slot("bell", &["bloodforge", "bloodspit"]);
    assert!(has(&l, |e| matches!(e, LoadoutError::Excluded { .. })));
}

#[test]
fn leap_never_fits_a_charged_skill() {
    assert!(!sup("leap").fits(pool("nineturn").all_tags()));
    assert!(sup("leap").fits(pool("palm").all_tags()));
    let mut l = legal();
    l.actives[0] = slot("nineturn", &["leap"]);
    assert!(has(
        &l,
        |e| matches!(e, LoadoutError::SupportTags { support, .. } if support == "leap")
    ));
}

#[test]
fn extreme_forbids_every_conversion_and_fuse() {
    let k = Keystone::Extreme(Element::Fire);
    for s in SUPPORTS {
        let conv = s.conversion || s.is_fuse();
        assert_eq!(k.forbids_support(s), conv, "{}", s.id);
    }
    for id in [
        "fireforge",
        "metalwind",
        "waterintent",
        "thunderlead",
        "woodgrowth",
        "demonize",
        "coldqi",
        "fuse",
    ] {
        assert!(k.forbids_support(sup(id)), "{id}");
    }
    let mut l = legal();
    l.keystones = vec![k];
    l.actives[0] = slot("palm", &["fireforge"]);
    assert!(has(&l, |e| matches!(
        e,
        LoadoutError::KeystoneSupport { .. }
    )));
}

#[test]
fn cold_qi_forbids_supports_of_another_kind() {
    let k = Keystone::ColdQi;
    for id in [
        "fireforge",
        "metalwind",
        "thunderlead",
        "woodgrowth",
        "demonize",
        "heavy",
        "hollowfire",
    ] {
        assert!(k.forbids_support(sup(id)), "{id}");
    }
    for id in ["waterintent", "coldqi", "focus", "critical"] {
        assert!(!k.forbids_support(sup(id)), "{id}");
    }
}

// ---- budget, counts, tags ----

#[test]
fn a_legal_loadout_passes_and_counts_its_points() {
    let l = legal();
    assert_eq!(check_in(&l, POOL), Ok(()));
    assert_eq!(points_in(&l, POOL), 26);
    let mut l = l;
    l.actives[0] = slot("palm", &["focus", "heavy"]);
    l.keystones = vec![Keystone::Fury];
    assert_eq!(points_in(&l, POOL), 26 + 2 + 1 + KEYSTONE_POINTS);
}

#[test]
fn over_budget_is_refused() {
    let mut l = legal();
    // 26 + 2 keystones (8) + 12 supports (25)
    l.keystones = vec![Keystone::Fury, Keystone::Unyielding];
    l.actives[0] = slot("palm", &["focus", "afterimage", "rule"]);
    l.actives[1] = slot("sword", &["ninechanges", "malignant", "critical"]);
    l.actives[2] = slot("bolt", &["multishot", "spiritshift", "allure"]);
    l.actives[3] = slot("spark", &["hollowfire", "drain", "breakresist"]);
    let p = points_in(&l, POOL);
    assert!(p > BUDGET, "{p}");
    assert!(has(&l, |e| *e == LoadoutError::Budget { points: p }));
}

#[test]
fn counts_are_enforced() {
    let mut l = legal();
    l.actives.pop();
    assert!(has(&l, |e| *e == LoadoutError::Actives { count: ACTIVES - 1 }));

    let mut l = legal();
    l.actives[1] = slot("palm", &[]);
    assert!(has(&l, |e| matches!(
        e,
        LoadoutError::DuplicateActive { .. }
    )));

    let mut l = legal();
    l.actives[1] = slot("qibolt", &[]);
    assert!(has(&l, |e| matches!(e, LoadoutError::NotActive { .. })));
    // a top-tier skill is an active like any other
    l.actives[1] = slot("tribulation", &[]);
    assert!(!has(&l, |e| matches!(e, LoadoutError::NotActive { .. })));
    // and a loadout holds one of rank Tín Niệm at most (the pool's ranks, not this table's)
    let faith = |l: &Loadout| {
        arena_calc::loadout::check(l)
            .err()
            .unwrap_or_default()
            .into_iter()
            .find(|e| matches!(e, LoadoutError::Faith { .. }))
    };
    assert_eq!(faith(&l), None);
    l.actives[2] = slot("daofusion", &[]);
    assert_eq!(faith(&l), Some(LoadoutError::Faith { count: 2 }));
    l.actives[1] = slot("nothing", &[]);
    assert!(has(&l, |e| matches!(e, LoadoutError::UnknownSkill { .. })));

    let mut l = legal();
    l.actives[0] = slot("palm", &["focus", "heavy", "malignant", "critical"]);
    assert!(has(&l, |e| matches!(
        e,
        LoadoutError::SlotSupports { count: 4, .. }
    )));

    let mut l = legal();
    for (i, s) in l.actives.iter_mut().take(5).enumerate() {
        let ids: &[&str] = if i < 4 {
            &["focus", "malignant", "surehit"]
        } else {
            &["focus"]
        };
        s.supports = ids.iter().map(|x| x.to_string()).collect();
    }
    assert!(has(&l, |e| *e == LoadoutError::Supports { count: 13 }));

    let mut l = legal();
    l.actives[0] = slot("palm", &["focus", "focus"]);
    assert!(has(&l, |e| matches!(
        e,
        LoadoutError::DuplicateSupport { .. }
    )));
    l.actives[0] = slot("palm", &["nosuch"]);
    assert!(has(&l, |e| matches!(
        e,
        LoadoutError::UnknownSupport { .. }
    )));

    let mut l = legal();
    l.keystones = vec![Keystone::Fury, Keystone::Witch, Keystone::Unyielding];
    assert!(has(&l, |e| *e == LoadoutError::Keystones { count: 3 }));
    l.keystones = vec![
        Keystone::Extreme(Element::Fire),
        Keystone::Extreme(Element::Water),
    ];
    assert!(has(&l, |e| matches!(
        e,
        LoadoutError::DuplicateKeystone { .. }
    )));

    let mut l = legal();
    l.root = Some(Element::Force);
    assert!(has(&l, |e| *e
        == LoadoutError::Root {
            root: Element::Force
        }));
    l.root = Some(Element::Thunder);
    assert!(has(&l, |e| matches!(e, LoadoutError::Root { .. })));
    l.root = None;
    l.keystones = vec![Keystone::Mastery];
    assert!(has(&l, |e| matches!(e, LoadoutError::KeystoneRoot { .. })));
    l.root = Some(Element::Wood);
    assert_eq!(check_in(&l, POOL), Ok(()));
}

#[test]
fn a_support_must_fit_the_skills_tags() {
    let mut l = legal();
    l.actives[0] = slot("palm", &["multishot"]);
    assert!(has(
        &l,
        |e| matches!(e, LoadoutError::SupportTags { support, .. } if support == "multishot")
    ));
    l.actives[0] = slot("palm", &["breakresist"]);
    assert!(has(&l, |e| matches!(e, LoadoutError::SupportTags { .. })));
    l.actives[0] = slot("palm", &["hollowfire"]);
    assert!(has(&l, |e| matches!(e, LoadoutError::SupportTags { .. })));
    // kinds of damage count as tags
    assert!(sup("hollowfire").fits(pool("spark").all_tags()));
    assert!(sup("coldqi").fits(pool("bolt").all_tags()));
    assert!(!sup("coldqi").fits(pool("finger").all_tags()));
    // no crit support on an instant
    assert!(!sup("critical").fits(pool("thirdeye").all_tags()));
    // and the builder leaves a misfit inert
    let b = made("palm", &["multishot", "focus"], &[]);
    assert_eq!(b.inert, vec!["multishot"]);
    assert_eq!(b.projectiles, 0);
    assert_eq!(b.hit.more, vec![sp::FOCUS_MORE]);
}

#[test]
fn keystones_forbid_skills() {
    let cases = [
        (Keystone::ImmortalPool, "aura"),
        (Keystone::BlackHair, "aura"),
        (Keystone::AncientGod, "heal"),
        (Keystone::BornCursed, "flow"),
    ];
    for (k, skill) in cases {
        let mut l = legal();
        l.keystones = vec![k];
        l.actives[7] = slot(skill, &[]);
        assert!(
            has(&l, |e| matches!(e, LoadoutError::KeystoneSkill { .. })),
            "{k:?}"
        );
    }
    // a top-tier skill is judged like any: daofusion carries an aura tag here
    let mut l = legal();
    l.keystones = vec![Keystone::ImmortalPool];
    l.actives[7] = slot("daofusion", &[]);
    assert!(has(
        &l,
        |e| matches!(e, LoadoutError::KeystoneSkill { skill, .. } if skill == "daofusion")
    ));
    // twenty years: a sword raised to Rule by Quy Tắc is out, a rank-1 sword is in
    let mut l = legal();
    l.keystones = vec![Keystone::TwentyYears];
    assert_eq!(check_in(&l, POOL), Ok(()));
    l.actives[1] = slot("sword", &["rule"]);
    assert_eq!(check_in(&l, POOL), Ok(()), "Art + rule is Divine");
    assert!(Keystone::TwentyYears.forbids_skill(Tags::NONE, Rank::Rule));
    assert!(!Keystone::TwentyYears.forbids_skill(Tags::NONE, Rank::Divine));
}

#[test]
fn thunder_dragon_allows_one_kind_besides_thunder() {
    let mut l = legal();
    l.keystones = vec![Keystone::ThunderDragon];
    assert!(has(&l, |e| matches!(e, LoadoutError::KeystoneKinds { .. })));
    l.actives = [
        "palm", "sword", "bolt", "seal", "nineturn", "ward", "bell", "heal",
    ]
    .iter()
    .map(|s| slot(s, &[]))
    .collect();
    assert_eq!(check_in(&l, POOL), Ok(()));
    // a conversion adds a kind
    l.actives[0] = slot("palm", &["fireforge"]);
    assert!(has(&l, |e| matches!(e, LoadoutError::KeystoneKinds { .. })));
}

// ---- Dung ----

#[test]
fn fuse_takes_the_shape_of_the_second_and_the_kinds_of_the_first() {
    let might = BASE.might;
    let a = Skill::of(pool("spark"), 5, might);
    let b = Skill::of(pool("sword"), 5, might);
    let f = fuse(&a, &b);
    assert_eq!(f.split, vec![(Element::Fire, 1.0)]);
    assert!(f.tags.has(Tag::Attack) && f.tags.has(Tag::Sword) && f.tags.has(Tag::Melee));
    assert!(!f.tags.has(Tag::Spell) && !f.tags.has(Tag::Projectile));
    close(
        f.base,
        sp::FUSE_SHARE * (spell_base(100.0, 5) + might * 1.2),
    );
    close(f.windup, b.windup);
    close(f.recover, b.recover);
    close(f.cooldown, a.cooldown.max(b.cooldown));
    assert_eq!(f.costs, vec![(Resource::Qi, 40.0)]);
    assert_eq!(f.fused, Some("sword"));
    // it builds and lands as fire
    let built = build_skill(&f, &[], &Cultivator::new(), &EMPTY, &Situation::default());
    let o = land(&built.hit);
    assert!(o.dealt[Element::Fire.index()] > 0.0);
    close(o.dealt.iter().sum::<f64>(), o.dealt[Element::Fire.index()]);
}

#[test]
fn fuse_rejects_on_its_roll() {
    close(rejection(0.0, 500.0), sp::FUSE_BACKLASH * 500.0);
    close(rejection(sp::FUSE_REJECTION - 1e-9, 100.0), 20.0);
    close(rejection(sp::FUSE_REJECTION, 100.0), 0.0);
    close(rejection(0.9, 100.0), 0.0);
    assert!(sup("fuse").behaviours.contains(&Behaviour::Rejection {
        chance: 0.15,
        share: 0.2
    }));
}

#[test]
fn fuse_needs_a_partner_among_the_actives() {
    let mut l = legal();
    l.actives[0] = slot("palm", &["fuse"]);
    assert!(has(&l, |e| matches!(e, LoadoutError::FuseMissing { .. })));
    l.actives[0].fuse = Some("palm".to_string());
    assert!(has(&l, |e| matches!(e, LoadoutError::FusePartner { .. })));
    l.actives[0].fuse = Some("seal".to_string());
    assert!(has(&l, |e| matches!(e, LoadoutError::FusePartner { .. })));
    l.actives[0].fuse = Some("bolt".to_string());
    assert_eq!(check_in(&l, POOL), Ok(()));
    l.actives[0].supports.clear();
    assert!(has(&l, |e| matches!(e, LoadoutError::FuseUnneeded { .. })));
}

// ---- conversions through resolve ----

#[test]
fn each_phase_conversion_moves_half_of_force() {
    for (id, to) in [
        ("fireforge", Element::Fire),
        ("metalwind", Element::Metal),
        ("waterintent", Element::Water),
        ("thunderlead", Element::Thunder),
        ("woodgrowth", Element::Wood),
    ] {
        let b = made("palm", &[id], &[]);
        let o = land(&b.hit);
        let whole = BASE.might * 1.0;
        close(o.dealt[to.index()], whole * sp::PHASE_SHARE);
        assert!(o.dealt[Element::Force.index()] > 0.0, "{id}");
        assert!(o.dealt[Element::Force.index()] < whole * 0.5 + 1e-9, "{id}");
    }
}

#[test]
fn demonize_turns_every_earlier_kind_to_yin() {
    for id in ["palm", "spark", "bolt", "thirdeye"] {
        let b = made(id, &["demonize"], &[]);
        let o = land(&b.hit);
        let total: f64 = o.dealt.iter().sum();
        assert!(total > 0.0);
        close(o.dealt[Element::Yin.index()], total);
    }
    // soul is after yin: it stays soul
    let o = land(&made("bodyfix", &["demonize"], &[]).hit);
    assert!(o.dealt[Element::Yin.index()] == 0.0 && o.dealt[Element::Soul.index()] > 0.0);
}

#[test]
fn cold_rewrites_the_base_never_converting_backward() {
    let b = made("spark", &["coldqi"], &[]);
    assert!(b.hit.convert.iter().flatten().all(|&c| c == 0.0));
    let o = land(&b.hit);
    close(o.dealt[Element::Water.index()], 50.0);
    close(o.dealt[Element::Fire.index()], 50.0);
    let o = land(&made("bolt", &["coldqi"], &[]).hit);
    close(o.dealt[Element::Water.index()], 50.0);
    close(o.dealt[Element::Thunder.index()], 50.0);
}

#[test]
fn heavy_adds_earth_that_conversions_see() {
    let b = made("palm", &["heavy"], &[]);
    close(b.windup, 0.5 * sp::HEAVY_WINDUP);
    let o = land(&b.hit);
    close(
        o.dealt[Element::Earth.index()],
        BASE.might * sp::HEAVY_ADDED,
    );
    let o = land(&made("palm", &["heavy", "demonize"], &[]).hit);
    close(
        o.dealt[Element::Yin.index()],
        BASE.might * (1.0 + sp::HEAVY_ADDED),
    );
}

// ---- build ----

#[test]
fn an_attack_is_might_times_effectiveness_a_spell_its_level() {
    let mut c = Cultivator::new();
    c.might = 500.0;
    let b = build(pool("sword"), 1, &[], &c, &EMPTY, &Situation::default());
    close(b.hit.base[Element::Force.index()], 600.0);
    close(b.hit.effectiveness, 1.2);
    let b = build(pool("bolt"), 10, &[], &c, &EMPTY, &Situation::default());
    close(b.hit.base[Element::Thunder.index()], spell_base(100.0, 10));
    let b = build(pool("ward"), 10, &[], &c, &EMPTY, &Situation::default());
    assert!(b.hit.base.iter().all(|&x| x == 0.0));
}

#[test]
fn cost_multipliers_multiply_and_timings_follow_supports() {
    let b = made("palm", &["focus", "afterimage", "surehit"], &[]);
    close(b.costs[0].1, 20.0 * 1.2 * 1.5 * 1.2);
    assert_eq!(b.strikes, 2);
    close(b.windup, 0.5 * sp::SURE_WINDUP);
    assert!(!b.hit.evadable);
    assert_eq!(b.hit.more, vec![sp::FOCUS_MORE, sp::AFTERIMAGE_MORE]);
    let b = made("bolt", &["multishot"], &[]);
    assert_eq!(b.projectiles, 2);
    assert!(b.hit.evadable);
    let b = made("bolt", &["ninechanges"], &[]);
    close(b.windup, 0.5 * sp::CHANGES_WINDUP);
}

#[test]
fn rule_raises_a_rank_and_costs_more() {
    let b = made("sword", &["rule"], &[]);
    assert_eq!(b.rank, Rank::Divine);
    assert_eq!(b.hit.grade, Grade::Ranked(Rank::Divine));
    close(b.costs[0].1, 28.0);
}

#[test]
fn blood_is_paid_in_shares_of_life_and_strengthens() {
    let b = made("palm", &["bloodspit"], &[]);
    close(b.life, 0.1 * BASE.life);
    close(b.hit.more[0], BLOOD_POWER * 10.0);
    assert!(b.behaviours.contains(&Behaviour::PierceLayer));
    // never the last point
    let mut c = Cultivator::new();
    c.life = 101.0;
    let b = build(
        pool("palm"),
        1,
        &[sup("bloodspit")],
        &c,
        &EMPTY,
        &Situation::default(),
    );
    close(b.life, 100.0);
    // Huyết Luyện pays the qi in life
    let mut g = *pool("bell");
    g.split = FORCE;
    g.tags = &[Tag::Attack, Tag::Treasure, Tag::Melee];
    let b = build(
        &g,
        1,
        &[sup("bloodforge")],
        &Cultivator::new(),
        &EMPTY,
        &Situation::default(),
    );
    assert!(b.costs.iter().all(|c| c.0 != Resource::Qi));
    close(b.life, 20.0 * sp::BLOOD_FORGE_QI * BASE.life);
}

#[test]
fn crit_pen_ailment_and_fire_supports_reach_the_hit() {
    let b = made("palm", &["critical", "pierce", "malignant", "drain"], &[]);
    close(b.hit.crit, BASE.crit + sp::CRITICAL);
    close(b.hit.crit_multi, BASE.crit_multi + sp::CRITICAL_MULTI);
    close(b.hit.armour_pen, sp::PIERCE_ARMOUR);
    // Ác Tính: 50% increased effect of every ailment, read per ailment
    for a in Ailment::ALL {
        close(b.hit.ailment_effect[a.index()], 1.0 + sp::MALIGNANT_AILMENT);
    }
    close(b.hit.leech, sp::DRAIN_LEECH);
    let b = made("spark", &["breakresist", "hollowfire"], &[]);
    close(b.hit.pen[Element::Fire.index()], sp::BREAK_RESIST);
    close(b.hit.pen[Element::Force.index()], 0.0);
    assert_eq!(b.hit.fire_tier, sp::HOLLOW_TIER);
    assert!(b.hit.hollow_fire);
    // fire's tier by level
    let c = Cultivator::new();
    let s = Situation::default();
    assert_eq!(
        build(pool("spark"), 7, &[], &c, &EMPTY, &s).hit.fire_tier,
        0
    );
    assert_eq!(
        build(pool("spark"), 8, &[], &c, &EMPTY, &s).hit.fire_tier,
        1
    );
    assert_eq!(
        build(pool("spark"), 14, &[], &c, &EMPTY, &s).hit.fire_tier,
        2
    );
    // an instant is never evadable
    assert!(made("thirdeye", &[], &[]).hit.instant);
}

#[test]
fn situational_supports_read_the_situation() {
    let c = Cultivator::new();
    let sit = Situation {
        turns: 12,
        ..Situation::default()
    };
    let b = build(pool("nineturn"), 1, &[sup("nineturns")], &c, &EMPTY, &sit);
    assert_eq!(b.hit.more.len(), sp::TURNS_MAX as usize);
    let sit = Situation {
        travelled: 3.0,
        ..Situation::default()
    };
    let b = build(pool("bolt"), 1, &[sup("spiritshift")], &c, &EMPTY, &sit);
    close(b.hit.more[0], 0.3);
    let sit = Situation {
        travelled: 30.0,
        ..Situation::default()
    };
    let b = build(pool("bolt"), 1, &[sup("spiritshift")], &c, &EMPTY, &sit);
    close(b.hit.more[0], sp::SHIFT_MORE_MAX);
    let sit = Situation {
        target_guarded: true,
        ..Situation::default()
    };
    close(
        build(pool("bodyfix"), 1, &[sup("bait")], &c, &EMPTY, &sit).control,
        sp::BAIT_CONTROL,
    );
    close(made("bodyfix", &["bait"], &[]).control, 1.0);
}

// ---- keystones ----

#[test]
fn every_keystone_applies_at_the_start() {
    for k in Keystone::ALL {
        let mut c = Cultivator::new().with_root(Element::Fire);
        // its lasting stats through the sheet, its rules through `apply`
        let sheet = Sheet::with_keystones(&[k]);
        sheet.apply(&mut c);
        k.apply(&mut c);
        assert!(!k.name().is_empty() && !k.gain().is_empty() && !k.cost().is_empty());
        let base = Cultivator::new().with_root(Element::Fire);
        match k {
            Keystone::AncientGod => close(c.qi_max, ks::ANCIENT_QI_MAX),
            Keystone::BlackHair => {
                close(c.life_max, BASE.life * ks::BLACK_HAIR_STATS);
                close(c.life, c.life_max);
                close(c.shield_max, BASE.shield * ks::BLACK_HAIR_STATS);
                close(c.might, BASE.might * ks::BLACK_HAIR_STATS);
                // armour is read per blow from the table
                let armour = sheet.table(&Ctx::alone(&c), []).get(Stat::Armour, c.armour);
                close(armour, BASE.armour * ks::BLACK_HAIR_STATS);
            }
            Keystone::Mastery => {
                assert_eq!(c.immune, Some(Element::Fire));
                close(c.root_weak, ks::MASTERY_WEAK);
            }
            Keystone::Unyielding => {
                close(c.life_max, BASE.life * ks::UNYIELDING_LIFE);
                close(c.life, c.life_max);
            }
            Keystone::Fury => {
                close(c.shield_max, BASE.shield * ks::FURY_SHIELD);
                close(c.shield, c.shield_max);
            }
            Keystone::BornCursed => {
                assert!(c.ageless);
                c.lifespan = 0.0;
                assert!(!c.old());
            }
            _ => assert_eq!(c, base, "{k:?} changes nothing at the start"),
        }
    }
    // Hệ Đại Thành without a root does nothing
    let mut c = Cultivator::new();
    Keystone::Mastery.apply(&mut c);
    assert_eq!(c.immune, None);
}

#[test]
fn mastery_makes_the_root_immune_and_its_bane_harsher() {
    let mut t = bare().with_root(Element::Fire);
    Keystone::Mastery.apply(&mut t);
    let o = resolve(
        &Hit::split(100.0, &[(Element::Fire, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Fire.index()], 0.0);
    let o = resolve(
        &Hit::split(100.0, &[(Element::Water, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Water.index()], 150.0);
}

#[test]
fn keystones_shape_the_blows_they_strike() {
    let b = made("spark", &[], &[Keystone::Extreme(Element::Fire)]);
    close(b.hit.more_by[Element::Fire.index()], ks::EXTREME_MORE);
    close(
        land(&b.hit).dealt[Element::Fire.index()],
        100.0 * (1.0 + ks::EXTREME_MORE),
    );

    let o = land(&made("spark", &[], &[Keystone::ColdQi]).hit);
    close(o.dealt[Element::Water.index()], 100.0);
    close(o.dealt[Element::Fire.index()], 0.0);

    assert_eq!(
        made("bolt", &[], &[Keystone::ImmortalPool]).hit.more,
        vec![ks::POOL_MORE]
    );
    assert!(
        made("palm", &[], &[Keystone::ImmortalPool])
            .hit
            .more
            .is_empty()
    );
    assert_eq!(
        made("palm", &[], &[Keystone::TwentyYears]).hit.more,
        vec![ks::TWENTY_YEARS_MORE]
    );
    assert!(
        made("sword", &["rule"], &[Keystone::TwentyYears])
            .hit
            .more
            .is_empty()
    );

    let mut target = bare();
    target.resist[Element::Thunder.index()] = 0.5;
    target.resist[Element::Fire.index()] = 0.4;
    let sit = Situation {
        target: Some(&target),
        ..Situation::default()
    };
    let c = Cultivator::new();
    let b = build(
        pool("bolt"),
        1,
        &[],
        &c,
        &Sheet::with_keystones(&[Keystone::ThunderDragon]),
        &sit,
    );
    close(b.hit.pen[Element::Thunder.index()], 0.5);
    let b = build(
        pool("spark"),
        1,
        &[],
        &c,
        &Sheet::with_keystones(&[Keystone::BlackHair]),
        &sit,
    );
    close(b.hit.pen[Element::Fire.index()], 0.4);
    close(b.hit.armour_pen, 1.0);
    let mut t = target.clone();
    close(
        resolve(&b.hit, None, &mut t, NO_CRIT).dealt[Element::Fire.index()],
        100.0,
    );

    let sit = Situation {
        fury: 9,
        vortices: 3,
        lives_used: 1,
        streak: 4,
        snuffed: true,
        ..Situation::default()
    };
    let one = |id: &str, k: Keystone| {
        build(pool(id), 1, &[], &c, &Sheet::with_keystones(&[k]), &sit)
            .hit
            .more
    };
    assert_eq!(one("qiburst", Keystone::Fury), vec![ks::FURY_MORE * 5.0]);
    assert_eq!(
        one("qiburst", Keystone::DaoBody),
        vec![ks::VORTEX_MORE * 3.0]
    );
    assert!(one("palm", Keystone::DaoBody).is_empty());
    close(
        build(
            pool("qiburst"),
            1,
            &[],
            &c,
            &Sheet::with_keystones(&[Keystone::DaoBody]),
            &sit,
        )
        .windup,
        0.5 * ks::VORTEX_WINDUP,
    );
    assert_eq!(
        one("palm", Keystone::ThreeLives),
        vec![-ks::THREE_LIVES_LESS]
    );
    assert_eq!(one("palm", Keystone::SoulLamps), vec![ks::SOUL_LAMP_SNUFF]);
    let mut fist = *pool("palm");
    fist.tags = &[Tag::Attack, Tag::Fist, Tag::Melee];
    assert_eq!(
        build(
            &fist,
            1,
            &[],
            &c,
            &Sheet::with_keystones(&[Keystone::Faith]),
            &sit
        )
        .hit
        .more,
        vec![ks::FAITH_MORE * 4.0]
    );
}

#[test]
fn keystones_shape_the_blows_they_take() {
    let mut h = Hit::split(100.0, &[(Element::Thunder, 1.0)]);
    close(Keystone::ColdQi.on_taken(&mut h), 0.0);
    close(h.base[Element::Water.index()], 50.0);
    close(h.base[Element::Thunder.index()], 50.0);

    let mut h = Hit::split(100.0, &[(Element::Thunder, 1.0)]);
    close(
        Keystone::ThunderDragon.on_taken(&mut h),
        100.0 * ks::DRAGON_QI,
    );
    close(h.base[Element::Thunder.index()], 0.0);

    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    h.grade = Grade::Ranked(Rank::Rule);
    Keystone::SlaughterImmortal.on_taken(&mut h);
    close(h.armour_pen, ks::SLAUGHTER_PIERCE);
    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    Keystone::SlaughterImmortal.on_taken(&mut h);
    close(h.armour_pen, 0.0);
    assert_eq!(
        Keystone::brands(3),
        (3.0 * ks::SLAUGHTER_BRAND_ARMOUR, false)
    );
    assert!(Keystone::brands(ks::SLAUGHTER_BRANDS_MAX + 1).1);

    // Người Xem Cây: a time blow taken while moving lands harder, a lasting modifier on its sheet
    let watcher = Sheet::with_keystones(&[Keystone::TreeWatcher]);
    let land_on = |moving: bool, tags: Tags| {
        let mut h = Hit::split(100.0, &[(Element::Fire, 1.0)]);
        h.tags = tags;
        let def = Defence {
            sheet: &watcher,
            moving,
            ..Defence::default()
        };
        resolve_with(&h, None, &mut bare(), &def, NO_CRIT).dealt[Element::Fire.index()]
    };
    let time = Tags::of(&[Tag::Time]);
    close(land_on(false, time), 100.0);
    close(land_on(true, time), 100.0 * (1.0 + ks::TREE_TIME_TAKEN));
    close(land_on(true, Tags::NONE), 100.0);
}

#[test]
fn keystones_spare_a_death_by_their_count() {
    let mut c = Cultivator::new();
    c.life = 0.0;
    assert!(Keystone::Unyielding.on_death(&mut c, 0));
    close(c.life, 1.0);
    c.life = 0.0;
    assert!(!Keystone::Unyielding.on_death(&mut c, 1));
    assert!(Keystone::ThreeLives.on_death(&mut c, 1));
    close(c.life, ks::THREE_LIVES_REVIVE * BASE.life);
    assert!(!Keystone::ThreeLives.on_death(&mut c, ks::THREE_LIVES));
    assert!(Keystone::SoulLamps.on_death(&mut c, ks::SOUL_LAMPS - 1));
    assert!(!Keystone::SoulLamps.on_death(&mut c, ks::SOUL_LAMPS));
    assert!(!Keystone::Fury.on_death(&mut c, 0));
}

#[test]
fn every_keystone_has_its_own_name_and_none_is_frozen() {
    for (i, k) in Keystone::ALL.iter().enumerate() {
        assert!(
            Keystone::ALL[..i]
                .iter()
                .all(|o| o.id() != k.id() && !o.same(*k))
        );
        let text = format!("{} {} {}", k.name(), k.gain(), k.cost()).to_lowercase();
        assert!(
            ["freeze", "frost", "chill", "ice ", "băng"]
                .iter()
                .all(|w| !text.contains(w)),
            "{k:?}"
        );
    }
    assert_eq!(Keystone::ALL.len(), 20);
}

// ---- random loadouts ----

fn dice(seed: u64) -> impl FnMut() -> f64 {
    let mut s = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    move || {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn random_loadouts_are_legal_and_replay() {
    let mut drawn = 0;
    for seed in 0..300 {
        let a = random_in(POOL, &mut dice(seed));
        let b = random_in(POOL, &mut dice(seed));
        assert_eq!(a, b, "seed {seed}");
        if let Some(l) = a {
            assert_eq!(check_in(&l, POOL), Ok(()), "seed {seed}: {l:?}");
            assert!(points_in(&l, POOL) <= BUDGET);
            drawn += 1;
        }
    }
    assert!(drawn > 250, "{drawn}");
}

#[test]
fn the_shared_pool_draws_legal_loadouts_once_it_is_filled() {
    if GEMS.is_empty() {
        return;
    }
    for seed in 0..50 {
        if let Some(l) = random(&mut dice(seed)) {
            assert_eq!(check(&l), Ok(()), "seed {seed}");
        }
    }
}

/// A drawn Cực Cảnh is on a kind the loadout deals (its actives with their supports, or the basics' force).
#[test]
fn a_drawn_extreme_takes_a_kind_the_loadout_deals() {
    let mut extremes = 0;
    for seed in 0..2000 {
        let Some(l) = random(&mut dice(seed)) else {
            continue;
        };
        let dealt = arena_calc::loadout::dealt_kinds(&l, GEMS);
        for k in &l.keystones {
            if let Keystone::Extreme(e) = *k {
                extremes += 1;
                assert!(dealt[e.index()], "seed {seed}: {e:?} in {l:?}");
                assert_eq!(check(&l), Ok(()), "seed {seed}");
            }
        }
    }
    assert!(extremes > 20, "{extremes}");
}

#[test]
fn tags_used_by_supports_are_real() {
    for s in SUPPORTS {
        for t in s.all_of.iter().chain(s.any_of).chain(s.none_of) {
            assert!(Tag::ALL.contains(t));
        }
        assert!(s.kinds().iter().all(|e| e.index() < N));
    }
}

// ---- the PoE supports (design, 8, the second table), on skills of the shared pool ----

/// `gem` of the shared pool at level 1 with `supports`, by a fresh striker, in `sit`.
fn on(gem: &str, supports: &[&str], sit: &Situation) -> Built {
    let s: Vec<&Support> = supports.iter().map(|i| sup(i)).collect();
    let g = arena_calc::gem::gem(gem).unwrap_or(&GEMS[0]);
    build(g, 1, &s, &Cultivator::new(), &EMPTY, sit)
}

fn plain(gem: &str, supports: &[&str]) -> Built {
    on(gem, supports, &Situation::default())
}

fn has_more(b: &Built, v: f64) -> bool {
    b.hit.more.iter().any(|m| (m - v).abs() < 1e-9)
}

fn base_of(gem: &str) -> f64 {
    plain(gem, &[]).hit.base.iter().sum()
}

#[test]
fn every_support_fits_some_skill_of_the_pool() {
    for s in SUPPORTS {
        assert!(
            GEMS.iter()
                .any(|g| !arena_calc::loadout::BASICS.contains(&g.id) && s.fits(g.all_tags())),
            "{} fits no skill a loadout can take",
            s.id
        );
    }
}

#[test]
fn every_support_changes_the_built_skill() {
    for s in SUPPORTS {
        let Some(g) = GEMS
            .iter()
            .find(|g| g.deals_damage() && s.fits(g.all_tags()))
            .or_else(|| GEMS.iter().find(|g| s.fits(g.all_tags())))
        else {
            continue;
        };
        let without = build(g, 1, &[], &Cultivator::new(), &EMPTY, &Situation::default());
        let with = build(
            g,
            1,
            &[s],
            &Cultivator::new(),
            &EMPTY,
            &Situation::default(),
        );
        assert!(with.inert.is_empty(), "{} inert on {}", s.id, g.id);
        assert_ne!(with, without, "{} changes nothing on {}", s.id, g.id);
    }
}

#[test]
fn every_exclusion_is_refused_both_ways_by_the_loadout() {
    let mut pairs = 0;
    for s in SUPPORTS {
        for &x in s.excludes {
            let o = sup(x);
            assert_ne!(o.id, "", "{} excludes unknown {x}", s.id);
            assert_ne!(o.id, s.id, "{} excludes itself", s.id);
            assert!(excluded(s, o) && excluded(o, s));
            let g = GEMS
                .iter()
                .find(|g| s.fits(g.all_tags()) && o.fits(g.all_tags()));
            let Some(g) = g else {
                // the two never fit one skill: nothing to refuse
                continue;
            };
            let l = random(&mut dice(1)).unwrap_or_default();
            let mut l = Loadout {
                actives: l.actives,
                ..l
            };
            if l.actives.is_empty() {
                continue;
            }
            l.actives[0] = slot(g.id, &[s.id, x]);
            let errs = check(&l).err().unwrap_or_default();
            assert!(
                errs.iter()
                    .any(|e| matches!(e, LoadoutError::Excluded { .. })),
                "{} and {x} on {}",
                s.id,
                g.id
            );
            pairs += 1;
        }
    }
    assert!(pairs >= 40, "{pairs}");
}

#[test]
fn every_support_is_drawn_by_random() {
    let mut seen: Vec<&str> = Vec::new();
    for seed in 0..4000 {
        if let Some(l) = random(&mut dice(seed)) {
            for s in l.actives.iter().flat_map(|a| a.supports.iter()) {
                if let Some(x) = SUPPORTS.iter().find(|x| x.id == s)
                    && !seen.contains(&x.id)
                {
                    seen.push(x.id);
                }
            }
        }
        if seen.len() == SUPPORTS.len() {
            break;
        }
    }
    let missing: Vec<&str> = SUPPORTS
        .iter()
        .map(|s| s.id)
        .filter(|id| !seen.contains(id))
        .collect();
    assert!(missing.is_empty(), "never drawn: {missing:?}");
}

#[test]
fn added_damage_supports_add_a_quarter_of_the_base_of_their_kind() {
    let kinds = [
        ("addedfire", Element::Fire),
        ("addedwater", Element::Water),
        ("addedthunder", Element::Thunder),
        ("addedyin", Element::Yin),
        ("addedmetal", Element::Metal),
        ("addedwood", Element::Wood),
        ("addedearth", Element::Earth),
        ("addedlight", Element::Light),
        ("addedsoul", Element::Soul),
    ];
    for (id, e) in kinds {
        for gem in ["fireball", "palm"] {
            let b = plain(gem, &[id]);
            close(
                b.hit.added[e.index()] * b.hit.effectiveness,
                sp::ADDED_SHARE * base_of(gem),
            );
            assert!(sup(id).kinds().contains(&e));
        }
    }
    // a body skill takes none
    assert!(!sup("addedfire").fits(pool("heal").all_tags()));
}

#[test]
fn focus_and_destruction_supports() {
    // Ngũ Hành Ngưng: more, and no ailment laid at all
    let b = plain("fireball", &["elementalfocus"]);
    assert!(has_more(&b, sp::ELEMENTAL_FOCUS_MORE));
    assert!(b.hit.ailment_effect.iter().all(|&e| e == 0.0));
    let mut t = bare();
    let o = resolve(&b.hit, None, &mut t, NO_CRIT);
    assert!(o.dots.is_empty() && o.stacks.iter().all(|&n| n == 0.0));
    assert_eq!(t.ailments.stacks(Ailment::Burn), 0.0);
    // Khống Diệt: spell more, no crit
    let b = plain("fireball", &["controlleddestruction"]);
    assert!(has_more(&b, sp::CONTROLLED_MORE));
    close(b.hit.crit, 0.0);
    // Tụ Lực and Khoách Vực: the area
    let b = plain("quake", &["concentrated"]);
    assert!(has_more(&b, sp::CONCENTRATED_MORE));
    close(b.table.get(Stat::Area, 1.0), 1.0 + sp::CONCENTRATED_AREA);
    close(
        plain("quake", &["widearea"]).table.get(Stat::Area, 1.0),
        1.0 + sp::WIDE_AREA,
    );
    // Thuần Lực: force more, and force made fire deals nothing
    let b = plain("palm", &["brutality", "fireforge"]);
    close(b.hit.more_by[Element::Force.index()], sp::BRUTALITY_MORE);
    close(b.hit.more_by[Element::Fire.index()], -1.0);
    let o = land(&b.hit);
    close(o.before[Element::Fire.index()], 0.0);
    close(
        o.before[Element::Force.index()],
        0.5 * BASE.might * pool_gem("palm").base * (1.0 + sp::BRUTALITY_MORE),
    );
}

fn pool_gem(id: &str) -> &'static Gem {
    arena_calc::gem::gem(id).unwrap_or(&GEMS[0])
}

#[test]
fn projectile_supports() {
    let t = |id: &str, s: Stat, base: f64| plain("fireball", &[id]).table.get(s, base);
    close(t("piercing", Stat::Pierce, 0.0), sp::PIERCE_COUNT);
    close(t("fork", Stat::Fork, 0.0), sp::FORK_COUNT);
    close(t("chain", Stat::Chain, 0.0), sp::CHAIN_COUNT);
    close(t("fasterprojectiles", Stat::ProjectileSpeed, 1.0), 1.5);
    close(t("slowerprojectiles", Stat::ProjectileSpeed, 1.0), 0.7);
    assert!(has_more(&plain("fireball", &["chain"]), sp::CHAIN_MORE));
    assert!(has_more(
        &plain("fireball", &["slowerprojectiles"]),
        sp::SLOWER_SHOTS_MORE
    ));
    assert_eq!(plain("fireball", &["lessermultiple"]).projectiles, 1);
    assert!(excluded(sup("lessermultiple"), sup("multishot")));
    // Cận Xạ: more near, less far, nothing between
    let at = |id: &str, d: f64| {
        on(
            "fireball",
            &[id],
            &Situation {
                distance: d,
                ..Situation::default()
            },
        )
    };
    assert_eq!(at("pointblank", 1.0).hit.more, vec![sp::POINT_BLANK_MORE]);
    assert_eq!(at("pointblank", 10.0).hit.more, vec![sp::POINT_BLANK_LESS]);
    assert!(at("pointblank", 4.0).hit.more.is_empty());
    // Viễn Xạ: more each T, up to its cap
    close(at("farshot", 3.0).hit.more[0], 3.0 * sp::FAR_SHOT_PER_T);
    close(at("farshot", 30.0).hit.more[0], sp::FAR_SHOT_MAX);
}

#[test]
fn melee_supports() {
    let b = plain("palm", &["meleephysical"]);
    close(b.hit.more_by[Element::Force.index()], sp::MELEE_FORCE_MORE);
    close(
        b.windup,
        pool_gem("palm").windup / (1.0 + sp::MELEE_FORCE_SPEED),
    );
    let near = Situation {
        distance: 1.0,
        ..Situation::default()
    };
    assert!(has_more(
        &on("palm", &["closecombat"], &near),
        sp::CLOSE_MORE
    ));
    assert!(plain("palm", &["closecombat"]).hit.more.is_empty());
    // Tàn Nhẫn: every third use
    let nth = |n: u32| Situation {
        use_count: n,
        ..Situation::default()
    };
    let b = on("palm", &["ruthless"], &nth(3));
    assert!(has_more(&b, sp::RUTHLESS_MORE));
    close(
        b.hit.ailment_effect[Ailment::Daze.index()],
        1.0 + sp::RUTHLESS_DAZE,
    );
    assert!(on("palm", &["ruthless"], &nth(2)).hit.more.is_empty());
    // Chiến Quyền: every fourth, wider
    let b = on("palm", &["fistofwar"], &nth(8));
    assert!(has_more(&b, sp::FIST_MORE));
    close(b.table.get(Stat::Area, 1.0), 1.0 + sp::FIST_AREA);
    assert!(on("palm", &["fistofwar"], &nth(3)).hit.more.is_empty());
    // Lưu Lực, Trảm Tận
    assert!(
        plain("palm", &["impale"])
            .behaviours
            .contains(&Behaviour::StoredForce {
                share: sp::STORED_FORCE,
                delay: sp::STORED_DELAY,
            })
    );
    close(plain("palm", &["culling"]).hit.execute, sp::CULLING);
    // Phế Cước: the blow maims its target
    let b = plain("palm", &["maim"]);
    assert_eq!(b.hit.debuffs.len(), 1);
    let mut t = bare();
    resolve(&b.hit, None, &mut t, NO_CRIT);
    let taken = t.bearer_table(&EMPTY, false);
    close(taken.get(Stat::MoveSpeed, 1.0), 1.0 + sp::MAIM_SLOW);
    close(
        taken.get(Stat::TakenOf(Element::Force), 1.0),
        1.0 + sp::MAIM_TAKEN,
    );
    // Tốc Kích, Tam Liên Ảnh
    close(
        plain("palm", &["fasterattacks"]).windup,
        pool_gem("palm").windup / (1.0 + sp::FASTER_ATTACKS),
    );
    let b = plain("palm", &["triplestrike"]);
    assert_eq!(b.strikes, 3);
    assert!(has_more(&b, sp::TRIPLE_MORE));
}

#[test]
fn spell_supports() {
    let fb = pool_gem("fireball");
    close(
        plain("fireball", &["fastercasting"]).windup,
        fb.windup / (1.0 + sp::FASTER_CASTING),
    );
    let b = plain("fireball", &["spellecho"]);
    assert_eq!(b.strikes, 2);
    assert!(has_more(&b, sp::ECHO_MORE));
    close(b.windup, fb.windup / (1.0 + sp::ECHO_SPEED));
    // Linh Triều: only after spending its qi
    let spent = Situation {
        counts: arena_calc::stat::Counts {
            qi_spent: sp::SURGE_QI,
            ..Default::default()
        },
        ..Situation::default()
    };
    assert!(has_more(
        &on("fireball", &["arcanesurge"], &spent),
        sp::SURGE_MORE
    ));
    assert!(plain("fireball", &["arcanesurge"]).hit.more.is_empty());
    // Hiệu Lực: more, and more again over time
    let b = plain("fireball", &["efficacy"]);
    assert!(has_more(&b, sp::EFFICACY_MORE));
    close(
        b.hit.dot[Ailment::Burn.index()].more,
        (1.0 + sp::EFFICACY_MORE) * (1.0 + sp::EFFICACY_DOT),
    );
    // Linh Cảm: cheaper, and crit per mind star up to its cap
    let mut c = Cultivator::new();
    c.stars[arena_calc::cultivator::Star::Mind as usize] = 2.0;
    let b = build(
        fb,
        1,
        &[sup("inspiration")],
        &c,
        &EMPTY,
        &Situation::default(),
    );
    let star = arena_calc::cultivator::RES.star_crit;
    close(
        b.hit.crit,
        (BASE.crit + 2.0 * star) * (1.0 + 2.0 * sp::INSPIRATION_CRIT),
    );
    let qi = |b: &Built| {
        b.costs
            .iter()
            .find(|x| x.0 == Resource::Qi)
            .map_or(0.0, |x| x.1)
    };
    close(qi(&b), qi(&plain("fireball", &[])) * sp::INSPIRATION_COST);
}

#[test]
fn damage_over_time_and_ailment_supports() {
    close(
        plain("fireball", &["burningdamage"]).hit.dot[Ailment::Burn.index()].more,
        1.0 + sp::BURNING_MORE,
    );
    // Hỏa Thiêu Thân: fire added only on a burning target
    let mut burning = bare();
    burning.ailments.add(Ailment::Burn, 1.0, 1.0);
    let at = |t: &Cultivator| {
        on(
            "palm",
            &["immolate"],
            &Situation {
                target: Some(t),
                ..Situation::default()
            },
        )
    };
    let b = at(&burning);
    close(
        b.hit.added[Element::Fire.index()] * b.hit.effectiveness,
        sp::IMMOLATE_ADDED * base_of("palm"),
    );
    close(at(&bare()).hit.added[Element::Fire.index()], 0.0);
    let b = plain("palm", &["swiftaffliction"]);
    close(b.hit.dot[Ailment::Bleed.index()].more, 1.0 + sp::SWIFT_DOT);
    close(
        b.hit.ailment_duration[Ailment::Bleed.index()],
        1.0 + sp::SWIFT_DURATION,
    );
    let b = plain("palm", &["deadlyailments"]);
    assert_eq!(b.hit.more, vec![sp::DEADLY_HIT]);
    close(b.hit.dot[Ailment::Bleed.index()].more, 1.0 + sp::DEADLY_DOT);
    // Tàn Khốc: a hit worth a tenth of the target's life burns the whole more
    let b = plain("fireball", &["cruelty"]);
    close(b.hit.cruelty, sp::CRUELTY_MORE);
    let mut hit = Hit::split(1000.0, &[(Element::Fire, 1.0)]);
    let plain_burn = resolve(&hit, None, &mut bare(), NO_CRIT).dots[0].dps;
    hit.cruelty = sp::CRUELTY_MORE;
    let cruel = resolve(&hit, None, &mut bare(), NO_CRIT).dots[0].dps;
    close(cruel, plain_burn * (1.0 + sp::CRUELTY_MORE));
    // a hit of a twentieth, half of it
    let mut small = Hit::split(150.0, &[(Element::Fire, 1.0)]);
    let plain_small = resolve(&small, None, &mut bare(), NO_CRIT).dots[0].dps;
    small.cruelty = sp::CRUELTY_MORE;
    let cruel_small = resolve(&small, None, &mut bare(), NO_CRIT).dots[0].dps;
    close(cruel_small, plain_small * (1.0 + 0.5 * sp::CRUELTY_MORE));
    // Khát Huyết: more each bleed, up to its cap
    let mut t = bare();
    for _ in 0..3 {
        t.ailments.add(Ailment::Bleed, 1.0, 1.0);
    }
    let sit = Situation {
        target: Some(&t),
        ..Situation::default()
    };
    close(
        on("palm", &["bloodlust"], &sit).hit.more[0],
        3.0 * sp::BLOODLUST_PER,
    );
    close(
        plain("palm", &["witheringtouch"]).hit.inflict[Ailment::Wither.index()],
        sp::WITHER_ON_HIT,
    );
    // Hư Không: yin more, the phases less
    let b = plain("seize", &["voidmanipulation"]);
    close(b.hit.more_by[Element::Yin.index()], sp::VOID_MORE);
    close(b.hit.more_by[Element::Fire.index()], sp::VOID_LESS);
    // Nhiên Thiêu: its burn marks the target's fire resistance down
    let b = plain("fireball", &["combustion"]);
    let mut t = bare();
    resolve(&b.hit, None, &mut t, NO_CRIT);
    assert!(
        t.effects
            .iter()
            .any(|e| e.source == Source::Support("combustion"))
    );
    t.stars = [0.0; 3]; // the blow's body star raises every resistance; look at the mark alone
    close(
        t.bearer_table(&EMPTY, false)
            .flat(Stat::Resist(Element::Fire)),
        sp::COMBUSTION_RESIST,
    );
    // Tăng Ý: more and tighter with each blow lately, up to three
    let hits = Situation {
        counts: arena_calc::stat::Counts {
            hits: 5,
            ..Default::default()
        },
        ..Situation::default()
    };
    let b = on("quake", &["intensify"], &hits);
    close(b.hit.more[0], 3.0 * sp::INTENSIFY_MORE);
    close(b.table.get(Stat::Area, 1.0), 1.0 + 3.0 * sp::INTENSIFY_AREA);
    let b = plain("fireball", &["unleash"]);
    assert!(b.behaviours.contains(&Behaviour::Seals {
        max: sp::UNLEASH_SEALS,
        seconds: sp::UNLEASH_SECONDS,
    }));
    close(
        plain("bolt", &["overcharge"]).hit.ailment_effect[Ailment::Jolt.index()],
        1.0 + sp::OVERCHARGE_JOLT,
    );
    // Thủy Phệ: more on a soaked target, soak laid harder (never a chill)
    let mut soaked = bare();
    soaked.ailments.add(Ailment::Soak, 1.0, 1.0);
    let b = on(
        "jet",
        &["soaked"],
        &Situation {
            target: Some(&soaked),
            ..Situation::default()
        },
    );
    assert!(has_more(&b, sp::SOAKED_MORE));
    close(
        b.hit.ailment_effect[Ailment::Soak.index()],
        1.0 + sp::SOAKED_EFFECT,
    );
    assert!(plain("jet", &["soaked"]).hit.more.is_empty());
}

#[test]
fn crit_and_star_supports() {
    use arena_calc::cultivator::Star;
    use arena_calc::stat::Happened;
    close(
        plain("palm", &["critdamage"]).hit.crit_multi,
        BASE.crit_multi + sp::CRIT_DAMAGE,
    );
    let b = plain("fireball", &["powercharge"]);
    assert_eq!(
        b.hit.star_on,
        vec![(Happened::Crit, Star::Mind, sp::POWER_ON_CRIT)]
    );
    close(b.hit.crit, BASE.crit * (1.0 + sp::POWER_CRIT));
    // Thần Tinh Dẫn: a stun earns a body star
    let b = plain("palm", &["endurancecharge"]);
    let mut hit = b.hit.clone();
    hit.base[Element::Force.index()] = 3000.0;
    let star = Rolls {
        star: 0.5,
        ..NO_CRIT
    };
    let o = resolve(&hit, None, &mut bare(), star);
    assert!(!o.controls.is_empty());
    assert_eq!(o.stars, vec![Star::Body]);
    // a roll of 1 never earns one
    assert!(resolve(&hit, None, &mut bare(), NO_CRIT).stars.is_empty());
    // Yêu Tinh Dẫn: only against a foe with a soul
    let mut puppet = bare();
    puppet.soulless_body = true;
    let foe = bare();
    let at = |t: &Cultivator| {
        on(
            "palm",
            &["frenzycharge"],
            &Situation {
                target: Some(t),
                ..Situation::default()
            },
        )
        .hit
        .star_on
    };
    assert_eq!(
        at(&foe),
        vec![(Happened::Hit, Star::Speed, sp::FRENZY_ON_HIT)]
    );
    assert!(at(&puppet).is_empty());
    // Tam Tài: more each kind dealt lately
    let kinds = Situation {
        counts: arena_calc::stat::Counts {
            kinds: 0b111,
            ..Default::default()
        },
        ..Situation::default()
    };
    close(
        on("fireball", &["trinity"], &kinds).hit.more[0],
        3.0 * sp::TRINITY_PER,
    );
}

#[test]
fn recovery_curse_and_intent_supports() {
    close(
        plain("palm", &["lifeonhit"]).hit.on_hit.life,
        sp::LIFE_ON_HIT,
    );
    close(plain("palm", &["qileech"]).hit.qi_leech, sp::QI_LEECH);
    assert!(
        plain("palm", &["lifeonkill"])
            .behaviours
            .contains(&Behaviour::LifeOnKill {
                share: sp::LIFE_ON_KILL
            })
    );
    let b = plain("hex", &["hextouch"]);
    assert!(b.behaviours.contains(&Behaviour::CurseOnHit));
    close(
        b.table.get(Stat::CurseEffect, 1.0),
        1.0 + sp::HEXTOUCH_EFFECT,
    );
    let b = plain("hex", &["blasphemy"]);
    assert!(b.behaviours.contains(&Behaviour::CurseAura));
    close(b.reserve, sp::BLASPHEMY_SENSE);
    // Chú Vực with Khai Ngộ holds less
    close(
        plain("hex", &["blasphemy", "enlighten"]).reserve,
        sp::BLASPHEMY_SENSE * (1.0 + sp::ENLIGHTEN),
    );
    close(
        plain("summonrain", &["enlighten"]).reservation,
        1.0 + sp::ENLIGHTEN,
    );
    let b = plain("summonrain", &["generosity"]);
    assert!(b.behaviours.contains(&Behaviour::NotOnSelf));
    close(b.table.get(Stat::AuraEffect, 1.0), 1.0 + sp::GENEROSITY);
}

#[test]
fn device_and_minion_supports() {
    for (id, tag, count) in [
        ("trap", Tag::Trap, Stat::TrapCount),
        ("mine", Tag::Mine, Stat::MineCount),
        ("totem", Tag::Totem, Stat::TotemCount),
    ] {
        let b = plain("fireball", &[id]);
        assert!(b.hit.tags.has(tag), "{id}");
        assert!(!plain("fireball", &[]).hit.tags.has(tag));
        close(b.table.get(count, 0.0), sp::DEVICES);
        assert!(b.behaviours.contains(&Behaviour::Device { kind: tag }));
        // a melee blow is never a device
        assert!(!sup(id).fits(pool_gem("palm").all_tags()));
    }
    assert!(has_more(&plain("fireball", &["totem"]), sp::TOTEM_MORE));
    // the tag it takes on reaches the mods that need it
    let trapped = Mod::more(Stat::Damage, 0.5, Source::Other("trap mod")).needs(&[Tag::Trap]);
    let sheet = Sheet {
        extra: vec![trapped],
        ..Sheet::default()
    };
    let b = build(
        pool_gem("fireball"),
        1,
        &[sup("trap")],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    assert!(has_more(&b, 0.5));
    close(
        plain("beans", &["miniondamage"])
            .table
            .get(Stat::MinionDamage, 1.0),
        1.0 + sp::MINION_DAMAGE,
    );
    close(
        plain("beans", &["minionlife"])
            .table
            .get(Stat::MinionLife, 1.0),
        1.0 + sp::MINION_LIFE,
    );
    assert!(
        plain("fireball", &["phantasm"])
            .behaviours
            .contains(&Behaviour::SummonPhantasm {
                chance: sp::PHANTASM
            })
    );
}

#[test]
fn trigger_supports_cast_the_skill_they_are_hung_on() {
    use arena_calc::event::{
        CAST_ON_CRIT, CAST_ON_LOW_LIFE, CAST_ON_SHIELD_BREAK, CAST_WHEN_HURT, Effect,
    };
    let skill = Skill::of(pool_gem("fireball"), 7, 0.0);
    for (id, t) in [
        ("critcast", CAST_ON_CRIT),
        ("hurtcast", CAST_WHEN_HURT),
        ("lowlifecast", CAST_ON_LOW_LIFE),
    ] {
        let got = sup(id).trigger_for(&skill);
        assert!(got.is_some(), "{id} has no trigger");
        let Some(got) = got else { continue };
        assert_eq!(got.id, t.id);
        assert_eq!(got.on, t.on);
        let cast = match got.does {
            Effect::Cast(c) => Some((c.skill, c.level)),
            _ => None,
        };
        assert_eq!(cast, Some(("fireball", 7)), "{id}");
    }
    // Liên Hoàn Suy Yếu hangs on a guard, which deals nothing: it keeps its linked attack, at the guard's level
    let guard = Skill::of(pool_gem("ward"), 7, 0.0);
    let got = sup("chainweaken").trigger_for(&guard);
    assert_eq!(got.map(|t| t.id), Some(CAST_ON_SHIELD_BREAK.id));
    let cast = got.and_then(|t| match t.does {
        Effect::Cast(c) => Some((c.skill, c.level)),
        _ => None,
    });
    assert_eq!(cast, Some(("palm", 7)));
    for id in ["critcast", "hurtcast", "lowlifecast", "channelcast"] {
        assert!(
            plain("fireball", &[id])
                .behaviours
                .contains(&Behaviour::Triggered)
        );
        assert!(
            !sup(id).fits(pool_gem("void").all_tags()),
            "{id} on a channel"
        );
    }
    assert!(sup("focus").trigger_for(&skill).is_none());
}

#[test]
fn lifetap_pays_the_qi_in_life_for_more() {
    let b = plain("fireball", &["lifetap"]);
    let qi = pool_gem("fireball")
        .costs
        .iter()
        .find(|c| c.0 == Resource::Qi)
        .map_or(0.0, |c| c.1);
    assert!(b.costs.iter().all(|c| c.0 != Resource::Qi));
    close(b.life, qi * arena_calc::build::LIFE_PER_QI * BASE.life);
    assert!(has_more(&b, sp::LIFETAP_MORE));
    for x in ["bloodforge", "bloodspit"] {
        assert!(excluded(sup("lifetap"), sup(x)));
    }
}

#[test]
fn outburst_counts_each_curse_up_to_its_cap() {
    let at = |n: u8| {
        on(
            "fireball",
            &["outburst"],
            &Situation {
                target_curses: n,
                ..Situation::default()
            },
        )
        .hit
        .more
    };
    assert!(at(0).is_empty());
    assert_eq!(at(1), vec![sp::OUTBURST_MORE]);
    close(at(2)[0], 2.0 * sp::OUTBURST_MORE);
    close(at(3)[0], sp::OUTBURST_MAX);
}

//! Golden cases (`docs/specs/2026-11-17-damage-system-design.md`, 17.7): the hardest PoE-style situations, each built
//! through the public API (a sheet with items, passive nodes and keystones; skills from `GEMS`; supports from
//! `SUPPORTS`; curses, blessings and ailments already on the target; `build`, `resolve_with`, `tick_with` and the
//! event queue), each with its numbers worked out by hand in the comments, step by step: base, added, conversion,
//! increased and more, crit, fusion, root and the five phases, mitigation, frame, bars, ailments and damage over time.
//!
//! The spells are cast at level 1, where `spell_base(base, 1) = base`; their bases come from today's damage power
//! (`gem::spell_base_of`): Hỏa Cầu `Bf = 221.4130293…`, Thủy Tiễn `Bj = 258.3152008…`, Lôi `Bb = 239.8641151…`,
//! Ngưng Tuyền Hồn `Bs = 258.3152008…`, Diệt Chỉ `Bm = 387.4728013…`. Each case states its expectation as a multiple
//! of the base and checks the number too. Attacks swing bare hands (might 400) or a weapon, so their numbers are round.

use arena_calc::ailment::ForceAilment;
use arena_calc::ailment::{AIL, Ailment, Control};
use arena_calc::build::{Built, Situation, build};
use arena_calc::cultivator::{Active, BASE, Cultivator, Star};
use arena_calc::curse::{self, Curse, Curses, Kind, Lay};
use arena_calc::dot::{BLEED_RATE, BURN_RATE, DOT_CAP, Dot, OVERCOME_REMAINING};
use arena_calc::element::{Element, FUSE, Fusion, N, ROOT};
use arena_calc::event::{
    self, Blow, CAST_ON_CRIT, Cast, Dice, ON_HIT_VS_CURSED, Queue, Seat, events_of,
};
use arena_calc::gem::{GEMS, Gem, Tier, effectiveness_of, gem};
use arena_calc::grade::Rank;
use arena_calc::hit::{Defence, Outcome, Rolls, defence_table, resolve_with, spell_base};
use arena_calc::item::{ArmourBase, Item, ItemBase, ItemSlot, WeaponBase};
use arena_calc::keystone::Keystone;
use arena_calc::passive::{Node, Tree};
use arena_calc::sheet::{EMPTY, Sheet};
use arena_calc::stat::{Condition, Happened, Mod, Source, Stat};
use arena_calc::support::{SUPPORTS, Support};
use arena_calc::tag::Tag;

const NO_CRIT: Rolls = Rolls {
    evade: 1.0,
    crit: 1.0,
    ..Rolls::NONE
};

const CRIT: Rolls = Rolls {
    evade: 1.0,
    crit: 0.0,
    ..Rolls::NONE
};

/// Equal to a part in a billion (or 1e-9 near zero).
fn close(a: f64, b: f64) {
    assert!(
        (a - b).abs() <= 1e-9 * b.abs().max(1.0),
        "{a} != {b} (off by {})",
        a - b
    );
}

/// The design's worked examples strike with the sword blow the basics were before the arena went spells only
/// (Chém: force, a sword at arm's length, base 5): a fixture here, no longer a skill of the pool.
static SLASH: Gem = Gem {
    id: "slash",
    name: "Chém",
    tags: &[Tag::Attack, Tag::Sword, Tag::Near, Tag::Melee, Tag::Strike],
    split: &[(Element::Force, 1.0)],
    rank: Rank::Art,
    restriction: false,
    base: effectiveness_of(5.0),
    costs: &[],
    cooldown: 0.7,
    windup: 0.2,
    recover: 0.32,
    uses: None,
    bluff: false,
    force: ForceAilment::Bleed,
    tier: Tier::C,
    spell: "slash",
    source: "arena",
    effects: &[],
};

fn pool(id: &str) -> &'static Gem {
    if id == "slash" {
        return &SLASH;
    }
    gem(id).unwrap_or(&GEMS[0])
}

fn sup(id: &str) -> &'static Support {
    SUPPORTS
        .iter()
        .find(|s| s.id == id)
        .unwrap_or(&Support::NONE)
}

fn fire() -> usize {
    Element::Fire.index()
}

/// Hỏa Cầu's level-1 base.
fn bf() -> f64 {
    pool("fireball").base
}

/// No armour, no shield: what lands is what step 10 made, straight to life.
fn bare() -> Cultivator {
    let mut c = Cultivator::new();
    c.armour = 0.0;
    c.shield = 0.0;
    c.shield_max = 0.0;
    c
}

/// `gem` at level 1 with `supports`, by `striker` wearing `sheet`, aimed at `target`.
fn cast(
    gem: &str,
    supports: &[&str],
    striker: &Cultivator,
    sheet: &Sheet,
    target: &Cultivator,
) -> Built {
    let sups: Vec<&Support> = supports.iter().map(|s| sup(s)).collect();
    let sit = Situation {
        target: Some(target),
        ..Situation::default()
    };
    build(pool(gem), 1, &sups, striker, sheet, &sit)
}

fn land(b: &Built, by: &Cultivator, target: &mut Cultivator, rolls: Rolls) -> Outcome {
    resolve_with(&b.hit, Some(by), target, &Defence::default(), rolls)
}

fn burn_dps(c: &Cultivator) -> Vec<f64> {
    c.ailments.burns.0.iter().map(|d| d.dps).collect()
}

// ---- 1 and 15: Hỏa Cầu + Liên Châu + Độc Tâm + Thời Gian Kéo Dài, blessed, on a bound and marked target ----

/// The blessing "Hỏa Ý": 30% increased fire damage for 10 s.
const BLESSING: Source = Source::Blessing("hoa y");

/// Case 1, as one run: the striker blessed, the target bound (2 stacks of Triền) and marked (Khắc Ấn on fire, laid
/// by the curse rules and synced onto it), a fireball with three supports, one projectile resolved.
fn case_one() -> (Built, Outcome, Cultivator, Cultivator) {
    let mut striker = Cultivator::new();
    striker.put_on(Active {
        source: BLESSING,
        mods: vec![Mod::inc(Stat::DamageOf(Element::Fire), 0.3, BLESSING)],
        left: Some(10.0),
    });
    let mut target = bare();
    target.ailments.add(Ailment::Bind, 2.0, 1.0);
    let mut on_target = Curses::new(1, false);
    let mut on_striker = Curses::new(0, false);
    let laid = curse::cast(
        Curse::new(Kind::ExposeMark(Element::Fire), Rank::Art, 0, 1),
        &mut on_target,
        &mut on_striker,
    );
    assert_eq!(laid, Lay::Added);
    on_target.sync(&mut target, false);
    let b = cast(
        "fireball",
        &["multishot", "venomheart", "lingering"],
        &striker,
        &EMPTY,
        &target,
    );
    let o = land(&b, &striker, &mut target, NO_CRIT);
    (b, o, striker, target)
}

#[test]
fn case_01_fireball_volley_with_dot_supports_on_a_bound_marked_target() {
    let (b, o, striker, target) = case_one();
    let b_f = bf();
    // Liên Châu: two more projectiles (three in all), each one resolved on its own
    assert_eq!(b.projectiles, 2);
    // step 4, the hit: base Bf × (1 + 0.3 blessing's fire) × (1 − 0.3 Liên Châu) = 0.91 Bf = 201.4858567
    close(o.before[fire()], 0.91 * b_f);
    close(o.before[fire()], 201.485_856_694_170_3);
    // step 8: the curse's −20% fire resistance reached the defence table by itself (synced from `Curses`), no hand:
    // resistance 0 − 0.2 = −0.2, so the fire part takes × 1.2: 1.092 Bf = 241.7830280
    close(o.dealt[fire()], 1.092 * b_f);
    close(o.dealt[fire()], 241.783_028_033_004_4);
    close(target.life, BASE.life - 1.092 * b_f);
    // that is over 8% of full life (240): the target earned a body star, +4% resistances from now on, so its table
    // after the blow reads −0.2 + 0.04 = −0.16, the curse's modifier still in it
    close(target.stars[Star::Body as usize], 1.0);
    let t = defence_table(&target, Some(&striker), &b.hit, &Defence::default());
    close(t.get(Stat::Resist(Element::Fire), 0.0), -0.16);
    assert!(
        t.used
            .iter()
            .any(|m| m.source == Source::Curse("exposemark"))
    );
    // step 12, the burn: rate 0.5 × the part 0.91 Bf (step 4 with the DoT mods: the blessing's fire increase and
    // Liên Châu's "less" both reach it) × (1 + 0.3 Độc Tâm) × fed 1.5 (wood feeds fire: the target is bound)
    // = 0.88725 Bf = 196.4487103 a second, for 4 × (1 + 0.5 Thời Gian Kéo Dài) = 6 s
    assert_eq!(o.dots.len(), 1);
    close(
        o.dots[0].dps,
        BURN_RATE * 0.91 * b_f * 1.3 * (1.0 + AIL.fed),
    );
    close(o.dots[0].dps, 196.448_710_276_816_06);
    close(o.dots[0].time, 6.0);
    close(target.ailments.stacks(Ailment::Burn), 1.0);
}

// ---- 2: a bleed build ----

#[test]
fn case_02_bleed_build_presses_eight_bleeds_on_a_running_target() {
    let striker = Cultivator::new();
    // four bleeds: Thừa Thắng (35% more against 5 or more) sleeps. Chém bare-handed: 400 × 0.5 = 200 force
    let mut four = bare();
    for dps in [10.0, 20.0, 30.0, 40.0] {
        four.ailments.lay(Dot::new(Ailment::Bleed, dps, 5.0));
    }
    let b = cast("slash", &["pressadvantage"], &striker, &EMPTY, &four);
    assert!(b.hit.more.is_empty());
    close(land(&b, &striker, &mut four, NO_CRIT).dealt[0], 200.0);

    // seven bleeds of 10..70 a second: Thừa Thắng wakes, 200 × 1.35 = 270 (armour 0: H / (H + 5D) = 0)
    let mut t = bare();
    for dps in [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0] {
        t.ailments.lay(Dot::new(Ailment::Bleed, dps, 5.0));
    }
    let b = cast("slash", &["pressadvantage"], &striker, &EMPTY, &t);
    let o = land(&b, &striker, &mut t, NO_CRIT);
    close(o.dealt[0], 270.0);
    // the eighth bleed: 0.35 × 270 = 94.5 a second (the more is a Damage more: the bleed takes it too), for 5 s;
    // reaching the cap from below is the bleed's full
    close(o.dots[0].dps, BLEED_RATE * 270.0);
    close(o.dots[0].dps, 94.5);
    close(o.dots[0].time, 5.0);
    assert_eq!(o.full, vec![Ailment::Bleed]);
    assert_eq!(t.ailments.bleeds.len(), DOT_CAP);
    let sum = |c: &Cultivator| -> f64 { c.ailments.bleeds.0.iter().map(|d| d.dps).sum() };
    close(sum(&t), 280.0 + 94.5);

    // a ninth at the cap replaces the weakest (10), and is no new full: 374.5 − 10 + 94.5 = 459
    let b = cast("slash", &["pressadvantage"], &striker, &EMPTY, &t);
    let o = land(&b, &striker, &mut t, NO_CRIT);
    assert!(o.full.is_empty());
    assert_eq!(t.ailments.bleeds.len(), DOT_CAP);
    close(sum(&t), 459.0);
    assert!(t.ailments.bleeds.0.iter().all(|d| d.dps >= 20.0));
    // one weaker than all eight is dropped
    t.ailments.lay(Dot::new(Ailment::Bleed, 5.0, 5.0));
    close(sum(&t), 459.0);

    // each 270 blow was over 8% of full life (240): two body stars, damage taken × (1 − 2 × 4%) = 0.92, and that
    // reaches the bleeds too (`Taken` at each tick)
    close(t.stars[Star::Body as usize], 2.0);
    // every bleed bleeds, the same whether the target runs or not (`BLEED_MOVING` 1): 1 s moving 459 × 0.92 =
    // 422.28, then 1 s still 422.28
    let life = t.life;
    let ticked = t.tick_with(1.0, true, &EMPTY);
    close(ticked.lost, arena_calc::dot::BLEED_MOVING * 459.0 * 0.92);
    close(life - t.life, 422.28);
    close(t.tick_with(1.0, false, &EMPTY).lost, 422.28);
}

// ---- 3: force to fire to yin, with an item and a passive node ----

#[test]
fn case_03_force_to_fire_to_yin_with_increases_to_all_three_kinds() {
    let src = Source::Item(ItemSlot::Weapon);
    let mut damage = [0.0; N];
    damage[0] = 100.0;
    let mut sheet = Sheet::default();
    sheet.gear.wear(Item {
        name: "Hỏa Kiếm".into(),
        slot: ItemSlot::Weapon,
        base: ItemBase::Weapon(WeaponBase {
            damage,
            speed: 1.0,
            crit: 0.05,
            reach: 1.0,
        }),
        mods: vec![
            Mod::flat(Stat::Added(Element::Fire), 40.0, src),
            Mod::inc(Stat::DamageOf(Element::Force), 0.2, src),
            Mod::inc(Stat::DamageOf(Element::Fire), 0.3, src),
        ],
    });
    let node = Source::Passive(1);
    sheet.tree = Tree {
        nodes: vec![Node {
            id: 1,
            name: "Âm Hỏa".into(),
            mods: vec![
                Mod::inc(Stat::Damage, 0.1, node),
                Mod::inc(Stat::DamageOf(Element::Yin), 0.4, node),
                Mod::flat(Stat::Convert(Element::Fire, Element::Yin), 0.5, node),
            ],
            keystone: None,
            links: Vec::new(),
        }],
        starts: vec![1],
    };
    sheet.allocated = vec![1];
    assert_eq!(sheet.tree.check(&sheet.allocated, 1), Ok(()));

    let striker = Cultivator::new();
    let mut t = bare();
    let b = cast("slash", &["fireforge"], &striker, &sheet, &t);
    let (force, yin) = (0, Element::Yin.index());
    // step 1: the weapon's 100 force × Chém's 0.5 = 50; step 2: the weapon's 40 fire × 0.5 = 20
    close(b.hit.base[force], 50.0);
    close(b.hit.added[fire()] * b.hit.effectiveness, 20.0);
    // step 3: Hỏa Luyện 50% force → fire, the node 50% fire → yin, forward only
    close(b.hit.convert[force][fire()], 0.5);
    close(b.hit.convert[fire()][yin], 0.5);
    let o = land(&b, &striker, &mut t, NO_CRIT);
    // the parts and every kind each has been:
    //   force 25 (force)              × (1 + 0.1 + 0.2)             = 32.5
    //   fire  10 (fire)               × (1 + 0.1 + 0.3)             = 14
    //   fire  12.5 (force, fire)      × (1 + 0.1 + 0.2 + 0.3)       = 20     → fire 34
    //   yin   10 (fire, yin)          × (1 + 0.1 + 0.3 + 0.4)       = 18
    //   yin   12.5 (force, fire, yin) × (1 + 0.1 + 0.2 + 0.3 + 0.4) = 25     → yin 43
    close(o.dealt[force], 32.5);
    close(o.dealt[fire()], 34.0);
    close(o.dealt[yin], 43.0);
    close(o.total(), 109.5);
    close(t.life, BASE.life - 109.5);
    // the DoT reads the same buckets: a bleed 0.35 × 32.5 = 11.375, a burn 0.5 × 34 = 17
    let bleed = o.dots.iter().find(|d| d.ailment == Ailment::Bleed);
    let burn = o.dots.iter().find(|d| d.ailment == Ailment::Burn);
    close(bleed.map_or(0.0, |d| d.dps), 11.375);
    close(burn.map_or(0.0, |d| d.dps), 17.0);
    // wither from the yin part: 1 × 43 / 3000 × 20 = 0.2866…, a fraction carried
    close(o.stacks[Ailment::Wither.index()], 43.0 / 3000.0 * 20.0);
}

// ---- 4: cast on crit and a buff on hitting a cursed target, through the queue ----

#[test]
fn case_04_cast_on_crit_against_a_cursed_target_through_the_queue() {
    // a target of 10 000 life: no blow here passes 8% of it, so no body star muddles the numbers
    let mut target = bare();
    target.life_max = 10_000.0;
    target.life = 10_000.0;
    let mut fighters = [Cultivator::new(), target];
    let triggers = [CAST_ON_CRIT, ON_HIT_VS_CURSED];
    let seats = [
        Seat {
            triggers: &triggers,
            ..Seat::default()
        },
        Seat {
            cursed: true,
            ..Seat::default()
        },
    ];
    // Chém bare-handed crits (roll 0 < 5%): 200 × 1.5 = 300
    let b = cast("slash", &[], &fighters[0], &EMPTY, &fighters[1]);
    let [striker, target] = &mut fighters;
    let (life, shield) = (target.life, target.shield);
    let o = land(&b, striker, target, CRIT);
    assert!(o.crit);
    close(o.dealt[0], 300.0);
    let blow = Blow {
        by: Some(0),
        on: 1,
        skill: Some("slash"),
        tags: b.hit.tags,
        depth: 0,
        life_before: life,
        shield_before: shield,
    };
    let mut q = Queue::new();
    q.extend(events_of(&o, &blow, target));
    // the rolls: only Nguyền Ấn Truy Kích (50%) draws one; 0.3 fires it. Bạo Kích Dẫn Lôi is sure and draws none
    let rolls = [0.3, 0.9];
    let mut dice = Dice::new(&rolls);
    let out = q.run(&mut fighters, &seats, &mut dice);
    assert_eq!(dice.used(), 1);
    let fired: Vec<&str> = out.fired.iter().map(|f| f.trigger).collect();
    assert_eq!(fired, vec!["cursedhunt", "castoncrit"]);
    assert_eq!(
        out.casts,
        vec![Cast {
            by: 0,
            at: Some(1),
            trigger: "castoncrit",
            skill: "jithunder",
            level: 10,
            factor: 0.8,
            base: None,
            depth: 1,
        }]
    );
    close(q.cooldown(0, "castoncrit"), 0.25);
    close(fighters[0].recent.since(Happened::Crit), 0.0);
    close(fighters[1].recent.since(Happened::BeenCrit), 0.0);

    // the triggered Cực Lôi at level 10: 15/10 × 432.5 × 500/637.5 = 508.8235294; the buff's 20% increased and the
    // trigger's 0.8 "more": 508.82 × 1.2 × 0.8 = 488.4705882
    let c = out.casts[0];
    let sit = Situation {
        target: Some(&fighters[1]),
        ..Situation::default()
    };
    let mut tb = build(pool(c.skill), c.level, &[], &fighters[0], &EMPTY, &sit);
    tb.hit.more.push(c.factor - 1.0);
    tb.hit.tags = tb.hit.tags.with(Tag::Trigger);
    let j10 = spell_base(pool("jithunder").base, 10);
    close(j10, 508.823_529_411_764_7);
    let [striker, target] = &mut fighters;
    let to = land(&tb, striker, target, NO_CRIT);
    close(to.dealt[Element::Thunder.index()], j10 * 1.2 * c.factor);
    close(to.dealt[Element::Thunder.index()], 488.470_588_235_294_1);
    // its events are a level deeper and carry the trigger tag: no cast on crit, and the 0.9 roll misses the buff
    let blow = Blow {
        skill: Some(c.skill),
        tags: tb.hit.tags,
        depth: c.depth,
        ..blow
    };
    q.extend(events_of(&to, &blow, target));
    let rolls = [0.9];
    let mut dice = Dice::new(&rolls);
    let out = q.run(&mut fighters, &seats, &mut dice);
    assert!(out.fired.is_empty());
    assert_eq!(dice.used(), 1);

    // the next Chém carries the buff: 200 × (1 + 0.2) = 240
    let b = cast("slash", &[], &fighters[0], &EMPTY, &fighters[1]);
    close(b.hit.increased, 0.2);
    let [striker, target] = &mut fighters;
    close(land(&b, striker, target, NO_CRIT).dealt[0], 240.0);
}

// ---- 5: fusion, and water cashing in the burn ----

#[test]
fn case_05_fire_then_water_makes_wind_and_cashes_in_the_burn_left() {
    let striker = Cultivator::new();
    let mut t = bare();
    t.resist[fire()] = 0.5;
    t.resist[Element::Water.index()] = 0.5;
    let (b_f, b_j) = (bf(), pool("jet").base);
    // Hỏa Cầu: Bf fire, half resisted: 0.5 Bf = 110.7065147; a burn of 0.5 Bf a second for 4 s; Bf in the window
    let o = land(
        &cast("fireball", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[fire()], 0.5 * b_f);
    close(t.in_window(Element::Fire), b_f);
    // half a second: the burn 0.5 Bf × 0.5 s × (1 − 0.5) = 0.125 Bf = 27.6766287; 3.5 s left on it
    close(t.tick_with(0.5, false, &EMPTY).lost, 0.125 * b_f);
    close(t.ailments.burns.0[0].left, 3.5);
    // Thủy Tiễn within the second: Bj water
    //  step 6, wind: 0.4 × min(Bf in the window, Bj) = 0.4 Bf = 88.5652117, the fire spent; it passes resistance
    //  step 7, water overcomes the burn: cleared, cashing in what it would still burn, 0.5 Bf × 3.5 s = 1.75 Bf
    //          (× OVERCOME_REMAINING 1): water Bj + 1.75 Bf = 645.7880022
    //  step 8: water × (1 − 0.5) = 0.5 Bj + 0.875 Bf = 322.8940011; wind × taken 1 only
    let o = land(
        &cast("jet", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    assert_eq!(o.cleared, vec![(Ailment::Burn, 1.0)]);
    let water = Element::Water.index();
    close(o.before[water], b_j + 0.5 * b_f * 3.5 * OVERCOME_REMAINING);
    close(o.before[water], 645.788_002_224_904_9);
    close(o.dealt[water], 322.894_001_112_452_43);
    assert_eq!(o.fusion.map(|f| f.0), Some(Fusion::Wind));
    close(o.fusion.map_or(0.0, |f| f.1), FUSE.share * b_f);
    close(o.fusion.map_or(0.0, |f| f.1), 88.565_211_733_701_25);
    close(t.in_window(Element::Fire), 0.0);
    close(t.wind_lock, FUSE.wind_lock);
    assert!(t.ailments.burns.is_empty());
    close(t.tick_with(1.0, false, &EMPTY).lost, 0.0);

    // the exploit closed: eight tiny fire blows of 1 lay eight burns of 0.5 a second; water now cashes in only the
    // strongest's 0.5 × 4 = 2 (the old rule paid 8 × 2% × 3000 = 480)
    let mut t = bare();
    for _ in 0..8 {
        let mut tiny = cast("fireball", &[], &striker, &EMPTY, &t);
        tiny.hit.base[fire()] = 1.0;
        land(&tiny, &striker, &mut t, NO_CRIT);
    }
    assert_eq!(t.ailments.burns.len(), 8);
    t.window.clear();
    let o = land(
        &cast("jet", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    assert_eq!(o.cleared, vec![(Ailment::Burn, 8.0)]);
    close(o.before[water], b_j + 2.0);
}

// ---- 6: thunder conducts through soak, jolt raises what it takes ----

#[test]
fn case_06_thunder_on_a_soaked_jolted_target() {
    let striker = Cultivator::new();
    let mut t = bare();
    t.ailments.add(Ailment::Soak, 4.0, 1.0);
    t.ailments.add(Ailment::Jolt, 3.0, 1.0);
    let b_b = pool("bolt").base;
    let o = land(
        &cast("bolt", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    let th = Element::Thunder.index();
    // step 7, conduct: soak cleared, 4 stacks × 2% × 3000 = 240 more thunder: Bb + 240 = 479.8641151
    assert_eq!(o.cleared, vec![(Ailment::Soak, 4.0)]);
    close(o.before[th], b_b + 4.0 * AIL.overcome * BASE.life);
    close(o.before[th], 479.864_115_112_107_6);
    // step 8 reads the table after step 7: soak's thunder taken is gone, the 3 jolt stacks give 1 + 3 × 4% = 1.12
    close(o.dealt[th], 1.12 * (b_b + 240.0));
    close(o.dealt[th], 537.447_808_925_560_6);
    // step 12: jolt 1 × 479.86 / 3000 × 20 = 3.1990941 stacks on 3: the fifth fills, a jolt of 0.3 s (no grit yet)
    close(
        o.stacks[Ailment::Jolt.index()],
        (b_b + 240.0) / 3000.0 * 20.0,
    );
    assert_eq!(o.full, vec![Ailment::Jolt]);
    assert_eq!(o.controls, vec![(Control::Jolt, 0.3)]);
    close(t.ailments.stacks(Ailment::Jolt), 0.0);
    close(t.ailments.on[Ailment::Jolt.index()].lock, AIL.lock);
}

// ---- 7: mind stars crit into body stars and armour ----

#[test]
fn case_07_a_mind_star_crit_into_body_stars_and_armour() {
    // the target wears 400 armour: 600 + 400 = 1000; two body stars: damage taken × (1 − 2 × 4%) = 0.92
    let mut def_sheet = Sheet::default();
    def_sheet.gear.wear(Item {
        name: "Huyền Giáp".into(),
        slot: ItemSlot::Body,
        base: ItemBase::Armour(ArmourBase {
            armour: 400.0,
            evasion: 0.0,
            shield: 0.0,
        }),
        mods: Vec::new(),
    });
    let mut t = Cultivator::new();
    def_sheet.apply(&mut t);
    close(t.armour, 1000.0);
    t.stars[Star::Body as usize] = 2.0;

    // three mind stars: crit 5% + 3 × 3% = 14%, multiplier 1.5 + 3 × 0.1 = 1.8. A 0.1 roll crits only with them
    let mut striker = Cultivator::new();
    let plain = cast("slash", &[], &striker, &EMPTY, &t);
    close(plain.hit.crit, 0.05);
    striker.stars[Star::Mind as usize] = 3.0;
    let b = cast("slash", &[], &striker, &EMPTY, &t);
    close(b.hit.crit, 0.14);
    close(b.hit.crit_multi, 1.8);
    let def = Defence {
        sheet: &def_sheet,
        ..Defence::default()
    };
    let o = resolve_with(
        &b.hit,
        Some(&striker),
        &mut t,
        &def,
        Rolls {
            evade: 1.0,
            crit: 0.1,
            ..Rolls::NONE
        },
    );
    assert!(o.crit);
    // step 5: 200 × 1.8 = 360; step 8: armour 1000 / (1000 + 5 × 360) = 0.3571429 off, then × 0.92:
    // 360 × 1800/2800 × 0.92 = 212.9142857, all on the shield (600)
    close(o.dealt[0], 360.0 * (1.0 - 1000.0 / 2800.0) * 0.92);
    close(o.dealt[0], 212.914_285_714_285_7);
    close(o.to_shield, o.dealt[0]);
    close(o.to_life, 0.0);
    // the crit reaches the bleed: 0.35 × 360 = 126 a second
    close(o.dots[0].dps, 126.0);
    // under 8% of full life (240): no new body star
    close(t.stars[Star::Body as usize], 2.0);
}

// ---- 8: the root ----

#[test]
fn case_08_the_root_and_dao_fire() {
    let striker = Cultivator::new();
    let (b_f, b_j) = (bf(), pool("jet").base);
    // water on a fire root: × 1.25 = 322.8940011
    let mut t = bare().with_root(Element::Fire);
    let o = land(
        &cast("jet", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Water.index()], ROOT.overcomes * b_j);
    close(o.dealt[Element::Water.index()], 322.894_001_112_452_43);
    // fire on a fire root: × 0.7 = 154.9891205, the target takes 10% of it back as qi (50 + 15.4989121); the burn
    // is cut with it: 0.5 × 0.7 Bf = 77.4945603
    let mut t = bare().with_root(Element::Fire);
    t.qi = 50.0;
    let o = land(
        &cast("fireball", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[fire()], 0.7 * b_f);
    close(o.dealt[fire()], 154.989_120_533_977_17);
    close(o.target_qi, 0.07 * b_f);
    close(t.qi, 65.498_912_053_397_72);
    close(o.dots[0].dps, 0.35 * b_f);
    // light on a yin root: Liên Hoa bare-handed 400 × 1.8 = 720, × 1.5 = 1080
    let mut t = bare().with_root(Element::Yin);
    let o = land(
        &cast("lotus", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Light.index()], 1080.0);
    // fire on a water root: purple fire (Hư Hỏa, tier 2) is overcome, × 0.8 = 177.1304235, its burn 0.5 × 0.8 Bf
    // × 1.5 (purple) = 0.6 Bf; black fire (tier 3, dao fire) overcomes the water root, × 1.25 = 276.7662867, its
    // burn 0.5 × 1.25 Bf × 2 (black) = 1.25 Bf
    let mut t = bare().with_root(Element::Water);
    let purple = cast("fireball", &["hollowfire"], &striker, &EMPTY, &t);
    assert_eq!(purple.hit.fire_tier, 2);
    let o = land(&purple, &striker, &mut t.clone(), NO_CRIT);
    close(o.dealt[fire()], 0.8 * b_f);
    close(o.dealt[fire()], 177.130_423_467_402_5);
    close(o.dots[0].dps, 0.6 * b_f);
    let mut black = purple.clone();
    black.hit.fire_tier = 3;
    let o = land(&black, &striker, &mut t, NO_CRIT);
    close(o.dealt[fire()], 1.25 * b_f);
    close(o.dealt[fire()], 276.766_286_667_816_4);
    close(o.dots[0].dps, 1.25 * b_f);
}

// ---- 9: ignite ----

#[test]
fn case_09_only_the_strongest_burn_burns_then_the_weak_one_takes_over() {
    let striker = Cultivator::new();
    let b_f = bf();
    let mut t = bare();
    // weak: Hỏa Cầu + Thời Gian Kéo Dài, 0.5 Bf a second for 6 s
    let o = land(
        &cast("fireball", &["lingering"], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dots[0].dps, 0.5 * b_f);
    close(o.dots[0].time, 6.0);
    // 1 s of it alone: 0.5 Bf = 110.7065147
    close(t.tick_with(1.0, false, &EMPTY).lost, 0.5 * b_f);
    // strong: Hỏa Cầu + Hỏa Ngục, 0.5 × 1.5 = 0.75 Bf = 166.0597720 a second for 4 s
    t.window.clear();
    land(
        &cast("fireball", &["inferno"], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    let dps = burn_dps(&t);
    assert_eq!(dps.len(), 2);
    close(dps[1], 0.75 * b_f);
    close(
        t.ailments.burns.strongest().map_or(0.0, |d| d.dps),
        0.75 * b_f,
    );
    // 4.5 s: the strong one burns its 4 s (3 Bf), then the weak one its next 0.5 s (0.25 Bf): 3.25 Bf = 719.5923453
    close(t.tick_with(4.5, false, &EMPTY).lost, 3.25 * b_f);
    assert_eq!(t.ailments.burns.len(), 1);
    close(t.ailments.burns.0[0].left, 0.5);
    // the weak one's last 0.5 s: 0.25 Bf = 55.3532573, then nothing
    close(t.tick_with(1.0, false, &EMPTY).lost, 0.25 * b_f);
    close(t.tick_with(1.0, false, &EMPTY).lost, 0.0);
}

// ---- 10: hit damage and damage over time apart ----

#[test]
fn case_10_inferno_burns_more_and_hits_less_and_each_side_scales_alone() {
    let striker = Cultivator::new();
    let b_f = bf();
    // Hỏa Ngục: the hit × 0.75 = 166.0597720; the burn 0.5 × 1.5 = 0.75 Bf
    let mut t = bare();
    let b = cast("fireball", &["inferno"], &striker, &EMPTY, &t);
    assert_eq!(b.hit.more, vec![-0.25]);
    close(b.hit.dot[Ailment::Burn.index()].more, 1.5);
    let o = land(&b, &striker, &mut t, NO_CRIT);
    close(o.dealt[fire()], 0.75 * b_f);
    close(o.dealt[fire()], 166.059_772_000_689_83);
    close(o.dots[0].dps, 0.75 * b_f);
    // a sheet with 40% increased hit damage and 60% increased damage over time: the hit Bf × 1.4 × 0.75 = 1.05 Bf
    // = 232.4836808, untouched by the DoT increase; the burn 0.5 × Bf × 1.6 × 1.5 = 1.2 Bf = 265.6956352,
    // untouched by the hit increase
    let sheet = Sheet {
        extra: vec![
            Mod::inc(Stat::HitDamage, 0.4, Source::Other("hit")),
            Mod::inc(Stat::DotDamage, 0.6, Source::Other("dot")),
        ],
        ..Sheet::default()
    };
    let mut t = bare();
    let b = cast("fireball", &["inferno"], &striker, &sheet, &t);
    let o = land(&b, &striker, &mut t, NO_CRIT);
    close(o.dealt[fire()], 1.05 * b_f);
    close(o.dealt[fire()], 232.483_680_800_965_77);
    close(o.dots[0].dps, 1.2 * b_f);
    close(o.dots[0].dps, 265.695_635_201_103_7);
}

// ---- 11: conditional item and passive modifiers ----

#[test]
fn case_11_conditional_item_and_passive_mods_switch_with_the_moment() {
    let mut sheet = Sheet::default();
    sheet.gear.wear(Item {
        name: "Độc Bội".into(),
        slot: ItemSlot::Amulet,
        base: ItemBase::Trinket,
        mods: vec![
            Mod::more(Stat::Damage, 0.2, Source::Item(ItemSlot::Amulet))
                .when(Condition::TargetAiling),
        ],
    });
    sheet.tree = Tree {
        nodes: vec![Node {
            id: 7,
            name: "Huyết Tẩy".into(),
            mods: vec![
                Mod::inc(Stat::Damage, 0.5, Source::Passive(7))
                    .when(Condition::Recently(Happened::Kill, 4.0)),
            ],
            keystone: None,
            links: Vec::new(),
        }],
        starts: vec![7],
    };
    sheet.allocated = vec![7];
    let mut fighters = [Cultivator::new(), bare()];
    let hit = |f: &[Cultivator; 2]| -> f64 {
        let b = cast("slash", &[], &f[0], &sheet, &f[1]);
        land(&b, &f[0], &mut f[1].clone(), NO_CRIT).dealt[0]
    };
    // nothing holds: Chém 200
    close(hit(&fighters), 200.0);
    // the target burns: the amulet's 20% more, 240
    fighters[1].ailments.lay(Dot::new(Ailment::Burn, 10.0, 4.0));
    close(hit(&fighters), 240.0);
    // a kill (the event marks it): 200 × (1 + 0.5) × 1.2 = 360
    event::mark(
        &[event::Event::new(event::EventKind::Kill, 0)],
        &mut fighters,
    );
    close(hit(&fighters), 360.0);
    // 4.5 s on, the kill is no longer recent: 240
    fighters[0].recent.tick(4.5);
    close(hit(&fighters), 240.0);
}

// ---- 12: shield, yin and soul ----

#[test]
fn case_12_yin_passes_the_shield_soul_hits_the_sense_full_sunder_breaks_it() {
    let striker = Cultivator::new();
    let mut t = Cultivator::new();
    // Phệ Linh bare-handed: 400 × 1.6 = 640 yin, straight to life past the 600 shield;
    // wither 640 / 3000 × 20 = 4.27 stacks
    let o = land(
        &cast("devour", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.to_life, 640.0);
    close(o.to_shield, 0.0);
    close(t.shield, 600.0);
    close(t.life, 2360.0);
    close(t.ailments.stacks(Ailment::Wither), 4.0);
    // 640 is over 8% of full life (240): a body star (4% less taken, 4% resistances). Shattered here, so the next
    // blows read plain
    close(t.stars[Star::Body as usize], 1.0);
    close(t.shatter(Star::Body), 1.0);
    // Ngưng Tuyền Hồn: Bs soul: 100 to the sense, the rest (158.3152009) at half to life: 79.1576004; the sense
    // runs out, the soul is lost
    let b_s = pool("springsoul").base;
    let o = land(
        &cast("springsoul", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.to_sense, 100.0);
    close(o.to_life, (b_s - 100.0) * 0.5);
    close(o.to_life, 79.157_600_444_980_97);
    close(t.sense, 0.0);
    close(t.soul_lost, BASE.soulless);
    close(t.shield, 600.0);
    // Diệt Chỉ: Bm metal on 4 sunder stacks: the shield takes it (600 − 387.4728013 = 212.5271987 left), then
    // 387.47 / 3000 × 20 = 2.58 stacks fill the sunder: the shield breaks, 3 s before it regrows
    t.ailments.add(Ailment::Sunder, 4.0, 1.0);
    let b_m = pool("finger").base;
    let o = land(
        &cast("finger", &[], &striker, &EMPTY, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.to_shield, b_m);
    close(o.to_life, 0.0);
    assert_eq!(o.full, vec![Ailment::Sunder]);
    close(t.shield, 0.0);
    close(t.shield_wait, BASE.shield_delay + BASE.shield_broken);
    close(t.life, 2360.0 - (b_s - 100.0) * 0.5);
}

// ---- 13: keystones ----

#[test]
fn case_13_extreme_immortal_pool_and_unyielding() {
    let striker = Cultivator::new();
    let b_f = bf();
    let sheet = Sheet::with_keystones(&[Keystone::Extreme(Element::Fire), Keystone::ImmortalPool]);
    // Cực Cảnh (fire 60% more) and Tiên Trì (spells 40% more) on a fire spell: Bf × 1.6 × 1.4 = 2.24 Bf
    // = 495.9651857; both are mores of all its damage, so its burn too: 0.5 × 2.24 Bf = 1.12 Bf
    let mut t = bare();
    let o = land(
        &cast("fireball", &[], &striker, &sheet, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[fire()], 2.24 * b_f);
    close(o.dealt[fire()], 495.965_185_708_727);
    close(o.dots[0].dps, 1.12 * b_f);
    // a water spell takes Tiên Trì only: 1.4 Bj = 361.6412812
    let mut t = bare();
    let o = land(
        &cast("jet", &[], &striker, &sheet, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Water.index()], 361.641_281_245_946_7);
    // an attack of force takes neither: 200
    let mut t = bare();
    let o = land(
        &cast("slash", &[], &striker, &sheet, &t),
        &striker,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[0], 200.0);

    // Bất Khuất: 15% less full life (3000 × 0.85 = 2550); once a round a killing blow leaves 1
    let keep = Sheet::with_keystones(&[Keystone::Unyielding]);
    let mut t = Cultivator::new();
    keep.apply(&mut t);
    close(t.life_max, 2550.0);
    t.life = 500.0;
    let o = resolve_with(
        &cast("devour", &[], &striker, &EMPTY, &t).hit,
        Some(&striker),
        &mut t,
        &Defence {
            sheet: &keep,
            ..Defence::default()
        },
        NO_CRIT,
    );
    close(o.to_life, 500.0);
    close(t.life, 0.0);
    assert!(Keystone::Unyielding.on_death(&mut t, 0));
    close(t.life, 1.0);
    t.life = 0.0;
    assert!(!Keystone::Unyielding.on_death(&mut t, 1));
    close(t.life, 0.0);
}

// ---- 14: curse effect from the curser's sheet ----

#[test]
fn case_14_the_cursers_sheet_makes_its_mark_stronger() {
    // a ring's 30% and a node's 20% increased curse effect: 1.5
    let mut curser_sheet = Sheet::default();
    curser_sheet.gear.wear(Item {
        name: "Chú Giới".into(),
        slot: ItemSlot::RingLeft,
        base: ItemBase::Trinket,
        mods: vec![Mod::inc(
            Stat::CurseEffect,
            0.3,
            Source::Item(ItemSlot::RingLeft),
        )],
    });
    curser_sheet.tree = Tree {
        nodes: vec![Node {
            id: 3,
            name: "Nguyền Tâm".into(),
            mods: vec![Mod::inc(Stat::CurseEffect, 0.2, Source::Passive(3))],
            keystone: None,
            links: Vec::new(),
        }],
        starts: vec![3],
    };
    curser_sheet.allocated = vec![3];
    let curser = Cultivator::new();
    let effect = curse::curse_effect(&curser, &curser_sheet);
    close(effect, 1.5);

    let b_f = bf();
    let fire_on = |effect: Option<f64>| -> (f64, f64) {
        let mut t = bare();
        t.resist[fire()] = 0.25;
        let mut on_t = Curses::new(1, false);
        let mut on_c = Curses::new(0, false);
        if let Some(k) = effect {
            let mark = Curse::new(Kind::ExposeMark(Element::Fire), Rank::Art, 0, 1);
            assert_eq!(curse::cast_with(mark, &mut on_t, &mut on_c, k), Lay::Added);
            on_t.sync(&mut t, false);
        }
        let o = land(
            &cast("fireball", &[], &curser, &curser_sheet, &t),
            &curser,
            &mut t,
            NO_CRIT,
        );
        (o.dealt[fire()], on_t.resist(Element::Fire))
    };
    // 25% fire resistance: Bf × 0.75 = 166.0597720
    let (plain, _) = fire_on(None);
    close(plain, 0.75 * b_f);
    // Khắc Ấn at effect 1: 25% − 20% = 5%, × 0.95 = 210.3423779
    let (marked, cut) = fire_on(Some(1.0));
    close(cut, -0.2);
    close(marked, 0.95 * b_f);
    close(marked, 210.342_377_867_540_45);
    // at the sheet's 1.5: −30%, the resistance −5%, × 1.05 = 232.4836808
    let (strong, cut) = fire_on(Some(effect));
    close(cut, -0.3);
    close(strong, 1.05 * b_f);
    close(strong, 232.483_680_800_965_77);
}

// ---- 15: determinism ----

#[test]
fn case_15_the_whole_of_case_one_twice_is_bit_identical() {
    let (b1, o1, s1, t1) = case_one();
    let (b2, o2, s2, t2) = case_one();
    assert_eq!(b1, b2);
    assert_eq!(o1, o2);
    assert_eq!(s1, s2);
    assert_eq!(t1, t2);
    let bits = |o: &Outcome, t: &Cultivator| -> Vec<u64> {
        o.dealt
            .iter()
            .chain(&o.before)
            .chain(o.dots.iter().map(|d| &d.dps))
            .chain([&o.to_life, &o.to_shield, &t.life])
            .map(|x| x.to_bits())
            .collect()
    };
    assert_eq!(bits(&o1, &t1), bits(&o2, &t2));
    // and so is the burn they leave, tick by tick
    let (mut a, mut b) = (t1, t2);
    for _ in 0..8 {
        let (x, y) = (
            a.tick_with(0.75, false, &EMPTY),
            b.tick_with(0.75, false, &EMPTY),
        );
        assert_eq!(x.lost.to_bits(), y.lost.to_bits());
    }
    assert_eq!(a, b);
}

// ---- 16 to 19: the richer modifiers and the PoE supports (design, 8 and 17.1) ----

/// Chém's base with bare hands: might 400 × its effectiveness 0.5 = 200.
fn bs_slash() -> f64 {
    let b = BASE.might * pool("slash").base;
    close(b, 200.0);
    b
}

#[test]
fn case_16_gain_as_extra_after_conversion_carries_both_kinds() {
    // Chém (all force, B = 400 × E) with Hỏa Luyện (50% force to fire), worn: 40% of fire gained as extra thunder,
    // 50% increased force, 20% increased thunder. A bare target (no armour, no shield, no resistance).
    let sheet = Sheet {
        extra: vec![
            Mod::flat(
                Stat::Extra(Element::Fire, Element::Thunder),
                0.4,
                Source::Other("ring"),
            ),
            Mod::inc(Stat::DamageOf(Element::Force), 0.5, Source::Other("ring")),
            Mod::inc(Stat::DamageOf(Element::Thunder), 0.2, Source::Other("ring")),
        ],
        ..Sheet::default()
    };
    let striker = Cultivator::new();
    let mut target = bare();
    let b = cast("slash", &["fireforge"], &striker, &sheet, &target);
    let o = land(&b, &striker, &mut target, NO_CRIT);
    let big = bs_slash();
    // step 3: force B/2 (been force), fire B/2 (been force, fire)
    // then gained: 0.4 × B/2 = 0.2B of thunder, been force, fire and thunder
    // step 4: force 0.5B × 1.5 = 0.75B; fire 0.5B × 1.5 = 0.75B (it was force); thunder 0.2B × (1 + 0.5 + 0.2)
    //         = 0.34B. Nothing mitigates.
    // with B = 200: force 150, fire 150, thunder 68, 368 in all
    close(o.dealt[Element::Force.index()], 150.0);
    close(o.dealt[fire()], 150.0);
    close(o.dealt[Element::Thunder.index()], 68.0);
    close(o.total(), 1.84 * big);
    // the burn is laid from fire's part alone: 0.5 a second of 0.75B = 75
    assert_eq!(burn_dps(&target), vec![BURN_RATE * 0.75 * big]);
    close(burn_dps(&target)[0], 75.0);
}

#[test]
fn case_17_bloodlust_counts_six_bleeds_and_stops_at_its_cap() {
    // Chém with Khát Huyết (8% more each bleed on the target, at most 48%) on a target carrying six bleeds.
    let striker = Cultivator::new();
    let mut target = bare();
    target.ailments.add(Ailment::Bleed, 6.0, 1.0);
    assert_eq!(target.ailments.stacks(Ailment::Bleed), 6.0);
    let b = cast("slash", &["bloodlust"], &striker, &EMPTY, &target);
    // six bleeds: one more of 6 × 0.08 = 0.48
    assert_eq!(b.hit.more.len(), 1);
    close(b.hit.more[0], 0.48);
    let o = land(&b, &striker, &mut target, NO_CRIT);
    let big = bs_slash();
    close(o.dealt[Element::Force.index()], 1.48 * big);
    close(o.dealt[Element::Force.index()], 296.0);
    // the seventh bleed snapshots the same more (a "more" to all damage): 0.35 × 1.48B a second
    assert_eq!(target.ailments.stacks(Ailment::Bleed), 7.0);
    let laid = target.ailments.bleeds.0.last().map_or(0.0, |d| d.dps);
    close(laid, BLEED_RATE * 1.48 * big);
    close(laid, 103.6);
    // eight would be 0.64: the cap holds it at 0.48
    target.ailments.add(Ailment::Bleed, 1.0, 1.0);
    let b = cast("slash", &["bloodlust"], &striker, &EMPTY, &target);
    close(b.hit.more[0], 0.48);
}

#[test]
fn case_18_culling_strike_finishes_what_the_blow_leaves_under_a_tenth() {
    // Chém with Trảm Tận (10%): a bare target of 3000 life dies when the blow (B = 200) leaves it at or under 300.
    let striker = Cultivator::new();
    let big = bs_slash();
    // left at 250: culled, the whole B + 250 to life
    let mut target = bare();
    target.life = big + 250.0;
    let b = cast("slash", &["culling"], &striker, &EMPTY, &target);
    close(b.hit.execute, 0.1);
    let o = land(&b, &striker, &mut target, NO_CRIT);
    assert!(o.executed);
    close(o.to_life, big + 250.0);
    close(o.to_life, 450.0);
    close(target.life, 0.0);
    // left at 301: it lives on 301
    let mut target = bare();
    target.life = big + 301.0;
    let o = land(&b, &striker, &mut target, NO_CRIT);
    assert!(!o.executed);
    close(target.life, 301.0);
}

#[test]
fn case_19_a_totem_casts_for_less_and_takes_the_totem_tag() {
    // Hỏa Cầu on Pháp Đàn (30% less), worn: 50% more for totem skills (it needs the tag the support gives).
    let sheet = Sheet {
        extra: vec![Mod::more(Stat::Damage, 0.5, Source::Other("totem node")).needs(&[Tag::Totem])],
        ..Sheet::default()
    };
    let striker = Cultivator::new();
    let mut target = bare();
    let b = cast("fireball", &["totem"], &striker, &sheet, &target);
    assert!(b.hit.tags.has(Tag::Totem));
    close(b.table.get(Stat::TotemCount, 0.0), 1.0);
    // Bf × 0.7 × 1.5 = 1.05 Bf
    let o = land(&b, &striker, &mut target, NO_CRIT);
    close(o.dealt[fire()], 1.05 * bf());
    // without the node, 0.7 Bf
    let mut target = bare();
    let b = cast("fireball", &["totem"], &striker, &EMPTY, &target);
    let o = land(&b, &striker, &mut target, NO_CRIT);
    close(o.dealt[fire()], 0.7 * bf());
}

// ---- 20 and 21: the field behaviours (design, 17.8) ----

#[test]
fn case_20_a_chained_fireball_through_three_targets() {
    use arena_calc::play::{Point, Target};
    use arena_calc::projectile::{CHAIN_FACTOR, Powers, Shot, Via, trace};
    // Hỏa Cầu with Liên Kích (2 chains, 25% less) at A on the line; B 2.5 T from A, C 3.2 T from A and 3.54 T from
    // B: A, then B (the nearer), then C. Each hop lands Bf × 0.75 × CHAIN_FACTOR^hops on a bare target.
    let striker = Cultivator::new();
    let probe = bare();
    let b = cast("fireball", &["chain"], &striker, &EMPTY, &probe);
    let p = Powers::of_built(&b);
    assert_eq!(p.chain, 2);
    let targets = [
        Target::new(0, 3.0, 0.5, 0.3),
        Target::new(1, 4.5, 2.5, 0.3),
        Target::new(2, 1.0, 3.0, 0.3),
    ];
    let shot = Shot {
        origin: Point::new(0.0, 0.5),
        direction: Point::new(1.0, 0.0),
        speed: 10.0,
        range: 20.0,
    };
    let strikes = trace(&shot, &p, &targets);
    assert_eq!(
        strikes
            .iter()
            .map(|s| (s.target, s.via))
            .collect::<Vec<_>>(),
        [(0, Via::Direct), (1, Via::Chain), (2, Via::Chain)]
    );
    let mut bodies = [bare(), bare(), bare()];
    for (hops, s) in strikes.iter().enumerate() {
        let target = &mut bodies[s.target as usize];
        let o = land_blow(&s.blow(&b.hit), &striker, target);
        let want = 0.75 * bf() * libm::pow(CHAIN_FACTOR, hops as f64);
        close(o.dealt[fire()], want);
        // CHAIN_FACTOR is 1: each hop 0.75 Bf = 166.0597720…
        assert!((o.dealt[fire()] - 166.059_772).abs() < 1e-5);
        close(target.life_max - target.life, want);
    }
}

fn land_blow(hit: &arena_calc::hit::Hit, by: &Cultivator, target: &mut Cultivator) -> Outcome {
    resolve_with(hit, Some(by), target, &Defence::default(), NO_CRIT)
}

#[test]
fn case_21_a_spell_totem_over_three_seconds() {
    use arena_calc::device::{Kind, Placed, spawn};
    use arena_calc::play::Point;
    // Hỏa Cầu on Pháp Đàn: the totem casts its windup (0.26 s) after it stands, then every max(0.26 + 0.24, 1.5) =
    // 1.5 s: at 0.26 and 1.76 within 3 s (3.26 is past). Each cast 0.7 Bf on a bare target: 1.4 Bf in all.
    let striker = Cultivator::new();
    let mut target = bare();
    let b = cast("fireball", &["totem"], &striker, &EMPTY, &target);
    let s = spawn(&b).unwrap_or_else(|| unreachable!("a totem"));
    assert_eq!(s.kind, Kind::Totem);
    let mut placed = Placed::new(s);
    placed.place(Point::new(2.0, 0.0));
    let mut casts = 0;
    let mut dealt = 0.0;
    for _ in 0..30 {
        for _ in placed.tick(0.1).casts {
            casts += 1;
            dealt += land(&b, &striker, &mut target, NO_CRIT).dealt[fire()];
        }
    }
    assert_eq!(casts, 2);
    close(dealt, 1.4 * bf());
    close(target.life_max - target.life, 1.4 * bf());
}

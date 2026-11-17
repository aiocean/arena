//! The modifier engine and every source speaking it (`stat`, `sheet`, `item`, `passive`): the arithmetic, tags and
//! conditions, and each source (items, passive nodes, supports, keystones, curses, intents, domains, stars,
//! ailments) reaching a blow through the gathered table, with nothing added by hand.

use arena_calc::ailment::{AIL, Ailment, ForceAilment};
use arena_calc::aura::{AURA, AURAS, AuraKind, Auras, sync};
use arena_calc::build::{Situation, build};
use arena_calc::cultivator::{Active, BASE, Cultivator, RES, Star};
use arena_calc::curse::{Curse, Curses, Kind, NUMBERS};
use arena_calc::domain::{DOMAINS, DomainKind, Field};
use arena_calc::element::{Element, N};
use arena_calc::gem::{Gem, Resource, Tier};
use arena_calc::grade::Rank;
use arena_calc::hit::{Defence, Hit, Rolls, resolve, resolve_with, scale};
use arena_calc::item::{Gear, Item, ItemBase, ItemSlot, WeaponBase};
use arena_calc::keystone::{self as ks, Keystone};
use arena_calc::passive::{Node, Tree};
use arena_calc::sheet::{EMPTY, Sheet};
use arena_calc::stat::{
    Condition, Counts, Ctx, FIGHT_READS, Happened, Mod, ModKind, Per, Source, Stat, Table,
};
use arena_calc::support::{self as sp, Support, support};
use arena_calc::tag::{Tag, Tags};

const NO_CRIT: Rolls = Rolls {
    evade: 1.0,
    crit: 1.0,
    ..Rolls::NONE
};

const SRC: Source = Source::Other("test");

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn bare() -> Cultivator {
    let mut c = Cultivator::new();
    c.armour = 0.0;
    c.shield = 0.0;
    c.shield_max = 0.0;
    c
}

const fn gem(
    id: &'static str,
    tags: &'static [Tag],
    split: &'static [(Element, f64)],
    base: f64,
) -> Gem {
    Gem {
        id,
        name: id,
        tags,
        split,
        rank: Rank::Art,
        restriction: false,
        base,
        costs: &[(Resource::Qi, 20.0)],
        cooldown: 4.0,
        windup: 0.5,
        recover: 0.4,
        uses: None,
        bluff: false,
        force: ForceAilment::Bleed,
        tier: Tier::C,
        spell: "bolt",
        source: "arena",
        effects: &[],
    }
}

const SPARK: Gem = gem(
    "spark",
    &[Tag::Spell, Tag::Projectile],
    &[(Element::Fire, 1.0)],
    100.0,
);
const PALM: Gem = gem(
    "palm",
    &[Tag::Attack, Tag::Palm, Tag::Melee, Tag::Slam],
    &[(Element::Force, 1.0)],
    1.0,
);

fn sup(id: &str) -> &'static Support {
    support(id).unwrap_or(&Support::NONE)
}

/// Fire dealt by `hit` on a bare target with no sheet.
fn fire(hit: &Hit) -> f64 {
    resolve(hit, None, &mut bare(), NO_CRIT).dealt[Element::Fire.index()]
}

fn gather(mods: &[Mod]) -> Table {
    let c = Cultivator::new();
    Table::gather(mods, &Ctx::alone(&c))
}

// ---- the arithmetic ----

#[test]
fn flat_then_increases_summed_then_each_more_then_an_override() {
    let t = gather(&[
        Mod::flat(Stat::Life, 100.0, SRC),
        Mod::inc(Stat::Life, 0.2, SRC),
        Mod::inc(Stat::Life, 0.3, SRC),
        Mod::more(Stat::Life, 0.5, SRC),
        Mod::more(Stat::Life, -0.2, SRC),
    ]);
    close(t.get(Stat::Life, 900.0), 1000.0 * 1.5 * 1.5 * 0.8);
    close(t.flat(Stat::Life), 100.0);
    close(t.increased(Stat::Life), 0.5);
    close(t.more(Stat::Life), 1.2);
    // untouched stats are their base
    close(t.get(Stat::Shield, 600.0), 600.0);
    // increases never take a stat below zero
    let t = gather(&[Mod::inc(Stat::Armour, -3.0, SRC)]);
    close(t.get(Stat::Armour, 600.0), 0.0);
    // an override wins, the last one gathered
    let t = gather(&[
        Mod::inc(Stat::ArmourPen, 0.5, SRC),
        Mod::new(Stat::ArmourPen, ModKind::Override, 0.7, SRC),
        Mod::new(Stat::ArmourPen, ModKind::Override, 1.0, SRC),
    ]);
    close(t.get(Stat::ArmourPen, 0.2), 1.0);
    // scaling leaves an override alone
    let o = Mod::new(Stat::ArmourPen, ModKind::Override, 1.0, SRC).scaled(0.5);
    close(o.value, 1.0);
    close(Mod::more(Stat::Damage, 0.4, SRC).scaled(1.5).value, 0.6);
}

#[test]
fn combined_stats_sum_increases_and_multiply_mores() {
    let t = gather(&[
        Mod::inc(Stat::Damage, 0.2, SRC),
        Mod::inc(Stat::HitDamage, 0.3, SRC),
        Mod::more(Stat::Damage, 0.5, SRC),
        Mod::more(Stat::HitDamage, 0.2, SRC),
    ]);
    let a = t.combined(&[Stat::Damage, Stat::HitDamage]);
    close(a.increased, 0.5);
    close(a.more, 1.5 * 1.2);
    close(a.factor(), 1.5 * 1.5 * 1.2);
}

// ---- tags and conditions ----

#[test]
fn a_tag_gated_mod_holds_only_for_skills_with_its_tags() {
    let m = Mod::inc(Stat::Damage, 0.5, SRC).needs(&[Tag::Spell, Tag::Fire]);
    let c = Cultivator::new();
    let mut ctx = Ctx::alone(&c);
    ctx.skill = Tags::of(&[Tag::Spell]);
    assert!(!m.applies(&ctx));
    ctx.skill = Tags::of(&[Tag::Spell, Tag::Fire, Tag::Projectile]);
    assert!(m.applies(&ctx));
    // through a build: a sheet's spell-only increase skips an attack
    let sheet = Sheet {
        extra: vec![Mod::inc(Stat::Damage, 0.5, SRC).needs(&[Tag::Spell])],
        ..Sheet::default()
    };
    let c = Cultivator::new();
    let sit = Situation::default();
    close(build(&SPARK, 1, &[], &c, &sheet, &sit).hit.increased, 0.5);
    close(build(&PALM, 1, &[], &c, &sheet, &sit).hit.increased, 0.0);
}

#[test]
fn conditions_read_the_moment() {
    let me = Cultivator::new();
    let mut foe = Cultivator::new();
    let at = |other: &Cultivator, cursed: bool, distance: f64| -> [bool; 6] {
        let ctx = Ctx {
            other: Some(other),
            other_cursed: cursed,
            distance,
            moving: false,
            skill: Tags::NONE,
            ..Ctx::alone(&me)
        };
        [
            Condition::TargetCursed.holds(&ctx),
            Condition::TargetHas(Ailment::Bleed, 5.0).holds(&ctx),
            Condition::TargetAiling.holds(&ctx),
            Condition::TargetLowLife(0.35).holds(&ctx),
            Condition::Near(2.0).holds(&ctx),
            Condition::Far(2.0).holds(&ctx),
        ]
    };
    assert_eq!(
        at(&foe, false, 5.0),
        [false, false, false, false, false, true]
    );
    assert_eq!(
        at(&foe, true, 1.0),
        [true, false, false, false, true, false]
    );
    foe.ailments.add(Ailment::Bleed, 4.0, 1.0);
    assert_eq!(at(&foe, false, 5.0)[1..3], [false, true]);
    foe.ailments.add(Ailment::Bleed, 1.0, 1.0);
    assert!(at(&foe, false, 5.0)[1]);
    foe.life = 0.3 * foe.life_max;
    assert!(at(&foe, false, 5.0)[3]);

    // this fighter's own: low life, recently
    let mut me = Cultivator::new();
    let low = Condition::SelfLowLife(0.5);
    let killed = Condition::Recently(Happened::Kill, 4.0);
    assert!(!low.holds(&Ctx::alone(&me)) && !killed.holds(&Ctx::alone(&me)));
    me.life = 0.5 * me.life_max;
    me.recent.mark(Happened::Kill);
    assert!(low.holds(&Ctx::alone(&me)) && killed.holds(&Ctx::alone(&me)));
    me.recent.tick(4.5);
    assert!(!killed.holds(&Ctx::alone(&me)));
    // combinators
    static BOTH: [Condition; 2] = [Condition::SelfLowLife(0.5), Condition::Moving];
    assert!(!Condition::All(&BOTH).holds(&Ctx::alone(&me)));
    assert!(Condition::Any(&BOTH).holds(&Ctx::alone(&me)));
    assert!(Condition::Not(&Condition::Moving).holds(&Ctx::alone(&me)));
}

#[test]
fn the_conditional_supports_hold_only_when_their_condition_does() {
    let c = Cultivator::new();
    let plain = Situation::default();
    let more = |s: &str, sit: &Situation, striker: &Cultivator| {
        build(&PALM, 1, &[sup(s)], striker, &EMPTY, sit).hit.more
    };
    // Bạo Phát on a cursed target
    assert!(more("outburst", &plain, &c).is_empty());
    let cursed = Situation {
        target_cursed: true,
        ..Situation::default()
    };
    assert_eq!(more("outburst", &cursed, &c), vec![sp::OUTBURST_MORE]);
    // Thừa Thắng on a target bleeding five stacks
    let mut foe = Cultivator::new();
    foe.ailments.add(Ailment::Bleed, 4.0, 1.0);
    let at = Situation {
        target: Some(&foe),
        ..Situation::default()
    };
    assert!(more("pressadvantage", &at, &c).is_empty());
    let mut foe = Cultivator::new();
    foe.ailments.add(Ailment::Bleed, 5.0, 1.0);
    let at = Situation {
        target: Some(&foe),
        ..Situation::default()
    };
    assert_eq!(more("pressadvantage", &at, &c), vec![sp::PRESS_MORE]);
    // Thị Huyết after a kill
    assert!(more("bloodthirst", &plain, &c).is_empty());
    let mut killer = Cultivator::new();
    killer.recent.mark(Happened::Kill);
    killer.recent.tick(sp::BLOODTHIRST_SECONDS - 0.5);
    assert_eq!(
        more("bloodthirst", &plain, &killer),
        vec![sp::BLOODTHIRST_MORE]
    );
}

#[test]
fn the_damage_over_time_supports_reach_the_table_and_the_blow() {
    let c = Cultivator::new();
    let sit = Situation::default();
    // Thời Gian Kéo Dài: every ailment lasts longer
    let b = build(&SPARK, 1, &[sup("lingering")], &c, &EMPTY, &sit);
    for a in Ailment::ALL {
        close(b.hit.ailment_duration[a.index()], 1.0 + sp::LINGER_DURATION);
        close(b.hit.ailment_effect[a.index()], 1.0);
    }
    // Độc Tâm: the damage over time multiplier
    let b = build(&SPARK, 1, &[sup("venomheart")], &c, &EMPTY, &sit);
    close(b.table.get(Stat::DotMulti, 1.0), 1.0 + sp::VENOM_DOT_MULTI);
    assert!(!sup("venomheart").fits(Tags::of(&[Tag::Spell, Tag::Thunder])));
    // Hỏa Ngục Thiêu Đốt: burn more, the hit less
    let b = build(&SPARK, 1, &[sup("inferno")], &c, &EMPTY, &sit);
    assert_eq!(b.hit.more, vec![sp::INFERNO_HIT]);
    close(
        b.table.more(Stat::AilmentDamage(Ailment::Burn)),
        1.0 + sp::INFERNO_BURN,
    );
    close(fire(&b.hit), 100.0 * (1.0 + sp::INFERNO_HIT));
    assert!(!sup("inferno").fits(PALM.all_tags()));
}

// ---- items and passive nodes ----

#[test]
fn an_item_mod_and_a_passive_node_reach_a_blow() {
    let mut gear = Gear::default();
    gear.wear(Item {
        name: "Hỏa Linh Giới".to_string(),
        slot: ItemSlot::RingLeft,
        base: ItemBase::Trinket,
        mods: vec![Mod::inc(
            Stat::Damage,
            0.5,
            Source::Item(ItemSlot::RingLeft),
        )],
    });
    let tree = Tree {
        nodes: vec![Node {
            id: 7,
            name: "Hỏa Mạch".to_string(),
            mods: vec![Mod::inc(
                Stat::DamageOf(Element::Fire),
                0.3,
                Source::Passive(7),
            )],
            keystone: None,
            links: Vec::new(),
        }],
        starts: vec![7],
    };
    let sheet = Sheet {
        gear,
        tree,
        allocated: vec![7],
        ..Sheet::default()
    };
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(b.hit.increased, 0.5);
    close(b.hit.increased_by[Element::Fire.index()], 0.3);
    close(fire(&b.hit), 100.0 * 1.8);
    // unallocated, the node gives nothing
    let sheet = Sheet {
        allocated: Vec::new(),
        ..sheet
    };
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(fire(&b.hit), 150.0);
}

#[test]
fn an_attack_starts_from_its_weapon_and_its_crit() {
    let mut damage = [0.0; N];
    damage[Element::Force.index()] = 200.0;
    damage[Element::Fire.index()] = 50.0;
    let mut gear = Gear::default();
    gear.wear(Item {
        name: "Thanh Phong Kiếm".to_string(),
        slot: ItemSlot::Weapon,
        base: ItemBase::Weapon(WeaponBase {
            damage,
            speed: 1.2,
            crit: 0.1,
            reach: 1.5,
        }),
        mods: Vec::new(),
    });
    let sheet = Sheet {
        gear,
        ..Sheet::default()
    };
    let b = build(
        &PALM,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(b.hit.base[Element::Force.index()], 200.0);
    close(b.hit.base[Element::Fire.index()], 50.0);
    close(b.hit.crit, 0.1);
    // a spell ignores the weapon
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(b.hit.base[Element::Fire.index()], 100.0);
    close(b.hit.crit, BASE.crit);
}

// ---- curses, intents, domains on the target ----

#[test]
fn a_curse_lowers_resistance_at_step_8_without_the_caller_adding_it() {
    let mut target = bare();
    target.resist[Element::Fire.index()] = 0.3;
    let hit = Hit::split(100.0, &[(Element::Fire, 1.0)]);
    close(
        resolve(&hit, None, &mut target.clone(), NO_CRIT).dealt[Element::Fire.index()],
        70.0,
    );
    let mut curses = Curses::new(1, false);
    curses.lay(Curse::new(Kind::ExposeMark(Element::Fire), Rank::Art, 0, 1));
    curses.tick(0.1, &mut target, false);
    assert!(
        target
            .effects
            .iter()
            .any(|e| e.source == Source::Curse("exposemark"))
    );
    let o = resolve(&hit, None, &mut target.clone(), NO_CRIT);
    close(
        o.dealt[Element::Fire.index()],
        100.0 * (1.0 - (0.3 - NUMBERS.expose)),
    );
    close(curses.resist(Element::Fire), -NUMBERS.expose);
    // the curse ends: the next sync takes it off
    curses.tick(NUMBERS.expose_time, &mut target, false);
    assert!(target.effects.is_empty());
    close(
        resolve(&hit, None, &mut target, NO_CRIT).dealt[Element::Fire.index()],
        70.0,
    );
}

#[test]
fn a_doom_curse_raises_damage_taken_by_its_stacks() {
    let mut target = bare();
    let mut curses = Curses::new(1, false);
    curses.lay(Curse::new(Kind::AncestorDoom, Rank::Art, 0, 1));
    for _ in 0..3 {
        curses.on_cast(Tags::of(&[Tag::Attack]), false);
    }
    curses.sync(&mut target, false);
    let hit = Hit::split(100.0, &[(Element::Fire, 1.0)]);
    close(
        fire_on(&hit, &mut target),
        100.0 * (1.0 + 3.0 * NUMBERS.doom_step),
    );
    close(curses.taken(), 1.0 + 3.0 * NUMBERS.doom_step);
}

fn fire_on(hit: &Hit, t: &mut Cultivator) -> f64 {
    resolve(hit, None, t, NO_CRIT).dealt[Element::Fire.index()]
}

#[test]
fn an_intent_reaches_the_blows_of_its_holder_and_on_its_foe() {
    let mut holder = Cultivator::new();
    let mut foe = bare();
    let mut auras = Auras::new(3);
    assert!(
        auras
            .light(AuraKind::Slaughter, Rank::Art, true, &mut holder)
            .is_ok()
    );
    assert!(
        auras
            .light(AuraKind::Severance, Rank::Art, true, &mut holder)
            .is_ok()
    );
    auras.tick(AURA.windup, &mut holder);
    holder.intent = 50.0;
    sync(&auras, &EMPTY, &mut holder, 0.0, &mut foe, 1.0, false);
    // Sát lục: the holder's blows more by its intent
    let b = build(&SPARK, 1, &[], &holder, &EMPTY, &Situation::default());
    let more = AURAS[AuraKind::Slaughter.index()].more_per_intent * 50.0;
    assert_eq!(b.hit.more, vec![more]);
    close(auras.more(&holder), 1.0 + more);
    // Tuyệt tình: the foe inside takes more
    let taken = AURAS[AuraKind::Severance.index()].press.taken;
    close(
        fire_on(
            &Hit::split(100.0, &[(Element::Fire, 1.0)]),
            &mut foe.clone(),
        ),
        100.0 * (1.0 + taken),
    );
    // out of reach, the next sync takes it off
    sync(&auras, &EMPTY, &mut holder, 0.0, &mut foe, 10.0, false);
    close(
        fire_on(&Hit::split(100.0, &[(Element::Fire, 1.0)]), &mut foe),
        100.0,
    );
    // the holder's aura effect scales them
    let strong = Sheet {
        extra: vec![Mod::inc(Stat::AuraEffect, 1.0, SRC)],
        ..Sheet::default()
    };
    sync(&auras, &strong, &mut holder, 0.0, &mut foe, 1.0, false);
    let b = build(&SPARK, 1, &[], &holder, &EMPTY, &Situation::default());
    assert_eq!(b.hit.more, vec![2.0 * more]);
}

#[test]
fn a_domain_cuts_the_power_of_a_foe_inside() {
    let mut field = Field::default();
    field.raise(0, DomainKind::TruePressure, Rank::Art, 0.0, [0.0, 1.0], 0.0);
    let mut owner = Cultivator::new();
    let mut foe = Cultivator::new();
    field.sync([&mut owner, &mut foe], [0.0, 1.0], [false, false]);
    let power = DOMAINS[DomainKind::TruePressure.index()].foe_power;
    let b = build(&SPARK, 1, &[], &foe, &EMPTY, &Situation::default());
    close(fire(&b.hit), 100.0 * power);
    let b = build(&SPARK, 1, &[], &owner, &EMPTY, &Situation::default());
    close(fire(&b.hit), 100.0);
}

// ---- stars and ailments ----

#[test]
fn stars_are_modifiers() {
    // mind stars: crit chance and multiplier on the blows it strikes
    let mut c = Cultivator::new();
    c.stars[Star::Mind as usize] = 2.0;
    let b = build(&SPARK, 1, &[], &c, &EMPTY, &Situation::default());
    close(b.hit.crit, BASE.crit + 2.0 * RES.star_crit);
    close(
        b.hit.crit_multi,
        BASE.crit_multi + 2.0 * RES.star_crit_multi,
    );
    close(
        b.hit.increased_by[Element::Soul.index()],
        2.0 * RES.star_soul,
    );
    // speed stars: faster windup and recover
    let mut c = Cultivator::new();
    c.stars[Star::Speed as usize] = 3.0;
    let b = build(&SPARK, 1, &[], &c, &EMPTY, &Situation::default());
    close(b.windup, 0.5 / (1.0 + 3.0 * RES.star_speed));
    close(b.recover, 0.4 / (1.0 + 3.0 * RES.star_speed));
    // body stars: less taken, more resistance (capped)
    let mut t = bare();
    t.stars[Star::Body as usize] = 2.0;
    let body = 2.0 * RES.star_body;
    close(
        fire_on(&Hit::split(100.0, &[(Element::Fire, 1.0)]), &mut t.clone()),
        100.0 * (1.0 - body) * (1.0 - body),
    );
    close(t.resist_of(Element::Fire), body);
    t.resist[Element::Fire.index()] = 0.74;
    close(t.resist_of(Element::Fire), BASE.resist_cap);
    // a resistance cap raised on the sheet lets it climb
    let def_sheet = Sheet {
        extra: vec![Mod::flat(Stat::ResistCap(Element::Fire), 0.05, SRC)],
        ..Sheet::default()
    };
    let def = Defence {
        sheet: &def_sheet,
        ..Defence::default()
    };
    let o = resolve_with(
        &Hit::split(100.0, &[(Element::Fire, 1.0)]),
        None,
        &mut t,
        &def,
        NO_CRIT,
    );
    close(
        o.dealt[Element::Fire.index()],
        100.0 * (1.0 - body) * (1.0 - 0.8),
    );
}

#[test]
fn ailments_on_the_bearer_are_modifiers() {
    let mut t = bare();
    t.ailments.add(Ailment::Jolt, 2.0, 1.0);
    t.ailments.add(Ailment::Wither, 3.0, 1.0);
    t.ailments.add(Ailment::Bind, 2.0, 1.0);
    t.ailments.add(Ailment::Dread, 1.0, 1.0);
    let ctx = Ctx::alone(&t);
    let table = EMPTY.table(&ctx, []);
    close(table.get(Stat::Taken, 1.0), 1.0 + 2.0 * AIL.jolt_taken);
    close(
        table.get(Stat::TakenOf(Element::Yin), 1.0),
        1.0 + 3.0 * AIL.wither_yin,
    );
    close(table.get(Stat::Healing, 1.0), 1.0 - 3.0 * AIL.wither_heal);
    close(table.get(Stat::MoveSpeed, 1.0), 1.0 - 2.0 * AIL.bind_slow);
    // dread lengthens the bearer's windups, not its recover
    let b = build(&SPARK, 1, &[], &t, &EMPTY, &Situation::default());
    close(b.windup, 0.5 * (1.0 + AIL.dread_windup));
    close(b.recover, 0.4);
    // healing reads it
    t.life = 1000.0;
    close(t.heal(100.0), 100.0 * (1.0 - 3.0 * AIL.wither_heal));
}

// ---- conversion ----

#[test]
fn converted_damage_is_still_scaled_by_its_first_kinds_increase() {
    let sheet = Sheet {
        extra: vec![Mod::inc(Stat::DamageOf(Element::Force), 0.5, SRC)],
        ..Sheet::default()
    };
    let c = Cultivator::new();
    let b = build(
        &PALM,
        1,
        &[sup("fireforge")],
        &c,
        &sheet,
        &Situation::default(),
    );
    close(
        b.hit.convert[Element::Force.index()][Element::Fire.index()],
        sp::PHASE_SHARE,
    );
    let p = scale(&b.hit, 1.0);
    let whole = BASE.might;
    // the half that became fire keeps force's increase
    close(p[Element::Fire.index()], whole * sp::PHASE_SHARE * 1.5);
    close(
        p[Element::Force.index()],
        whole * (1.0 - sp::PHASE_SHARE) * 1.5,
    );
    // a fire increase touches only the fire part
    let sheet = Sheet {
        extra: vec![Mod::inc(Stat::DamageOf(Element::Fire), 0.5, SRC)],
        ..Sheet::default()
    };
    let b = build(
        &PALM,
        1,
        &[sup("fireforge")],
        &c,
        &sheet,
        &Situation::default(),
    );
    let p = scale(&b.hit, 1.0);
    close(p[Element::Fire.index()], whole * sp::PHASE_SHARE * 1.5);
    close(p[Element::Force.index()], whole * (1.0 - sp::PHASE_SHARE));
}

// ---- keystones on the sheet ----

#[test]
fn keystones_live_on_the_sheet() {
    // Tiên Trì: spells more, from the sheet alone
    let sheet = Sheet::with_keystones(&[Keystone::ImmortalPool]);
    let c = Cultivator::new();
    let sit = Situation::default();
    assert_eq!(
        build(&SPARK, 1, &[], &c, &sheet, &sit).hit.more,
        vec![ks::POOL_MORE]
    );
    // Tóc Đen: the bars baked at the start, armour read per blow
    let sheet = Sheet::with_keystones(&[Keystone::BlackHair]);
    let mut t = Cultivator::new();
    sheet.apply(&mut t);
    close(t.life_max, BASE.life * ks::BLACK_HAIR_STATS);
    close(build(&PALM, 1, &[], &c, &sheet, &sit).hit.armour_pen, 1.0);
    t.shield = 0.0;
    let def = Defence {
        sheet: &sheet,
        ..Defence::default()
    };
    let o = resolve_with(
        &Hit::split(100.0, &[(Element::Force, 1.0)]),
        None,
        &mut t,
        &def,
        NO_CRIT,
    );
    let h = BASE.armour * ks::BLACK_HAIR_STATS;
    close(o.dealt[0], 100.0 * (1.0 - h / (h + BASE.armour_k * 100.0)));
    // a tree's keystone counts like the loadout's
    let tree = Tree {
        nodes: vec![Node {
            id: 1,
            name: "Tiên Trì".to_string(),
            mods: Vec::new(),
            keystone: Some(Keystone::ImmortalPool),
            links: Vec::new(),
        }],
        starts: vec![1],
    };
    let sheet = Sheet {
        tree,
        allocated: vec![1],
        ..Sheet::default()
    };
    assert_eq!(
        build(&SPARK, 1, &[], &c, &sheet, &sit).hit.more,
        vec![ks::POOL_MORE]
    );
}

// ---- the empty sheet is the old fight ----

#[test]
fn the_empty_sheet_gives_exactly_the_old_numbers() {
    let c = Cultivator::new();
    let sit = Situation::default();
    let b = build(&PALM, 1, &[], &c, &EMPTY, &sit);
    let mut want = Hit::split(BASE.might, &[(Element::Force, 1.0)]);
    want.tags = PALM.all_tags();
    want.evadable = PALM.all_tags().evadable();
    assert_eq!(b.hit, want);
    assert_eq!(b.windup, 0.5);
    assert_eq!(b.recover, 0.4);
    assert_eq!(b.cooldown, 4.0);
    assert_eq!(b.costs, vec![(Resource::Qi, 20.0)]);
    assert_eq!(
        (b.strikes, b.projectiles, b.life, b.control),
        (1, 0, 0.0, 1.0)
    );
    assert!(b.table.used.is_empty());
    let b = build(&SPARK, 1, &[], &c, &EMPTY, &sit);
    assert_eq!(b.hit.base[Element::Fire.index()], 100.0);
    assert_eq!((b.hit.crit, b.hit.crit_multi), (BASE.crit, BASE.crit_multi));
    // a blow on a target with no sheet is the same with or without the empty defence
    let mut t = Cultivator::new();
    t.ailments.add(Ailment::Jolt, 2.0, 1.0);
    t.ailments.add(Ailment::Sunder, 2.0, 1.0);
    t.stars[Star::Body as usize] = 1.0;
    let h = Hit::split(1000.0, &[(Element::Force, 0.5), (Element::Thunder, 0.5)]);
    let a = resolve(&h, None, &mut t.clone(), NO_CRIT);
    let b = resolve_with(&h, None, &mut t.clone(), &Defence::default(), NO_CRIT);
    assert_eq!(a, b);
    // and the old step 8 by hand: jolt and body stars on all, sunder on armour, body stars on resistance
    let taken = (1.0 - RES.star_body) * (1.0 + 2.0 * AIL.jolt_taken);
    let armour = BASE.armour * (1.0 - 2.0 * AIL.sunder);
    let force = 500.0 * (1.0 - armour / (armour + BASE.armour_k * 500.0)) * taken;
    let thunder = 500.0 * (1.0 - RES.star_body) * taken;
    close(a.dealt[Element::Force.index()], force);
    close(a.dealt[Element::Thunder.index()], thunder);
}

// ---- "per" scaling (design, 17.1) ----

fn sheet_of(mods: Vec<Mod>) -> Sheet {
    Sheet {
        extra: mods,
        ..Sheet::default()
    }
}

#[test]
fn each_per_counts_what_it_names() {
    let mut me = Cultivator::new();
    me.stars[Star::Mind as usize] = 2.0;
    me.intent = 57.0;
    me.life = 0.755 * me.life_max;
    me.souls = 4.0;
    let mut foe = Cultivator::new();
    foe.life = 0.4 * foe.life_max;
    for _ in 0..6 {
        foe.ailments
            .lay(arena_calc::dot::Dot::new(Ailment::Bleed, 1.0, 5.0));
    }
    foe.ailments.add(Ailment::Soak, 2.0, 1.0);
    let ctx = Ctx {
        other: Some(&foe),
        curses: 2,
        distance: 7.5,
        counts: Counts {
            kills: 3,
            hits: 5,
            qi_spent: 65.0,
            kinds: 0b101_0001,
        },
        ..Ctx::alone(&me)
    };
    let cases = [
        (Per::One, 1.0),
        (Per::Stacks(Ailment::Bleed), 6.0),
        (Per::Stacks(Ailment::Soak), 2.0),
        (Per::DistinctAilments, 2.0),
        (Per::Curses, 2.0),
        (Per::Stars(Star::Mind), 2.0),
        (Per::Stars(Star::Body), 0.0),
        (Per::Intent(10.0), 5.0),
        (Per::MissingLife, 24.0),
        (Per::TargetMissingLife, 60.0),
        (Per::Souls, 4.0),
        (Per::Distance(2.0), 3.0),
        (Per::Kills, 3.0),
        (Per::Hits, 5.0),
        (Per::QiSpent(20.0), 3.0),
        (Per::KindsDealt, 3.0),
    ];
    for (p, n) in cases {
        close(p.count(&ctx), n);
    }
    // nothing to count without a target or a known distance, and a step of 0 counts nothing
    let alone = Ctx::alone(&me);
    close(Per::Stacks(Ailment::Bleed).count(&alone), 0.0);
    close(Per::TargetMissingLife.count(&alone), 0.0);
    close(Per::Distance(1.0).count(&alone), 0.0);
    close(Per::Intent(0.0).count(&ctx), 0.0);
}

#[test]
fn a_per_mod_counts_each_time_up_to_its_cap() {
    let c = Cultivator::new();
    let mut foe = Cultivator::new();
    for _ in 0..6 {
        foe.ailments
            .lay(arena_calc::dot::Dot::new(Ailment::Bleed, 1.0, 5.0));
    }
    let ctx = Ctx {
        other: Some(&foe),
        ..Ctx::alone(&c)
    };
    let per = Mod::more(Stat::Damage, 0.05, SRC).per(Per::Stacks(Ailment::Bleed));
    close(per.value_in(&ctx), 0.3);
    close(per.max(0.2).value_in(&ctx), 0.2);
    // the cap holds in size both ways, and an override is never scaled
    let less = Mod::more(Stat::Damage, -0.1, SRC)
        .per(Per::Stacks(Ailment::Bleed))
        .max(0.4);
    close(less.value_in(&ctx), -0.4);
    let over = Mod::new(Stat::Damage, ModKind::Override, 2.0, SRC).per(Per::Stacks(Ailment::Bleed));
    close(over.value_in(&ctx), 2.0);
    // a cap with no per caps the plain value
    close(
        Mod::flat(Stat::Life, 50.0, SRC).max(20.0).value_in(&ctx),
        20.0,
    );
    // gathered at its value in the moment: one more of 1 + 0.3, kept so in `used`
    let t = Table::gather([&per], &ctx);
    close(t.more(Stat::Damage), 1.3);
    close(t.used[0].value, 0.3);
    // the same mod with no bleeds counts nothing
    let t = Table::gather([&per], &Ctx::alone(&c));
    close(t.more(Stat::Damage), 1.0);
}

#[test]
fn the_new_conditions_read_the_moment() {
    let mut me = Cultivator::new();
    let mut foe = Cultivator::new();
    let mark = Source::Other("mark");
    let plain = |me: &Cultivator, foe: &Cultivator, c: Condition| {
        c.holds(&Ctx {
            other: Some(foe),
            ..Ctx::alone(me)
        })
    };
    assert!(plain(&me, &foe, Condition::TargetFullLife));
    assert!(plain(&me, &foe, Condition::TargetHasSoul));
    assert!(!plain(&me, &foe, Condition::TargetDistinctAilments(1)));
    assert!(!plain(&me, &foe, Condition::TargetHasEffect(mark)));
    assert!(!plain(&me, &foe, Condition::SelfHasEffect(mark)));
    assert!(!plain(&me, &foe, Condition::TargetMoving));
    assert!(!plain(&me, &foe, Condition::TargetCurses(1)));
    assert!(!plain(&me, &foe, Condition::EveryNth(3)));
    assert!(!plain(&me, &foe, Condition::QiSpent(10.0)));
    assert!(plain(&me, &foe, Condition::SelfQiAbove(1.0)));
    foe.life -= 1.0;
    foe.ailments.add(Ailment::Soak, 1.0, 1.0);
    foe.ailments.add(Ailment::Sunder, 1.0, 1.0);
    foe.put_on(Active {
        source: mark,
        mods: Vec::new(),
        left: Some(1.0),
    });
    me.put_on(Active {
        source: mark,
        mods: Vec::new(),
        left: None,
    });
    me.qi = 40.0;
    assert!(!plain(&me, &foe, Condition::TargetFullLife));
    assert!(plain(&me, &foe, Condition::TargetDistinctAilments(2)));
    assert!(!plain(&me, &foe, Condition::TargetDistinctAilments(3)));
    assert!(plain(&me, &foe, Condition::TargetHasEffect(mark)));
    assert!(plain(&me, &foe, Condition::SelfHasEffect(mark)));
    assert!(plain(&me, &foe, Condition::SelfQiAbove(0.4)));
    assert!(!plain(&me, &foe, Condition::SelfQiAbove(0.41)));
    // a soulless body, or one whose soul is lost, has none
    foe.soul_lost = 1.0;
    assert!(!plain(&me, &foe, Condition::TargetHasSoul));
    foe.soul_lost = 0.0;
    foe.soulless_body = true;
    assert!(!plain(&me, &foe, Condition::TargetHasSoul));
    // what the context carries: the other side moving, its curses, the use, what happened lately
    for (n, holds) in [
        (0, false),
        (1, false),
        (2, false),
        (3, true),
        (6, true),
        (7, false),
    ] {
        let ctx = Ctx {
            use_count: n,
            ..Ctx::alone(&me)
        };
        assert_eq!(Condition::EveryNth(3).holds(&ctx), holds, "use {n}");
    }
    let ctx = Ctx {
        other_moving: true,
        curses: 2,
        counts: Counts {
            qi_spent: 30.0,
            ..Counts::default()
        },
        ..Ctx::alone(&me)
    };
    assert!(Condition::TargetMoving.holds(&ctx));
    assert!(Condition::TargetCurses(2).holds(&ctx));
    assert!(!Condition::TargetCurses(3).holds(&ctx));
    assert!(Condition::TargetCursed.holds(&ctx));
    assert!(Condition::QiSpent(30.0).holds(&ctx));
    // a caller that only knows "cursed" counts one curse
    let ctx = Ctx {
        other_cursed: true,
        ..Ctx::alone(&me)
    };
    assert!(Condition::TargetCurses(1).holds(&ctx));
    // the situation carries them into a cast
    let sit = Situation {
        target: Some(&foe),
        target_moving: true,
        target_curses: 2,
        use_count: 3,
        ..Situation::default()
    };
    let ctx = sit.ctx(&me, Tags::NONE);
    assert!(ctx.other_moving && ctx.other_cursed && ctx.curses == 2 && ctx.use_count == 3);
}

// ---- every stat is read here or left to the fight ----

/// One of every stat: the `match` below fails to build when a variant is added and not listed.
fn every_stat() -> Vec<Stat> {
    let e = Element::Fire;
    let a = Ailment::Bleed;
    let all = vec![
        Stat::Damage,
        Stat::DamageOf(e),
        Stat::HitDamage,
        Stat::DotDamage,
        Stat::AilmentDamage(a),
        Stat::DotMulti,
        Stat::DotMultiOf(e),
        Stat::Added(e),
        Stat::Convert(e, e),
        Stat::Extra(e, e),
        Stat::CritChance,
        Stat::CritMulti,
        Stat::ChanceDoubleDamage,
        Stat::Penetration(e),
        Stat::ArmourPen,
        Stat::ShieldBypass,
        Stat::Execute,
        Stat::Leech,
        Stat::QiLeech,
        Stat::LifeOnHit,
        Stat::QiOnHit,
        Stat::SenseOnHit,
        Stat::IntentOnHit,
        Stat::Inflict(a),
        Stat::StarOn(Happened::Hit, Star::Body),
        Stat::AilmentEffect,
        Stat::AilmentEffectOf(a),
        Stat::ControlEffect,
        Stat::ControlDuration,
        Stat::AilmentDuration,
        Stat::AilmentDurationOf(a),
        Stat::Projectiles,
        Stat::Chain,
        Stat::Pierce,
        Stat::Fork,
        Stat::ProjectileSpeed,
        Stat::Knockback,
        Stat::Repeats,
        Stat::Speed,
        Stat::Windup,
        Stat::CooldownRecovery,
        Stat::Cost,
        Stat::CostAsLife,
        Stat::Reservation,
        Stat::Reserved,
        Stat::ExtraCooldownUse,
        Stat::SkillDuration,
        Stat::Area,
        Stat::CurseEffect,
        Stat::AuraEffect,
        Stat::CurseCount,
        Stat::MinionDamage,
        Stat::MinionLife,
        Stat::MinionCount,
        Stat::TrapCount,
        Stat::MineCount,
        Stat::TotemCount,
        Stat::DeviceLife,
        Stat::Life,
        Stat::Shield,
        Stat::Sense,
        Stat::QiMax,
        Stat::StarsMax,
        Stat::Might,
        Stat::Armour,
        Stat::Evasion,
        Stat::Resist(e),
        Stat::ResistCap(e),
        Stat::AilmentResist(a),
        Stat::Taken,
        Stat::TakenOf(e),
        Stat::DotTaken,
        Stat::HitTaken,
        Stat::ControlTaken,
        Stat::LifeRegen,
        Stat::ShieldRegen,
        Stat::QiRegen,
        Stat::Healing,
        Stat::MoveSpeed,
    ];
    for s in &all {
        match s {
            Stat::Damage
            | Stat::DamageOf(_)
            | Stat::HitDamage
            | Stat::DotDamage
            | Stat::AilmentDamage(_)
            | Stat::DotMulti
            | Stat::DotMultiOf(_)
            | Stat::Added(_)
            | Stat::Convert(..)
            | Stat::Extra(..)
            | Stat::CritChance
            | Stat::CritMulti
            | Stat::ChanceDoubleDamage
            | Stat::Penetration(_)
            | Stat::ArmourPen
            | Stat::ShieldBypass
            | Stat::Execute
            | Stat::Leech
            | Stat::QiLeech
            | Stat::LifeOnHit
            | Stat::QiOnHit
            | Stat::SenseOnHit
            | Stat::IntentOnHit
            | Stat::Inflict(_)
            | Stat::StarOn(..)
            | Stat::AilmentEffect
            | Stat::AilmentEffectOf(_)
            | Stat::ControlEffect
            | Stat::ControlDuration
            | Stat::AilmentDuration
            | Stat::AilmentDurationOf(_)
            | Stat::Projectiles
            | Stat::Chain
            | Stat::Pierce
            | Stat::Fork
            | Stat::ProjectileSpeed
            | Stat::Knockback
            | Stat::Repeats
            | Stat::Speed
            | Stat::Windup
            | Stat::CooldownRecovery
            | Stat::Cost
            | Stat::CostAsLife
            | Stat::Reservation
            | Stat::Reserved
            | Stat::ExtraCooldownUse
            | Stat::SkillDuration
            | Stat::Area
            | Stat::CurseEffect
            | Stat::AuraEffect
            | Stat::CurseCount
            | Stat::MinionDamage
            | Stat::MinionLife
            | Stat::MinionCount
            | Stat::TrapCount
            | Stat::MineCount
            | Stat::TotemCount
            | Stat::DeviceLife
            | Stat::Life
            | Stat::Shield
            | Stat::Sense
            | Stat::QiMax
            | Stat::StarsMax
            | Stat::Might
            | Stat::Armour
            | Stat::Evasion
            | Stat::Resist(_)
            | Stat::ResistCap(_)
            | Stat::AilmentResist(_)
            | Stat::Taken
            | Stat::TakenOf(_)
            | Stat::DotTaken
            | Stat::HitTaken
            | Stat::ControlTaken
            | Stat::LifeRegen
            | Stat::ShieldRegen
            | Stat::QiRegen
            | Stat::Healing
            | Stat::MoveSpeed => {}
        }
    }
    all
}

/// A stat's variant name (`DamageOf(Fire)` is `DamageOf`).
fn variant(s: Stat) -> String {
    let d = format!("{s:?}");
    d.split('(').next().unwrap_or_default().to_string()
}

/// Whether the crate's source (whitespace taken out) reads `Stat::<name>` from a table: `.get(`, `.flat(`, `.acc(`,
/// `.more(` (a table's, not `Mod::more(`), `.increased(`, `count(`, or inside a `.combined(&[...])`.
fn read_in(src: &str, name: &str) -> bool {
    let needle = format!("Stat::{name}");
    let ends = |rest: &str| rest.starts_with(['(', ',', ')', ']']);
    let direct = [
        ".get(",
        ".flat(",
        ".acc(",
        ".more(",
        ".increased(",
        "count(",
    ];
    for (i, _) in src.match_indices(&needle) {
        let rest = &src[i + needle.len()..];
        if !ends(rest) {
            continue;
        }
        let before = &src[..i];
        if direct.iter().any(|d| before.ends_with(d)) {
            return true;
        }
        // inside `.combined(&[ ... ])`: the last `.combined(&[` before it, with no `]` between
        if let Some(j) = before.rfind(".combined(&[")
            && !before[j..].contains(']')
        {
            return true;
        }
    }
    false
}

#[test]
fn every_stat_is_read_in_the_crate_or_left_to_the_fight() {
    let src: String = [
        include_str!("../src/build.rs"),
        include_str!("../src/hit.rs"),
        include_str!("../src/dot.rs"),
        include_str!("../src/sheet.rs"),
        include_str!("../src/stat.rs"),
        include_str!("../src/cultivator.rs"),
        include_str!("../src/curse.rs"),
        include_str!("../src/aura.rs"),
        include_str!("../src/play.rs"),
        include_str!("../src/projectile.rs"),
        include_str!("../src/motion.rs"),
        include_str!("../src/charge.rs"),
        include_str!("../src/device.rs"),
    ]
    .concat()
    .split_whitespace()
    .collect();
    let fight: Vec<String> = FIGHT_READS.iter().map(|&s| variant(s)).collect();
    for s in every_stat() {
        let name = variant(s);
        let read = read_in(&src, &name);
        let left = fight.contains(&name);
        assert!(read || left, "{name} is neither read nor in FIGHT_READS");
        assert!(!(read && left), "{name} is read here and in FIGHT_READS");
    }
    // every FIGHT_READS entry is a stat, once
    for (i, n) in fight.iter().enumerate() {
        assert!(!fight[..i].contains(n), "{n} twice");
    }
    // the field-bound stats are read by the field behaviours now
    for s in [
        Stat::MoveSpeed,
        Stat::SkillDuration,
        Stat::Area,
        Stat::Chain,
        Stat::Pierce,
        Stat::Fork,
        Stat::ProjectileSpeed,
        Stat::Knockback,
        Stat::MinionDamage,
        Stat::MinionLife,
        Stat::MinionCount,
        Stat::TrapCount,
        Stat::MineCount,
        Stat::TotemCount,
        Stat::DeviceLife,
        Stat::ExtraCooldownUse,
    ] {
        assert!(read_in(&src, &variant(s)), "{s:?} is not read");
    }
}

// ---- the new stats, each where it is read ----

/// `hit` from a bare striker on a bare target with `sheet`, rolled `rolls`.
fn land_on(
    hit: &Hit,
    target: &mut Cultivator,
    sheet: &Sheet,
    rolls: Rolls,
) -> arena_calc::hit::Outcome {
    let by = Cultivator::new();
    let def = Defence {
        sheet,
        ..Defence::default()
    };
    resolve_with(hit, Some(&by), target, &def, rolls)
}

#[test]
fn gain_as_extra_adds_a_part_that_has_been_both_kinds_after_conversion() {
    let sheet = sheet_of(vec![
        Mod::flat(Stat::Extra(Element::Fire, Element::Thunder), 0.5, SRC),
        Mod::inc(Stat::DamageOf(Element::Fire), 0.2, SRC),
        Mod::inc(Stat::DamageOf(Element::Thunder), 0.1, SRC),
    ]);
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(
        b.hit.extra[Element::Fire.index()][Element::Thunder.index()],
        0.5,
    );
    let parts = scale(&b.hit, 1.0);
    // fire 100 × 1.2; thunder gained 50, which has been fire and thunder: × (1 + 0.2 + 0.1)
    close(parts[Element::Fire.index()], 120.0);
    close(parts[Element::Thunder.index()], 65.0);
    // the burn sees only fire's part (the thunder part is not fire now)
    close(
        arena_calc::hit::dot_scale(&b.hit, Ailment::Burn, 1.0),
        120.0,
    );
}

#[test]
fn culling_takes_the_rest_of_a_target_left_under_its_line() {
    let mut hit = Hit::split(200.0, &[(Element::Fire, 1.0)]);
    hit.execute = 0.1;
    let mut t = bare();
    t.life = 0.15 * t.life_max; // 450 → 250 after the blow, under 300
    let o = land_on(&hit, &mut t, &EMPTY, NO_CRIT);
    assert!(o.executed);
    close(t.life, 0.0);
    close(o.to_life, 450.0);
    // above the line it lives
    let mut t = bare();
    let o = land_on(&hit, &mut t, &EMPTY, NO_CRIT);
    assert!(!o.executed);
    close(t.life, BASE.life - 200.0);
}

#[test]
fn a_blow_returns_life_qi_sense_and_intent_and_drinks_qi() {
    let sheet = sheet_of(vec![
        Mod::flat(Stat::LifeOnHit, 20.0, SRC),
        Mod::flat(Stat::QiOnHit, 3.0, SRC),
        Mod::flat(Stat::SenseOnHit, 2.0, SRC),
        Mod::flat(Stat::IntentOnHit, 1.0, SRC),
        Mod::flat(Stat::QiLeech, 0.01, SRC),
    ]);
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    let o = land_on(&b.hit, &mut bare(), &EMPTY, NO_CRIT);
    close(o.gains.life, 20.0);
    close(o.gains.qi, 3.0 + 0.01 * 100.0);
    close(o.gains.sense, 2.0);
    close(o.gains.intent, 1.0);
}

#[test]
fn double_damage_is_its_own_roll_at_step_five() {
    let sheet = sheet_of(vec![Mod::flat(Stat::ChanceDoubleDamage, 0.3, SRC)]);
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(b.hit.double, 0.3);
    let yes = Rolls {
        double: 0.29,
        ..Rolls::NONE
    };
    let o = land_on(&b.hit, &mut bare(), &EMPTY, yes);
    assert!(o.doubled);
    close(o.dealt[Element::Fire.index()], 200.0);
    let no = Rolls {
        double: 0.3,
        ..Rolls::NONE
    };
    let o = land_on(&b.hit, &mut bare(), &EMPTY, no);
    assert!(!o.doubled);
    close(o.dealt[Element::Fire.index()], 100.0);
}

#[test]
fn shield_bypass_sends_a_share_past_the_shield() {
    let mut hit = Hit::split(100.0, &[(Element::Fire, 1.0)]);
    hit.shield_bypass = 0.25;
    let mut t = Cultivator::new();
    t.armour = 0.0;
    let o = land_on(&hit, &mut t, &EMPTY, NO_CRIT);
    close(o.to_shield, 75.0);
    close(o.to_life, 25.0);
}

#[test]
fn hits_taken_and_damage_over_time_taken_are_apart() {
    let sheet = sheet_of(vec![Mod::more(Stat::HitTaken, -0.5, SRC)]);
    let hit = Hit::split(100.0, &[(Element::Fire, 1.0)]);
    let mut t = bare();
    let o = land_on(&hit, &mut t, &sheet, NO_CRIT);
    close(o.dealt[Element::Fire.index()], 50.0);
    // the burn it laid ticks whole (only `DotTaken` cuts it)
    let before = t.life;
    t.tick_with(1.0, false, &sheet);
    close(before - t.life, arena_calc::dot::BURN_RATE * 100.0);
}

#[test]
fn cost_as_life_qi_max_and_stars_max_reach_their_readers() {
    let sheet = sheet_of(vec![Mod::flat(Stat::CostAsLife, 0.5, SRC)]);
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    let qi: f64 = b
        .costs
        .iter()
        .filter(|c| c.0 == Resource::Qi)
        .map(|c| c.1)
        .sum();
    close(qi, 10.0);
    close(b.life, 10.0 * arena_calc::build::LIFE_PER_QI * BASE.life);
    let mut c = Cultivator::new();
    sheet_of(vec![Mod::flat(Stat::QiMax, 50.0, SRC)]).apply(&mut c);
    close(c.qi_max, BASE.qi + 50.0);
    close(c.qi, c.qi_max);
    // a dodge earns speed stars up to the dodger's cap
    let sheet = sheet_of(vec![Mod::flat(Stat::StarsMax, 2.0, SRC)]);
    let mut hit = Hit::split(100.0, &[(Element::Force, 1.0)]);
    hit.evadable = true;
    let mut t = bare();
    for _ in 0..6 {
        let dodge = Rolls {
            evade: 0.0,
            ..Rolls::NONE
        };
        land_on(&hit, &mut t, &sheet, dodge);
    }
    close(t.stars[Star::Speed as usize], RES.stars_max + 2.0);
}

#[test]
fn stars_on_what_happens_for_the_striker_and_the_target() {
    let sheet = sheet_of(vec![
        Mod::flat(Stat::StarOn(Happened::Hit, Star::Speed), 0.5, SRC),
        Mod::flat(Stat::StarOn(Happened::Crit, Star::Mind), 1.0, SRC),
    ]);
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    assert_eq!(b.hit.star_on.len(), 2);
    let roll = |crit: f64, star: f64| Rolls {
        crit,
        star,
        ..Rolls::NONE
    };
    let o = land_on(&b.hit, &mut bare(), &EMPTY, roll(1.0, 0.4));
    assert_eq!(o.stars, vec![Star::Speed]);
    let o = land_on(&b.hit, &mut bare(), &EMPTY, roll(0.0, 0.6));
    assert_eq!(o.stars, vec![Star::Mind]);
    let o = land_on(&b.hit, &mut bare(), &EMPTY, roll(1.0, 0.6));
    assert!(o.stars.is_empty());
    // the target's own: a body star when hit
    let taker = sheet_of(vec![Mod::flat(
        Stat::StarOn(Happened::BeenHit, Star::Body),
        1.0,
        SRC,
    )]);
    let mut t = bare();
    land_on(
        &Hit::split(10.0, &[(Element::Fire, 1.0)]),
        &mut t,
        &taker,
        roll(1.0, 0.5),
    );
    close(t.stars[Star::Body as usize], 1.0);
}

#[test]
fn control_duration_control_taken_and_control_effect() {
    let sheet = sheet_of(vec![
        Mod::inc(Stat::ControlDuration, 1.0, SRC),
        Mod::inc(Stat::ControlEffect, 0.5, SRC),
    ]);
    let b = build(
        &PALM,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(b.hit.control, 2.0);
    close(b.control, 2.0);
    // control effect: the stacks of an ailment that deals no damage, never burn or bleed
    close(b.hit.ailment_effect[Ailment::Daze.index()], 1.5);
    close(b.hit.ailment_effect[Ailment::Bleed.index()], 1.0);
    close(b.hit.ailment_effect[Ailment::Burn.index()], 1.0);
    // a stun (full daze) lasts twice as long, and half that on a target that takes controls at half
    let mut hit = Hit::split(3000.0, &[(Element::Force, 1.0)]);
    hit.force_ailment = ForceAilment::Daze;
    hit.control = 2.0;
    let o = land_on(&hit, &mut bare(), &EMPTY, NO_CRIT);
    let stun = Ailment::Daze.data().seconds;
    assert_eq!(o.controls.len(), 1);
    close(o.controls[0].1, 2.0 * stun);
    let half = sheet_of(vec![Mod::more(Stat::ControlTaken, -0.5, SRC)]);
    let o = land_on(&hit, &mut bare(), &half, NO_CRIT);
    close(o.controls[0].1, stun);
}

#[test]
fn inflict_lays_stacks_on_every_blow_and_ailment_resist_cuts_what_parts_lay() {
    let sheet = sheet_of(vec![Mod::flat(Stat::Inflict(Ailment::Wither), 2.0, SRC)]);
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    let mut t = bare();
    let o = land_on(&b.hit, &mut t, &EMPTY, NO_CRIT);
    close(o.stacks[Ailment::Wither.index()], 2.0);
    close(t.ailments.stacks(Ailment::Wither), 2.0);
    // metal 150 lays 150 / 3000 × 20 = 1 sunder; half resisted, a half
    let hit = Hit::split(150.0, &[(Element::Metal, 1.0)]);
    let o = land_on(&hit, &mut bare(), &EMPTY, NO_CRIT);
    close(o.stacks[Ailment::Sunder.index()], 1.0);
    let resist = sheet_of(vec![Mod::flat(
        Stat::AilmentResist(Ailment::Sunder),
        0.5,
        SRC,
    )]);
    let o = land_on(&hit, &mut bare(), &resist, NO_CRIT);
    close(o.stacks[Ailment::Sunder.index()], 0.5);
}

#[test]
fn curse_count_reservation_and_reserved() {
    let c = Cultivator::new();
    assert_eq!(arena_calc::curse::curse_limit(&c, &EMPTY, false), 1);
    assert_eq!(arena_calc::curse::curse_limit(&c, &EMPTY, true), 2);
    let more = sheet_of(vec![Mod::flat(Stat::CurseCount, 1.0, SRC)]);
    assert_eq!(arena_calc::curse::curse_limit(&c, &more, false), 2);
    // an intent lit at a reservation holds that share of its sense, and frees exactly it
    let mut c = Cultivator::new();
    let mut auras = Auras::new(2);
    let full = AuraKind::ALL[0].data().reserve;
    assert!(
        auras
            .light_with(AuraKind::ALL[0], Rank::Art, true, &mut c, 0.5)
            .is_ok()
    );
    close(c.sense_reserved, 0.5 * full);
    close(auras.reserved(), 0.5 * full);
    assert!(auras.put_out(AuraKind::ALL[0], &mut c));
    close(c.sense_reserved, 0.0);
    // a skill's reservation and the sense its supports hold
    let sheet = sheet_of(vec![
        Mod::more(Stat::Reservation, -0.3, SRC),
        Mod::flat(Stat::Reserved, 30.0, SRC),
    ]);
    let b = build(
        &SPARK,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    );
    close(b.reservation, 0.7);
    close(b.reserve, 21.0);
}

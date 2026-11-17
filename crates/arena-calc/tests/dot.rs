//! Damage over time as its own branch, scaled by the striker (`docs/specs/2026-11-17-damage-system-design.md`,
//! 17.5), and the bearer's rates read from its table (17.1): burn and bleed instances, their snapshot, their
//! stacking, their mitigation, the five-phase rules on them, hollow fire, regen, a curser's curse effect, and the
//! events a tick makes.

use arena_calc::ailment::{AIL, Ailment, ForceAilment};
use arena_calc::build::{Built, Situation, build};
use arena_calc::cultivator::{Active, BASE, Cultivator};
use arena_calc::curse::{self, Curse, Curses, Kind, Lay, NUMBERS};
use arena_calc::dot::{self, BLEED_RATE, BURN_RATE, DOT_CAP, Dot, HOLLOW_FIRE_EFFECT};
use arena_calc::element::Element;
use arena_calc::event::{self, Dice, Effect, EventKind, Filter, Gain, Queue, Seat, Trigger};
use arena_calc::gem::{Gem, Resource, Tier};
use arena_calc::grade::Rank;
use arena_calc::hit::{Defence, Hit, Outcome, Rolls, resolve, resolve_with};
use arena_calc::sheet::{EMPTY, Sheet};
use arena_calc::stat::{Condition, Happened, Mod, Source, Stat};
use arena_calc::tag::Tag;

const NO_CRIT: Rolls = Rolls {
    evade: 1.0,
    crit: 1.0,
    ..Rolls::NONE
};

const SRC: Source = Source::Other("test");
const QI: &[(Resource, f64)] = &[(Resource::Qi, 10.0)];

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

/// A spell of `split` with a base of 100 at level 1 (`spell_base(100, 1) = 100`).
const fn spell(id: &'static str, split: &'static [(Element, f64)]) -> Gem {
    Gem {
        id,
        name: id,
        tags: &[Tag::Spell, Tag::Far, Tag::Projectile],
        split,
        rank: Rank::Art,
        restriction: false,
        base: 100.0,
        costs: QI,
        cooldown: 1.0,
        windup: 0.3,
        recover: 0.3,
        uses: None,
        bluff: false,
        force: ForceAilment::Bleed,
        tier: Tier::C,
        spell: "bolt",
        source: "arena",
        effects: &[],
    }
}

const FIRE_BOLT: Gem = spell("firebolt", &[(Element::Fire, 1.0)]);
const FORCE_BOLT: Gem = spell("forcebolt", &[(Element::Force, 1.0)]);
const WATER_BOLT: Gem = spell("waterbolt", &[(Element::Water, 1.0)]);

/// No shield, no armour: a hit lands whole.
fn bare() -> Cultivator {
    let mut c = Cultivator::new();
    c.armour = 0.0;
    c.shield = 0.0;
    c.shield_max = 0.0;
    c
}

/// `gem` at level 1, struck by a base fighter wearing `mods`.
fn made(gem: &Gem, mods: &[Mod]) -> Built {
    let sheet = Sheet {
        extra: mods.to_vec(),
        ..Sheet::default()
    };
    build(
        gem,
        1,
        &[],
        &Cultivator::new(),
        &sheet,
        &Situation::default(),
    )
}

fn land(hit: &Hit, t: &mut Cultivator) -> Outcome {
    resolve(hit, None, t, NO_CRIT)
}

fn fire(amount: f64) -> Hit {
    Hit::split(amount, &[(Element::Fire, 1.0)])
}

fn total_dps(c: &Cultivator, a: Ailment) -> f64 {
    c.ailments
        .dots(a)
        .map_or(0.0, |v| v.0.iter().map(|d| d.dps).sum())
}

#[test]
fn a_fire_part_lays_a_burn_of_half_its_damage_a_second_for_four_seconds() {
    let b = made(&FIRE_BOLT, &[]);
    let mut t = bare();
    let o = land(&b.hit, &mut t);
    // the fire part after step 7 is 100: 0.5 × 100 = 50 a second, 4 s
    close(o.before[Element::Fire.index()], 100.0);
    assert_eq!(o.dots.len(), 1);
    close(o.dots[0].dps, BURN_RATE * 100.0);
    close(o.dots[0].time, 4.0);
    close(o.stacks[Ailment::Burn.index()], 1.0);
    close(t.ailments.stacks(Ailment::Burn), 1.0);
    // the hit took 100; four seconds of burn take 4 × 50 = 200 more, then it is gone
    let lost = t.tick(4.0, false);
    close(lost, 200.0);
    close(t.life, BASE.life - 300.0);
    close(t.ailments.stacks(Ailment::Burn), 0.0);
    close(t.tick(1.0, false), 0.0);
}

#[test]
fn damage_over_time_modifiers_never_touch_hits_and_hit_modifiers_never_touch_burns() {
    let fire = Element::Fire.index();
    let one = |mods: &[Mod]| {
        let b = made(&FIRE_BOLT, mods);
        let o = land(&b.hit, &mut bare());
        (o.dealt[fire], o.dots[0].dps)
    };
    // 100% increased damage over time and +0.5 multiplier: the hit stays 100, the burn 50 × 2 × 1.5 = 150
    let (hit, burn) = one(&[
        Mod::inc(Stat::DotDamage, 1.0, SRC),
        Mod::flat(Stat::DotMulti, 0.5, SRC),
    ]);
    close(hit, 100.0);
    close(burn, 150.0);
    // the fire multiplier counts for a burn, the force one does not: 50 × (1 + 0.3) = 65
    let (_, burn) = one(&[
        Mod::flat(Stat::DotMultiOf(Element::Fire), 0.3, SRC),
        Mod::flat(Stat::DotMultiOf(Element::Force), 1.0, SRC),
    ]);
    close(burn, 65.0);
    // burn more 40% (Hỏa Ngục's kind): the hit 100, the burn 70; bleed damage nothing to a burn
    let (hit, burn) = one(&[
        Mod::more(Stat::AilmentDamage(Ailment::Burn), 0.4, SRC),
        Mod::more(Stat::AilmentDamage(Ailment::Bleed), 1.0, SRC),
    ]);
    close(hit, 100.0);
    close(burn, 70.0);
    // 100% increased hit damage: the hit 200, the burn 50
    let (hit, burn) = one(&[Mod::inc(Stat::HitDamage, 1.0, SRC)]);
    close(hit, 200.0);
    close(burn, 50.0);
    // 50% increased damage, and 20% more: both, 100 × 1.5 × 1.2 = 180 and 50 × 1.5 × 1.2 = 90
    let (hit, burn) = one(&[
        Mod::inc(Stat::Damage, 0.5, SRC),
        Mod::more(Stat::Damage, 0.2, SRC),
    ]);
    close(hit, 180.0);
    close(burn, 90.0);
    // 50% increased fire damage: both, 150 and 75
    let (hit, burn) = one(&[Mod::inc(Stat::DamageOf(Element::Fire), 0.5, SRC)]);
    close(hit, 150.0);
    close(burn, 75.0);
}

#[test]
fn ailment_duration_and_effect_scale_instances_and_stacks() {
    // 50% increased ailment duration: a burn lasts 4 × 1.5 = 6 s
    let b = made(&FIRE_BOLT, &[Mod::inc(Stat::AilmentDuration, 0.5, SRC)]);
    close(land(&b.hit, &mut bare()).dots[0].time, 6.0);
    // bleed duration alone: a bleed lasts 5 × 2 = 10 s, a burn its 4 s
    let mods = [Mod::inc(Stat::AilmentDurationOf(Ailment::Bleed), 1.0, SRC)];
    close(
        land(&made(&FORCE_BOLT, &mods).hit, &mut bare()).dots[0].time,
        10.0,
    );
    close(
        land(&made(&FIRE_BOLT, &mods).hit, &mut bare()).dots[0].time,
        4.0,
    );
    // 50% increased ailment effect: a burn 50 × 1.5 = 75
    let b = made(&FIRE_BOLT, &[Mod::inc(Stat::AilmentEffect, 0.5, SRC)]);
    close(land(&b.hit, &mut bare()).dots[0].dps, 75.0);

    // a stacking ailment reads them too: 300 water lays 1 × 300 / 3000 × 20 = 2 soak stacks, × 1.5 effect = 3,
    // each lasting 4 × 1.5 = 6 s
    let mut h = Hit::split(300.0, &[(Element::Water, 1.0)]);
    h.ailment_effect[Ailment::Soak.index()] = 1.5;
    h.ailment_duration[Ailment::Soak.index()] = 1.5;
    let mut t = bare();
    let o = land(&h, &mut t);
    close(o.stacks[Ailment::Soak.index()], 3.0);
    close(t.ailments.stacks(Ailment::Soak), 3.0);
    for left in &t.ailments.on[Ailment::Soak.index()].left {
        close(*left, 6.0);
    }
}

#[test]
fn only_the_strongest_burn_deals_damage_and_the_weaker_wait() {
    let mut t = bare();
    // (both under 8% of full life, so no body star joins the bearer's table)
    land(&fire(100.0), &mut t);
    land(&fire(200.0), &mut t);
    // two instances, 50 and 100 a second: one second deals 100, not 150
    close(t.ailments.stacks(Ailment::Burn), 2.0);
    close(t.ailments.burns.strongest().map_or(0.0, |d| d.dps), 100.0);
    let life = t.life;
    close(t.tick(1.0, false), 100.0);
    close(t.life, life - 100.0);

    // when the strongest runs out mid-tick the next takes over: 150 × 1 + 50 × 1 = 200 over two seconds
    let mut t = bare();
    t.ailments.lay(Dot::new(Ailment::Burn, 150.0, 1.0));
    t.ailments.lay(Dot::new(Ailment::Burn, 50.0, 4.0));
    close(t.tick(2.0, false), 200.0);
    close(t.ailments.stacks(Ailment::Burn), 1.0);
}

#[test]
fn eight_bleeds_all_bleed_and_a_ninth_replaces_the_weakest() {
    let mut t = bare();
    // seven bleeds of 10, 20, ... 70
    for i in 1..=7 {
        assert!(
            !t.ailments
                .lay(Dot::new(Ailment::Bleed, 10.0 * f64::from(i), 5.0))
        );
    }
    // the eighth from a hit: 100 force × 0.35 = 35 a second; it fills the bleed
    let o = land(&made(&FORCE_BOLT, &[]).hit, &mut t);
    close(o.dots[0].dps, BLEED_RATE * 100.0);
    assert_eq!(o.full, vec![Ailment::Bleed]);
    close(t.ailments.stacks(Ailment::Bleed), DOT_CAP as f64);
    // 10 + ... + 70 = 280, + 35 = 315
    close(total_dps(&t, Ailment::Bleed), 315.0);
    // weaker than all eight: dropped
    assert!(!t.ailments.lay(Dot::new(Ailment::Bleed, 5.0, 5.0)));
    close(total_dps(&t, Ailment::Bleed), 315.0);
    // stronger: takes the 10's place, 315 − 10 + 100 = 405, still eight
    t.ailments.lay(Dot::new(Ailment::Bleed, 100.0, 5.0));
    close(t.ailments.stacks(Ailment::Bleed), 8.0);
    close(total_dps(&t, Ailment::Bleed), 405.0);
    // every one bleeds: a second still takes 405
    close(t.tick(1.0, false), 405.0);
}

#[test]
fn a_bleed_runs_twice_as_fast_on_the_move_and_meets_the_shield_first() {
    // 100 force: 35 a second for 5 s, armour ignored, the shield first
    let o = land(&made(&FORCE_BOLT, &[]).hit, &mut bare());
    let d = o.dots[0];
    close(d.dps, 35.0);
    close(d.time, 5.0);
    let mut t = Cultivator::new();
    t.armour = 10_000.0;
    t.ailments.lay(d);
    let mut moving = t.clone();
    let still = t.tick_with(1.0, false, &EMPTY);
    let bleed = still.of(Ailment::Bleed).copied();
    close(bleed.map_or(0.0, |b| b.to_shield), 35.0);
    close(bleed.map_or(1.0, |b| b.to_life), 0.0);
    let run = moving.tick_with(1.0, true, &EMPTY);
    close(run.of(Ailment::Bleed).map_or(0.0, |b| b.to_shield), 70.0);
    assert_eq!(run.of(Ailment::Bleed).map(|b| b.kind), Some(Element::Force));
}

#[test]
fn a_burn_takes_fire_resistance_and_the_bearers_dot_taken_but_not_penetration() {
    let resist = Sheet {
        extra: vec![Mod::flat(Stat::Resist(Element::Fire), 0.5, SRC)],
        ..Sheet::default()
    };
    // a burn of 100 a second against 50% fire resistance: 50
    let mut t = bare();
    t.ailments.lay(Dot::new(Ailment::Burn, 100.0, 4.0));
    let mut t2 = t.clone();
    close(t.tick_with(1.0, false, &resist).lost, 50.0);
    // the empty sheet: 100
    close(t2.tick(1.0, false), 100.0);

    // + 20% increased damage over time taken and 10% more fire taken: 100 × 0.5 × 1.2 × 1.1 = 66
    let taken = Sheet {
        extra: vec![
            Mod::flat(Stat::Resist(Element::Fire), 0.5, SRC),
            Mod::inc(Stat::DotTaken, 0.2, SRC),
            Mod::more(Stat::TakenOf(Element::Fire), 0.1, SRC),
        ],
        ..Sheet::default()
    };
    let mut t = bare();
    t.ailments.lay(Dot::new(Ailment::Burn, 100.0, 4.0));
    close(t.tick_with(1.0, false, &taken).lost, 66.0);

    // damage over time taken never touches a hit: 100 fire × 0.5 × 1.1 = 55
    let mut t = bare();
    let def = Defence {
        sheet: &taken,
        ..Defence::default()
    };
    let o = resolve_with(&fire(100.0), None, &mut t, &def, NO_CRIT);
    close(o.dealt[Element::Fire.index()], 55.0);

    // penetration is a hit's: the hit passes the resistance, the burn it lays still meets it
    let mut h = fire(100.0);
    h.pen[Element::Fire.index()] = 0.5;
    let mut t = bare();
    let def = Defence {
        sheet: &resist,
        ..Defence::default()
    };
    let o = resolve_with(&h, None, &mut t, &def, NO_CRIT);
    close(o.dealt[Element::Fire.index()], 100.0);
    close(o.dots[0].dps, 50.0);
    // 50 a second, halved by the resistance
    close(t.tick_with(1.0, false, &resist).lost, 25.0);
}

#[test]
fn a_burn_from_converted_force_still_takes_increased_force() {
    // 100 force, all converted to fire, 50% increased force
    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    h.convert[Element::Force.index()][Element::Fire.index()] = 1.0;
    h.increased_by[Element::Force.index()] = 0.5;
    let o = land(&h, &mut bare());
    // the hit: 100 × 1.5 = 150 fire; the burn: 0.5 × 100 × 1.5 = 75
    close(o.dealt[Element::Fire.index()], 150.0);
    close(o.dots[0].dps, 75.0);
    assert_eq!(o.dots[0].ailment, Ailment::Burn);

    // through build: Convert, increased force, and increased hit damage that the burn never sees
    let b = made(
        &FORCE_BOLT,
        &[
            Mod::flat(Stat::Convert(Element::Force, Element::Fire), 1.0, SRC),
            Mod::inc(Stat::DamageOf(Element::Force), 0.5, SRC),
            Mod::inc(Stat::HitDamage, 1.0, SRC),
        ],
    );
    let o = land(&b.hit, &mut bare());
    // the hit: 100 × (1 + 0.5 + 1) = 250; the burn: 0.5 × 100 × 1.5 = 75; no force left, no bleed
    close(o.dealt[Element::Fire.index()], 250.0);
    assert_eq!(o.dots.len(), 1);
    close(o.dots[0].dps, 75.0);
}

#[test]
fn a_fed_burn_has_half_again_its_effect_not_more_instances() {
    // wood feeds fire: on a bound target, 100 fire lays one burn of 50 × 1.5 = 75
    let mut t = bare();
    t.ailments.add(Ailment::Bind, 2.0, 1.0);
    let o = land(&fire(100.0), &mut t);
    close(o.stacks[Ailment::Burn.index()], 1.0);
    close(t.ailments.stacks(Ailment::Burn), 1.0);
    close(o.dots[0].dps, 50.0 * (1.0 + AIL.fed));
}

#[test]
fn water_overcoming_burn_clears_the_instances_and_cashes_them_in() {
    let mut t = bare();
    for _ in 0..3 {
        land(&fire(100.0), &mut t);
    }
    close(t.ailments.stacks(Ailment::Burn), 3.0);
    let o = land(&made(&WATER_BOLT, &[]).hit, &mut t);
    assert_eq!(o.cleared, vec![(Ailment::Burn, 3.0)]);
    // three burns of 50 a second with 4 s left: only the strongest would burn, 50 × 4 = 200 (× OVERCOME_REMAINING 1)
    close(
        o.before[Element::Water.index()],
        100.0 + 50.0 * 4.0 * dot::OVERCOME_REMAINING,
    );
    close(o.before[Element::Water.index()], 300.0);
    assert!(t.ailments.burns.is_empty());
    // nothing burns any more
    close(t.tick(1.0, false), 0.0);
}

#[test]
fn many_tiny_burns_cash_in_only_what_they_would_have_burnt() {
    // eight fire hits of 1 lay eight burns of 0.5 a second; the old rule paid 8 × 2% × 3000 = 480 for them
    let mut t = bare();
    for _ in 0..8 {
        land(&fire(1.0), &mut t);
    }
    close(t.ailments.stacks(Ailment::Burn), 8.0);
    let o = land(&made(&WATER_BOLT, &[]).hit, &mut t);
    assert_eq!(o.cleared, vec![(Ailment::Burn, 8.0)]);
    // only the strongest burns: 0.5 × 4 = 2
    close(o.before[Element::Water.index()], 102.0);
}

#[test]
fn the_remaining_damage_follows_the_ignite_and_bleed_rules() {
    let mut burns = dot::Instances::default();
    burns.lay(Dot::new(Ailment::Burn, 10.0, 6.0));
    burns.lay(Dot::new(Ailment::Burn, 40.0, 2.0));
    // the strong one 40 × 2, then the weak one takes over for its last 4 s: 80 + 40
    close(burns.remaining(), 120.0);
    let mut bleeds = dot::Instances::default();
    bleeds.lay(Dot::new(Ailment::Bleed, 10.0, 6.0));
    bleeds.lay(Dot::new(Ailment::Bleed, 40.0, 2.0));
    // every bleed: 60 + 80, never doubled for moving
    close(bleeds.remaining(), 140.0);
    close(dot::Instances::default().remaining(), 0.0);
}

#[test]
fn hollow_fire_lays_a_half_copy_of_the_strongest_burn_on_each_cast() {
    let mut h = fire(200.0);
    h.hollow_fire = true;
    let mut t = bare();
    land(&h, &mut t);
    assert!(t.ailments.hollow_fire);
    // 100 a second for 4 s; two seconds take 200
    close(t.tick(2.0, false), 200.0);
    // a cast lays a copy of 100 × 0.5 = 50 with a fresh 4 s
    t.on_cast();
    close(t.ailments.stacks(Ailment::Burn), 2.0);
    let copy = t.ailments.burns.0[1];
    close(copy.dps, 100.0 * HOLLOW_FIRE_EFFECT);
    close(copy.left, 4.0);
    // three seconds: the strongest's last 2 s at 100, then the copy for 1 s at 50: 250
    close(t.tick(3.0, false), 250.0);
    close(t.ailments.stacks(Ailment::Burn), 1.0);
    // no hollow fire, no copy
    let mut t = bare();
    land(&fire(200.0), &mut t);
    t.on_cast();
    close(t.ailments.stacks(Ailment::Burn), 1.0);
    // hollow fire goes out with the last burn
    let mut t = bare();
    land(&h, &mut t);
    t.tick(4.0, false);
    assert!(!t.ailments.hollow_fire);
}

#[test]
fn the_fire_tier_scales_the_burn_and_black_fire_full_draws_sense() {
    let mut h = fire(100.0);
    h.fire_tier = 2;
    let o = land(&h, &mut bare());
    // purple: 50 × 1.5 = 75
    close(o.dots[0].dps, 50.0 * dot::FIRE_TIER[2]);
    // black fire filling the burn draws sense
    let mut h = fire(100.0);
    h.fire_tier = 3;
    let mut t = bare();
    for _ in 0..DOT_CAP - 1 {
        land(&h, &mut t);
    }
    let sense = t.sense;
    let o = land(&h, &mut t);
    assert_eq!(o.full, vec![Ailment::Burn]);
    close(t.sense, sense - AIL.dao_fire_sense);
}

#[test]
fn qi_shield_and_life_regen_come_from_the_bearers_table() {
    // two soak stacks: 3 × (1 − 2 × 8%) = 2.52 qi a second
    let mut t = Cultivator::new();
    t.qi = 0.0;
    t.ailments.add(Ailment::Soak, 2.0, 1.0);
    t.tick(1.0, false);
    close(t.qi, BASE.qi_regen * (1.0 - 2.0 * AIL.soak_qi));
    // with 50% increased qi regen worn: 3 × (1 + 0.5 − 0.16) = 4.02
    let sheet = Sheet {
        extra: vec![
            Mod::inc(Stat::QiRegen, 0.5, SRC),
            Mod::flat(Stat::ShieldRegen, 40.0, SRC),
            Mod::flat(Stat::LifeRegen, 10.0, SRC),
        ],
        ..Sheet::default()
    };
    let mut t = Cultivator::new();
    t.qi = 0.0;
    t.shield = 0.0;
    t.life = 1000.0;
    t.ailments.add(Ailment::Soak, 2.0, 1.0);
    t.tick_with(1.0, false, &sheet);
    close(t.qi, BASE.qi_regen * (1.0 + 0.5 - 2.0 * AIL.soak_qi));
    // shield 60 + 40 = 100 a second, life 10 a second
    close(t.shield, BASE.shield_regen + 40.0);
    close(t.life, 1010.0);
    // Ly Phong's "no qi regen" as a timed effect: nothing
    let mut t = Cultivator::new();
    t.qi = 0.0;
    t.put_on(Active {
        source: Source::Domain("Ly Phong"),
        mods: vec![Mod::more(Stat::QiRegen, -1.0, Source::Domain("Ly Phong"))],
        left: None,
    });
    t.tick(1.0, false);
    close(t.qi, 0.0);
    // a conditional rate holds only while it holds: +100% qi regen while moving
    let moving = Sheet {
        extra: vec![Mod::inc(Stat::QiRegen, 1.0, SRC).when(Condition::Moving)],
        ..Sheet::default()
    };
    let mut t = Cultivator::new();
    t.qi = 0.0;
    t.tick_with(1.0, false, &moving);
    close(t.qi, BASE.qi_regen);
    t.tick_with(1.0, true, &moving);
    close(t.qi, BASE.qi_regen * 3.0);
}

#[test]
fn a_cursers_curse_effect_strengthens_the_curse_it_lays() {
    let curser = Cultivator::new();
    let sheet = Sheet {
        extra: vec![Mod::inc(Stat::CurseEffect, 0.5, SRC)],
        ..Sheet::default()
    };
    let effect = curse::curse_effect(&curser, &sheet);
    close(effect, 1.5);
    close(curse::curse_effect(&curser, &EMPTY), 1.0);

    let mut on = Curses::new(1, false);
    let mut by = Curses::new(0, false);
    let c = Curse::new(Kind::ExposeMark(Element::Fire), Rank::Art, 0, 1);
    assert_eq!(curse::cast_with(c, &mut on, &mut by, effect), Lay::Added);
    let laid = on.get(Kind::ExposeMark(Element::Fire)).copied();
    close(laid.map_or(0.0, |c| c.potency), 1.5);
    // Khắc Ấn cuts fire resistance by 20% × 1.5 = 30%; its time does not grow
    let mods = laid.map(|c| c.mods(false)).unwrap_or_default();
    close(mods[0].value, -NUMBERS.expose * 1.5);
    close(
        laid.map_or(0.0, |c| c.left),
        Kind::ExposeMark(Element::Fire).time(),
    );
    // on the bearer: its burn meets −30% fire resistance, 100 × 1.3 = 130
    let mut t = bare();
    on.tick(0.0, &mut t, false);
    t.ailments.lay(Dot::new(Ailment::Burn, 100.0, 4.0));
    close(t.tick(1.0, false), 130.0);
}

#[test]
fn a_tick_of_damage_over_time_is_an_event_that_triggers_listen_for() {
    let mut f = vec![Cultivator::new(), bare()];
    f[1].ailments.lay(Dot::new(Ailment::Burn, 40.0, 4.0));
    f[1].ailments.lay(Dot::new(Ailment::Bleed, 30.0, 5.0));
    let ticked = f[1].tick_with(1.0, false, &EMPTY);
    close(ticked.lost, 70.0);
    let ev = event::dot_events(&ticked, 1, &f[1]);
    let kinds: Vec<EventKind> = ev.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            EventKind::Dotted(Ailment::Burn),
            EventKind::Dotted(Ailment::Bleed)
        ]
    );
    close(ev[0].damage, 40.0);
    close(ev[0].kinds[Element::Fire.index()], 40.0);
    close(ev[1].kinds[Element::Force.index()], 30.0);

    // a trigger on any damage over time taken, and one on any ailment laid
    const DOTTED: Trigger = Trigger {
        id: "dotted",
        name: "dotted",
        on: EventKind::AnyDotted,
        when: Condition::Always,
        filter: Filter::ANY,
        chance: 1.0,
        cooldown: 0.0,
        threshold: 0.0,
        does: Effect::Gain(Gain {
            resource: Resource::Qi,
            amount: 5.0,
        }),
    };
    const LAID: Trigger = Trigger {
        id: "laid",
        on: EventKind::AnyAilmentLaid,
        ..DOTTED
    };
    let seats = [
        Seat {
            triggers: &[LAID],
            ..Seat::default()
        },
        Seat {
            triggers: &[DOTTED],
            ..Seat::default()
        },
    ];
    for c in &mut f {
        c.qi = 0.0;
    }
    let mut q = Queue::new();
    q.extend(ev);
    q.push(event::Event::new(EventKind::AilmentLaid(Ailment::Jolt), 0).against(1));
    q.push(event::Event::new(EventKind::AilmentFull(Ailment::Jolt), 0).against(1));
    let out = q.run(&mut f, &seats, &mut Dice::new(&[]));
    // two ticks for the bearer, one laid for the striker (the full is not "laid")
    assert_eq!(out.fired.len(), 3);
    close(f[1].qi, 10.0);
    close(f[0].qi, 5.0);
    assert_eq!(f[1].recent.since(Happened::BeenDotted), 0.0);
    assert!(EventKind::AnyAilmentFull.takes(EventKind::AilmentFull(Ailment::Burn)));
    assert!(!EventKind::AnyAilmentFull.takes(EventKind::AilmentLaid(Ailment::Burn)));
    assert!(EventKind::Hit.takes(EventKind::Hit));
}

#[test]
fn every_new_event_marks_its_happened() {
    let cases = [
        (EventKind::Evaded, Happened::Evaded),
        (EventKind::Cursed, Happened::Cursed),
        (EventKind::MinionDied, Happened::MinionDied),
        (EventKind::LowLife(event::LifeMark::Low), Happened::LowLife),
        (EventKind::Rewound, Happened::Rewound),
        (EventKind::DomainCollapsed, Happened::DomainCollapsed),
        (EventKind::Dotted(Ailment::Bleed), Happened::BeenDotted),
    ];
    for (k, h) in cases {
        assert_eq!(k.happened(), Some(h));
        let mut f = vec![Cultivator::new()];
        event::mark(&[event::Event::new(k, 0)], &mut f);
        assert_eq!(f[0].recent.since(h), 0.0);
    }
    assert_eq!(EventKind::AnyDotted.happened(), None);
    assert_eq!(Happened::ALL.len(), Happened::COUNT);
}

#[test]
fn the_same_fight_of_burns_and_bleeds_is_the_same_every_time() {
    let run = || {
        let mut t = Cultivator::new().with_root(Element::Metal);
        let mut h = Hit::split(
            420.0,
            &[
                (Element::Force, 0.5),
                (Element::Fire, 0.3),
                (Element::Wood, 0.2),
            ],
        );
        h.convert[Element::Force.index()][Element::Fire.index()] = 0.25;
        h.increased_by[Element::Force.index()] = 0.3;
        h.dot[Ailment::Burn.index()].increased = 0.7;
        h.dot[Ailment::Bleed.index()].multi = 1.4;
        h.crit = 0.5;
        h.hollow_fire = true;
        let mut log = Vec::new();
        for i in 0..40 {
            let o = resolve(
                &h,
                None,
                &mut t,
                Rolls {
                    evade: 1.0,
                    crit: f64::from(i % 3) / 3.0,
                    ..Rolls::NONE
                },
            );
            log.push(o.total().to_bits());
            if i % 5 == 0 {
                t.on_cast();
            }
            let ticked = t.tick_with(1.0 / 7.0, i % 2 == 0, &EMPTY);
            log.push(ticked.lost.to_bits());
            log.push(t.life.to_bits());
            log.push(t.shield.to_bits());
        }
        log
    };
    assert_eq!(run(), run());
}

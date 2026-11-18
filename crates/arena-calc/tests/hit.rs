//! The damage system's core: one blow through the twelve steps, the five phases, fusion, ailments, the three bars
//! and grit (`docs/specs/2026-11-17-damage-system-design.md`, sections 4 and 5).

use arena_calc::ailment::{AIL, Ailment, Control, ForceAilment};
use arena_calc::cultivator::{BASE, Cultivator};
use arena_calc::dot::Dot;
use arena_calc::element::{Element, Fusion, N};
use arena_calc::grade::{Grade, Rank, dispels, shield_stops};
use arena_calc::hit::{Frame, Hit, Rolls, resolve, scale, spell_base};
use arena_calc::tag::{Tag, Tags};

const NO_CRIT: Rolls = Rolls {
    evade: 1.0,
    crit: 1.0,
    ..Rolls::NONE
};

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

#[test]
fn a_plain_spell_lands_whole_on_a_bare_target() {
    let mut t = bare();
    let o = resolve(
        &Hit::split(300.0, &[(Element::Fire, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Fire.index()], 300.0);
    close(o.to_life, 300.0);
    close(t.life, BASE.life - 300.0);
}

#[test]
fn conversion_runs_forward_scales_past_whole_and_remembers_what_it_was() {
    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    h.convert[Element::Force.index()][Element::Fire.index()] = 1.0;
    h.convert[Element::Force.index()][Element::Thunder.index()] = 1.0;
    // backward: ignored
    h.convert[Element::Fire.index()][Element::Metal.index()] = 1.0;
    let p = scale(&h, 1.0);
    close(p[Element::Fire.index()], 50.0);
    close(p[Element::Thunder.index()], 50.0);
    close(p[Element::Force.index()], 0.0);
    close(p[Element::Metal.index()], 0.0);

    // "increased force" still touches the force that became fire
    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    h.convert[Element::Force.index()][Element::Fire.index()] = 1.0;
    h.increased_by[Element::Force.index()] = 0.5;
    h.increased_by[Element::Fire.index()] = 0.5;
    close(scale(&h, 1.0)[Element::Fire.index()], 200.0);
}

#[test]
fn chained_conversion_carries_every_kind_it_passed() {
    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    h.convert[Element::Force.index()][Element::Fire.index()] = 1.0;
    h.convert[Element::Fire.index()][Element::Yin.index()] = 1.0;
    h.more_by[Element::Fire.index()] = 0.6;
    let p = scale(&h, 1.0);
    close(p[Element::Yin.index()], 160.0);
}

#[test]
fn increases_add_and_mores_multiply() {
    let mut h = Hit::split(100.0, &[(Element::Metal, 1.0)]);
    h.increased = 0.3;
    h.increased_by[Element::Metal.index()] = 0.2;
    h.more = vec![0.5, 0.2];
    close(
        scale(&h, 1.0)[Element::Metal.index()],
        100.0 * 1.5 * 1.5 * 1.2,
    );
}

#[test]
fn added_damage_is_scaled_by_effectiveness() {
    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    h.added[Element::Earth.index()] = 40.0;
    h.effectiveness = 0.5;
    close(scale(&h, 1.0)[Element::Earth.index()], 20.0);
}

#[test]
fn crit_multiplies_but_never_an_instant() {
    let mut t = bare();
    let mut h = Hit::split(100.0, &[(Element::Light, 1.0)]);
    h.crit = 1.0;
    let o = resolve(
        &h,
        None,
        &mut t,
        Rolls {
            evade: 1.0,
            crit: 0.0,
            ..Rolls::NONE
        },
    );
    assert!(o.crit);
    close(o.dealt[Element::Light.index()], 150.0);
    h.instant = true;
    let o = resolve(
        &h,
        None,
        &mut t,
        Rolls {
            evade: 1.0,
            crit: 0.0,
            ..Rolls::NONE
        },
    );
    assert!(!o.crit);
}

#[test]
fn evasion_dodges_attacks_and_projectiles_but_not_instants() {
    let mut t = bare();
    t.evasion = 0.5;
    let mut h = Hit::split(100.0, &[(Element::Force, 1.0)]);
    h.evadable = Tags::of(&[Tag::Attack, Tag::Melee]).evadable();
    let o = resolve(
        &h,
        None,
        &mut t,
        Rolls {
            evade: 0.1,
            crit: 1.0,
            ..Rolls::NONE
        },
    );
    assert!(o.evaded);
    assert!(!Tags::of(&[Tag::Spell, Tag::Instant, Tag::Projectile]).evadable());
    assert!(!Tags::of(&[Tag::Spell, Tag::Area]).evadable());
}

#[test]
fn armour_cuts_small_force_blows_more_than_big_ones() {
    let mut t = Cultivator::new();
    t.shield = 0.0;
    let small = resolve(
        &Hit::split(100.0, &[(Element::Force, 1.0)]),
        None,
        &mut t.clone(),
        NO_CRIT,
    );
    let big = resolve(
        &Hit::split(1000.0, &[(Element::Force, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    // 600 / (600 + 500) and 600 / (600 + 5000)
    close(small.dealt[0], 100.0 * (1.0 - 600.0 / 1100.0));
    close(big.dealt[0], 1000.0 * (1.0 - 600.0 / 5600.0));
    assert!(big.dealt[0] / 1000.0 > small.dealt[0] / 100.0);
}

#[test]
fn resistance_is_capped_and_penetration_works_past_the_cap() {
    let mut t = bare();
    t.resist[Element::Fire.index()] = 0.9;
    let mut h = Hit::split(100.0, &[(Element::Fire, 1.0)]);
    let o = resolve(&h, None, &mut t.clone(), NO_CRIT);
    close(o.dealt[Element::Fire.index()], 25.0);
    h.pen[Element::Fire.index()] = 0.25;
    let o = resolve(&h, None, &mut t, NO_CRIT);
    close(o.dealt[Element::Fire.index()], 50.0);
}

#[test]
fn the_five_phases_against_a_root() {
    let hit = |e| Hit::split(100.0, &[(e, 1.0)]);
    let on = |root: Element, e: Element| {
        let mut t = bare().with_root(root);
        resolve(&hit(e), None, &mut t, NO_CRIT).dealt[e.index()]
    };
    close(on(Element::Fire, Element::Water), 125.0); // water overcomes fire
    close(on(Element::Fire, Element::Metal), 80.0); // fire overcomes metal
    close(on(Element::Fire, Element::Fire), 70.0);
    close(on(Element::Fire, Element::Wood), 100.0); // wood feeds fire
    close(on(Element::Light, Element::Yin), 150.0);
    close(on(Element::Yin, Element::Light), 150.0);
    close(on(Element::Fire, Element::Thunder), 100.0);

    let mut t = bare().with_root(Element::Fire);
    t.qi = 0.0;
    let o = resolve(&hit(Element::Fire), None, &mut t, NO_CRIT);
    close(o.target_qi, 7.0);
    close(t.qi, 7.0);

    // dao fire overcomes a water root
    let mut t = bare().with_root(Element::Water);
    let mut h = hit(Element::Fire);
    h.dao_fire = true;
    close(
        resolve(&h, None, &mut t, NO_CRIT).dealt[Element::Fire.index()],
        125.0,
    );
}

#[test]
fn an_overcoming_blow_clears_the_ailment_and_cashes_its_stacks() {
    // fire overcomes metal: three stacks of sunder cash in 3 × 2% of full life
    let mut t = bare();
    t.ailments.add(Ailment::Sunder, 3.0, 1.0);
    let o = resolve(
        &Hit::split(100.0, &[(Element::Fire, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    assert_eq!(o.cleared, vec![(Ailment::Sunder, 3.0)]);
    close(
        o.before[Element::Fire.index()],
        100.0 + 3.0 * AIL.overcome * BASE.life,
    );
    assert_eq!(t.ailments.stacks(Ailment::Sunder), 0.0);
    // burn instances cash in what they would still burn: bare ones (no damage) bring nothing
    let mut t = bare();
    t.ailments.add(Ailment::Burn, 3.0, 1.0);
    let o = resolve(
        &Hit::split(100.0, &[(Element::Water, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    assert_eq!(o.cleared, vec![(Ailment::Burn, 3.0)]);
    close(o.before[Element::Water.index()], 100.0);
    assert_eq!(t.ailments.stacks(Ailment::Burn), 0.0);
}

#[test]
fn thunder_conducts_through_soak() {
    let mut t = bare();
    t.ailments.add(Ailment::Soak, 4.0, 1.0);
    let mut cleared = t.clone();
    let o = resolve(
        &Hit::split(100.0, &[(Element::Thunder, 1.0)]),
        None,
        &mut cleared,
        NO_CRIT,
    );
    assert_eq!(o.cleared, vec![(Ailment::Soak, 4.0)]);
    // soak cleared at step 7, so step 8's soak bonus is gone
    close(
        o.dealt[Element::Thunder.index()],
        100.0 + 4.0 * AIL.overcome * BASE.life,
    );
}

#[test]
fn a_fed_ailment_lays_half_again() {
    // water feeds wood: 1 × 300 / 3000 × 20 = 2 bind stacks, 3 on a soaked target
    let mut plain = bare();
    let o1 = resolve(
        &Hit::split(300.0, &[(Element::Wood, 1.0)]),
        None,
        &mut plain,
        NO_CRIT,
    );
    close(o1.stacks[Ailment::Bind.index()], 2.0);
    let mut soaked = bare();
    soaked.ailments.add(Ailment::Soak, 2.0, 1.0);
    let o2 = resolve(
        &Hit::split(300.0, &[(Element::Wood, 1.0)]),
        None,
        &mut soaked,
        NO_CRIT,
    );
    close(o2.stacks[Ailment::Bind.index()], 3.0);

    // wood feeds fire: a burn is one instance either way, with half again its effect (0.5 × 300 = 150, then 225)
    let mut plain = bare();
    let o1 = resolve(
        &Hit::split(300.0, &[(Element::Fire, 1.0)]),
        None,
        &mut plain,
        NO_CRIT,
    );
    let mut bound = bare();
    bound.ailments.add(Ailment::Bind, 2.0, 1.0);
    let o2 = resolve(
        &Hit::split(300.0, &[(Element::Fire, 1.0)]),
        None,
        &mut bound,
        NO_CRIT,
    );
    close(o1.stacks[Ailment::Burn.index()], 1.0);
    close(o2.stacks[Ailment::Burn.index()], 1.0);
    close(o1.dots[0].dps, 150.0);
    close(o2.dots[0].dps, 225.0);
}

#[test]
fn fusion_spends_the_window_once() {
    let mut t = bare();
    resolve(
        &Hit::split(200.0, &[(Element::Fire, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    let o = resolve(
        &Hit::split(100.0, &[(Element::Water, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    // the water also clears the burn: fusion reads the window, made before
    assert_eq!(o.fusion.map(|f| f.0), Some(Fusion::Wind));
    close(o.fusion.map_or(0.0, |f| f.1), 0.4 * 100.0);
    assert!(t.wind_lock > 0.0);
    let o = resolve(
        &Hit::split(100.0, &[(Element::Water, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    assert!(o.fusion.is_none());
}

#[test]
fn fusion_ignores_resistance_and_the_window_runs_out() {
    let mut t = bare();
    t.resist = [0.75; N];
    resolve(
        &Hit::split(100.0, &[(Element::Yin, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    let o = resolve(
        &Hit::split(100.0, &[(Element::Light, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    assert_eq!(o.fusion.map(|f| f.0), Some(Fusion::Duality));
    close(o.fusion.map_or(0.0, |f| f.1), 40.0);
    close(o.recoil, 10.0);

    let mut t = bare();
    resolve(
        &Hit::split(100.0, &[(Element::Thunder, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    t.tick(1.1, false);
    let o = resolve(
        &Hit::split(100.0, &[(Element::Fire, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    assert!(o.fusion.is_none());
}

#[test]
fn no_pair_makes_ice() {
    for f in Fusion::ALL {
        assert!(!["ice", "frost", "freeze"].contains(&f.name()));
    }
}

#[test]
fn yin_passes_the_shield_and_soul_strikes_the_sense() {
    let mut t = Cultivator::new();
    let o = resolve(
        &Hit::split(100.0, &[(Element::Yin, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.to_shield, 0.0);
    close(o.to_life, 100.0);

    let mut t = Cultivator::new();
    t.sense = 30.0;
    let o = resolve(
        &Hit::split(100.0, &[(Element::Soul, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.to_sense, 30.0);
    close(o.to_life, 35.0);
    assert!(t.soul_lost > 0.0);

    let mut t = Cultivator::new();
    t.soulless_body = true;
    let o = resolve(
        &Hit::split(100.0, &[(Element::Soul, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.to_life + o.to_sense, 0.0);
}

#[test]
fn the_shield_takes_first_and_waits_after_breaking() {
    let mut t = Cultivator::new();
    t.armour = 0.0;
    let o = resolve(
        &Hit::split(700.0, &[(Element::Metal, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.to_shield, 600.0);
    close(o.to_life, 100.0);
    t.tick(2.5, false);
    close(t.shield, 0.0);
    t.tick(1.0, false);
    assert!(t.shield > 0.0);
}

#[test]
fn ailments_fill_fire_their_effect_and_lock() {
    let mut t = bare();
    let mut h = Hit::split(900.0, &[(Element::Force, 1.0)]);
    h.force_ailment = ForceAilment::Daze;
    // 1.2 × 900 / 3000 × 20 = 7.2 stacks, then 14.4: full at 10
    resolve(&h, None, &mut t, NO_CRIT);
    let o = resolve(&h, None, &mut t, NO_CRIT);
    assert_eq!(o.full, vec![Ailment::Daze]);
    assert_eq!(o.controls, vec![(Control::Stun, 0.5)]);
    let o = resolve(&h, None, &mut t, NO_CRIT);
    assert!(o.full.is_empty());
    assert_eq!(t.ailments.stacks(Ailment::Daze), 0.0);
}

#[test]
fn full_soak_chokes_and_full_sunder_breaks_the_shield() {
    let mut t = bare();
    resolve(
        &Hit::split(1000.0, &[(Element::Water, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    assert!(t.choke > 0.0);
    let q = t.qi;
    t.tick(0.5, false);
    close(t.qi, q);

    let mut t = Cultivator::new();
    t.resist[Element::Metal.index()] = 0.75;
    resolve(
        &Hit::split(800.0, &[(Element::Metal, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(t.shield, 0.0);
}

#[test]
fn jolt_soak_and_wither_raise_what_is_taken() {
    let mut t = bare();
    t.ailments.add(Ailment::Jolt, 3.0, 1.0);
    t.ailments.add(Ailment::Wither, 2.0, 1.0);
    let o = resolve(
        &Hit::split(100.0, &[(Element::Yin, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Yin.index()], 100.0 * 1.12 * 1.06);
}

#[test]
fn light_and_yin_cancel_stack_for_stack() {
    let mut t = bare();
    t.ailments.add(Ailment::Wither, 4.0, 1.0);
    // 1 × 300 / 3000 × 20 = 2 stacks of glare, spent on 2 of wither
    resolve(
        &Hit::split(300.0, &[(Element::Light, 1.0)]),
        None,
        &mut t,
        NO_CRIT,
    );
    assert_eq!(t.ailments.stacks(Ailment::Wither), 2.0);
    assert_eq!(t.ailments.stacks(Ailment::Glare), 0.0);
}

#[test]
fn every_bleed_deals_moving_or_not_and_burn_passes_armour() {
    // five bleeds of 20 a second, all dealing: 100 in a second still, `BLEED_MOVING` times that moving; no shield, no
    // armour counted
    let mut t = bare();
    t.armour = 5000.0;
    for _ in 0..5 {
        t.ailments.lay(Dot::new(Ailment::Bleed, 20.0, 5.0));
    }
    let still = t.clone().tick(1.0, false);
    let moving = t.tick(1.0, true);
    close(still, 100.0);
    close(moving, arena_calc::dot::BLEED_MOVING * still);
    close(moving, 100.0);

    // a burn of 50 a second goes past the full shield and the armour, straight to life
    let mut t = Cultivator::new();
    t.ailments.lay(Dot::new(Ailment::Burn, 50.0, 4.0));
    close(t.tick(1.0, false), 50.0);
    close(t.shield, BASE.shield);
    close(t.life, BASE.life - 50.0);
}

#[test]
fn grit_halves_each_control_and_falls_back() {
    let mut t = Cultivator::new();
    close(t.control(0.8, false), 0.8);
    close(t.control(0.8, false), 0.4);
    close(t.control(0.8, true), 0.1);
    close(t.control(0.8, false), 0.1);
    t.tick(2.0, false);
    close(t.grit, 2.0);
}

#[test]
fn counter_and_punish_add_a_fifth() {
    let mut t = bare();
    let mut h = Hit::split(100.0, &[(Element::Earth, 1.0)]);
    h.frame = Frame::Counter;
    close(
        resolve(&h, None, &mut t, NO_CRIT).dealt[Element::Earth.index()],
        120.0,
    );
}

#[test]
fn illusions_deal_half_and_lay_nothing() {
    let mut t = bare();
    let mut h = Hit::split(900.0, &[(Element::Fire, 1.0)]);
    h.illusory = true;
    let o = resolve(&h, None, &mut t, NO_CRIT);
    close(o.dealt[Element::Fire.index()], 450.0);
    assert_eq!(t.ailments.stacks(Ailment::Burn), 0.0);
}

#[test]
fn wither_cuts_healing_and_rot_turns_it_to_harm() {
    let mut t = Cultivator::new();
    t.life = 1000.0;
    t.ailments.add(Ailment::Wither, 5.0, 1.0);
    close(t.heal(100.0), 50.0);
    t.rot = 1.0;
    close(t.heal(100.0), -50.0);
}

#[test]
fn intent_and_age_scale_a_strikers_blows() {
    let mut by = Cultivator::new();
    by.intent = 80.0;
    let mut t = bare();
    let o = resolve(
        &Hit::split(100.0, &[(Element::Metal, 1.0)]),
        Some(&by),
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Metal.index()], 130.0);
    by.intent = 0.0;
    by.lifespan = 0.0;
    let o = resolve(
        &Hit::split(100.0, &[(Element::Metal, 1.0)]),
        Some(&by),
        &mut t,
        NO_CRIT,
    );
    close(o.dealt[Element::Metal.index()], 80.0);
}

#[test]
fn rejection_bursts_after_mixing_immortal_and_ancient() {
    let mut c = Cultivator::new();
    let mut burst = false;
    for i in 0..10 {
        burst |= c.cast_school(i % 2 == 0);
        c.tick(0.1, false);
    }
    assert!(burst);
}

#[test]
fn ranks_override_shields_and_restrictions_stand_outside() {
    let r = |n| Grade::Ranked(Rank::from_level(n));
    assert!(shield_stops(r(2), r(2)));
    assert!(!shield_stops(r(2), r(3)));
    assert!(shield_stops(Grade::Restriction, r(4)));
    assert!(!shield_stops(Grade::Restriction, r(5)));
    assert!(!dispels(Rank::Faith, Grade::Restriction, false));
    assert!(!dispels(Rank::Origin, r(4), true));
    assert!(dispels(Rank::Faith, r(4), true));
    assert_eq!(Rank::of_skill(Rank::Divine, true, true), Rank::Origin);
    assert_eq!(Rank::of_skill(Rank::Rule, true, false), Rank::Rule);
}

#[test]
fn a_spells_base_grows_seven_percent_a_level() {
    close(spell_base(100.0, 1), 100.0);
    close(spell_base(100.0, 2), 107.0);
    close(spell_base(100.0, 30), spell_base(100.0, 20));
}

#[test]
fn the_same_blow_on_the_same_state_is_the_same_everywhere() {
    let run = || {
        let mut t = Cultivator::new().with_root(Element::Wood);
        let mut h = Hit::split(420.0, &[(Element::Metal, 0.5), (Element::Fire, 0.5)]);
        h.convert[Element::Metal.index()][Element::Thunder.index()] = 0.3;
        h.crit = 0.5;
        let mut log = Vec::new();
        for i in 0..20 {
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
            t.tick(1.0 / 60.0, i % 2 == 0);
        }
        log
    };
    assert_eq!(run(), run());
}

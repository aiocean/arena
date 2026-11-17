//! The field's rules in the damage system: auras and the contest between them (rules 7.1, 7.2), domains (7.3), one-use
//! cards, Lộ bài, the feint and the bait (8.4, research 10), and the rows of rules 10 that fall to them.

use arena_calc::aura::{
    AURA, AuraKind, Auras, CONTEST, Contest, Mood, Side, Unlit, counters, force, overlap,
};
use arena_calc::card::{BAIT, CARD, Card, FEINT, Feint, GuardWatch, bait, fusable};
use arena_calc::cultivator::{BASE, Cultivator};
use arena_calc::domain::{
    DOMAIN, DomainKind, Field, MinionFate, Raised, Rules, ShotRule, can_raise, cost_factor,
};
use arena_calc::grade::{Grade, Rank};
use arena_calc::tag::{Tag, Tags};

const TICK: f64 = 1.0 / 60.0;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// Lights `kind` and runs out its windup.
fn lit(auras: &mut Auras, kind: AuraKind, rank: Rank, own: bool, c: &mut Cultivator) {
    assert_eq!(auras.light(kind, rank, own, c), Ok(()));
    auras.tick(AURA.windup, c);
    assert!(auras.lit(kind));
}

// --- 7.1 Ý cảnh ---

#[test]
fn every_aura_holds_20_to_35_sense_and_the_lists_split_self_from_pressing() {
    for k in AuraKind::ALL {
        let r = k.data().reserve;
        assert!((20.0..=35.0).contains(&r), "{k:?} holds {r}");
    }
    let selfs = [
        AuraKind::Slaughter,
        AuraKind::Years,
        AuraKind::Belief,
        AuraKind::War,
    ];
    for k in AuraKind::ALL {
        assert_eq!(k.pressing(), !selfs.contains(&k), "{k:?}");
    }
}

#[test]
fn lighting_reserves_sense_and_lowers_the_cap() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(3);
    a.light(AuraKind::Dispassion, Rank::Art, true, &mut c).ok();
    assert!(close(c.sense_reserved, 30.0));
    assert!(close(c.sense, 70.0));
    // Regen stops at the cap left.
    c.tick(10.0, false);
    assert!(close(c.sense, 70.0));
    assert!(a.put_out(AuraKind::Dispassion, &mut c));
    assert!(close(c.sense_reserved, 0.0));
}

#[test]
fn slots_hold_one_to_three_and_sense_must_suffice() {
    assert_eq!(Auras::new(0).slots, 1);
    assert_eq!(Auras::new(9).slots, 3);
    let mut c = Cultivator::new();
    let mut a = Auras::new(1);
    assert_eq!(a.light(AuraKind::War, Rank::Art, true, &mut c), Ok(()));
    assert_eq!(
        a.light(AuraKind::War, Rank::Art, true, &mut c),
        Err(Unlit::AlreadyOn)
    );
    assert_eq!(
        a.light(AuraKind::Ease, Rank::Art, true, &mut c),
        Err(Unlit::NoSlot)
    );

    let mut c = Cultivator::new();
    let mut a = Auras::new(3);
    for k in [AuraKind::Moods, AuraKind::Rebirth] {
        assert_eq!(a.light(k, Rank::Art, true, &mut c), Ok(()));
    }
    // 70 held, 30 free: Tuế nguyệt (30) fits, nothing more would.
    assert_eq!(a.light(AuraKind::SoulLock, Rank::Art, true, &mut c), Ok(()));
    assert!(close(c.sense_reserved, 100.0));
    let mut b = Auras::new(3);
    assert_eq!(
        b.light(AuraKind::Charm, Rank::Art, true, &mut c),
        Err(Unlit::NoSense)
    );
}

#[test]
fn an_aura_works_only_after_a_one_second_windup() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(1);
    a.light(AuraKind::Ease, Rank::Art, true, &mut c).ok();
    assert!(!a.lit(AuraKind::Ease));
    assert_eq!(a.press_on(0.0, 1.0, false).speed, 0.0);
    for _ in 0..59 {
        a.tick(TICK, &mut c);
    }
    assert!(!a.lit(AuraKind::Ease));
    a.tick(TICK, &mut c);
    assert!(a.lit(AuraKind::Ease));
    assert!(a.press_on(0.0, 1.0, false).speed > 0.0);
}

#[test]
fn soul_loss_puts_every_aura_out_and_relighting_takes_the_windup_again() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(2);
    lit(&mut a, AuraKind::Ease, Rank::Art, true, &mut c);
    lit(&mut a, AuraKind::War, Rank::Art, true, &mut c);
    // Running the sense out costs the soul.
    c.hurt_soul(500.0);
    assert!(c.soul_lost > 0.0);
    a.tick(TICK, &mut c);
    assert!(a.on.is_empty());
    assert_eq!(a.out.len(), 2);
    assert!(close(c.sense_reserved, 0.0));
    // Not while the soul is lost.
    assert_eq!(a.relight(&mut c), 0);
    assert_eq!(
        a.light(AuraKind::Years, Rank::Art, true, &mut c),
        Err(Unlit::SoulLost)
    );
    c.tick(BASE.soulless, false);
    c.sense = c.sense_max;
    assert_eq!(a.relight(&mut c), 2);
    assert!(a.out.is_empty());
    assert!(!a.lit(AuraKind::Ease));
    a.tick(AURA.windup, &mut c);
    assert!(a.lit(AuraKind::Ease) && a.lit(AuraKind::War));
    assert!(close(c.sense_reserved, 45.0));
}

#[test]
fn pressing_auras_reach_three_bodies() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(1);
    lit(&mut a, AuraKind::Severance, Rank::Art, true, &mut c);
    assert!(a.press_on(0.0, 3.0, false).taken > 0.0);
    assert_eq!(a.press_on(0.0, 3.01, false), a.press_on(0.0, 100.0, false));
    assert_eq!(a.press_on(0.0, 3.01, false).taken, 0.0);
}

#[test]
fn self_auras_do_what_they_say() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(3);
    lit(&mut a, AuraKind::Slaughter, Rank::Art, true, &mut c);
    lit(&mut a, AuraKind::Belief, Rank::Art, true, &mut c);
    lit(&mut a, AuraKind::War, Rank::Art, true, &mut c);
    c.intent = 50.0;
    assert!(a.more(&c) > 1.0);
    assert!(a.unbroken_fists());
    assert!(a.no_flinch(true));
    assert!(!a.no_flinch(false));
    assert_eq!(a.press_on(0.0, 1.0, false), a.press_on(0.0, 100.0, false));
}

#[test]
fn years_slows_a_foes_projectile_40_percent_but_not_ones_own_nor_a_higher_one() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(1);
    lit(&mut a, AuraKind::Years, Rank::Divine, true, &mut c);
    let art = Grade::Ranked(Rank::Art);
    assert!(close(a.shot_speed(0.0, 2.0, art, true), 0.6));
    assert!(close(a.shot_speed(0.0, 2.0, art, false), 1.0));
    assert!(close(a.shot_speed(0.0, 5.0, art, true), 1.0));
    assert!(close(
        a.shot_speed(0.0, 2.0, Grade::Ranked(Rank::Rule), true),
        1.0
    ));
    assert!(close(a.shot_speed(0.0, 2.0, Grade::Restriction, true), 0.6));
}

#[test]
fn frenzy_gives_qi_when_a_foe_casts_inside() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(1);
    lit(&mut a, AuraKind::Frenzy, Rank::Art, true, &mut c);
    c.qi = 10.0;
    let got = a.on_foe_cast(0.0, 2.0, &mut c);
    assert!(got > 0.0 && close(c.qi, 10.0 + got));
    assert_eq!(a.on_foe_cast(0.0, 9.0, &mut c), 0.0);
    c.qi = c.qi_max;
    assert_eq!(a.on_foe_cast(0.0, 2.0, &mut c), 0.0);
}

#[test]
fn dispassion_spills_onto_sense() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(1);
    lit(&mut a, AuraKind::Dispassion, Rank::Art, true, &mut c);
    assert!(a.spill_sense() > 0.0);
}

#[test]
fn moods_turn_through_four_states() {
    let p = AURA.mood_period;
    assert_eq!(Mood::at(0.0), Mood::Joy);
    assert_eq!(Mood::at(p), Mood::Anger);
    assert_eq!(Mood::at(2.0 * p), Mood::Sorrow);
    assert_eq!(Mood::at(3.0 * p), Mood::Resentment);
    assert_eq!(Mood::at(4.0 * p), Mood::Joy);
    let mut c = Cultivator::new();
    let mut a = Auras::new(1);
    lit(&mut a, AuraKind::Moods, Rank::Art, true, &mut c);
    let first = a.press_on(0.0, 1.0, false);
    a.tick(p, &mut c);
    let second = a.press_on(0.0, 1.0, false);
    assert_ne!(first, second);
    // Tâm Xám shrugs off the emotions.
    assert_eq!(a.press_on(0.0, 1.0, true), a.press_on(0.0, 100.0, false));
}

#[test]
fn a_dispel_takes_auras_not_higher_and_faith_wipes_them_all() {
    let mut c = Cultivator::new();
    let mut a = Auras::new(2);
    lit(&mut a, AuraKind::Ease, Rank::Art, true, &mut c);
    lit(&mut a, AuraKind::War, Rank::Origin, true, &mut c);
    assert_eq!(a.dispel(Rank::Divine, &mut c), 1);
    assert!(a.lit(AuraKind::War) && !a.lit(AuraKind::Ease));
    assert!(close(c.sense_reserved, 20.0));
    assert_eq!(a.wipe(Rank::Origin, &mut c), 0);
    assert_eq!(a.wipe(Rank::Faith, &mut c), 1);
    assert!(close(c.sense_reserved, 0.0));
}

// --- 7.2 Tranh đạo niệm ---

#[test]
fn the_counters_table() {
    use AuraKind::*;
    let pairs = [
        (Rebirth, Severance),
        (Rebirth, Years),
        (Lust, Dispassion),
        (Slaughter, Ease),
        (Slaughter, Charm),
        (SoulLock, Lust),
        (SoulLock, Charm),
        (Dispassion, Memory),
        (Dispassion, Infatuation),
    ];
    let mut n = 0;
    for a in AuraKind::ALL {
        for b in AuraKind::ALL {
            let want = pairs.contains(&(a, b));
            assert_eq!(counters(a, b), want, "{a:?} > {b:?}");
            n += usize::from(want);
        }
    }
    assert_eq!(n, pairs.len());
}

#[test]
fn force_is_rank_plus_realised_plus_counter_plus_sense() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(2);
    let mut b = Auras::new(1);
    lit(&mut a, AuraKind::Dispassion, Rank::Rule, true, &mut ca);
    lit(&mut b, AuraKind::Memory, Rank::Rule, false, &mut cb);
    // 3 + 0.5 + 1 + 70 / 100.
    assert!(close(force(&a, &b, &ca), 3.0 + 0.5 + 1.0 + 0.7));
    // 3 + 0 + 0 + 75 / 100.
    assert!(close(force(&b, &a, &cb), 3.0 + 0.75));
    // A self aura alone enters no contest.
    let mut s = Auras::new(1);
    lit(
        &mut s,
        AuraKind::War,
        Rank::Faith,
        true,
        &mut Cultivator::new(),
    );
    assert_eq!(force(&s, &b, &ca), 0.0);
}

#[test]
fn a_counter_against_a_self_aura_still_counts() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(1);
    let mut b = Auras::new(2);
    lit(&mut a, AuraKind::Rebirth, Rank::Art, false, &mut ca);
    lit(&mut b, AuraKind::Ease, Rank::Art, false, &mut cb);
    let without = force(&a, &b, &ca);
    lit(&mut b, AuraKind::Years, Rank::Art, false, &mut cb);
    assert!(close(force(&a, &b, &ca) - without, CONTEST.counter));
}

#[test]
fn the_contest_opens_only_when_two_pressing_zones_overlap() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(1);
    let mut b = Auras::new(1);
    lit(&mut a, AuraKind::Ease, Rank::Art, true, &mut ca);
    lit(&mut b, AuraKind::Memory, Rank::Art, true, &mut cb);
    assert!(overlap(&a, 0.0, &b, 5.9));
    assert!(!overlap(&a, 0.0, &b, 6.0));
    let mut self_only = Auras::new(1);
    lit(
        &mut self_only,
        AuraKind::War,
        Rank::Art,
        true,
        &mut Cultivator::new(),
    );
    assert!(!overlap(&a, 0.0, &self_only, 1.0));
}

fn run(
    contest: &mut Contest,
    secs: f64,
    a: (&mut Auras, &mut Cultivator, f64),
    b: (&mut Auras, &mut Cultivator, f64),
) -> Option<usize> {
    let steps = libm::round(secs / TICK) as usize;
    for _ in 0..steps {
        let lost = contest.tick(
            TICK,
            Side {
                auras: a.0,
                fighter: a.1,
                at: a.2,
            },
            Side {
                auras: b.0,
                fighter: b.1,
                at: b.2,
            },
        );
        if lost.is_some() {
            return lost;
        }
    }
    None
}

#[test]
fn the_bar_moves_a_quarter_of_the_force_gap_a_second() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(1);
    let mut b = Auras::new(1);
    lit(&mut a, AuraKind::Ease, Rank::Rule, true, &mut ca);
    lit(&mut b, AuraKind::Memory, Rank::Art, true, &mut cb);
    // Equal sense, so only rank and self-realised differ: 3.5 − 1.5 = 2 a second → 0.5 a second.
    cb.sense = ca.sense;
    let mut k = Contest::default();
    assert_eq!(
        run(&mut k, 1.0, (&mut a, &mut ca, 0.0), (&mut b, &mut cb, 2.0)),
        None
    );
    assert!(k.open);
    assert!((k.x - 0.5).abs() < 1e-6, "x = {}", k.x);
    assert!(close(k.force[0] - k.force[1], 2.0));
    // A bot reads the bar and both forces, each from its own side.
    assert_eq!(k.view(0), [k.x, k.force[0], k.force[1]]);
    assert_eq!(k.view(1), [-k.x, k.force[1], k.force[0]]);
}

#[test]
fn the_bar_at_an_end_costs_the_loser_its_soul_and_its_auras() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(1);
    let mut b = Auras::new(1);
    lit(&mut a, AuraKind::Ease, Rank::Rule, true, &mut ca);
    lit(&mut b, AuraKind::Memory, Rank::Art, true, &mut cb);
    cb.sense = ca.sense;
    let mut k = Contest::default();
    let lost = run(&mut k, 3.0, (&mut a, &mut ca, 0.0), (&mut b, &mut cb, 2.0));
    assert_eq!(lost, Some(1));
    assert!(close(cb.soul_lost, BASE.soulless));
    assert!(close(cb.sense_reserved, 0.0));
    assert!(b.on.is_empty() && b.out.len() == 1);
    assert_eq!(k.x, 0.0);
    // The winner keeps its aura.
    assert!(a.lit(AuraKind::Ease) && ca.soul_lost == 0.0);
}

#[test]
fn the_side_with_the_lower_force_loses_from_side_a_too() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(1);
    let mut b = Auras::new(2);
    lit(&mut a, AuraKind::Ease, Rank::Art, false, &mut ca);
    lit(&mut b, AuraKind::Slaughter, Rank::Art, false, &mut cb);
    // Slaughter is a self aura: give B a pressing one it overcomes with.
    lit(&mut b, AuraKind::Severance, Rank::Art, false, &mut cb);
    let mut k = Contest::default();
    let lost = run(&mut k, 10.0, (&mut a, &mut ca, 0.0), (&mut b, &mut cb, 1.0));
    assert_eq!(lost, Some(0));
    assert!(ca.soul_lost > 0.0);
}

#[test]
fn the_bar_returns_to_zero_when_the_zones_part() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(1);
    let mut b = Auras::new(1);
    lit(&mut a, AuraKind::Ease, Rank::Rule, true, &mut ca);
    lit(&mut b, AuraKind::Memory, Rank::Art, true, &mut cb);
    let mut k = Contest::default();
    run(&mut k, 1.0, (&mut a, &mut ca, 0.0), (&mut b, &mut cb, 2.0));
    assert!(k.x > 0.0);
    run(&mut k, TICK, (&mut a, &mut ca, 0.0), (&mut b, &mut cb, 8.0));
    assert!(!k.open);
    assert_eq!(k.x, 0.0);
}

#[test]
fn reversal_spends_all_qi_once_a_round_to_reset_the_bar() {
    let mut ca = Cultivator::new();
    let mut cb = Cultivator::new();
    let mut a = Auras::new(1);
    let mut b = Auras::new(1);
    lit(&mut a, AuraKind::Ease, Rank::Rule, true, &mut ca);
    lit(&mut b, AuraKind::Memory, Rank::Art, true, &mut cb);
    let mut k = Contest::default();
    run(&mut k, 1.0, (&mut a, &mut ca, 0.0), (&mut b, &mut cb, 2.0));
    assert!(k.x > 0.0);
    // The bar leans against B, not A: A cannot.
    assert!(!k.reverse(0, &mut ca));
    cb.qi = 40.0;
    assert!(!k.reverse(1, &mut cb), "below 50 qi");
    cb.qi = 80.0;
    assert!(k.reverse(1, &mut cb));
    assert_eq!(k.x, 0.0);
    assert_eq!(cb.qi, 0.0);
    run(&mut k, 1.0, (&mut a, &mut ca, 0.0), (&mut b, &mut cb, 2.0));
    cb.qi = 100.0;
    assert!(!k.reverse(1, &mut cb), "once a round");
    k.new_round();
    assert_eq!(k.reversed, [false, false]);
    assert_eq!(k.x, 0.0);
}

// --- 7.3 Lĩnh vực ---

const MID: f64 = 10.0;

fn raise(f: &mut Field, owner: usize, kind: DomainKind, rank: Rank, at: f64) -> Raised {
    f.raise(owner, kind, rank, at, [5.0, 15.0], MID)
}

#[test]
fn every_domain_lasts_six_to_ten_seconds() {
    for k in DomainKind::ALL {
        let life = k.data().life;
        assert!((DOMAIN.life_min..=DOMAIN.life_max).contains(&life), "{k:?}");
    }
}

#[test]
fn one_domain_a_side_the_newer_replaces_the_older_without_collapse() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::FixedRule, Rank::Art, 5.0);
    let r = raise(&mut f, 0, DomainKind::WindRain, Rank::Art, 5.0);
    assert_eq!(r, Raised::Stands(vec![]));
    assert_eq!(f.domains[0].map(|d| d.kind), Some(DomainKind::WindRain));
    assert!(!f.locked(0) && !f.locked(1));
}

#[test]
fn a_lone_domain_turns_the_whole_field() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::TruePressure, Rank::Art, MID);
    assert!(f.in_domain_of(0, -50.0) && f.in_domain_of(0, 50.0));
    assert!(close(f.rules(1, 30.0, false).power, 0.4));
    assert!(close(f.rules(0, 30.0, false).power, 1.0));
}

#[test]
fn two_equal_domains_split_the_field_at_the_midpoint() {
    // Rules 10: two domains of equal rank split the field.
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::WindRain, Rank::Divine, 4.0);
    raise(&mut f, 1, DomainKind::FixedRule, Rank::Divine, 12.0);
    assert!(f.in_domain_of(0, 7.99));
    assert!(f.in_domain_of(1, 8.0));
    assert_eq!(f.rules(0, 2.0, true).domain, Some(DomainKind::WindRain));
    assert_eq!(f.rules(0, 14.0, true).domain, Some(DomainKind::FixedRule));
    // The halves follow the centres, whichever side stands left.
    let mut g = Field::default();
    raise(&mut g, 0, DomainKind::WindRain, Rank::Divine, 12.0);
    raise(&mut g, 1, DomainKind::FixedRule, Rank::Divine, 4.0);
    assert!(g.in_domain_of(1, 2.0) && g.in_domain_of(0, 14.0));
}

#[test]
fn a_domain_runs_out_and_its_collapse_locks_blinks_inside_for_a_second() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::WindRain, Rank::Art, 4.0);
    raise(&mut f, 1, DomainKind::FixedRule, Rank::Art, 16.0);
    let pos = [2.0, 6.0];
    let life = DomainKind::FixedRule.data().life;
    let mut gone = Vec::new();
    let mut t = 0.0;
    while t < life + TICK {
        gone.extend(f.tick(TICK, pos));
        t += TICK;
    }
    assert_eq!(gone.len(), 1);
    let c = gone[0];
    assert_eq!(c.kind, DomainKind::FixedRule);
    // Both stood in side 0's half: nobody was inside the half that fell.
    assert_eq!(c.locked, [false, false]);
    assert!(f.domains[1].is_none() && f.domains[0].is_some());
}

#[test]
fn a_collapse_locks_those_inside_and_the_lock_lifts_after_a_second() {
    let mut f = Field::default();
    raise(&mut f, 1, DomainKind::YellowSprings, Rank::Art, 15.0);
    let c = f.remove(1, [5.0, 15.0]);
    assert_eq!(c.map(|c| c.locked), Some([true, true]));
    assert!(!f.rules(0, 5.0, false).blink && !f.rules(1, 15.0, false).blink);
    let ticks = libm::round(DOMAIN.collapse_lock / TICK) as usize;
    for _ in 1..ticks {
        f.tick(TICK, [5.0, 15.0]);
    }
    assert!(f.locked(0));
    f.tick(TICK, [5.0, 15.0]);
    assert!(!f.locked(0) && f.rules(0, 5.0, false) == Rules::FREE);
}

#[test]
fn still_wind_overrides_a_domain_of_no_higher_rank() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::YellowSprings, Rank::Divine, 5.0);
    let r = raise(&mut f, 1, DomainKind::StillWind, Rank::Divine, 15.0);
    let Raised::Stands(gone) = r else {
        unreachable!("Ly Phong stands")
    };
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].kind, DomainKind::YellowSprings);
    assert!(f.domains[0].is_none());
    // Not a higher one.
    let mut g = Field::default();
    raise(&mut g, 0, DomainKind::YellowSprings, Rank::Rule, 5.0);
    assert_eq!(
        raise(&mut g, 1, DomainKind::StillWind, Rank::Divine, 15.0),
        Raised::Stands(vec![])
    );
    assert!(g.domains[0].is_some() && g.domains[1].is_some());
    // A newer domain against a standing Ly Phong of no lower rank never stands.
    let mut h = Field::default();
    raise(&mut h, 1, DomainKind::StillWind, Rank::Rule, 15.0);
    assert_eq!(
        raise(&mut h, 0, DomainKind::RemnantNight, Rank::Rule, 5.0),
        Raised::Overridden
    );
    assert!(h.domains[0].is_none());
}

#[test]
fn faith_wipes_every_domain() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::WindRain, Rank::Origin, 5.0);
    raise(&mut f, 1, DomainKind::FixedRule, Rank::Origin, 15.0);
    assert!(f.wipe(Rank::Origin, [5.0, 15.0]).is_empty());
    let gone = f.wipe(Rank::Faith, [5.0, 15.0]);
    assert_eq!(gone.len(), 2);
    assert_eq!(gone[0].locked, [true, false]);
    assert_eq!(gone[1].locked, [false, true]);
    assert!(f.domains.iter().all(Option::is_none));
    // A Faith domain raised wipes the foe's, even a higher Ly Phong's claim.
    let mut g = Field::default();
    raise(&mut g, 1, DomainKind::StillWind, Rank::Origin, 15.0);
    let r = raise(&mut g, 0, DomainKind::RemnantNight, Rank::Faith, 5.0);
    assert!(matches!(r, Raised::Stands(ref v) if v.len() == 1));
    assert!(g.domains[1].is_none());
}

#[test]
fn a_dispel_takes_a_domain_not_higher() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::WindRain, Rank::Rule, 5.0);
    assert!(f.dispel(0, Rank::Divine, [5.0, 15.0]).is_none());
    assert!(f.dispel(0, Rank::Rule, [5.0, 15.0]).is_some());
}

#[test]
fn yellow_springs_slows_and_swallows_foe_shots_and_pulls_from_its_third_form() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::YellowSprings, Rank::Divine, 5.0);
    let art = Grade::Ranked(Rank::Art);
    let foe = f.shot(8.0, 1, art);
    assert!(foe.speed < 1.0 && foe.swallow_after.is_some() && !foe.removed);
    assert_eq!(f.shot(8.0, 0, art), ShotRule::FREE);
    assert_eq!(f.shot(8.0, 1, Grade::Ranked(Rank::Rule)), ShotRule::FREE);
    assert_eq!(f.rules(1, 9.0, false).pull, 0.0);
    assert_eq!(f.grow(0), 2);
    assert_eq!(f.grow(0), 3);
    assert_eq!(f.grow(0), 3);
    let r = f.rules(1, 9.0, false);
    assert!(r.pull > 0.0 && close(r.pull_to, 5.0));
    assert_eq!(f.rules(0, 9.0, false).pull, 0.0);
    assert_eq!(f.minion_dies(9.0, 1), MinionFate::Turns(0));
    assert_eq!(f.minion_dies(9.0, 0), MinionFate::Dies);
}

#[test]
fn remnant_night_is_free_at_dawn_and_sunrise_comes_when_it_runs_out() {
    assert_eq!(cost_factor(DomainKind::RemnantNight, 0.5), 0.0);
    assert_eq!(cost_factor(DomainKind::RemnantNight, DOMAIN.dawn), 1.0);
    assert_eq!(cost_factor(DomainKind::StillWind, 0.5), 1.0);
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::RemnantNight, Rank::Faith, 5.0);
    assert!(f.rules(1, 15.0, false).night);
    let gone = f.tick(DomainKind::RemnantNight.data().life, [5.0, 15.0]);
    assert_eq!(gone.len(), 1);
    assert!(gone[0].sunrise && gone[0].locked[1]);
    // Removed early: no sunrise.
    let mut g = Field::default();
    raise(&mut g, 0, DomainKind::RemnantNight, Rank::Faith, 5.0);
    assert_eq!(g.remove(0, [5.0, 15.0]).map(|c| c.sunrise), Some(false));
}

#[test]
fn still_wind_stops_qi_for_everyone_and_removes_every_shot_and_wind() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::StillWind, Rank::Divine, 5.0);
    for side in 0..2 {
        let r = f.rules(side, 5.0, false);
        assert!(!r.qi_regen && !r.qi_skills && !r.wind);
        assert!(f.shot(5.0, side, Grade::Ranked(Rank::Art)).removed);
    }
    assert!(!f.shot(5.0, 1, Grade::Ranked(Rank::Origin)).removed);
}

#[test]
fn fixed_rule_binds_whoever_moves() {
    let mut f = Field::default();
    raise(&mut f, 1, DomainKind::FixedRule, Rank::Art, 15.0);
    for side in 0..2 {
        let still = f.rules(side, 10.0, false);
        let moving = f.rules(side, 10.0, true);
        assert!(still.escape && still.fly);
        assert!(!moving.escape && !moving.fly);
    }
}

#[test]
fn true_pressure_is_raised_only_at_the_centre() {
    assert!(can_raise(DomainKind::TruePressure, MID + 0.5, MID));
    assert!(!can_raise(DomainKind::TruePressure, MID + 3.0, MID));
    assert!(can_raise(DomainKind::WindRain, MID + 30.0, MID));
    let mut f = Field::default();
    assert_eq!(
        raise(&mut f, 0, DomainKind::TruePressure, Rank::Art, 2.0),
        Raised::OffCentre
    );
}

#[test]
fn wind_rain_starves_foes_of_world_qi_and_pays_incense_for_dying_minions() {
    let mut f = Field::default();
    raise(&mut f, 0, DomainKind::WindRain, Rank::Art, 5.0);
    assert!(!f.rules(1, 9.0, false).world_qi);
    assert!(f.rules(0, 9.0, false).world_qi);
    assert_eq!(f.minion_dies(9.0, 1), MinionFate::Incense(0, 1.0));
    assert_eq!(f.minion_dies(9.0, 0), MinionFate::Incense(0, 1.0));
}

// --- 8.4 Lá bài một lần, Lộ bài; research 10 ---

fn tags(t: &[Tag]) -> Tags {
    Tags::of(t)
}

#[test]
fn uses_count_over_the_match_and_never_refill() {
    let mut c = Card::new(2, true);
    assert!(c.spend());
    c.new_round();
    assert!(c.spend());
    c.new_round();
    assert!(!c.usable() && !c.spend());
    assert_eq!(Card::of(None, false), None);
    assert_eq!(Card::of(Some(1), true), Some(Card::new(1, true)));
}

#[test]
fn a_show_costs_no_use_and_catches_a_guard_or_an_escape() {
    let mut card = Card::new(1, true);
    let mut b = card.show(3).unwrap_or_else(|| unreachable!());
    assert_eq!(card.uses, 1);
    assert_eq!(b.slot, 3);
    // Not before the windup ends.
    assert_eq!(b.answer(tags(&[Tag::Guard])), None);
    let mut t = 0.0;
    while t < CARD.bluff_windup - TICK / 2.0 {
        b.tick(TICK, false);
        t += TICK;
    }
    assert!(!b.shown());
    b.tick(TICK, false);
    assert!(b.shown());
    // An attack or standing still does nothing.
    assert_eq!(b.answer(tags(&[Tag::Attack, Tag::Melee])), None);
    let caught = b.answer(tags(&[Tag::Body, Tag::Movement]));
    let caught = caught.unwrap_or_else(|| unreachable!());
    assert_eq!(caught.cooldown, 2.0);
    let mut bluffer = Cultivator::new();
    caught.reward(&mut bluffer);
    assert_eq!(bluffer.intent, 10.0);
    // Once.
    assert_eq!(b.answer(tags(&[Tag::Guard])), None);
}

#[test]
fn the_window_closes_after_one_and_a_half_seconds() {
    let mut card = Card::new(1, true);
    let mut b = card.show(0).unwrap_or_else(|| unreachable!());
    b.tick(CARD.bluff_windup, false);
    assert!(b.tick(CARD.bluff_window - 0.01, false));
    assert!(b.answer(tags(&[Tag::Guard])).is_some());
    let mut b = card.show(0).unwrap_or_else(|| unreachable!());
    b.tick(CARD.bluff_windup, false);
    assert!(!b.tick(CARD.bluff_window, false));
    assert_eq!(b.answer(tags(&[Tag::Guard])), None);
}

#[test]
fn two_shows_a_card_a_round_scare_the_third_does_not() {
    let mut card = Card::new(1, true);
    for n in 1..=3 {
        let mut b = card.show(0).unwrap_or_else(|| unreachable!());
        b.tick(CARD.bluff_windup, false);
        let caught = b.answer(tags(&[Tag::Guard])).is_some();
        assert_eq!(caught, n <= 2, "show {n}");
    }
    card.new_round();
    let mut b = card.show(0).unwrap_or_else(|| unreachable!());
    b.tick(CARD.bluff_windup, false);
    assert!(b.answer(tags(&[Tag::Guard])).is_some());
}

#[test]
fn a_show_at_a_controlled_foe_scares_no_one() {
    // Rules 10: the foe cannot answer, so the show has no threat.
    let mut card = Card::new(1, true);
    let mut b = card.show(0).unwrap_or_else(|| unreachable!());
    b.tick(CARD.bluff_windup, true);
    b.tick(0.1, false);
    assert_eq!(b.answer(tags(&[Tag::Guard])), None);
}

#[test]
fn no_show_without_lo_bai_or_without_a_use_left() {
    assert_eq!(Card::new(2, false).show(0), None);
    let mut spent = Card::new(1, true);
    spent.spend();
    assert_eq!(spent.show(0), None);
}

#[test]
fn a_one_use_card_cannot_be_fused() {
    // Rules 10: fusing would copy the card.
    assert!(fusable(None, None));
    assert!(!fusable(Some(1), None));
    assert!(!fusable(None, Some(2)));
}

#[test]
fn the_feint_wastes_a_guard_raised_against_it() {
    let mut f = Feint::new();
    assert!(!f.meets(tags(&[Tag::Movement])));
    assert!(f.meets(tags(&[Tag::Body, Tag::Guard])));
    assert!(!f.meets(tags(&[Tag::Guard])), "spent");
    let mut g = Feint::new();
    assert!(!g.tick(FEINT.life));
    assert!(!g.meets(tags(&[Tag::Guard])), "gone");
}

#[test]
fn bait_doubles_a_control_on_a_target_that_just_guarded() {
    let mut w = GuardWatch::default();
    assert_eq!(bait(0.5, &w), 0.5);
    w.on_skill(tags(&[Tag::Attack]));
    assert_eq!(bait(0.5, &w), 0.5);
    w.on_skill(tags(&[Tag::Body, Tag::Guard]));
    assert_eq!(bait(0.5, &w), 0.5 * BAIT.control);
    w.tick(BAIT.window);
    assert_eq!(bait(0.5, &w), 1.0);
    w.tick(0.01);
    assert_eq!(bait(0.5, &w), 0.5);
    w.on_skill(tags(&[Tag::Guard]));
    w.new_round();
    assert_eq!(bait(0.5, &w), 0.5);
}

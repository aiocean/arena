//! What a mind and a policy are told: the slots (the basics and the bar, as indexes into its skills), and
//! each skill's effects mapped onto the minds' fixed slots (design 16).

mod common;

use arena_duel::duel::Duel;
use arena_duel::seat::Brain;
use arena_duel::sense::sense_of;
use common::{all_effects, config};

#[test]
fn slots_are_the_basics_and_the_bar() {
    let mut duel = Duel::new(config(3, all_effects()), [Brain::make(), Brain::make()]);
    for _ in 0..120 {
        duel.step(1.0 / 60.0);
    }
    for me in 0..2 {
        let f = &duel.state().fighters[me];
        let s = sense_of(duel.state(), me);
        assert_eq!(s.slots.len(), 6);
        let name = |i: usize| s.slots[i].map(|k| f.spells[k]);
        for (i, b) in arena_calc::loadout::BASICS.iter().enumerate() {
            assert_eq!(name(i), Some(*b));
        }
        for (i, slot) in f.bar.iter().enumerate() {
            assert_eq!(name(3 + i), slot.spell(), "bar slot {i}");
        }
        assert_eq!(f.bar.len(), 3);
    }
}

#[test]
fn a_skill_says_what_it_inflicts_by_its_kinds_of_damage_and_its_controls() {
    use arena_calc::ailment::{AIL, Ailment};
    use arena_calc::dot::{BURN_RATE, BURN_TIME};
    use arena_duel::sense::skill_of;
    let bell = skill_of("bell").inflicts;
    assert_eq!(bell.stun, 0.6, "the bell stuns the one striking it");
    // fire burns, thunder jolts (the shock slots), water soaks (the chill slots), metal sunders (defence down)
    let fireball = skill_of("fireball").inflicts;
    assert_eq!(fireball.burn, BURN_TIME);
    assert!((fireball.burn_rate - BURN_RATE).abs() < 1e-12);
    let bolt = skill_of("bolt").inflicts;
    assert_eq!(bolt.shock, 1.0);
    assert_eq!(bolt.shock_max, Ailment::Jolt.data().cap);
    assert_eq!(bolt.shock_rate, AIL.jolt_taken);
    let jet = skill_of("jet").inflicts;
    assert_eq!(jet.chill, 1.0);
    assert_eq!(jet.chill_max, Ailment::Soak.data().cap);
    assert_eq!(jet.freeze, 0.0, "nothing freezes");
    let finger = skill_of("finger").inflicts;
    assert_eq!(finger.def_down, Ailment::Sunder.data().time);
    assert!((finger.def_loss - AIL.sunder).abs() < 1e-12);
    // Định Thân holds (the freeze slot), Hóa Ma Chỉ takes the soul (the silence slot)
    assert_eq!(skill_of("bodyfix").inflicts.freeze, 0.4);
    assert_eq!(skill_of("demonfinger").inflicts.silence, 1.0);
    assert_eq!(skill_of("slash").inflicts, arena_mind::Inflicts::default());
}

#[test]
fn a_fighter_senses_its_own_act_and_the_foes() {
    use arena_duel::fighter::{Act, Phase};
    use arena_mind::{ActPhase, Doing};
    let mut duel = Duel::new(config(3, all_effects()), [Brain::make(), Brain::make()]);
    duel.step(1.0 / 60.0);
    duel.state_mut().fighters[1].act = Some(Act {
        spell: "tribulation",
        phase: Phase::Recover,
        t: 0.1,
        windup: 0.5,
        recover: 0.4,
        target: None,
        circle: None,
        blocked: false,
    });
    let me = sense_of(duel.state(), 1).me;
    let a = me.act.unwrap_or_else(|| unreachable!("it is acting"));
    assert_eq!(a.phase, ActPhase::Recover);
    assert_eq!(a.t, 0.1);
    assert_eq!(a.recover, 0.4);
    assert_eq!(sense_of(duel.state(), 0).me.act, None);
    let Doing::Act { phase, .. } = sense_of(duel.state(), 0).foe.doing else {
        unreachable!("the foe is acting")
    };
    assert_eq!(phase, ActPhase::Recover);
}

#[test]
fn a_lost_soul_silences_and_a_root_seals() {
    let mut duel = Duel::new(config(3, all_effects()), [Brain::make(), Brain::make()]);
    duel.step(1.0 / 60.0);
    duel.state_mut().fighters[0].c.soul_lost = 1.0;
    duel.state_mut().fighters[0].root = 1.0;
    let s = sense_of(duel.state(), 0);
    assert!(s.me.silenced && s.me.sealed);
    assert_eq!(
        sense_of(duel.state(), 1).foe.frozen,
        1.0,
        "a root shows where a freeze did"
    );
}

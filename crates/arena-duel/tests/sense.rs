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
    assert_eq!(skill_of("blink").inflicts, arena_mind::Inflicts::default());
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

#[test]
fn the_minds_counts_are_the_damage_systems() {
    use arena_calc::{ailment, aura, curse, keystone, loadout, support};
    assert_eq!(arena_mind::AILMENTS, ailment::AILMENTS);
    assert_eq!(arena_mind::AILMENTS, ailment::Ailment::ALL.len());
    assert_eq!(arena_mind::SUPPORTS, support::SUPPORTS.len());
    let mut curses: Vec<usize> = curse::Kind::SAMPLES.iter().map(|k| k.index()).collect();
    curses.sort_unstable();
    curses.dedup();
    assert_eq!(curses, (0..arena_mind::CURSES).collect::<Vec<_>>());
    assert_eq!(arena_mind::AURAS, aura::AURA_KINDS);
    assert_eq!(arena_mind::KEYSTONES, keystone::Keystone::ALL.len());
    assert_eq!(arena_mind::ROOTS, loadout::ROOTS.len());
}

/// Seat 0 casts `skill` once, seat 1 stands, both with plain loadouts.
fn one_cast(skill: &'static str) -> Duel {
    common::duel_of(
        5,
        [common::caster(skill), common::idle()],
        [common::loadout(&[]), common::loadout(&[])],
    )
}

#[test]
fn a_cast_shows_its_frames_to_both_and_its_cooldown_after() {
    let mut d = one_cast("qibolt");
    let mut seen = (false, false, false);
    let mut cooled = false;
    for _ in 0..90 {
        d.step(1.0 / 60.0);
        let (mine, theirs) = (sense_of(d.state(), 0), sense_of(d.state(), 1));
        assert_eq!(
            mine.me_act, theirs.foe_act,
            "one cast, seen from both sides"
        );
        let Some(a) = mine.me_act else { continue };
        assert!(a.window > 0.0 && a.recover > 0.0, "{a:?}");
        if a.to_punish > 0.0 {
            seen.0 = true;
            assert!(a.to_window > 0.0 && !a.in_window);
        }
        seen.1 |= a.in_window;
        seen.2 |= a.hitbox.is_some();
        let x = mine.slotx[0].unwrap_or_default();
        assert!(x.cooldown > 0.0, "the qi bolt is slot 0: {x:?}");
        cooled |= x.stored < x.max_uses && x.cooldown_left > 0.0;
    }
    // a cast has no blade's hitbox: the clip draws no blow (every skill is a spell)
    assert_eq!(
        seen,
        (true, true, false),
        "wound up, in its window, with no blade"
    );
    assert!(cooled, "the qi bolt cooled down after it was cast");
    let s = sense_of(d.state(), 0);
    let foe = s.foe_slots[0].map(|f| f.built.windup);
    assert_eq!(foe, sense_of(d.state(), 1).slotx[0].map(|x| x.windup));
}

#[test]
fn a_plain_skill_builds_to_its_own_damage_and_a_support_shows() {
    let mut l = common::loadout(&["fireball"]);
    l.actives[0].supports = vec!["fork".to_string()];
    let mut d = common::duel_of(
        5,
        [common::idle(), common::idle()],
        [l, common::loadout(&[])],
    );
    d.step(1.0 / 60.0);
    let s = d.state_mut();
    s.fighters[0].bar[0] = common::common("fireball");
    s.fighters[1].bar[0] = common::common("fireball");
    let (mine, theirs) = (sense_of(d.state(), 0), sense_of(d.state(), 1));
    let plain = theirs.slotx[3].unwrap_or_default();
    let want = arena_duel::sense::skill_of("fireball");
    assert!(
        (plain.damage - want.damage).abs() < 1e-9 * want.damage,
        "{plain:?}"
    );
    assert!((plain.qi - want.cost).abs() < 1e-9);
    assert_eq!(plain.supports, 0);
    let forked = mine.slotx[3].unwrap_or_default();
    let at = arena_calc::support::SUPPORTS
        .iter()
        .position(|s| s.id == "fork")
        .unwrap_or(usize::MAX);
    assert_eq!(forked.supports, 1 << at);
    assert!(forked.fork >= 1.0, "{forked:?}");
    assert_eq!(theirs.foe_slots[3].map(|f| f.built.fork), Some(forked.fork));
}

#[test]
fn a_bleed_and_a_unit_show_where_they_are() {
    use arena_calc::ailment::Ailment;
    use arena_calc::dot::Dot;
    let mut d = common::duel_of(
        7,
        [common::always("beans"), common::idle()],
        [common::loadout(&["beans"]), common::loadout(&[])],
    );
    d.step(1.0 / 60.0);
    let s = d.state_mut();
    s.fighters[0].bar[0] = common::common("beans");
    s.fighters[0].c.souls = 30.0;
    s.fighters[1].body.x = s.fighters[0].body.x + 500.0;
    let life = s.fighters[1].c.life_max;
    s.fighters[1]
        .c
        .ailments
        .bleeds
        .lay(Dot::new(Ailment::Bleed, 5.0, 3.0));
    let s = sense_of(d.state(), 0);
    let bleed = s.effects[1].ailments[Ailment::Bleed.index()];
    assert_eq!((bleed.stacks, bleed.left), (1.0, 3.0));
    assert!((bleed.strength - 5.0 / life).abs() < 1e-12);
    assert_eq!(s.effects[0].ailments[Ailment::Bleed.index()].stacks, 0.0);
    common::run(&mut d, 1.0);
    let (mine, theirs) = (sense_of(d.state(), 0), sense_of(d.state(), 1));
    assert!(!mine.units.is_empty(), "the beans stand up");
    for u in &mine.units {
        assert!(u.mine && u.kind == arena_mind::UnitKind::Minion, "{u:?}");
        assert!(u.hp > 0.0 && u.hp <= 1.0);
    }
    assert!(theirs.units.iter().all(|u| !u.mine));
}

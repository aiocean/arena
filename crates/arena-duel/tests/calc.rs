//! The damage system in the real fight (`strike`): every blow built and resolved by `arena-calc`, ailments and damage
//! over time, controls on the body, keystones' spare lives, souls, and triggers casting at once.

mod common;

use arena_calc::ailment::Ailment;
use arena_calc::keystone::Keystone;
use arena_calc::loadout::{Loadout, Slot as LoadoutSlot};
use arena_duel::act::advance;
use arena_duel::data::spell;
use arena_duel::fighter::{Act, Fighter, Phase, State};
use arena_duel::js::{js_sign, or_one};
use arena_duel::out::Out;
use common::{all_effects, loadout};

/// Both fighters with `a` and `b`, 80 px apart facing each other, nobody dodging or crit.
fn still(seed: u32, a: Loadout, b: Loadout) -> State {
    let mut s = State::new(
        seed,
        0.0,
        880.0,
        600.0,
        384.0,
        all_effects(),
        [Fighter::new(0, a), Fighter::new(1, b)],
    );
    s.fighters[0].body.start_round(300.0);
    s.fighters[1].body.start_round(380.0);
    s.fighters[0].body.face = 1.0;
    s.fighters[1].body.face = -1.0;
    for f in &mut s.fighters {
        f.c.evasion = 0.0;
        f.c.crit = 0.0;
    }
    s
}

/// Fighter `by`'s `name` comes out now.
fn strike(s: &mut State, by: usize, name: &'static str) -> Vec<Out> {
    let windup = spell(name).map_or(0.0, |sp| sp.windup);
    let fx = s.fighters[1 - by].body.x;
    s.fighters[by].act = Some(Act {
        spell: name,
        phase: Phase::Windup,
        t: windup,
        windup,
        recover: 0.3,
        target: Some(fx),
        circle: None,
        blocked: false,
    });
    s.fighters[by].body.face = js_sign(or_one(fx - s.fighters[by].body.x));
    s.out.clear();
    advance(s, by, 1.0 / 60.0);
    s.out.clone()
}

fn with(skill: &str, supports: &[&str]) -> Loadout {
    let mut l = loadout(&[skill]);
    l.actives[0] = LoadoutSlot {
        skill: skill.to_string(),
        supports: supports.iter().map(|s| s.to_string()).collect(),
        fuse: None,
    };
    assert!(arena_calc::loadout::check(&l).is_ok(), "{l:?}");
    l
}

#[test]
fn a_blow_is_built_and_resolved_by_the_damage_system() {
    let mut s = still(1, loadout(&["palm"]), loadout(&[]));
    let (life, shield) = (s.fighters[1].c.life, s.fighters[1].c.shield);
    let out = strike(&mut s, 0, "palm");
    let damage = out.iter().find_map(|o| match o {
        Out::Hit {
            spell: "palm",
            damage,
            ..
        } => Some(*damage),
        _ => None,
    });
    let damage = damage.unwrap_or(0.0);
    assert!(damage > 0.0);
    // the shield takes the force first, the rest goes to life
    let took = (life - s.fighters[1].c.life) + (shield - s.fighters[1].c.shield);
    assert!((took - damage).abs() < 1e-6, "{took} {damage}");
    // a palm shakes (Chấn)
    assert!(s.fighters[1].c.ailments.stacks(Ailment::Daze) > 0.0);
}

#[test]
fn fire_burns_over_time_and_shows_it() {
    let mut s = still(2, loadout(&["fireball"]), loadout(&[]));
    s.fighters[1].body.x = 600.0;
    strike(&mut s, 0, "fireball");
    let mut ticks = 0;
    for _ in 0..240 {
        let ids: Vec<u32> = s.shots.iter().map(|x| x.id).collect();
        for id in ids {
            arena_duel::shot::shot(&mut s, id, 1.0 / 60.0);
        }
        s.out.clear();
        arena_duel::strike::wear(&mut s, 1.0 / 60.0);
        ticks += s
            .out
            .iter()
            .filter(|o| matches!(o, Out::Tick { by: 0, .. }))
            .count();
    }
    assert!(ticks >= 2, "the burn shows as it burns: {ticks}");
}

#[test]
fn controls_land_on_the_body() {
    // Định Thân holds the body where it stands; Hóa Ma Chỉ takes the soul, and a soulless fighter casts no spell
    let mut s = still(3, loadout(&["bodyfix", "demonfinger"]), loadout(&["bolt"]));
    strike(&mut s, 0, "bodyfix");
    let mut held = s.fighters[1].body.shocked > 0.0;
    for _ in 0..120 {
        let ids: Vec<u32> = s.shots.iter().map(|x| x.id).collect();
        for id in ids {
            arena_duel::shot::shot(&mut s, id, 1.0 / 60.0);
        }
        held |= s.fighters[1].body.shocked > 0.0;
    }
    assert!(held, "held");
    let mut s = still(3, loadout(&["demonfinger"]), loadout(&["bolt"]));
    s.fighters[1].body.x = 700.0;
    strike(&mut s, 0, "demonfinger");
    for _ in 0..120 {
        let ids: Vec<u32> = s.shots.iter().map(|x| x.id).collect();
        for id in ids {
            arena_duel::shot::shot(&mut s, id, 1.0 / 60.0);
        }
    }
    assert!(s.fighters[1].silenced(), "its soul is lost");
    s.fighters[1].bar = vec![common::common("bolt")];
    assert!(!arena_duel::bar::ready(&s.fighters[1], "bolt"), "no spell");
    assert!(
        arena_duel::bar::ready(&s.fighters[1], "slash"),
        "the blade answers"
    );
}

#[test]
fn a_root_stops_walking_and_blinking() {
    let mut s = still(4, loadout(&[]), loadout(&[]));
    arena_duel::strike::control(&mut s, 1, arena_calc::ailment::Control::Root, 1.0);
    assert!(s.fighters[1].root > 0.0);
    assert!(!arena_duel::bar::ready(&s.fighters[1], "blink"));
}

#[test]
fn unyielding_stands_up_once_a_round_from_a_killing_blow() {
    let mut l = loadout(&[]);
    l.keystones = vec![Keystone::Unyielding];
    let mut s = still(5, loadout(&["palm"]), l);
    s.fighters[1].c.life = 1.0;
    s.fighters[1].c.shield = 0.0;
    strike(&mut s, 0, "palm");
    assert!(!s.fighters[1].alive());
    assert!(arena_duel::strike::spare(&mut s, 1));
    assert!(s.fighters[1].alive() && s.fighters[1].unyielding_used);
    assert!(
        s.out
            .iter()
            .any(|o| matches!(o, Out::Unyielding { by: 1, .. }))
    );
    s.fighters[1].c.life = 0.0;
    assert!(!arena_duel::strike::spare(&mut s, 1), "once a round");
}

#[test]
fn soul_damage_earns_souls() {
    let mut s = still(6, loadout(&["soulsearch"]), loadout(&[]));
    s.fighters[1].c.sense = 1.0e6;
    s.fighters[1].c.sense_max = 1.0e6;
    let before = s.fighters[0].c.souls;
    strike(&mut s, 0, "soulsearch");
    assert!(s.fighters[0].c.souls > before);
}

#[test]
fn chainweaken_fires_its_linked_attack_when_the_shield_breaks() {
    let mut s = still(7, loadout(&["palm"]), with("ward", &["chainweaken"]));
    s.fighters[1].c.shield = 1.0;
    strike(&mut s, 0, "palm");
    arena_duel::strike::run_queue(&mut s);
    assert!(
        s.out.iter().any(|o| matches!(
            o,
            Out::Hit {
                by: 1,
                spell: "palm",
                ..
            }
        )),
        "{:?}",
        s.out
    );
}

#[test]
fn a_whole_fight_through_the_damage_system_replays_from_its_seed() {
    let go = || common::run(&mut common::fight(11), 30.0);
    let a = go();
    assert!(a.iter().any(|o| matches!(o, Out::Hit { .. })));
    assert_eq!(a, go());
}

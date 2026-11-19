//! Leaving the ground costs qi (`data::JUMP_QI`): a jump, paid as it crouches, and a take-off into flight; with
//! less, the fighter stays on the ground. Flying higher from the air costs nothing more.

mod common;

use arena_duel::data::JUMP_QI;
use arena_duel::duel::apply;
use arena_duel::fighter::{Fighter, State};
use arena_mind::Choice;

fn duel() -> State {
    let kit = ["lotus", "palm", "qiburst", "bell"];
    let mut s = State::new(
        1,
        0.0,
        880.0,
        600.0,
        384.0,
        common::all_effects(),
        [
            Fighter::new(0, common::loadout(&kit)),
            Fighter::new(1, common::loadout(&kit)),
        ],
    );
    for (i, at) in [(0, 0.22), (1, 0.78)] {
        s.fighters[i].body.start_round(880.0 * at);
    }
    s
}

#[test]
fn a_jump_costs_its_qi_once() {
    let mut s = duel();
    s.fighters[0].c.qi = 50.0;
    apply(&mut s, 0, &Choice::Jump);
    assert!(s.fighters[0].body.squat > 0.0);
    assert_eq!(s.fighters[0].c.qi, 50.0 - JUMP_QI);
    // choosing it again while it crouches neither pays twice nor crouches again
    let squat = s.fighters[0].body.squat;
    s.fighters[0].body.squat = squat / 2.0;
    apply(&mut s, 0, &Choice::Jump);
    assert_eq!(s.fighters[0].c.qi, 50.0 - JUMP_QI);
    assert_eq!(s.fighters[0].body.squat, squat / 2.0);
}

#[test]
fn short_of_qi_it_stays_on_the_ground() {
    let mut s = duel();
    s.fighters[0].c.qi = JUMP_QI - 0.5;
    apply(&mut s, 0, &Choice::Jump);
    assert_eq!(s.fighters[0].body.squat, 0.0);
    apply(&mut s, 0, &Choice::Fly { height: 200.0 });
    assert!(!s.fighters[0].fly);
    assert_eq!(s.fighters[0].body.vy, 0.0);
    assert_eq!(s.fighters[0].c.qi, JUMP_QI - 0.5);
}

#[test]
fn a_take_off_costs_a_jump_and_flying_higher_nothing() {
    let mut s = duel();
    s.fighters[0].c.qi = 50.0;
    apply(&mut s, 0, &Choice::Fly { height: 200.0 });
    assert!(s.fighters[0].fly);
    assert_eq!(s.fighters[0].c.qi, 50.0 - JUMP_QI);
    s.fighters[0].body.air = 40.0;
    apply(&mut s, 0, &Choice::Fly { height: 300.0 });
    assert_eq!(s.fighters[0].hover, 300.0);
    assert_eq!(s.fighters[0].c.qi, 50.0 - JUMP_QI);
}

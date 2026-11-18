//! Spacing: two bodies never overlap (their hurtboxes kept apart on parry), a blink never lands inside the foe, and a
//! spell's blow pushes its target away, further the harder it hits.

mod common;

use arena_duel::body::{Blow, Body};
use arena_duel::collide::{body_gap, overlap_x};
use arena_duel::data::body_spec;
use arena_duel::duel::Duel;
use arena_duel::frames::hurtboxes;

const TICK: f64 = 1.0 / 60.0;

/// Two idle fighters with no loadout, at `x0` and `x1`.
fn standing(x0: f64, x1: f64) -> Duel {
    let mut duel = common::duel_of(
        3,
        [common::idle(), common::idle()],
        [common::loadout(&[]), common::loadout(&[])],
    );
    duel.state_mut().fighters[0].body.x = x0;
    duel.state_mut().fighters[1].body.x = x1;
    duel
}

/// How far the two bodies overlap along x (0: clear), fighter 1 on its side of fighter 0.
fn overlapping(duel: &Duel) -> f64 {
    let s = duel.state();
    let dir = if s.fighters[1].body.x >= s.fighters[0].body.x {
        1.0
    } else {
        -1.0
    };
    overlap_x(&hurtboxes(s, 0), &hurtboxes(s, 1), dir, 0.0)
}

#[test]
fn two_bodies_on_one_spot_are_parted_until_their_hurtboxes_are_clear() {
    for (x0, x1) in [(400.0, 400.0), (400.0, 420.0), (430.0, 400.0)] {
        let mut duel = standing(x0, x1);
        assert!(overlapping(&duel) > 0.0);
        duel.step(TICK);
        assert_eq!(overlapping(&duel), 0.0, "{x0} {x1}");
        // and kept a little room apart, not only touching
        let s = duel.state();
        let gap = body_gap(s);
        assert!(gap > 0.0);
        let dir = (s.fighters[1].body.x - s.fighters[0].body.x).signum();
        assert!(overlap_x(&hurtboxes(s, 0), &hurtboxes(s, 1), dir, gap) < 1e-9);
    }
}

#[test]
fn against_a_wall_the_other_body_gives_the_whole_way() {
    let left = common::config(3, Vec::new()).left;
    let mut duel = standing(left, left + 10.0);
    duel.step(TICK);
    let s = duel.state();
    assert_eq!(overlapping(&duel), 0.0);
    assert!(s.fighters[0].body.x >= left + s.fighters[0].body.spec.half_width);
    assert!(s.fighters[1].body.x > s.fighters[0].body.x);
}

#[test]
fn a_blink_aimed_into_the_foe_lands_clear_of_it() {
    for (from, foe, aim) in [
        (300.0, 420.0, 420.0),
        (300.0, 420.0, 440.0),
        (560.0, 420.0, 410.0),
    ] {
        let mut duel = common::duel_of(
            3,
            [blinker(aim), common::idle()],
            [common::loadout(&[]), common::loadout(&[])],
        );
        duel.state_mut().fighters[0].body.x = from;
        duel.state_mut().fighters[1].body.x = foe;
        let mut blinked = false;
        for _ in 0..120 {
            let x = duel.state().fighters[0].body.x;
            duel.step(TICK);
            if !blinked && duel.state().fighters[0].body.x != x {
                blinked = true;
                // the very tick it lands, before the bodies are kept apart again
                assert_eq!(overlapping(&duel), 0.0, "{from} {foe} {aim}");
            }
        }
        assert!(blinked, "{from} {foe} {aim}");
        // it stays on the side of the foe's centre it aimed at (the side it came from when it aimed at the centre)
        let s = duel.state();
        let side = if aim == foe { from - foe } else { aim - foe };
        assert_eq!(
            (s.fighters[0].body.x - foe).signum(),
            side.signum(),
            "{from} {foe} {aim}"
        );
    }
}

/// A seat that blinks at `aim` once.
fn blinker(aim: f64) -> arena_duel::seat::MakeSeat {
    Box::new(move |_, _| {
        let mut cast = false;
        Box::new(arena_duel::seat::Script(Box::new(move |_, _| {
            if cast {
                return None;
            }
            cast = true;
            Some(arena_mind::Choice::Cast {
                skill: "blink".to_string(),
                target: Some(aim),
            })
        })))
    })
}

#[test]
fn a_harder_blow_pushes_further_up_to_a_most() {
    let push = |power: f64| {
        let mut b = Body::new(400.0);
        b.spec = body_spec();
        b.hit(Blow {
            power,
            dir: 1.0,
            flying: false,
        });
        b.vx
    };
    let spec = body_spec();
    assert_eq!(push(0.0), spec.knockback);
    assert!(push(4.0) > push(2.0) && push(8.0) > push(4.0));
    // a heavy blow throws, and pushes `launch_knockback` as far, but never past the most
    assert!(push(100.0) <= spec.knockback_max * spec.launch_knockback + 1e-9);
    assert_eq!(push(100.0), push(1000.0));
}

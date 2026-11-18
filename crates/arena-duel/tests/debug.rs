//! The hit tests' shapes follow the frame a fighter shows (`arena_duel::frames`, `arena_duel::debug`): standing, its
//! hurtboxes reach its head; crouched, they are low; in the air, they are where its body is.

use arena_duel::data::Look;
use arena_duel::debug::{Mark, shapes};
use arena_duel::duel::{Duel, DuelConfig};
use arena_duel::frames::{hurtboxes, len};
use arena_duel::pick::CLIPS;
use arena_duel::pose::Pose;
use arena_duel::seat::Brain;

fn duel() -> Duel {
    let config = DuelConfig {
        seed: 7,
        left: 88.0,
        right: 1192.0,
        ground: 632.0,
        sky: 196.0,
        effects: arena_duel::effects::all(),
    };
    Duel::new(config, [Brain::make(), Brain::make()])
}

/// The highest point of fighter `f`'s hurtboxes, and the lowest.
fn span(d: &Duel, f: usize) -> (f64, f64) {
    let hb = hurtboxes(d.state(), f);
    let top = hb.iter().map(|h| h[3]).fold(f64::MIN, f64::max);
    let bottom = hb.iter().map(|h| h[1]).fold(f64::MAX, f64::min);
    (top, bottom)
}

#[test]
fn every_clip_has_frame_data_for_every_look() {
    for look in [Look::VuongLam, Look::ThatThai] {
        for clip in CLIPS {
            assert!(len(look, clip) > 0, "{look:?} has no frames for {clip}");
        }
    }
}

#[test]
fn hurtboxes_follow_the_pose() -> Result<(), String> {
    let mut d = duel();
    for f in 0..2 {
        let standing = span(&d, f);
        assert!(
            standing.0 > 200.0 && standing.1 < 10.0,
            "fighter {f} standing: {standing:?}"
        );

        // crouched, well into its duck clip: the boxes are low
        {
            let b = &mut d.state_mut().fighters[f].body;
            b.pose = Pose::Duck;
            b.pose_t = 0.4;
        }
        let ducking = span(&d, f);
        assert!(
            ducking.0 < standing.0 - 40.0,
            "fighter {f} ducking {ducking:?} vs standing {standing:?}"
        );

        // in the air: the boxes are where the body is
        {
            let b = &mut d.state_mut().fighters[f].body;
            b.pose = Pose::Fly;
            b.pose_t = 0.0;
            b.air = 150.0;
        }
        let flying = span(&d, f);
        assert!(
            flying.1 >= 150.0,
            "fighter {f} in the air at 150: {flying:?}"
        );
        {
            let b = &mut d.state_mut().fighters[f].body;
            b.pose = Pose::Idle;
            b.air = 0.0;
        }
    }
    // and the debug shapes draw exactly those boxes
    let bodies = shapes(d.state())
        .into_iter()
        .filter(|s| s.mark == Mark::Body)
        .count();
    let boxes = hurtboxes(d.state(), 0).len() + hurtboxes(d.state(), 1).len();
    assert_eq!(bodies, boxes);
    Ok(())
}

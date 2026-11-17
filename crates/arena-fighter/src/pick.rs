//! Which frame of which animation a fighter shows: its pose, how long it has held it, and for a wind-up how far
//! along it is. Pure: the same appearance always picks the same frame.

use libm::floor;
use pixfx_draw::math::js_round;
use pixfx_fighter::paint::Appearance;
use pixfx_fighter::pose::{Gather, Pose};

/// Every animation a fighter's sheet must hold.
pub const CLIPS: &[&str] = &[
    "stance-idle",
    "walk",
    "jump",
    "duck",
    "slash",
    "overhead",
    "cast",
    "dash",
    "guard",
    "meditate",
    "hit",
    "frozen",
    "shocked",
    "launched",
    "tumble",
    "knockdown",
    "defeat",
    "roll",
    "getting-up",
    "win",
];

/// A frame to show: animation `clip`, frame `frame` of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shot {
    pub clip: &'static str,
    pub frame: usize,
}

/// How a part of an animation is played.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Play {
    /// Over and over, `fps` frames a second from the pose's start.
    Loop,
    /// Once, then holding its last frame.
    Once,
    /// At this point of the part, 0 its first frame and 1 its last (a wind-up's progress).
    At(f64),
}

/// Seconds a struck fighter holds the first frame of its `hit`, the furthest it reels, before it recovers: the blow
/// reads on the body after the hit-stop lets go.
pub const REEL: f64 = 0.1;

/// The part of an animation a pose plays: frames from `from` to `to` as fractions of the animation, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Part {
    clip: &'static str,
    from: f64,
    to: f64,
    play: Play,
}

const fn part(clip: &'static str, from: f64, to: f64, play: Play) -> Part {
    Part {
        clip,
        from,
        to,
        play,
    }
}

const fn whole(clip: &'static str, play: Play) -> Part {
    part(clip, 0.0, 1.0, play)
}

/// The part of an animation `a`'s pose plays. A blade blow and a spell are each one animation split in two: the
/// wind-up (`Draw`, `Channel`) plays its first part by the wind-up's progress, the follow-through (`Slash`,
/// `Cast`) the rest. A blow from the air (`dive`) is the overhead cut; a spell gathered by lunging is the dash.
fn part_of(a: &Appearance) -> Part {
    let progress = a.act.and_then(|k| k.windup).map(|w| w.progress);
    let windup = |clip, to| match progress {
        Some(p) => part(clip, 0.0, to, Play::At(p)),
        None => part(clip, 0.0, to, Play::Once),
    };
    let blade = if a.act.is_some_and(|k| k.dive) {
        "overhead"
    } else {
        "slash"
    };
    let spell = if a.act.is_some_and(|k| k.how == Gather::Lunge) {
        "dash"
    } else {
        "cast"
    };
    match a.pose {
        Pose::Idle => whole("stance-idle", Play::Loop),
        Pose::Walk => whole("walk", Play::Loop),
        Pose::Jump => part("jump", 0.0, 0.5, Play::Once),
        Pose::Fly => part("jump", 0.5, 0.5, Play::Once),
        Pose::Land => part("jump", 0.625, 1.0, Play::Once),
        Pose::Crouch => part("jump", 0.125, 0.125, Play::Once),
        Pose::Duck => part("duck", 0.0, 0.5, Play::Once),
        Pose::Draw => windup(blade, 0.5),
        Pose::Slash => part(blade, 0.5, 1.0, Play::Once),
        Pose::Channel => windup(spell, 0.5),
        Pose::Cast => part(spell, 0.5, 1.0, Play::Once),
        Pose::Guard => whole("guard", Play::Once),
        Pose::Meditate => whole("meditate", Play::Once),
        Pose::Hurt => whole("hit", Play::Once),
        Pose::Frozen => whole("frozen", Play::Once),
        Pose::Shocked => whole("shocked", Play::Loop),
        Pose::Launched => whole("launched", Play::Once),
        Pose::Tumble => whole("tumble", Play::Loop),
        Pose::Fall => part("knockdown", 0.0, 0.625, Play::Once),
        Pose::Down if a.out => whole("defeat", Play::Once),
        Pose::Down => part("knockdown", 1.0, 1.0, Play::Once),
        Pose::Roll => whole("roll", Play::Once),
        Pose::Sit => part("getting-up", 0.0, 0.5, Play::Once),
        Pose::Kneel => part("getting-up", 0.5, 1.0, Play::Once),
        Pose::Win => whole("win", Play::Once),
    }
}

/// The frame `a` shows, from an animation of `len(clip)` frames played at `fps` frames a second.
pub fn pick(a: &Appearance, fps: f64, len: impl Fn(&str) -> usize) -> Shot {
    let p = part_of(a);
    let last = len(p.clip).saturating_sub(1) as f64;
    let (first, end) = (js_round(p.from * last), js_round(p.to * last));
    let span = (end - first).max(0.0);
    let t = if a.pose == Pose::Hurt {
        a.pose_t - REEL
    } else {
        a.pose_t
    };
    let step = if t.is_finite() {
        floor(t.max(0.0) * fps)
    } else {
        0.0
    };
    let along = match p.play {
        Play::Loop => step % (span + 1.0),
        Play::Once => step.min(span),
        Play::At(k) => js_round(k.clamp(0.0, 1.0) * span),
    };
    Shot {
        clip: p.clip,
        frame: (first + along) as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixfx_fighter::paint::{Act, Windup};

    fn stand(pose: Pose, pose_t: f64) -> Appearance {
        Appearance {
            pose,
            pose_t,
            face: 1.0,
            x: 100.0,
            air: 0.0,
            vx: 0.0,
            vy: 0.0,
            out: false,
            flash: false,
            untouchable: false,
            frozen: false,
            shocked: false,
            hit_t: 9.0,
            hit_power: 0.0,
            dodged: 0.0,
            act: None,
            jolt: [0, 0],
        }
    }

    const NINE: fn(&str) -> usize = |_| 9;

    #[test]
    fn idle_loops_through_its_frames() {
        let at = |t| pick(&stand(Pose::Idle, t), 10.0, NINE);
        assert_eq!(
            at(0.0),
            Shot {
                clip: "stance-idle",
                frame: 0
            }
        );
        assert_eq!(at(0.85).frame, 8);
        assert_eq!(at(0.9).frame, 0, "the tenth step wraps to the first frame");
    }

    #[test]
    fn a_blow_holds_its_last_frame() {
        assert_eq!(
            pick(&stand(Pose::Hurt, 5.0), 10.0, NINE),
            Shot {
                clip: "hit",
                frame: 8
            }
        );
    }

    #[test]
    fn a_struck_fighter_holds_its_furthest_reel_before_it_recovers() {
        let at = |t| pick(&stand(Pose::Hurt, t), 10.0, NINE).frame;
        assert_eq!(at(0.0), 0);
        assert_eq!(at(REEL + 0.05), 0, "still reeling");
        assert_eq!(at(REEL + 0.1), 1, "then it recovers");
    }

    #[test]
    fn a_blade_blow_is_one_animation_split_between_windup_and_follow_through() {
        let windup = |p| {
            let mut a = stand(Pose::Draw, 0.0);
            a.act = Some(Act {
                how: Gather::Palm,
                dive: false,
                windup: Some(Windup {
                    progress: p,
                    long: false,
                }),
            });
            pick(&a, 10.0, NINE)
        };
        assert_eq!(
            windup(0.0),
            Shot {
                clip: "slash",
                frame: 0
            }
        );
        assert_eq!(
            windup(1.0),
            Shot {
                clip: "slash",
                frame: 4
            }
        );
        assert_eq!(
            pick(&stand(Pose::Slash, 0.0), 10.0, NINE),
            Shot {
                clip: "slash",
                frame: 4
            }
        );
        assert_eq!(
            pick(&stand(Pose::Slash, 9.0), 10.0, NINE),
            Shot {
                clip: "slash",
                frame: 8
            }
        );
    }

    #[test]
    fn a_blow_from_the_air_is_the_overhead_cut_and_a_lunge_the_dash() {
        let mut a = stand(Pose::Slash, 0.0);
        a.act = Some(Act {
            how: Gather::Palm,
            dive: true,
            windup: None,
        });
        assert_eq!(pick(&a, 10.0, NINE).clip, "overhead");
        let mut b = stand(Pose::Cast, 0.0);
        b.act = Some(Act {
            how: Gather::Lunge,
            dive: false,
            windup: None,
        });
        assert_eq!(pick(&b, 10.0, NINE).clip, "dash");
    }

    #[test]
    fn a_fighter_knocked_out_on_the_floor_plays_its_defeat() {
        let mut a = stand(Pose::Down, 0.0);
        assert_eq!(
            pick(&a, 10.0, NINE),
            Shot {
                clip: "knockdown",
                frame: 8
            }
        );
        a.out = true;
        assert_eq!(pick(&a, 10.0, NINE).clip, "defeat");
    }

    #[test]
    fn every_pose_picks_an_animation_the_sheet_must_hold() {
        use Pose::*;
        for pose in [
            Idle, Walk, Jump, Fly, Land, Crouch, Duck, Draw, Slash, Channel, Cast, Guard, Meditate,
            Hurt, Frozen, Shocked, Launched, Tumble, Fall, Down, Roll, Sit, Kneel, Win,
        ] {
            let shot = pick(&stand(pose, 0.3), 10.0, NINE);
            assert!(CLIPS.contains(&shot.clip), "{pose:?} picks {}", shot.clip);
            assert!(shot.frame < 9, "{pose:?} picks frame {}", shot.frame);
        }
    }
}

//! The shapes the rules test hits against, for tests and tools (the art kept in step with the rules) and the page's
//! hitbox overlay (`arena-game`'s `boxes`): each fighter's body (where it can be hit), the area a cast strikes while
//! the rules test it, every shot, field and unit on the field, and the points a blow and a bolt come from (the hand's
//! wrist and hilt, a bolt's launch). Read only: nothing
//! here decides anything. Each shape mirrors the test that uses it, named beside it, so a change to a test changes
//! the shape drawn here in the same change.
//!
//! Coordinates are the arena's pixels: `x` from the arena's left as the rules keep it, `y` up from the ground (the
//! rules' `air`).

use crate::data::Kind;
use crate::fighter::{Phase, State};
use crate::shot::{ShotKind, orb_at};

/// How long a strike's box stays drawn after the tick it struck on, seconds.
pub const STRIKE_SHOWN: f64 = 0.12;

/// What a shape is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A fighter's body, which a blow must touch (`shot::overlaps`, `unit::in_front`, `unit::enemies_near`).
    Body,
    /// A body that nothing can touch now (a dodge, a blink).
    Untouchable,
    /// Where a melee or blast cast struck: the rules test it once, on the tick its windup ends (the active frame), and
    /// a foe whose body reaches into it is hit; it is shown for `STRIKE_SHOWN` after so an eye can catch it.
    Strike,
    /// A shot in flight: a bolt or an orb hits a body it touches.
    Shot,
    /// An area that strikes where it stands or falls: a zone, a vortex.
    Field,
    /// A trap, mine, totem or minion standing on the field.
    Unit,
    /// The wrist of the hand held out ahead: the arm's hurtbox ends there (`frames::Frame::wrist`), a point.
    Wrist,
    /// The hilt (or a palm's fingertips): a blade's hitbox starts there (`frames::Frame::hilt`), a point.
    Hilt,
    /// Where a bolt leaves the casting hand (`frames::cast_hand`), a point.
    Launch,
}

/// A shape in the arena's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// From (`x0`, `y0`) to (`x1`, `y1`).
    Rect {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    },
    Circle {
        x: f64,
        y: f64,
        r: f64,
    },
}

/// One shape to draw, whose it is, and what it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    /// The seat it belongs to; `None` for nobody's.
    pub owner: Option<usize>,
    pub mark: Mark,
    pub shape: Shape,
    /// The skill it comes from, if any.
    pub skill: &'static str,
}

/// Every shape the rules test hits against this tick.
pub fn shapes(state: &State) -> Vec<Drawn> {
    let mut out = Vec::new();
    for (me, f) in state.fighters.iter().enumerate() {
        let b = &f.body;
        // the hurtboxes of the frame the fighter shows (frames::hurtboxes), as shot::overlaps and unit::strike_hits
        // test them
        let mark = if b.untouchable() {
            Mark::Untouchable
        } else {
            Mark::Body
        };
        for h in crate::frames::hurtboxes(state, me) {
            out.push(Drawn {
                owner: Some(me),
                mark,
                shape: Shape::Rect {
                    x0: h[0],
                    y0: h[1],
                    x1: h[2],
                    y1: h[3],
                },
                skill: "",
            });
        }

        // the hand's points on the frame shown, placed as the boxes are (a point is a circle of radius 0)
        let point = |mark: Mark, x: f64, y: f64| Drawn {
            owner: Some(me),
            mark,
            shape: Shape::Circle { x, y, r: 0.0 },
            skill: "",
        };
        if let Some(fr) = crate::frames::frame_of(state, me) {
            for (mark, p) in [(Mark::Wrist, fr.wrist), (Mark::Hilt, fr.hilt)] {
                if let Some([px, py]) = p {
                    let [x, y, _, _] = crate::frames::place([px, py, px, py], b.x, b.air, b.face);
                    out.push(point(mark, x, y));
                }
            }
        }

        let Some(act) = f.act else { continue };
        if let Some((x, y)) = crate::frames::cast_hand(state, me) {
            out.push(point(Mark::Launch, x, y));
        }
        // the strike is tested on the tick the windup ends, when the act turns to its recovery, and a blade that
        // missed again on each tick while its clip is in the active window (a retry due, `act::due`)
        let retrying = state.pending.iter().any(|p| p.retry && p.payload.by == me)
            && crate::frames::in_window(state, me);
        if !retrying && (act.phase != Phase::Recover || act.t > STRIKE_SHOWN) {
            continue;
        }
        let Some(k) = f.kit.index(act.spell) else {
            continue;
        };
        let sp = f.kit.skills[k].motion;
        let foe = &state.fighters[1 - me].body.spec;
        let (reach, level, blade) = match sp.kind {
            // act::strike, Melee: unit::in_front -> unit::strike_hits, the blade's hitbox
            Kind::Melee => (sp.range.unwrap_or(0.0), b.spec.chest + 16.0, true),
            // act::strike, Blast: unit::strike_hits, its own box
            Kind::Blast => (sp.range.unwrap_or(0.0), foe.body, false),
            _ => continue,
        };
        let face = if b.face < 0.0 { -1.0 } else { 1.0 };
        let [x0, y0, x1, y1] =
            crate::unit::strike_box(state, me, b.x, b.air, face, reach, level, blade);
        out.push(Drawn {
            owner: Some(me),
            mark: Mark::Strike,
            shape: Shape::Rect { x0, y0, x1, y1 },
            skill: act.spell,
        });
    }

    for s in &state.shots {
        let sp = s.spell;
        let r = sp.radius.unwrap_or(0.0);
        let drawn = match &s.kind {
            // shot::run_bolt: shot::overlaps at the bolt's place, give or take its radius
            ShotKind::Bolt { .. } => s
                .bolt_at(state)
                .map(|(x, y, _)| (Mark::Shot, Shape::Circle { x, y, r })),
            // shot::run, Orb: shot::overlaps(bx, -by, radius)
            ShotKind::Orb { .. } => {
                let (bx, by) = orb_at(s);
                Some((Mark::Shot, Shape::Circle { x: bx, y: -by, r }))
            }
            // shot::run, Zone: unit::enemies_near(x, air, radius, 40 grounded or 88)
            ShotKind::Zone { x, air, .. } => {
                let level = if sp.grounded { 40.0 } else { 88.0 };
                Some((
                    Mark::Field,
                    Shape::Rect {
                        x0: x - r,
                        y0: (air - level).max(0.0),
                        x1: x + r,
                        y1: air + level,
                    },
                ))
            }
            // shot::run, Void: unit::enemies_near(x, 0, under, any height)
            ShotKind::Void { x, .. } => {
                let under = sp.under.unwrap_or(0.0);
                Some((
                    Mark::Field,
                    Shape::Rect {
                        x0: x - under,
                        y0: 0.0,
                        x1: x + under,
                        y1: state.ground - state.top,
                    },
                ))
            }
            ShotKind::Spray => None,
        };
        if let Some((mark, shape)) = drawn {
            out.push(Drawn {
                owner: Some(s.owner),
                mark,
                shape,
                skill: s.name,
            });
        }
    }

    for u in &state.units {
        let r = u.radius();
        out.push(Drawn {
            owner: Some(u.owner),
            mark: Mark::Unit,
            shape: Shape::Rect {
                x0: u.x - r,
                y0: 0.0,
                x1: u.x + r,
                y1: 2.0 * r,
            },
            skill: u.skill,
        });
    }
    out
}

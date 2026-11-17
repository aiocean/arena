//! Swirl blast (`explosions/swirl-blast`): a ball rushes in from behind trailing a tail, swells into a big bright
//! blob, rolls over into a dome like a breaking wave, curls round and sweeps away as arcs along the ground. Painted
//! in tones through a ramp: the rules' palette, else the engine's ember colourway.

use super::common::{DEG, EMBER, Ink, Knot, curve, pose};
use crate::vfx::kit::{Layer, Opts, Painter, Rgba, Show};

/// Seconds a moment of the sheet lasts.
const MOMENT: f64 = 0.06;
const MOMENTS: f64 = 14.0;
pub const DURATION: f64 = MOMENTS * MOMENT;

/// The blast at each moment, six knots: the ball (x, y, radius, tone); the end of its tail (x, y, half-width,
/// tone); then two crescents, each a circle (centre x, y, radius, half-width) and a sweep (from, to in degrees
/// clockwise from the right, tone, unused): one wrapping over the ball as it rolls, one sweeping along the ground.
const BLAST: [[Knot; 6]; 14] = [
    [
        (-23.5, 5.5, 6.0, 11.0),
        (-31.5, 5.5, 0.5, 4.0),
        (3.0, 0.0, 19.0, 0.0),
        (-30.0, 60.0, 8.0, 0.0),
        (0.0, -25.5, 42.5, 0.0),
        (88.0, 124.0, 8.0, 0.0),
    ],
    [
        (-16.5, 6.0, 10.5, 15.0),
        (-30.5, 6.0, 0.8, 5.0),
        (3.0, 0.0, 19.0, 0.0),
        (-30.0, 60.0, 8.0, 0.0),
        (0.0, -25.5, 42.5, 0.0),
        (88.0, 124.0, 8.0, 0.0),
    ],
    [
        (2.0, 0.0, 18.0, 15.0),
        (-28.5, 6.0, 1.0, 6.0),
        (3.0, 0.0, 19.0, 3.0),
        (-30.0, 60.0, 8.0, 0.0),
        (0.0, -25.5, 42.5, 0.0),
        (88.0, 124.0, 8.0, 0.0),
    ],
    [
        (1.5, -2.5, 15.0, 15.0),
        (-22.0, 8.0, 1.0, 6.0),
        (1.5, -2.5, 21.0, 5.0),
        (-35.0, 65.0, 9.0, 0.0),
        (0.0, -25.5, 42.5, 0.0),
        (88.0, 124.0, 8.0, 0.0),
    ],
    [
        (-6.5, -6.5, 14.0, 15.0),
        (-20.0, 6.0, 0.8, 5.0),
        (-3.0, -3.0, 22.0, 2.5),
        (-15.0, 75.0, 5.0, 0.0),
        (0.0, -25.5, 42.5, 0.0),
        (88.0, 124.0, 8.0, 0.0),
    ],
    [
        (-9.5, -2.5, 15.5, 15.0),
        (-12.0, 10.0, 0.5, 4.0),
        (-6.0, -2.0, 20.0, 2.5),
        (-80.0, -10.0, 6.0, 0.0),
        (0.0, -25.5, 42.5, 0.0),
        (88.0, 124.0, 8.0, 0.0),
    ],
    [
        (-14.0, 2.0, 13.0, 15.0),
        (-6.0, 15.0, 1.5, 9.0),
        (-8.0, 2.0, 17.0, 3.0),
        (-120.0, -60.0, 12.0, 0.0),
        (0.0, -25.5, 42.5, 1.5),
        (88.0, 124.0, 8.0, 0.0),
    ],
    [
        (-10.5, 2.5, 10.0, 15.0),
        (-10.5, 2.5, 0.5, 6.0),
        (-8.0, 2.0, 17.0, 1.0),
        (-130.0, -70.0, 3.0, 0.0),
        (0.0, -25.5, 42.5, 3.5),
        (60.0, 126.0, 12.0, 0.0),
    ],
    [
        (-7.5, 9.5, 7.0, 9.0),
        (-7.5, 9.5, 0.5, 4.0),
        (-8.0, 2.0, 17.0, 0.0),
        (-130.0, -70.0, 0.0, 0.0),
        (0.0, -25.5, 42.5, 2.5),
        (59.0, 121.0, 9.0, 0.0),
    ],
    [
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 2.0, 17.0, 0.0),
        (-130.0, -70.0, 0.0, 0.0),
        (4.0, -25.0, 40.0, 1.5),
        (53.0, 122.0, 3.0, 0.0),
    ],
    [
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 2.0, 17.0, 0.0),
        (-130.0, -70.0, 0.0, 0.0),
        (4.0, -25.0, 40.0, 1.0),
        (47.0, 126.0, 2.0, 0.0),
    ],
    [
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 2.0, 17.0, 0.0),
        (-130.0, -70.0, 0.0, 0.0),
        (4.0, -25.0, 40.0, 0.7),
        (44.0, 120.0, 1.2, 0.0),
    ],
    [
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 2.0, 17.0, 0.0),
        (-130.0, -70.0, 0.0, 0.0),
        (4.0, -25.0, 40.0, 0.5),
        (43.0, 112.0, 0.6, 0.0),
    ],
    [
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 10.0, 0.0, 0.0),
        (-8.0, 2.0, 17.0, 0.0),
        (-130.0, -70.0, 0.0, 0.0),
        (4.0, -25.0, 40.0, 0.5),
        (44.0, 104.0, 0.3, 0.0),
    ],
];

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(Swirl {
        ramp: o.palette_or(&EMBER),
    })
}

struct Swirl {
    ramp: Vec<Rgba>,
}

impl Show for Swirl {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        let s = t / MOMENT;
        if layer == Layer::Back || s >= MOMENTS {
            return;
        }
        let [ball, tail, wrap, wrap_sweep, ground, ground_sweep] = pose(&BLAST, s);
        let mut ink = Ink::new().rim(0.45).grain(0.12, 0.8, 4.0);
        if ball.2 > 0.0 {
            ink.dot(ball.0, ball.1, ball.2, ball.3);
            ink.seg(
                (ball.0, ball.1),
                (tail.0, tail.1),
                (ball.2 * 0.5, tail.2),
                (ball.3, tail.3),
            );
        }
        // the wrap is thickest and brightest where it ends, the ground sweep where it starts (its far right end)
        for (circle, (from, to, v, _)) in [
            (wrap, wrap_sweep),
            (
                ground,
                (ground_sweep.1, ground_sweep.0, ground_sweep.2, 0.0),
            ),
        ] {
            if circle.3 > 0.0 {
                ink.arc(
                    (circle.0, circle.1),
                    circle.2,
                    from * DEG,
                    to * DEG,
                    (circle.3 * 0.3, circle.3),
                    (v * 0.5, v),
                );
            }
        }
        let flare = curve(&[(1.0, 0.0), (2.5, 1.0), (5.5, 0.6), (7.5, 0.0)], s);
        ink.paint(p, &self.ramp, 32.0, 0.08 * (1.0 + 2.0 * flare));
    }
}

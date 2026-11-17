//! Swirl portal (`spawn/swirl-portal`): a flat disc seen at a slant is swept in clockwise from the top into a full
//! bright oval, then swept away the same way, leaving swirling strands that drift to the top and fade. Painted in
//! tones through a ramp: the rules' palette, else the engine's ember colourway.

use super::common::{DEG, EMBER, Ink, LAST, color, curve};
use crate::vfx::kit::{Layer, Opts, Painter, Rgba, Show, TAU, libm};

/// Seconds a moment of the sheet lasts.
const MOMENT: f64 = 0.06;
const MOMENTS: f64 = 15.0;
pub const DURATION: f64 = MOMENTS * MOMENT;

/// The disc: a circle of this radius, seen squashed to half its height.
const RADIUS: f64 = 23.5;
const SQUASH: f64 = 2.04;

/// The swept part, in degrees clockwise from the top: it grows to a full turn, then its start follows round.
const FILL_TO: [(f64, f64); 6] = [
    (0.0, 0.0),
    (1.0, 60.0),
    (2.0, 120.0),
    (3.0, 240.0),
    (4.0, 330.0),
    (5.0, 360.0),
];
const FILL_FROM: [(f64, f64); 6] = [
    (5.0, 0.0),
    (6.0, 70.0),
    (7.0, 170.0),
    (8.0, 220.0),
    (9.0, 290.0),
    (10.0, 360.0 + TWIST),
];
/// How far in from the rim it is swept (0: to the centre), and its tone.
const FILL_IN: [(f64, f64); 5] = [
    (0.0, 21.0),
    (1.0, 15.0),
    (2.0, 10.0),
    (3.0, 3.0),
    (4.0, 0.0),
];
const FILL_TONE: [(f64, f64); 4] = [(0.0, 5.0), (1.0, 11.0), (2.0, 13.0), (3.0, 15.0)];
/// The strands that run ahead of the sweep while it fills and trail behind it while it empties.
const STRAND_TONE: [(f64, f64); 9] = [
    (0.0, 4.0),
    (2.0, 8.0),
    (4.0, 9.0),
    (8.0, 9.0),
    (9.0, 8.0),
    (10.0, 5.0),
    (11.0, 3.0),
    (12.0, 1.5),
    (13.0, 0.7),
];
const STRAND_AT: [(f64, f64); 10] = [
    (5.0, 0.0),
    (6.0, 70.0),
    (7.0, 170.0),
    (8.0, 220.0),
    (9.0, 290.0),
    (10.0, 330.0),
    (11.0, 360.0),
    (12.0, 395.0),
    (13.0, 425.0),
    (14.0, 445.0),
];
/// How far out the strands run, as a share of the disc's radius: they draw in as they fade.
const STRAND_REACH: [(f64, f64); 4] = [(11.0, 1.0), (12.0, 1.05), (13.0, 1.05), (14.0, 0.85)];
/// How long each strand is, in degrees: they stretch over the top as the sweep ends, then break up.
const STRAND_SPAN: [(f64, f64); 4] = [(10.0, 80.0), (11.0, 160.0), (12.0, 120.0), (13.0, 70.0)];
/// How far the sweep's edge lags at the centre, in degrees: it is a spiral, not a straight cut.
const TWIST: f64 = 60.0;
/// How far behind `STRAND_AT` the strands start, in degrees.
const STRAND_LAG: f64 = 80.0;

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(Portal {
        ramp: o.palette_or(&EMBER),
    })
}

struct Portal {
    ramp: Vec<Rgba>,
}

impl Show for Portal {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        let s = t / MOMENT;
        if layer == Layer::Back || s >= MOMENTS {
            return;
        }
        let (to, from) = (curve(&FILL_TO, s), curve(&FILL_FROM, s));
        let (inner, fill) = (curve(&FILL_IN, s), curve(&FILL_TONE, s));
        // strands, on the unsquashed circle: ahead of the sweep while it fills, behind it as it empties
        let lead = if s < 5.0 {
            to
        } else {
            curve(&STRAND_AT, s) - STRAND_LAG
        };
        let v = curve(&STRAND_TONE, s);
        let reach = curve(&STRAND_REACH, s);
        let span = curve(&STRAND_SPAN, s);
        let mut strands = Ink::new().grain(0.15, 0.7, 6.0);
        for (k, f) in [0.92, 0.72, 0.52].iter().enumerate() {
            let a = (lead - 90.0 - 15.0 * k as f64) * DEG;
            strands.arc(
                (0.0, 0.0),
                RADIUS * f * reach,
                a,
                a + span * DEG,
                (1.4, 0.6),
                (v, v * 0.6),
            );
        }
        let ramp = &self.ramp;
        p.shade(-32.0, -32.0, 31.0, 31.0, |x, y, o| {
            let (cx, cy) = (x, y * SQUASH);
            let r = libm::sqrt(cx * cx + cy * cy);
            // degrees clockwise from the top
            let a = (libm::atan2(cy, cx) + core::f64::consts::FRAC_PI_2).rem_euclid(TAU) / DEG;
            let lag = TWIST * (1.0 - r / RADIUS);
            let swept = r <= RADIUS
                && r >= inner
                && a + lag >= from
                && a + lag <= to.max(from) + if to >= 360.0 { lag } else { 0.0 };
            let fv = swept.then_some(if r > RADIUS - 2.5 { fill * 0.62 } else { fill });
            let tv = match (fv, strands.tone(cx, cy)) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            }?;
            o.k = 0.08 * (tv - 10.0).max(0.0);
            Some(color(ramp, tv / LAST))
        });
    }
}

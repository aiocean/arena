//! The rules' hand-written tests, as `frames::overlap`, `shot::overlaps` and `projectile::entry` do them.

use crate::{Backend, Geo};

pub struct Hand;

/// `arena_calc::projectile`'s private `entry`, copied.
pub fn entry(p: (f64, f64), dir: (f64, f64), c: (f64, f64), r: f64, reach: f64) -> Option<f64> {
    let (fx, fy) = (p.0 - c.0, p.1 - c.1);
    let b = fx * dir.0 + fy * dir.1;
    let cc = fx * fx + fy * fy - r * r;
    if cc <= 0.0 {
        return Some(0.0);
    }
    let disc = b * b - cc;
    if disc < 0.0 || b >= 0.0 {
        return None;
    }
    let s = -b - libm::sqrt(disc);
    (s >= 0.0 && s <= reach).then_some(s)
}

impl Backend for Hand {
    fn load(&mut self, _: &Geo) {}
    fn strike(&mut self, g: &Geo, me: usize) -> bool {
        g.hurt[1 - me]
            .iter()
            .any(|h| arena_duel::frames::overlap(g.strike[me], *h))
    }
    fn probe(&mut self, g: &Geo, i: usize) -> bool {
        let (f, x, y, r) = g.probes[i];
        g.hurt[f]
            .iter()
            .any(|h| x > h[0] - r && x < h[2] + r && y > h[1] - r && y < h[3] + r)
    }
    fn bolt(&mut self, g: &Geo, i: usize) -> Option<(u32, f64)> {
        let b = &g.bolts[i];
        let mut best: Option<(u32, f64)> = None;
        for &(id, x, y, r) in &g.circles[1 - b.owner] {
            if let Some(s) = entry((b.px, b.py), (b.dx, b.dy), (x, y), r.max(0.0), b.reach)
                && best.is_none_or(|(_, t)| s < t)
            {
                best = Some((id, s));
            }
        }
        best
    }
}

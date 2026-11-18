//! Where the rules play each effect, beside the fighters' hurtboxes and its skill's hitboxes: every skill of the pool cast once at a foe standing still
//! (`hits::plays`), each effect on the first tick its skill can hit while it plays (else when first played), its point, both
//! fighters' hurtboxes and the skill's hitboxes that tick,
//! relative to the point in sheet pixels (`UNIT` arena pixels each, `y` down, facing east: a mirrored play is turned
//! back). For a review page that draws an effect over the bodies it plays on.
//!
//! `cargo run -p arena-wiki --example fx_anchors -- out.json`

// nothing here decides the fight (it only reports it): std's float rounding is fine
#![allow(clippy::disallowed_methods)]

use arena_calc::gem::GEMS;
use arena_duel::data::UNIT;
use arena_wiki::hits;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn main() -> Result<(), String> {
    let out = std::env::args()
        .nth(1)
        .ok_or("usage: fx_anchors <out.json>")?;
    let mut by_fx: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for g in GEMS {
        for p in hits::plays(g.id) {
            // a box relative to the point, in sheet pixels, mirrored back to face east
            let rel = |b: &[f64; 4]| {
                let (mut x0, mut x1) = ((b[0] - p.x) / UNIT, (b[2] - p.x) / UNIT);
                if p.flip {
                    (x0, x1) = (-x1, -x0);
                }
                [x0, (b[1] - p.y) / UNIT, x1, (b[3] - p.y) / UNIT]
            };
            let bodies: Vec<Value> = p
                .bodies
                .iter()
                .map(|bs| json!(bs.iter().map(rel).collect::<Vec<_>>()))
                .collect();
            by_fx.entry(p.fx).or_default().push(json!({
                "skill": g.id,
                "t": (p.t * 100.0).round() / 100.0,
                "flip": p.flip,
                "ground": (hits::FLOOR - p.y) / UNIT,
                "hits": p.hits.iter().map(|(m, b)| json!({"mark": format!("{m:?}"), "box": rel(b)})).collect::<Vec<_>>(),
                "caster": bodies[0],
                "foe": bodies[1],
            }));
        }
    }
    let text = serde_json::to_string(&by_fx).map_err(|e| e.to_string())?;
    std::fs::write(&out, text).map_err(|e| format!("{out}: {e}"))?;
    eprintln!("{} effects placed", by_fx.len());
    Ok(())
}

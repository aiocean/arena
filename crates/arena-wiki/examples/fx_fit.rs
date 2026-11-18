//! Moves the anchors of the drawn effects named in a placing file, each the way it was judged by eye against the
//! rules' boxes (the review page of `fx_anchors`): every skill of the pool cast once at a foe standing still
//! (`hits::plays`), and for each named effect the tick its skill first hits while it plays (else when first played),
//! its point, the fighters' hurtboxes and the skill's hitboxes there. The effect's drawn shape is the middle of its
//! frames' boxes (each edge's median over the frames at least a quarter the largest's area) and its foot the lowest
//! of them. A placing says where that shape goes:
//!
//! - `box`: its middle on the skill's hitboxes' middle; `ground-box`: on the floor (`ground`), its foot on it, its
//!   middle over the hitboxes' middle
//! - `point`: its middle on the point the rules play it at
//! - `body`: its middle on the trunk of the fighter it plays on (legs and trunk, not the arm); `body-x`: only across
//! - `front`: standing just ahead of that fighter's trunk (east), its middle at the trunk's height
//! - `over-head`: its middle just over that fighter's head; `round-head`: its middle on the head; `from-head`: its
//!   foot on the head
//! - `floor`: as it is across, its foot on the floor (`ground`)
//! - `head-at-point`: its leading edge (east) on the point, as it is up and down (a bolt's head)
//! - `["shift", x, y]`: moved by what was measured by eye on its frames (sheet pixels, y down); `["ground-shift",
//!   x, y]`: the same, and drawn on the floor (`ground`) from then on
//!
//! An effect not named is left as it is.
//!
//! `cargo run -p arena-wiki --example fx_fit -- art/vfx crates/arena-game/assets/drawn/drawn.json placing.json report.json [--write]`
//! (`art/vfx/placing.json` the one in use: `{"<fx>": "<placing>"}`; `--write` rewrites the `motion.json`s, then `art/vfx/tools/vfx.rs --all`
//! builds the sheets again)

// nothing here decides the fight (it only places a picture): std's float rounding is fine
#![allow(clippy::disallowed_methods)]

use arena_calc::gem::GEMS;
use arena_duel::data::UNIT;
use arena_duel::debug::Mark;
use arena_vfx::drawn::Cell;
use arena_wiki::hits;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How far over the head a halo floats, and how far down the trunk box the head reaches (sheet pixels).
const OVER: f64 = 4.0;
const HEAD: f64 = 8.0;

/// An effect's drawn shape from its anchor: left, top, right, bottom (medians) and its foot (the lowest).
fn shape(cells: &[Cell]) -> Option<[f64; 5]> {
    let area = |c: &Cell| f64::from(c.w) * f64::from(c.h);
    let big = cells.iter().map(area).fold(0.0, f64::max);
    let kept: Vec<&Cell> = cells
        .iter()
        .filter(|c| area(c) >= big / 4.0 && big > 1.0)
        .collect();
    if kept.is_empty() {
        return None;
    }
    let median = |f: &dyn Fn(&Cell) -> f64| {
        let mut v: Vec<f64> = kept.iter().map(|c| f(c)).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let bottom = |c: &Cell| f64::from(c.dy) + f64::from(c.h);
    Some([
        median(&|c| f64::from(c.dx)),
        median(&|c| f64::from(c.dy)),
        median(&|c| f64::from(c.dx) + f64::from(c.w)),
        median(&bottom),
        kept.iter().map(|c| bottom(c)).fold(f64::MIN, f64::max),
    ])
}

fn union(bs: &[[f64; 4]]) -> Option<[f64; 4]> {
    (!bs.is_empty()).then(|| {
        bs.iter()
            .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, b| {
                [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ]
            })
    })
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [art, drawn, placing, report, ..] = args.as_slice() else {
        return Err(
            "usage: fx_fit <art/vfx> <drawn.json> <placing.json> <report.json> [--write]".into(),
        );
    };
    let write = args.iter().any(|a| a == "--write");
    let read = |p: &str| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let sheets = arena_vfx::drawn::read(&read(drawn)?)?;
    let placing: BTreeMap<String, Value> =
        serde_json::from_str(&read(placing)?).map_err(|e| format!("{placing}: {e}"))?;
    // each effect's art folder, by its id
    let mut dirs: BTreeMap<String, PathBuf> = BTreeMap::new();
    for e in std::fs::read_dir(art)
        .map_err(|e| format!("{art}: {e}"))?
        .flatten()
    {
        if let Ok(t) = std::fs::read_to_string(e.path().join("motion.json")) {
            let v: Value =
                serde_json::from_str(&t).map_err(|err| format!("{}: {err}", e.path().display()))?;
            if let Some(fx) = v["fx"].as_str() {
                dirs.insert(fx.to_owned(), e.path());
            }
        }
    }
    // the first play of each effect on a tick its skill can hit, else its first play
    let mut first: BTreeMap<&str, (&str, hits::Play)> = BTreeMap::new();
    for g in GEMS {
        for p in hits::plays(g.id) {
            let better = match first.get(p.fx) {
                None => true,
                Some((_, q)) => q.hits.is_empty() && !p.hits.is_empty(),
            };
            if better {
                first.insert(p.fx, (g.id, p));
            }
        }
    }
    let mut rows = Vec::new();
    for (fx, said) in &placing {
        // a placing is a name, or `[name, across, up and down]` for one measured by eye
        let (how, by) = match said {
            Value::Array(a) => (
                a.first()
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                [
                    a.get(1).and_then(Value::as_f64).unwrap_or(0.0),
                    a.get(2).and_then(Value::as_f64).unwrap_or(0.0),
                ],
            ),
            v => (v.as_str().unwrap_or_default().to_owned(), [0.0, 0.0]),
        };
        let d = sheets
            .iter()
            .find(|d| &d.fx == fx)
            .ok_or(format!("{fx}: not in {drawn}"))?;
        let (skill, p) = first
            .get(fx.as_str())
            .ok_or(format!("{fx}: no skill plays it in one cast"))?;
        let cells = d
            .layout
            .clips
            .values()
            .next()
            .ok_or(format!("{fx}: no frames"))?;
        let drawn_shape = shape(cells).ok_or(format!("{fx}: no frames"))?;
        // the rules' boxes from the point, sheet pixels, facing east
        let rel = |b: &[f64; 4]| {
            let (mut x0, mut x1) = ((b[0] - p.x) / UNIT, (b[2] - p.x) / UNIT);
            if p.flip {
                (x0, x1) = (-x1, -x0);
            }
            [x0, (b[1] - p.y) / UNIT, x1, (b[3] - p.y) / UNIT]
        };
        let floor = (hits::FLOOR - p.y) / UNIT;
        let hit = union(
            &p.hits
                .iter()
                .filter(|(m, _)| *m != Mark::Wrist)
                .map(|(_, b)| rel(b))
                .collect::<Vec<_>>(),
        );
        // the fighter it plays on: the one whose trunk is nearest the point
        let trunks: Vec<[f64; 4]> = p
            .bodies
            .iter()
            .filter_map(|bs| union(&bs.iter().take(2).map(rel).collect::<Vec<_>>()))
            .collect();
        let trunk = trunks.iter().copied().min_by(|a, b| {
            let d = |t: &[f64; 4]| ((t[0] + t[2]) / 2.0).abs() + ((t[1] + t[3]) / 2.0).abs();
            d(a).total_cmp(&d(b))
        });
        let was_ground = d.layout.motion.ground;
        let lift = if was_ground { floor } else { 0.0 };
        // the shape as the game draws it now, from the point
        let (l, t, r, b, foot) = (
            drawn_shape[0],
            drawn_shape[1] + lift,
            drawn_shape[2],
            drawn_shape[3] + lift,
            drawn_shape[4] + lift,
        );
        let (cx, cy) = ((l + r) / 2.0, (t + b) / 2.0);
        let mid = |bx: [f64; 4]| ((bx[0] + bx[2]) / 2.0, (bx[1] + bx[3]) / 2.0);
        let need = |what: &str, v: Option<[f64; 4]>| v.ok_or(format!("{fx}: `{how}` needs {what}"));
        // where the shape goes: (across, up and down, on the floor)
        let (sx, sy, ground) = match how.as_str() {
            "box" => {
                let (x, y) = mid(need("a hitbox", hit)?);
                (x - cx, y - cy, false)
            }
            "ground-box" => (mid(need("a hitbox", hit)?).0 - cx, floor - foot, true),
            "point" => (-cx, -cy, false),
            "body" => {
                let (x, y) = mid(need("a body", trunk)?);
                (x - cx, y - cy, was_ground)
            }
            "body-x" => (mid(need("a body", trunk)?).0 - cx, 0.0, was_ground),
            "front" => {
                let tr = need("a body", trunk)?;
                (tr[2] - l - 2.0, mid(tr).1 - cy, false)
            }
            "over-head" => {
                let tr = need("a body", trunk)?;
                (mid(tr).0 - cx, tr[1] - OVER - (b - t) / 2.0 - cy, false)
            }
            "round-head" => {
                let tr = need("a body", trunk)?;
                (mid(tr).0 - cx, tr[1] + HEAD / 2.0 - cy, false)
            }
            "from-head" => {
                let tr = need("a body", trunk)?;
                (mid(tr).0 - cx, tr[1] + HEAD - b, false)
            }
            "floor" => (0.0, floor - foot, true),
            "head-at-point" => (-r, 0.0, was_ground),
            "shift" => (by[0], by[1], false),
            "ground-shift" => (by[0], by[1], true),
            other => return Err(format!("{fx}: no placing `{other}`")),
        };
        let dir = dirs.get(fx).ok_or(format!("{fx}: no folder under {art}"))?;
        let path = dir.join("motion.json");
        let mut m: Value =
            serde_json::from_str(&read(&path.display().to_string())?).map_err(|e| e.to_string())?;
        let old = [
            m["anchor"][0].as_i64().unwrap_or(0),
            m["anchor"][1].as_i64().unwrap_or(0),
        ];
        // the anchor moves against the shape; going onto the floor or off it moves it by the point's height too
        let hop = match (was_ground, ground) {
            (false, true) => floor,
            (true, false) => -floor,
            _ => 0.0,
        };
        let new = [
            old[0] - sx.round() as i64,
            old[1] - (sy - hop).round() as i64,
        ];
        rows.push(json!({
            "fx": fx, "skill": skill, "placing": how, "ground": ground,
            "shift": [sx.round(), sy.round()], "anchor": { "old": old, "new": new },
        }));
        if write {
            m["anchor"] = json!(new);
            if ground {
                m["ground"] = json!(true);
            } else if let Some(o) = m.as_object_mut() {
                o.remove("ground");
            }
            std::fs::write(
                &path,
                serde_json::to_string_pretty(&m).map_err(|e| e.to_string())? + "\n",
            )
            .map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(
        Path::new(report),
        serde_json::to_string(&rows).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    for r in &rows {
        eprintln!(
            "{} {} shift {} anchor {}",
            r["fx"], r["placing"], r["shift"], r["anchor"]
        );
    }
    eprintln!(
        "{} placed{}",
        rows.len(),
        if write { " (written)" } else { "" }
    );
    Ok(())
}

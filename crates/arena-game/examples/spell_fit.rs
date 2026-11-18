//! Fit the art of the blows (melee and blasts: effects whose only hit shape is a blow's one-tick box) to the rules'
//! strike boxes, which are the truth (arena-c3, 2026-11-18): each such effect the audit finds off (`arena_game::audit`,
//! every skill cast once from each seat) is scaled so its largest frame matches the box (`scale`, in quarters, the
//! geometric mean of the box's size over the art's), moved so that frame's middle lands on the box's (`anchor`), and
//! led so that frame (its impact: the first at four fifths of its largest) lands on the tick the blow is tested
//! (`lead`, at most `LONGEST_LEAD`; one that would need more is listed too). A box the art would have to grow past `MOST` times
//! for, or whose shape is far from the art's (`ASPECT`), is listed for a decision instead.
//!
//! ```text
//! cargo run --release -p arena-game --example spell_fit -- art/vfx [--write]
//! cargo -Zscript art/vfx/tools/vfx.rs --all art/vfx crates/arena-game/assets/drawn   # then rebuild the sheets
//! ```

#![allow(clippy::disallowed_methods)]

use arena_duel::data::UNIT;
use arena_game::audit::{Sheets, audit_casts};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The most an effect is scaled by in one fit, and the least.
const MOST: f64 = 4.0;
const LEAST: f64 = 0.5;
/// How far the box's proportions may be from the art's before it is listed, either way.
const ASPECT: f64 = 3.0;
/// The longest lead given: more would cut away the art's build-up, and it would not read as the blow.
const LONGEST_LEAD: f64 = 0.35;

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let art = PathBuf::from(args.next().unwrap_or_else(|| "art/vfx".into()));
    let write = args.any(|a| a == "--write");
    // each effect's motion.json, by its fx
    let mut motions: BTreeMap<String, (PathBuf, Value)> = BTreeMap::new();
    for e in std::fs::read_dir(&art)
        .map_err(|e| format!("{}: {e}", art.display()))?
        .flatten()
    {
        let p = e.path().join("motion.json");
        if let Ok(text) = std::fs::read_to_string(&p) {
            let v: Value =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
            if let Some(fx) = v["fx"].as_str() {
                motions.insert(fx.to_string(), (p, v));
            }
        }
    }
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut sheets = Sheets::load(&assets)?;
    let ids: Vec<&'static str> = arena_duel::data::spells().iter().map(|s| s.name).collect();
    let reports = audit_casts(&mut sheets, &ids);
    let (mut fitted, mut listed) = (Vec::new(), Vec::new());
    for r in reports.iter().filter(|r| {
        r.drawn && !r.ok() && !r.blows.is_empty() && r.marks.iter().all(|m| m == "strike")
    }) {
        let Some((path, motion)) = motions.get(&r.fx) else {
            listed.push(format!("{}: no motion.json found", r.fx));
            continue;
        };
        let m = |f: fn(&arena_game::audit::Blow) -> f64| median(r.blows.iter().map(f).collect());
        let (hw, hh) = (m(|b| b.hit[2] - b.hit[0]), m(|b| b.hit[3] - b.hit[1]));
        let (aw, ah) = (m(|b| b.art[2] - b.art[0]), m(|b| b.art[3] - b.art[1]));
        let (hx, hy) = (
            m(|b| (b.hit[0] + b.hit[2]) / 2.0),
            m(|b| (b.hit[1] + b.hit[3]) / 2.0),
        );
        let (ax, ay) = (
            m(|b| (b.art[0] + b.art[2]) / 2.0),
            m(|b| (b.art[1] + b.art[3]) / 2.0),
        );
        let late = m(|b| b.peak_age - b.hit_age);
        if aw < 1.0 || ah < 1.0 {
            listed.push(format!("{}: its art has no size to scale", r.fx));
            continue;
        }
        let grow = ((hw / aw) * (hh / ah)).sqrt();
        let aspect = (hw / hh) / (aw / ah);
        let scale0 = motion["scale"].as_f64().unwrap_or(1.0);
        let px = motion["px"].as_f64().unwrap_or(1.0);
        if grow > MOST || aspect > ASPECT || aspect < 1.0 / ASPECT {
            listed.push(format!(
                "{} ({}): the box {hw:.0}×{hh:.0} against the art at its largest {aw:.0}×{ah:.0} (rules px): it would grow {grow:.1}×, the box {aspect:.1}× as wide for its height",
                r.fx,
                r.spells.join(", ")
            ));
            continue;
        }
        let scale = ((scale0 * grow * 4.0).round() / 4.0).clamp(LEAST, MOST.max(scale0));
        let f = scale / scale0;
        // after scaling round its anchor, the art's middle moves with it: shift the anchor so it lands on the box's
        let (dx, dy) = (hx - f * ax, hy - f * ay);
        let k = px * scale * UNIT;
        let anchor = motion["anchor"].as_array().map(|a| {
            (
                a.first().and_then(Value::as_f64).unwrap_or(0.0),
                a.get(1).and_then(Value::as_f64).unwrap_or(0.0),
            )
        });
        let Some((ax0, ay0)) = anchor else {
            listed.push(format!("{}: its motion.json has no anchor", r.fx));
            continue;
        };
        let anchor = ((ax0 - dx / k).round(), (ay0 + dy / k).round());
        let lead0 = motion["lead"].as_f64().unwrap_or(0.0);
        if lead0 + late > LONGEST_LEAD {
            listed.push(format!(
                "{} ({}): its impact comes {:.0} ms after the blow is tested; leading it that far would cut its build-up (fitted in size and place only)",
                r.fx,
                r.spells.join(", "),
                late * 1000.0
            ));
        }
        let late = late.min(LONGEST_LEAD - lead0).max(-lead0 - 0.5);
        let lead = ((lead0 + late) * 100.0).round() / 100.0;
        fitted.push(format!(
            "{}: scale {scale0} -> {scale}, anchor ({ax0}, {ay0}) -> ({}, {}), lead {lead0} -> {lead} (box {hw:.0}×{hh:.0}, art {aw:.0}×{ah:.0}, art peaked {:.0} ms after the blow)",
            r.fx,
            anchor.0,
            anchor.1,
            late * 1000.0
        ));
        if write {
            let mut v = motion.clone();
            v["scale"] = json!(scale);
            v["anchor"] = json!([anchor.0 as i64, anchor.1 as i64]);
            v["lead"] = json!(lead);
            let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())? + "\n";
            std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    println!("fitted {}:", fitted.len());
    for f in &fitted {
        println!("  {f}");
    }
    println!("listed for a decision {}:", listed.len());
    for l in &listed {
        println!("  {l}");
    }
    if !write {
        println!("(a dry run: --write rewrites the motion.json files)");
    }
    Ok(())
}

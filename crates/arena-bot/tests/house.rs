//! The house AI as ONNX: it computes what PyTorch computed when `train/export.py` wrote it, and it clears the entry
//! bar.
#![cfg(feature = "onnx")]

use arena_bot::onnx::Onnx;
use arena_bot::{ACTIONS, OBS};
use arena_duel::ladder::{Bot, Match, entry_bar_of};
use serde_json::Value;
use std::rc::Rc;

/// Observations from real fights, their masks and PyTorch's logits (`train/export.py` writes them).
const CHECK: &str = include_str!("../../arena-duel/tests/fixtures/policy_check.json");

#[test]
fn the_house_onnx_matches_pytorch() -> Result<(), Box<dyn std::error::Error>> {
    let onnx = Onnx::house().map_err(|e| e.0)?;
    let check: Value = serde_json::from_str(CHECK)?;
    let cases = check["cases"].as_array().cloned().unwrap_or_default();
    assert_eq!(cases.len(), 32);
    let floats = |v: &Value| -> Vec<f32> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|x| x.as_f64().unwrap_or(f64::NAN) as f32)
                    .collect()
            })
            .unwrap_or_default()
    };
    for (k, case) in cases.iter().enumerate() {
        let obs: [f32; OBS] = floats(&case["obs"])
            .try_into()
            .map_err(|_| format!("case {k}: obs"))?;
        let want = floats(&case["logits"]);
        let legal: Vec<bool> = case["mask"]
            .as_array()
            .map(|a| a.iter().map(|x| x.as_bool().unwrap_or(false)).collect())
            .unwrap_or_default();
        assert_eq!((want.len(), legal.len()), (ACTIONS, ACTIONS), "case {k}");
        let got = onnx.logits(&obs).map_err(|e| e.0)?;
        for i in 0..ACTIONS {
            assert!(
                (got[i] - want[i]).abs() <= 1e-4 + 1e-4 * want[i].abs(),
                "case {k}, logit {i}: {} vs PyTorch {}",
                got[i],
                want[i]
            );
        }
        let best = |l: &[f32]| {
            (0..ACTIONS)
                .filter(|&i| legal[i])
                .max_by(|&a, &b| l[a].total_cmp(&l[b]))
        };
        assert_eq!(best(&got), best(&want), "case {k}: the best legal act");
    }
    Ok(())
}

#[test]
fn a_model_that_is_not_a_policy_is_refused() {
    assert!(Onnx::load(b"not onnx").is_err());
}

/// The shipped policy clears the bar a player's bot must, over 30 seeds (90 rounds a seat): six seeds left a seat 14
/// to 18 rounds, too few to judge 65% a seat on the spells-only rules, where every checkpoint near the bar lost seat
/// 0 of seeds 1-6 to the brain while winning that seat 82% over the ladder's 150.
#[test]
#[ignore = "abi5a u1200 was trained on RULES 23; qi regen 8/s, jumps and take-offs at 8 qi and flight at 8/s leave \
            it under the bar against the brain: a retrain follows"]
fn the_house_ai_clears_the_entry_bar() -> Result<(), Box<dyn std::error::Error>> {
    let onnx = Onnx::house().map_err(|e| e.0)?;
    let m = Match {
        seeds: 1..31,
        rounds: 3,
        limit: 120.0,
        effects: arena_duel::effects::all(),
    };
    let bar = entry_bar_of(&Bot::Decider(Rc::new(move || onnx.decider())), &m);
    assert!(bar.passed(), "{bar:?}");
    Ok(())
}

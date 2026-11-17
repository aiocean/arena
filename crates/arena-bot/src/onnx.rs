//! An ONNX policy run natively with tract, as a `Decider`: the house AI (`weights/policy.onnx`) in the ladder and the
//! tests, and a trainer's opponents. It picks as `bots/onnx` does on the page and in the tournament: a softmax over
//! the legal acts read with the seat's draw. Natively the logits may differ from the module's in the last bits; the
//! tournament runs modules (`host`).
//!
//! Each `OnnxBot` is measured as a module is (`meter::Tally`, runtime "onnx"): its think time as metric events, its
//! faults, and its totals as `bot.end` when dropped; ONNX has no fuel, so those fields are left out (null).

use crate::meter::{Summary, Tally};
use crate::{ACTIONS, OBS};
use arena_duel::seat::{Decider, Fault, SharedDecider};
use arena_mind::policy::sample;
use sha2::Digest;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use tract_onnx::prelude::*;

/// The house AI the arena ships, as ONNX.
pub const HOUSE: &[u8] = include_bytes!("../../arena-duel/weights/policy.onnx");

/// A loaded, optimised ONNX policy: the observation, float32 `[1, 1741]`, to the acts' logits, `[1, 27]` or `[27]`.
/// Cheap to clone; many seats share one.
#[derive(Clone)]
pub struct Onnx {
    plan: Arc<TypedRunnableModel<TypedModel>>,
    /// The name its telemetry carries (`bot`): `house`, or `onnx-` and the first 12 of the model's SHA-256.
    name: Arc<str>,
}

impl Onnx {
    /// Read and optimise an ONNX model, refusing one whose input or output is not the policy's.
    pub fn load(bytes: &[u8]) -> Result<Onnx, Fault> {
        let plan = tract_onnx::onnx()
            .model_for_read(&mut &bytes[..])
            .and_then(|m| m.with_input_fact(0, f32::fact([1, OBS]).into()))
            .and_then(|m| m.into_optimized())
            .and_then(|m| m.into_runnable())
            .map_err(|e| Fault(format!("not a policy model: {e}")))?;
        let hash: String = sha2::Sha256::digest(bytes)
            .iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect();
        let onnx = Onnx {
            plan: Arc::new(plan),
            name: format!("onnx-{hash}").into(),
        };
        onnx.logits(&[0.0; OBS])?;
        Ok(onnx)
    }

    /// The house AI.
    pub fn house() -> Result<Onnx, Fault> {
        Onnx::load(HOUSE).map(|o| o.named("house"))
    }

    /// The same policy under `name` in its telemetry.
    pub fn named(mut self, name: &str) -> Onnx {
        self.name = name.into();
        self
    }

    /// The name its telemetry carries.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Every act's logit for `obs`.
    pub fn logits(&self, obs: &[f32; OBS]) -> Result<[f32; ACTIONS], Fault> {
        let fault = |e: TractError| Fault(e.to_string());
        let input = Tensor::from_shape(&[1, OBS], obs).map_err(fault)?;
        let out = self.plan.run(tvec!(input.into())).map_err(fault)?;
        let l = out
            .first()
            .ok_or_else(|| Fault("the model has no output".into()))?
            .as_slice::<f32>()
            .map_err(fault)?;
        <[f32; ACTIONS]>::try_from(l)
            .map_err(|_| Fault(format!("the model gives {} logits, not {ACTIONS}", l.len())))
    }

    /// A seat's decider for this policy.
    pub fn decider(&self) -> SharedDecider {
        Rc::new(RefCell::new(OnnxBot::new(self.clone())))
    }
}

/// An ONNX policy answering the duel: it keeps no memory, so one serves any number of fights. One is one instance in
/// the telemetry: its `bot.end` sums every fight it answered.
pub struct OnnxBot {
    onnx: Onnx,
    tally: Tally,
}

impl OnnxBot {
    pub fn new(onnx: Onnx) -> OnnxBot {
        let tally = Tally::new("onnx", Arc::clone(&onnx.name), None);
        OnnxBot { onnx, tally }
    }

    /// What it has done so far, as `bot.end` will report it.
    pub fn summary(&self) -> Summary {
        self.tally.summary()
    }
}

impl Decider for OnnxBot {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        self.tally.started();
        Ok(())
    }

    fn round(&mut self) -> Result<(), Fault> {
        Ok(())
    }

    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        let began = Instant::now();
        let r = self.onnx.logits(obs).map(|l| sample(&l, legal, u));
        self.tally.decided(began.elapsed(), None);
        match r {
            Ok(act) => Ok(i32::try_from(act).unwrap_or(0)),
            Err(f) => {
                self.tally.fault("decide", "error", &f);
                Err(f)
            }
        }
    }
}

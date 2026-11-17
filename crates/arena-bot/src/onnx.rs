//! An ONNX policy run natively with tract, as a `Decider`: the house AI (`weights/policy.onnx`) in the ladder and the
//! tests, and a trainer's opponents. It picks as `bots/onnx` does on the page and in the tournament: a softmax over
//! the legal acts read with the seat's draw. Natively the logits may differ from the module's in the last bits; the
//! tournament runs modules (`host`).

use crate::{ACTIONS, OBS};
use arena_mind::policy::sample;
use pixfx_duel::seat::{Decider, Fault, SharedDecider};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tract_onnx::prelude::*;

/// The house AI the arena ships, as ONNX.
pub const HOUSE: &[u8] = include_bytes!("../../pixfx-duel/weights/policy.onnx");

/// A loaded, optimised ONNX policy: the observation, float32 `[1, 354]`, to the acts' logits, `[1, 30]` or `[30]`.
/// Cheap to clone; many seats share one.
#[derive(Clone)]
pub struct Onnx(Arc<TypedRunnableModel<TypedModel>>);

impl Onnx {
    /// Read and optimise an ONNX model, refusing one whose input or output is not the policy's.
    pub fn load(bytes: &[u8]) -> Result<Onnx, Fault> {
        let plan = tract_onnx::onnx()
            .model_for_read(&mut &bytes[..])
            .and_then(|m| m.with_input_fact(0, f32::fact([1, OBS]).into()))
            .and_then(|m| m.into_optimized())
            .and_then(|m| m.into_runnable())
            .map_err(|e| Fault(format!("not a policy model: {e}")))?;
        let onnx = Onnx(Arc::new(plan));
        onnx.logits(&[0.0; OBS])?;
        Ok(onnx)
    }

    /// The house AI.
    pub fn house() -> Result<Onnx, Fault> {
        Onnx::load(HOUSE)
    }

    /// Every act's logit for `obs`.
    pub fn logits(&self, obs: &[f32; OBS]) -> Result<[f32; ACTIONS], Fault> {
        let fault = |e: TractError| Fault(e.to_string());
        let input = Tensor::from_shape(&[1, OBS], obs).map_err(fault)?;
        let out = self.0.run(tvec!(input.into())).map_err(fault)?;
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
        Rc::new(RefCell::new(OnnxBot(self.clone())))
    }
}

/// An ONNX policy answering the duel: it keeps no memory, so one serves any number of fights.
pub struct OnnxBot(pub Onnx);

impl Decider for OnnxBot {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        Ok(())
    }

    fn round(&mut self) -> Result<(), Fault> {
        Ok(())
    }

    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        let act = sample(&self.0.logits(obs)?, legal, u);
        Ok(i32::try_from(act).unwrap_or(0))
    }
}

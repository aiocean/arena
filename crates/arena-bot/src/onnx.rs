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
/// The house AI before it (`weights/policy-prev.onnx`): the live page's Vương Lâm, against the house AI (owner,
/// 2026-11-19). A new house AI moves the old `policy.onnx` here.
pub const PREVIOUS: &[u8] = include_bytes!("../../arena-duel/weights/policy-prev.onnx");

/// A loaded, optimised ONNX policy: the observation, float32 `[1, 2299]`, to the acts' logits, `[1, 29]` or `[29]`.
/// A recurrent one (`train/net.py`, `Policy(recurrent=...)`) also takes its state, float32 `[1, size]`, as a second
/// input and gives the next as a second output; its seat carries the state and clears it each round.
/// Cheap to clone; many seats share one.
#[derive(Clone)]
pub struct Onnx {
    plan: Arc<TypedRunnableModel<TypedModel>>,
    /// The size of a recurrent policy's state; 0 for one that keeps none.
    state: usize,
    /// The name its telemetry carries (`bot`): `house`, or `onnx-` and the first 12 of the model's SHA-256.
    name: Arc<str>,
}

impl Onnx {
    /// Read and optimise an ONNX model, refusing one whose input or output is not the policy's.
    pub fn load(bytes: &[u8]) -> Result<Onnx, Fault> {
        let model = tract_onnx::onnx()
            .model_for_read(&mut &bytes[..])
            .map_err(|e| Fault(format!("not a policy model: {e}")))?;
        // a second input is a recurrent policy's state, [1, size]
        let state = match model.inputs.len() {
            1 => 0,
            2 => model
                .input_fact(1)
                .ok()
                .and_then(|f| f.shape.as_concrete_finite().ok().flatten())
                .and_then(|d| d.get(1).copied())
                .ok_or_else(|| Fault("a recurrent policy's state must be [1, size]".into()))?,
            n => return Err(Fault(format!("a policy takes 1 or 2 inputs, not {n}"))),
        };
        let plan = Ok(model)
            .and_then(|m| m.with_input_fact(0, f32::fact([1, OBS]).into()))
            .and_then(|m| {
                if state > 0 {
                    m.with_input_fact(1, f32::fact([1, state]).into())
                } else {
                    Ok(m)
                }
            })
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
            state,
            name: format!("onnx-{hash}").into(),
        };
        onnx.logits(&[0.0; OBS])?;
        Ok(onnx)
    }

    /// The house AI.
    pub fn house() -> Result<Onnx, Fault> {
        Onnx::load(HOUSE).map(|o| o.named("house"))
    }

    /// The house AI before this one.
    pub fn previous() -> Result<Onnx, Fault> {
        Onnx::load(PREVIOUS).map(|o| o.named("previous"))
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

    /// The size of its state (0: it keeps none).
    pub fn state(&self) -> usize {
        self.state
    }

    /// Every act's logit for `obs`; a recurrent policy reads it from a fresh state.
    pub fn logits(&self, obs: &[f32; OBS]) -> Result<[f32; ACTIONS], Fault> {
        let mut h = vec![0.0; self.state];
        self.step(obs, &mut h)
    }

    /// Every act's logit for `obs`, from state `h` (empty for a policy that keeps none), which it moves on.
    pub fn step(&self, obs: &[f32; OBS], h: &mut [f32]) -> Result<[f32; ACTIONS], Fault> {
        let fault = |e: TractError| Fault(e.to_string());
        let input = Tensor::from_shape(&[1, OBS], obs).map_err(fault)?;
        let mut inputs = tvec!(input.into());
        if self.state > 0 {
            inputs.push(
                Tensor::from_shape(&[1, self.state], h)
                    .map_err(fault)?
                    .into(),
            );
        }
        let out = self.plan.run(inputs).map_err(fault)?;
        if self.state > 0 {
            let next = out
                .get(1)
                .ok_or_else(|| Fault("a recurrent policy gives its next state".into()))?
                .as_slice::<f32>()
                .map_err(fault)?;
            if next.len() != h.len() {
                return Err(Fault(format!(
                    "the next state has {} numbers, not {}",
                    next.len(),
                    h.len()
                )));
            }
            h.copy_from_slice(next);
        }
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

/// An ONNX policy answering the duel. One that keeps no memory serves any number of fights; a recurrent one carries
/// its state here, cleared at each fight and round as `bots/onnx` clears it. One is one instance in the telemetry: its
/// `bot.end` sums every fight it answered.
pub struct OnnxBot {
    onnx: Onnx,
    h: Vec<f32>,
    tally: Tally,
}

impl OnnxBot {
    pub fn new(onnx: Onnx) -> OnnxBot {
        let tally = Tally::new("onnx", Arc::clone(&onnx.name), None);
        let h = vec![0.0; onnx.state];
        OnnxBot { onnx, h, tally }
    }

    /// A recurrent policy's state as it stands (empty for one that keeps none).
    pub fn state(&self) -> &[f32] {
        &self.h
    }

    /// What it has done so far, as `bot.end` will report it.
    pub fn summary(&self) -> Summary {
        self.tally.summary()
    }
}

impl Decider for OnnxBot {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        self.tally.started();
        self.h.fill(0.0);
        Ok(())
    }

    fn round(&mut self) -> Result<(), Fault> {
        self.h.fill(0.0);
        Ok(())
    }

    /// One that keeps no memory: a forked seat shares it (its telemetry then counts the copies' decisions too).
    fn stateless(&self) -> bool {
        self.onnx.state == 0
    }

    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        let began = Instant::now();
        let r = self
            .onnx
            .step(obs, &mut self.h)
            .map(|l| sample(&l, legal, u));
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

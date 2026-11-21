//! A bot module of the `arena-bot-6` ABI (`docs/bot.md`, "The bot module") that runs an ONNX model with tract.
//!
//! The model takes the observation, float32 `[1, 2299]`, as its first input and gives one logit per act, float32
//! `[1, 29]` or `[29]`, as its first output. The bot picks the act as the shipped bot does: a softmax over the legal
//! acts, read with the arena's draw `u` (`arena_mind::policy::sample`). Change `arena_decide` to pick otherwise.
//!
//! A recurrent model (`train/net.py`, `Policy(recurrent=...)`) also takes its state, float32 `[1, size]`, as a second
//! input and gives the next state as a second output: the bot keeps it from decision to decision and clears it at each
//! fight and round, as the arena's native seat does (`arena_bot::onnx::OnnxBot`).
//!
//! The model is read and optimised once, in the first `arena_start` of a match (it costs about a quarter of that
//! call's fuel for a network of the shipped one's size).

use arena_mind::policy::{ACTIONS, OBS, sample};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, Ordering};
use tract_onnx::prelude::tract_data::internal::bail;
use tract_onnx::prelude::*;

static MODEL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.onnx"));

/// What the arena writes before each decision: the observation (2299 `f32` LE), then the legal mask (29 bytes).
static INPUT: [AtomicU8; OBS * 4 + ACTIONS] = [const { AtomicU8::new(0) }; OBS * 4 + ACTIONS];

type Plan = TypedRunnableModel<TypedModel>;

thread_local! {
    static PLAN: RefCell<Option<Plan>> = const { RefCell::new(None) };
    /// A recurrent model's state (empty for one that keeps none).
    static STATE: RefCell<Vec<f32>> = const { RefCell::new(Vec::new()) };
}

/// The optimised model and the size of its state (0: it keeps none).
fn plan() -> TractResult<(Plan, usize)> {
    let mut model = tract_onnx::onnx().model_for_read(&mut &MODEL[..])?;
    let state = if model.inputs.len() == 2 {
        let shape = model.input_fact(1)?.shape.as_concrete_finite()?;
        match shape.as_deref() {
            Some([1, n]) => *n,
            _ => bail!("a recurrent model's state must be [1, size]"),
        }
    } else {
        0
    };
    model = model.with_input_fact(0, f32::fact([1, OBS]).into())?;
    if state > 0 {
        model = model.with_input_fact(1, f32::fact([1, state]).into())?;
    }
    Ok((model.into_optimized()?.into_runnable()?, state))
}

fn forget() {
    STATE.with(|s| s.borrow_mut().fill(0.0));
}

fn byte(i: usize) -> u8 {
    INPUT[i].load(Ordering::Relaxed)
}

fn logits(obs: Vec<f32>) -> TractResult<[f32; ACTIONS]> {
    let mut inputs = tvec!(Tensor::from_shape(&[1, OBS], &obs)?.into());
    STATE.with(|s| -> TractResult<()> {
        let s = s.borrow();
        if !s.is_empty() {
            inputs.push(Tensor::from_shape(&[1, s.len()], &s)?.into());
        }
        Ok(())
    })?;
    let out = PLAN.with(|p| match p.borrow().as_ref() {
        Some(plan) => plan.run(inputs),
        None => bail!("arena_start was not called"),
    })?;
    STATE.with(|s| -> TractResult<()> {
        let mut s = s.borrow_mut();
        if !s.is_empty() {
            let next = out.get(1).map(|t| t.as_slice::<f32>()).transpose()?;
            match next {
                Some(n) if n.len() == s.len() => s.copy_from_slice(n),
                _ => bail!("a recurrent model gives its next state as its second output"),
            }
        }
        Ok(())
    })?;
    let l = out[0].as_slice::<f32>()?;
    let mut logits = [0.0; ACTIONS];
    if l.len() != ACTIONS {
        bail!("the model gives {} outputs, not {ACTIONS}", l.len());
    }
    logits.copy_from_slice(l);
    Ok(logits)
}

#[unsafe(no_mangle)]
pub extern "C" fn arena_abi() -> i32 {
    6
}

#[unsafe(no_mangle)]
pub extern "C" fn arena_input() -> i32 {
    INPUT.as_ptr() as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn arena_start(_seed: i32) {
    PLAN.with(|p| {
        if p.borrow().is_none() {
            // a model that does not load traps here: the bot loses the round
            let Ok((plan, state)) = plan() else {
                std::process::abort()
            };
            *p.borrow_mut() = Some(plan);
            STATE.with(|s| *s.borrow_mut() = vec![0.0; state]);
        }
    });
    forget();
}

#[unsafe(no_mangle)]
pub extern "C" fn arena_round() {
    forget();
}

#[unsafe(no_mangle)]
pub extern "C" fn arena_decide(u: f32) -> i32 {
    let obs = (0..OBS)
        .map(|i| {
            f32::from_le_bytes([
                byte(4 * i),
                byte(4 * i + 1),
                byte(4 * i + 2),
                byte(4 * i + 3),
            ])
        })
        .collect();
    let mut legal = [false; ACTIONS];
    for (i, l) in legal.iter_mut().enumerate() {
        *l = byte(OBS * 4 + i) != 0;
    }
    match logits(obs) {
        Ok(l) => sample(&l, &legal, u) as i32,
        Err(_) => std::process::abort(),
    }
}

fn no_random(_: &mut [u8]) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}
getrandom::register_custom_getrandom!(no_random);

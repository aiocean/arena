//! A bot module of the `arena-bot-1` ABI (`docs/bot.md`, "The bot module") that runs an ONNX model with tract.
//!
//! The model takes the observation, float32 `[1, 354]`, as its first input and gives one logit per act, float32
//! `[1, 30]` or `[30]`, as its first output. The bot picks the act as the shipped bot does: a softmax over the legal
//! acts, read with the arena's draw `u` (`pixfx_mind::policy::sample`). Change `arena_decide` to pick otherwise.
//!
//! The model is read and optimised once, in the first `arena_start` of a match (it costs about a quarter of that
//! call's fuel for a network of the shipped one's size).

use pixfx_mind::policy::{ACTIONS, OBS, sample};
use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, Ordering};
use tract_onnx::prelude::tract_data::internal::bail;
use tract_onnx::prelude::*;

static MODEL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/model.onnx"));

/// What the arena writes before each decision: the observation (354 `f32` LE), then the legal mask (30 bytes).
static INPUT: [AtomicU8; OBS * 4 + ACTIONS] = [const { AtomicU8::new(0) }; OBS * 4 + ACTIONS];

type Plan = TypedRunnableModel<TypedModel>;

thread_local! {
    static PLAN: RefCell<Option<Plan>> = const { RefCell::new(None) };
}

fn plan() -> TractResult<Plan> {
    tract_onnx::onnx()
        .model_for_read(&mut &MODEL[..])?
        .with_input_fact(0, f32::fact([1, OBS]).into())?
        .into_optimized()?
        .into_runnable()
}

fn byte(i: usize) -> u8 {
    INPUT[i].load(Ordering::Relaxed)
}

fn logits(obs: Vec<f32>) -> TractResult<[f32; ACTIONS]> {
    let input = Tensor::from_shape(&[1, OBS], &obs)?;
    let out = PLAN.with(|p| match p.borrow().as_ref() {
        Some(plan) => plan.run(tvec!(input.into())),
        None => bail!("arena_start was not called"),
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
    1
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
            let Ok(plan) = plan() else {
                std::process::abort()
            };
            *p.borrow_mut() = Some(plan);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn arena_round() {}

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

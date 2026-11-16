//! The `mlp` template (ABI `arena-bot-1`): a net's logits on the observation, one action sampled from them on the
//! arena's draw. Swap `WEIGHTS` for your own file (`train/export.py` writes the format) to field your own net.
//!
//! The host writes the observation (354 `f32`, then 30 legal flags as bytes) at `arena_input()` before each
//! `arena_decide(u)`, which answers an action in 0..30. `arena_logits()` points at the 30 `f32` of the last decision.
//! This crate denies `unsafe_code` as the arena's crates do, allowing it only on the exports (`#[unsafe(no_mangle)]`).

use pixfx_mind::policy::{ACTIONS, Net, OBS, sample};
use std::cell::RefCell;

/// The net this bot plays.
const WEIGHTS: &[u8] = include_bytes!("../../../crates/pixfx-duel/weights/policy.bin");

const FLAGS_AT: usize = OBS * 4;

struct Bot {
    net: Option<Net>,
    input: [u8; FLAGS_AT + ACTIONS],
    logits: [f32; ACTIONS],
}

thread_local! {
    static BOT: RefCell<Bot> = const {
        RefCell::new(Bot { net: None, input: [0; FLAGS_AT + ACTIONS], logits: [0.0; ACTIONS] })
    };
}

/// The ABI version this module speaks.
#[allow(unsafe_code)] // exporting a function needs `no_mangle`
#[unsafe(no_mangle)]
pub extern "C" fn arena_abi() -> i32 {
    1
}

/// Where the host writes the observation and the legal flags.
#[allow(unsafe_code)] // exporting a function needs `no_mangle`
#[unsafe(no_mangle)]
pub extern "C" fn arena_input() -> i32 {
    BOT.with_borrow(|b| b.input.as_ptr() as usize as i32)
}

/// A fight starts: loads the net. `seed` is the bot's own, for bots that want randomness.
#[allow(unsafe_code)] // exporting a function needs `no_mangle`
#[unsafe(no_mangle)]
pub extern "C" fn arena_start(_seed: i32) {
    BOT.with_borrow_mut(|b| {
        match Net::load(WEIGHTS) {
            Ok(net) => b.net = Some(net),
            // A bot that cannot load its net must not play on: the host sees the trap.
            Err(_) => core::arch::wasm32::unreachable(),
        }
    });
}

/// A round starts; the net keeps no memory between rounds.
#[allow(unsafe_code)] // exporting a function needs `no_mangle`
#[unsafe(no_mangle)]
pub extern "C" fn arena_round() {}

/// The action for the observation at `arena_input()`, drawn with `u` in [0, 1); 0 (wait) before `arena_start`.
#[allow(unsafe_code)] // exporting a function needs `no_mangle`
#[unsafe(no_mangle)]
pub extern "C" fn arena_decide(u: f32) -> i32 {
    BOT.with_borrow_mut(|b| {
        let Some(net) = &b.net else { return 0 };
        let (floats, flags) = b.input.split_at(FLAGS_AT);
        let mut obs = [0.0_f32; OBS];
        for (x, bytes) in obs.iter_mut().zip(floats.as_chunks::<4>().0) {
            *x = f32::from_le_bytes(*bytes);
        }
        let mut legal = [false; ACTIONS];
        for (l, &flag) in legal.iter_mut().zip(flags) {
            *l = flag != 0;
        }
        let logits = net.logits(&obs);
        let act = sample(&logits, &legal, u) as i32;
        b.logits = logits;
        act
    })
}

/// The 30 `f32` logits of the last decision, for a viewer of the bot's mind.
#[allow(unsafe_code)] // exporting a function needs `no_mangle`
#[unsafe(no_mangle)]
pub extern "C" fn arena_logits() -> i32 {
    BOT.with_borrow(|b| b.logits.as_ptr() as usize as i32)
}

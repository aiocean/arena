//! Bot modules for JS: `admitBot` admits a player's module as a submission is admitted (`arena_bot::admit`), the
//! budgets the page runs it on (`botBudgets`), and a `Corner`, what a corner seats: the page's instance of an
//! admitted module, which the duel asks through `BotInstance` as the native host does.

use crate::js::to_js;
use pixfx_duel::seat::{Decider, Fault, SharedDecider};
use pixfx_mind::policy::{ACTIONS, OBS};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// One instance of an admitted bot module, as `duel.js` runs it: each call given its fuel, a trap or an empty
    /// counter thrown.
    pub type BotInstance;

    #[wasm_bindgen(method, catch)]
    fn start(this: &BotInstance, seed: u32) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    fn round(this: &BotInstance) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    fn decide(this: &BotInstance, obs: &[f32], legal: &[u8], u: f32) -> Result<f64, JsValue>;
}

fn fault(e: JsValue) -> Fault {
    Fault(e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

impl Decider for BotInstance {
    fn start(&mut self, seed: u32) -> Result<(), Fault> {
        BotInstance::start(self, seed).map_err(fault)
    }

    fn round(&mut self) -> Result<(), Fault> {
        BotInstance::round(self).map_err(fault)
    }

    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        let legal = legal.map(u8::from);
        let act = BotInstance::decide(self, obs, &legal, u).map_err(fault)?;
        // what arena_decide returned: an i32, so always whole and in range; anything else is a wait
        Ok(if act.fract() == 0.0 && act.abs() <= f64::from(i32::MAX) {
            act as i32
        } else {
            -1
        })
    }
}

/// A module the arena accepted: the metered bytes the page instantiates, and their hash, the bot's identity.
#[wasm_bindgen]
pub struct AdmittedBot {
    inner: arena_bot::Admitted,
}

#[wasm_bindgen]
impl AdmittedBot {
    #[wasm_bindgen(getter)]
    pub fn bytes(&self) -> Vec<u8> {
        self.inner.bytes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn hash(&self) -> String {
        self.inner.hash.clone()
    }
}

/// The module in `bytes` (a `Uint8Array` of a `.wasm`) as the arena admits it; throws with the reason it is refused.
#[wasm_bindgen(js_name = admitBot)]
pub fn admit_bot(bytes: &[u8]) -> Result<AdmittedBot, JsError> {
    arena_bot::admit(bytes)
        .map(|inner| AdmittedBot { inner })
        .map_err(|e| JsError::new(&e.to_string()))
}

/// What a bot module runs on: `{abi, decideFuel, setupFuel, fuel, obs, actions}`, the ABI version `arena_abi`
/// returns, each call's fuel, the counter's export and the input's shape.
#[wasm_bindgen(js_name = botBudgets)]
pub fn bot_budgets() -> Result<JsValue, JsError> {
    to_js(&serde_json::json!({
        "abi": arena_bot::ABI,
        "decideFuel": arena_bot::DECIDE_FUEL,
        "setupFuel": arena_bot::SETUP_FUEL,
        "fuel": arena_bot::FUEL_EXPORT,
        "obs": OBS,
        "actions": ACTIONS,
    }))
}

/// What a corner seats: one instance of a bot module (one per corner: an instance remembers, so a mirror match takes
/// two).
#[wasm_bindgen]
pub struct Corner {
    pub(crate) bot: SharedDecider,
}

#[wasm_bindgen]
impl Corner {
    /// The corner plays the bot module `instance` runs; every duel it seats is a fight of one match.
    pub fn bot(instance: BotInstance) -> Corner {
        Corner {
            bot: Rc::new(RefCell::new(instance)),
        }
    }
}

// the ABI's buffer is the policy's observation and mask
const _: () = assert!(arena_bot::OBS == OBS && arena_bot::ACTIONS == ACTIONS);

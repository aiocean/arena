//! The game in the browser: the page starts it on its canvas (`run`), drops a player's bot module into a corner
//! (`setCorner`), starts the fight over on a seed (`restart`) and reads the status line (`status`). A bot module is
//! admitted as a submission is (`arena_bot::admit`: metered, its fuel in its own counter) and run by the browser's
//! WebAssembly, one instance a corner; the house AI is the module built from its ONNX model (`bots/policy.wasm`,
//! fetched once).

use crate::bots::{Corner, Seat, Seated, Seating};
use crate::rules::Shown;
use arena_bot::{ACTIONS, DECIDE_FUEL, FUEL_EXPORT, OBS, SETUP_FUEL};
use arena_duel::seat::{Decider, Fault, SharedDecider};
use bevy::prelude::*;
use js_sys::{BigInt, Function, Object, Reflect, Uint8Array, WebAssembly};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};

/// Where the house AI's module is, from the page.
const HOUSE: &str = "bots/policy.wasm";

/// What the page asked since the last frame.
enum Ask {
    Corner(usize, Corner),
    Restart(Option<u32>),
    House,
}

/// A finished compile: the seating version it was asked for, the corners or why they could not be made, their names
/// and the seed.
type Made = (u32, Result<[Seated; 2], String>, [String; 2], u32);

thread_local! {
    static ASKED: RefCell<Vec<Ask>> = const { RefCell::new(Vec::new()) };
    /// Corners made by a finished compile: the seating they were asked for, or why they could not be.
    static MADE: RefCell<Option<Made>> = const { RefCell::new(None) };
    static STATUS: RefCell<String> = const { RefCell::new(String::new()) };
    static HOUSE_BYTES: RefCell<Option<Arc<Vec<u8>>>> = const { RefCell::new(None) };
}

/// Start the game on the page's `#scene` canvas, on `seed` (a random one when not given).
#[wasm_bindgen]
pub fn run(seed: Option<u32>) {
    let start = crate::Start { seed, ..default() };
    crate::app(start).run();
}

/// Watch the server's matches live, on the page's `#scene` canvas, from the MoQ relay at `url` (a subscribe token in
/// its path). A bot dropped into a corner later turns watching off and fights on the page, as `run` does.
#[wasm_bindgen]
pub fn live(url: String) {
    let start = crate::Start {
        live: Some(url),
        ..default()
    };
    crate::app(start).run();
}

/// Put bot module `bytes` (named `name`) into corner `corner` (0 Thunder, 1 Frost); the fight starts over.
#[wasm_bindgen(js_name = setCorner)]
pub fn set_corner(corner: usize, name: String, bytes: Vec<u8>) {
    let c = Corner::Module {
        name,
        bytes: Arc::new(bytes),
    };
    ASKED.with_borrow_mut(|a| a.push(Ask::Corner(corner.min(1), c)));
}

/// Start the fight over on `seed` (a new random one when not given).
#[wasm_bindgen]
pub fn restart(seed: Option<u32>) {
    ASKED.with_borrow_mut(|a| a.push(Ask::Restart(seed)));
}

/// The house bot back in both corners.
#[wasm_bindgen]
pub fn house() {
    ASKED.with_borrow_mut(|a| a.push(Ask::House));
}

/// The status line: the seed, the round, who sits where and the score, or why a bot was refused.
#[wasm_bindgen]
pub fn status() -> String {
    STATUS.with_borrow(Clone::clone)
}

pub struct WebPlugin;

impl Plugin for WebPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Wanted>()
            .add_systems(Update, (asked, made, say).in_set(crate::Frame::Pace));
    }
}

/// The corners and seed the page wants now.
#[derive(Resource, Default)]
struct Wanted {
    corners: Option<[Corner; 2]>,
    seed: u32,
}

fn asked(
    mut wanted: ResMut<Wanted>,
    shown: Res<Shown>,
    mut live: NonSendMut<crate::live::Live>,
    mut seat: MessageWriter<Seat>,
) {
    let asks: Vec<Ask> = ASKED.with_borrow_mut(std::mem::take);
    if asks.is_empty() {
        return;
    }
    // a player asking for corners or a seed fights on the page: watching stops
    live.on = false;
    let mut corners = wanted
        .corners
        .clone()
        .unwrap_or([Corner::House, Corner::House]);
    let mut seed = if shown.seed > 0 {
        shown.seed
    } else {
        wanted.seed
    };
    for ask in asks {
        match ask {
            Ask::Corner(i, c) => corners[i] = c,
            Ask::Restart(s) => seed = s.unwrap_or_else(crate::bots::any_seed),
            Ask::House => corners = [Corner::House, Corner::House],
        }
    }
    wanted.corners = Some(corners.clone());
    wanted.seed = seed;
    seat.write(Seat { corners, seed });
}

/// Called by `bots::seat` in the browser: the corners are compiled and instantiated in the background; the fight
/// waits for them (the last fight plays on meanwhile).
pub fn seat(ask: Seat, seating: &mut Seating) {
    let version = seating.version + 1;
    let names = ask.corners.clone().map(|c| c.name());
    let seed = ask.seed;
    spawn_local(async move {
        let mut made = Vec::new();
        let mut failed = None;
        for c in &ask.corners {
            match corner(c).await {
                Ok(s) => made.push(s),
                Err(e) => {
                    failed = Some(e);
                    break;
                }
            }
        }
        let result = match (failed, made.try_into()) {
            (Some(e), _) => Err(e),
            (None, Ok(two)) => Ok(two),
            (None, Err(_)) => Err("a corner was not made".to_string()),
        };
        MADE.with_borrow_mut(|m| *m = Some((version, result, names, seed)));
    });
}

fn made(mut seating: NonSendMut<Seating>) {
    let Some((version, result, names, seed)) = MADE.with_borrow_mut(Option::take) else {
        return;
    };
    match result {
        Ok(seated) => {
            seating.seated = Some(seated);
            seating.names = names;
            seating.seed = seed;
            seating.version = version.max(seating.version + 1);
            seating.error = None;
            let url = format!("?seed={seed}");
            if let Some(h) = web_sys::window().and_then(|w| w.history().ok()) {
                let _ = h.replace_state_with_url(&JsValue::NULL, "", Some(&url));
            }
        }
        Err(e) => seating.error = Some(e),
    }
}

fn say(shown: Res<Shown>, seating: NonSend<Seating>, live: NonSend<crate::live::Live>) {
    if live.on {
        let text = match (&live.stale, live.watching.current(), &shown.view) {
            (Some(e), _, _) => e.clone(),
            (None, Some(m), Some(v)) => format!(
                "Seed {} · live · round {} · Thunder ({}) {} – {} Frost ({})",
                m.seed, v.round, m.names[0], v.wins[0], v.wins[1], m.names[1]
            ),
            _ => live.status.clone(),
        };
        STATUS.with_borrow_mut(|s| *s = text);
        return;
    }
    let text = match (&seating.error, &shown.view) {
        (Some(e), _) => format!("Not a bot for this season: {e}"),
        (None, Some(v)) => format!(
            "Seed {} · round {} · Thunder ({}) {} – {} Frost ({})",
            shown.seed, v.round, shown.names[0], v.wins[0], v.wins[1], shown.names[1]
        ),
        (None, None) => "Seating the bots".to_string(),
    };
    STATUS.with_borrow_mut(|s| *s = text);
}

/// What sits in corner `c`, made ready: a module admitted, compiled and instantiated.
async fn corner(c: &Corner) -> Result<Seated, String> {
    let (name, bytes) = match c {
        Corner::Brain => return Ok(Seated::Brain),
        Corner::First => return Ok(Seated::First),
        Corner::House => ("house bot".to_string(), house_bytes().await?),
        Corner::Module { name, bytes } => (name.clone(), Arc::clone(bytes)),
    };
    let admitted = arena_bot::admit(&bytes).map_err(|e| format!("{name}: {e}"))?;
    let js = |e: JsValue| {
        format!(
            "{name}: {}",
            e.as_string().unwrap_or_else(|| format!("{e:?}"))
        )
    };
    let module: WebAssembly::Module = JsFuture::from(WebAssembly::compile(&Uint8Array::from(
        admitted.bytes.as_slice(),
    )))
    .await
    .map_err(js)?
    .unchecked_into();
    let instance: WebAssembly::Instance =
        JsFuture::from(WebAssembly::instantiate_module(&module, &Object::new()))
            .await
            .map_err(js)?
            .unchecked_into();
    let bot = JsBot::new(instance).map_err(js)?;
    let shared: SharedDecider = Rc::new(RefCell::new(bot));
    Ok(Seated::Bot(shared))
}

async fn house_bytes() -> Result<Arc<Vec<u8>>, String> {
    if let Some(b) = HOUSE_BYTES.with_borrow(Clone::clone) {
        return Ok(b);
    }
    let window = web_sys::window().ok_or("no window")?;
    let js = |e: JsValue| {
        format!(
            "the house bot: {}",
            e.as_string().unwrap_or_else(|| format!("{e:?}"))
        )
    };
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(HOUSE))
        .await
        .map_err(js)?
        .unchecked_into();
    if !response.ok() {
        return Err(format!("the house bot: {} {}", response.status(), HOUSE));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(js)?)
        .await
        .map_err(js)?;
    let bytes = Arc::new(Uint8Array::new(&buffer).to_vec());
    HOUSE_BYTES.with_borrow_mut(|h| *h = Some(Arc::clone(&bytes)));
    Ok(bytes)
}

/// An instance of an admitted module as the duel asks it: each call given its fuel through the module's counter, the
/// input written where `arena_input` says, a trap or an empty counter a fault (the corner gives up that round).
struct JsBot {
    exports: Object,
    fuel: Object,
    input: u32,
    start: Function,
    round: Function,
    decide: Function,
}

impl JsBot {
    fn new(instance: WebAssembly::Instance) -> Result<JsBot, JsValue> {
        let exports = instance.exports();
        let get = |n: &str| Reflect::get(&exports, &JsValue::from_str(n));
        let func = |n: &str| -> Result<Function, JsValue> { get(n)?.dyn_into::<Function>() };
        let fuel: Object = get(FUEL_EXPORT)?.unchecked_into();
        let bot = JsBot {
            fuel,
            input: 0,
            start: func("arena_start")?,
            round: func("arena_round")?,
            decide: func("arena_decide")?,
            exports: exports.clone(),
        };
        bot.refuel(SETUP_FUEL)?;
        let abi = func("arena_abi")?.call0(&JsValue::NULL)?;
        if abi.as_f64() != Some(f64::from(arena_bot::ABI)) {
            return Err(JsValue::from_str(&format!(
                "the module is not arena-bot-{}",
                arena_bot::ABI
            )));
        }
        bot.refuel(SETUP_FUEL)?;
        let input = func("arena_input")?
            .call0(&JsValue::NULL)?
            .as_f64()
            .unwrap_or(0.0) as u32;
        Ok(JsBot { input, ..bot })
    }

    fn refuel(&self, fuel: i64) -> Result<(), JsValue> {
        Reflect::set(&self.fuel, &JsValue::from_str("value"), &BigInt::from(fuel)).map(|_| ())
    }

    fn memory(&self) -> Result<Uint8Array, JsValue> {
        let memory: WebAssembly::Memory =
            Reflect::get(&self.exports, &JsValue::from_str("memory"))?.unchecked_into();
        Ok(Uint8Array::new(&memory.buffer()))
    }
}

fn fault(e: JsValue) -> Fault {
    Fault(e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

impl Decider for JsBot {
    fn start(&mut self, seed: u32) -> Result<(), Fault> {
        self.refuel(SETUP_FUEL).map_err(fault)?;
        self.start
            .call1(&JsValue::NULL, &JsValue::from(seed as i32))
            .map(|_| ())
            .map_err(fault)
    }

    fn round(&mut self) -> Result<(), Fault> {
        self.refuel(SETUP_FUEL).map_err(fault)?;
        self.round.call0(&JsValue::NULL).map(|_| ()).map_err(fault)
    }

    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        // the module's memory may have grown since the last call: a fresh view each time
        let mem = self.memory().map_err(fault)?;
        let mut bytes = Vec::with_capacity(OBS * 4 + ACTIONS);
        for o in obs {
            bytes.extend_from_slice(&o.to_le_bytes());
        }
        bytes.extend(legal.iter().map(|&l| u8::from(l)));
        mem.set(&Uint8Array::from(bytes.as_slice()), self.input);
        self.refuel(DECIDE_FUEL).map_err(fault)?;
        let act = self
            .decide
            .call1(&JsValue::NULL, &JsValue::from(u))
            .map_err(fault)?;
        let act = act.as_f64().unwrap_or(-1.0);
        // what arena_decide returned: an i32, so always whole and in range; anything else is a wait
        Ok(if act.fract() == 0.0 && act.abs() <= f64::from(i32::MAX) {
            act as i32
        } else {
            -1
        })
    }
}

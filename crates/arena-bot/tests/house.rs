//! The house bot, a module of the `mlp` template around the shipped net: it plays the shipped policy's fight bit for bit.

use arena_bot::{WasmBot, check, engine};
use pixfx_duel::duel::{Duel, DuelConfig};
use pixfx_duel::out::Out;
use pixfx_duel::seat::{BotSeat, Brain, Decider, Fault, MakeSeat, PolicySeat, shipped};
use pixfx_mind::policy::{ACTIONS, OBS};
use std::rc::Rc;

type Test = Result<(), Box<dyn std::error::Error>>;

const HOUSE: &[u8] = include_bytes!("../../pixfx-duel/weights/house.wasm");

/// What a seat holds when its module will not instantiate: every call faults.
struct Broken(Fault);

impl Decider for Broken {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        Err(self.0.clone())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Err(self.0.clone())
    }
    fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        Err(self.0.clone())
    }
}

fn all_effects() -> Vec<String> {
    pixfx_effects::EFFECTS
        .iter()
        .map(|e| e.meta().id.to_string())
        .collect()
}

/// Every `Out` of a fight of `seconds` as one string, to compare fights whole.
fn play(seats: [MakeSeat; 2], seed: u32, seconds: f64) -> String {
    let config = DuelConfig {
        seed,
        left: 0.0,
        right: 220.0,
        ground: 150.0,
        sky: 96.0,
        effects: all_effects(),
    };
    let mut duel = Duel::new(config, seats);
    let mut outs: Vec<Out> = Vec::new();
    for _ in 0..pixfx_draw::math::js_round(seconds * 60.0) as usize {
        outs.extend(duel.step(1.0 / 60.0));
    }
    outs.iter().map(|o| format!("{o:?}\n")).collect()
}

#[test]
fn house_plays_the_shipped_policys_fight() -> Test {
    let engine = engine()?;
    let module = check(&engine, HOUSE)?;
    let net = shipped().map_err(|e| format!("{e:?}"))?;
    for seed in 1..=5 {
        let policy = play([PolicySeat::make(net.clone()), Brain::make()], seed, 60.0);
        let (engine, module) = (engine.clone(), module.clone());
        let new = Rc::new(move || -> Box<dyn Decider> {
            match WasmBot::new(&engine, &module) {
                Ok(bot) => Box::new(bot),
                Err(fault) => Box::new(Broken(fault)),
            }
        });
        let house = play([BotSeat::make(new, None), Brain::make()], seed, 60.0);
        assert_eq!(house, policy, "seed {seed}");
    }
    Ok(())
}

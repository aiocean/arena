//! Running an admitted module natively (wasmtime): a `Runner` compiles it once, each `Bot` is one instance (one
//! match), and a `Bot` answers the duel as a `Decider`. Each call is given its fuel through the module's own counter
//! (`FUEL_EXPORT`); a trap or an empty counter is a `Fault`, and the instance plays on with its memory as left.
//!
//! Each decision's wall time and fuel are metric events, each fault and a module that will not start an `arena::bot`
//! event, and an instance's totals one more when it is dropped (`arena-telemetry` exports them when it is configured;
//! with nothing listening they cost a clock read).

use crate::{ACTIONS, Admitted, DECIDE_FUEL, FUEL_EXPORT, OBS, SETUP_FUEL};
use arena_duel::seat::{Decider, Fault, SharedDecider};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use tracing::{Level, event};
use wasmtime::{Config, Engine, Global, Instance, Memory, Module, Store, TypedFunc, Val};

/// An admitted module, compiled.
#[derive(Clone)]
pub struct Runner {
    engine: Engine,
    module: Module,
}

impl Runner {
    pub fn new(admitted: &Admitted) -> Result<Runner, Fault> {
        let mut config = Config::new();
        // admission validated the accepted features; relaxed SIMD would differ by CPU, so it stays off here too
        config.wasm_relaxed_simd(false);
        let engine = Engine::new(&config).map_err(|e| Fault(format!("the engine: {e}")))?;
        let module = Module::new(&engine, &admitted.bytes)
            .map_err(|e| Fault(format!("compiling the module: {e}")))?;
        Ok(Runner { engine, module })
    }

    /// A fresh instance: one match.
    pub fn instance(&self) -> Result<Bot, Fault> {
        Bot::new(self)
    }

    /// Fresh instances for `arena_duel::ladder::Bot::Decider`; an instance that cannot be made faults on every call.
    pub fn decider(&self) -> SharedDecider {
        match self.instance() {
            Ok(bot) => Rc::new(RefCell::new(bot)),
            Err(fault) => {
                event!(target: "arena::bot", Level::ERROR, { fault = %fault.0 }, "bot.broken");
                Rc::new(RefCell::new(Broken(fault)))
            }
        }
    }
}

/// One instance of a bot module, for one match.
pub struct Bot {
    store: Store<()>,
    memory: Memory,
    fuel: Global,
    input: usize,
    abi: TypedFunc<(), i32>,
    start: TypedFunc<i32, ()>,
    round: TypedFunc<(), ()>,
    decide: TypedFunc<f32, i32>,
    /// The calls that trapped or ran out of fuel, and the most fuel one decision used.
    pub faults: Vec<Fault>,
    pub most_fuel: i64,
    /// Decisions made, and their wall time and fuel together.
    decisions: u64,
    thought: Duration,
    fuel_used: i64,
}

fn fault(e: impl std::fmt::Display) -> Fault {
    Fault(e.to_string())
}

impl Bot {
    fn new(runner: &Runner) -> Result<Bot, Fault> {
        let mut store = Store::new(&runner.engine, ());
        let instance = Instance::new(&mut store, &runner.module, &[]).map_err(fault)?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| Fault("no memory".into()))?;
        let fuel = instance
            .get_global(&mut store, FUEL_EXPORT)
            .ok_or_else(|| Fault("not a metered module".into()))?;
        let abi = instance
            .get_typed_func(&mut store, "arena_abi")
            .map_err(fault)?;
        let input_at: TypedFunc<(), i32> = instance
            .get_typed_func(&mut store, "arena_input")
            .map_err(fault)?;
        let start = instance
            .get_typed_func(&mut store, "arena_start")
            .map_err(fault)?;
        let round = instance
            .get_typed_func(&mut store, "arena_round")
            .map_err(fault)?;
        let decide = instance
            .get_typed_func(&mut store, "arena_decide")
            .map_err(fault)?;
        let mut bot = Bot {
            store,
            memory,
            fuel,
            input: 0,
            abi,
            start,
            round,
            decide,
            faults: Vec::new(),
            most_fuel: 0,
            decisions: 0,
            thought: Duration::ZERO,
            fuel_used: 0,
        };
        bot.refuel(SETUP_FUEL)?;
        let input = input_at.call(&mut bot.store, ()).map_err(fault)?;
        bot.input = u32::from_ne_bytes(input.to_ne_bytes()) as usize;
        Ok(bot)
    }

    fn refuel(&mut self, fuel: i64) -> Result<(), Fault> {
        self.fuel
            .set(&mut self.store, Val::I64(fuel))
            .map_err(fault)
    }

    fn left(&mut self) -> i64 {
        self.fuel.get(&mut self.store).i64().unwrap_or(0)
    }

    /// What `arena_abi` returns.
    pub fn abi(&mut self) -> Result<i32, Fault> {
        self.refuel(SETUP_FUEL)?;
        self.abi.call(&mut self.store, ()).map_err(fault)
    }

    fn noted<T>(&mut self, call: &'static str, r: Result<T, Fault>) -> Result<T, Fault> {
        if let Err(f) = &r {
            let kind = if self.left() <= 0 {
                "out_of_fuel"
            } else {
                "trap"
            };
            // a module that faults on every call is told about ten times, then every thousandth (all are counted)
            let n = self.faults.len() + 1;
            if n <= 10 || n.is_multiple_of(1000) {
                event!(target: "arena::bot", Level::WARN, {
                    call = call, kind = kind, fault = %f.0, faults = n,
                }, "bot.fault");
            }
            event!(target: "arena::metrics", Level::INFO, {
                monotonic_counter.arena_bot_faults_total = 1_u64, call = call, kind = kind,
            });
            self.faults.push(f.clone());
        }
        r
    }
}

impl Decider for Bot {
    fn start(&mut self, seed: u32) -> Result<(), Fault> {
        let r = self.refuel(SETUP_FUEL).and_then(|()| {
            let seed = i32::from_ne_bytes(seed.to_ne_bytes());
            self.start.call(&mut self.store, seed).map_err(fault)
        });
        self.noted("start", r)
    }

    fn round(&mut self) -> Result<(), Fault> {
        let r = self
            .refuel(SETUP_FUEL)
            .and_then(|()| self.round.call(&mut self.store, ()).map_err(fault));
        self.noted("round", r)
    }

    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        let mut buf = Vec::with_capacity(OBS * 4 + ACTIONS);
        for v in obs {
            buf.extend(v.to_le_bytes());
        }
        buf.extend(legal.iter().map(|&l| u8::from(l)));
        let began = Instant::now();
        let r = self
            .memory
            .write(&mut self.store, self.input, &buf)
            .map_err(|e| Fault(format!("writing the input at {}: {e}", self.input)))
            .and_then(|()| self.refuel(DECIDE_FUEL))
            .and_then(|()| self.decide.call(&mut self.store, u).map_err(fault));
        let took = began.elapsed();
        let used = DECIDE_FUEL - self.left();
        self.most_fuel = self.most_fuel.max(used);
        self.decisions += 1;
        self.thought += took;
        self.fuel_used += used;
        event!(target: "arena::metrics", Level::INFO, {
            histogram.arena_bot_think_seconds = took.as_secs_f64(), histogram.arena_bot_fuel = used as f64,
        });
        self.noted("decide", r)
    }
}

impl Drop for Bot {
    fn drop(&mut self) {
        if self.decisions > 0 || !self.faults.is_empty() {
            let n = self.decisions.max(1) as f64;
            event!(target: "arena::bot", Level::INFO, {
                decisions = self.decisions, faults = self.faults.len(),
                think_mean_ms = self.thought.as_secs_f64() * 1000.0 / n,
                fuel_mean = self.fuel_used as f64 / n, fuel_most = self.most_fuel, fuel_cap = DECIDE_FUEL,
            }, "bot.end");
        }
    }
}

/// A bot whose instance could not be made: every call faults.
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

// the ABI's buffer is the policy's observation and mask
const _: () = assert!(OBS == arena_mind::policy::OBS && ACTIONS == arena_mind::policy::ACTIONS);

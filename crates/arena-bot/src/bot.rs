use pixfx_duel::seat::{Decider, Fault};
use pixfx_mind::policy::{ACTIONS, OBS};
use wasmtime::{Engine, Instance, Memory, Module, ResourceLimiter, Store, Trap, TypedFunc};

use crate::LIMITS;

/// Sums every memory and every table the instance grows to against `LIMITS.memory` and `LIMITS.table_elements`: a grow
/// past either returns -1 to the bot, and a module declaring more than that fails to instantiate.
struct Budget {
    used: usize,
    table_used: usize,
}

impl ResourceLimiter for Budget {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _max: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let total = self.used.saturating_sub(current).saturating_add(desired);
        if total > LIMITS.memory {
            return Ok(false);
        }
        self.used = total;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        _max: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let total = self
            .table_used
            .saturating_sub(current)
            .saturating_add(desired);
        if total > LIMITS.table_elements {
            return Ok(false);
        }
        self.table_used = total;
        Ok(true)
    }
}

/// One instance of a checked module, for one fight: its own store and memories, fuel set before every call.
pub struct WasmBot {
    store: Store<Budget>,
    memory: Memory,
    abi: TypedFunc<(), i32>,
    input: TypedFunc<(), i32>,
    start: TypedFunc<i32, ()>,
    round: TypedFunc<(), ()>,
    decide: TypedFunc<f32, i32>,
}

fn fault(e: wasmtime::Error) -> Fault {
    match e.downcast_ref::<Trap>() {
        Some(Trap::OutOfFuel) => Fault::OutOfFuel,
        _ => Fault::Trap(format!("{e}")),
    }
}

impl WasmBot {
    /// A fresh instance; a trap in the module's own start function is a fault.
    pub fn new(engine: &Engine, module: &Module) -> Result<WasmBot, Fault> {
        let mut store = Store::new(
            engine,
            Budget {
                used: 0,
                table_used: 0,
            },
        );
        store.limiter(|budget| budget);
        store.set_fuel(LIMITS.fuel_setup).map_err(fault)?;
        let instance = Instance::new(&mut store, module, &[]).map_err(fault)?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| Fault::Trap("no memory export".into()))?;
        Ok(WasmBot {
            memory,
            abi: instance
                .get_typed_func(&mut store, "arena_abi")
                .map_err(fault)?,
            input: instance
                .get_typed_func(&mut store, "arena_input")
                .map_err(fault)?,
            start: instance
                .get_typed_func(&mut store, "arena_start")
                .map_err(fault)?,
            round: instance
                .get_typed_func(&mut store, "arena_round")
                .map_err(fault)?,
            decide: instance
                .get_typed_func(&mut store, "arena_decide")
                .map_err(fault)?,
            store,
        })
    }

    /// What `arena_abi` answers.
    pub(crate) fn abi(&mut self) -> Result<i32, Fault> {
        self.store.set_fuel(LIMITS.fuel_setup).map_err(fault)?;
        self.abi.call(&mut self.store, ()).map_err(fault)
    }
}

impl Decider for WasmBot {
    fn start(&mut self, seed: u32) -> Result<(), Fault> {
        self.store.set_fuel(LIMITS.fuel_setup).map_err(fault)?;
        // the seed's bits, as the i32 the ABI takes
        self.start.call(&mut self.store, seed as i32).map_err(fault)
    }

    fn round(&mut self) -> Result<(), Fault> {
        self.store.set_fuel(LIMITS.fuel_setup).map_err(fault)?;
        self.round.call(&mut self.store, ()).map_err(fault)
    }

    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        self.store.set_fuel(LIMITS.fuel_decide).map_err(fault)?;
        let at = self.input.call(&mut self.store, ()).map_err(fault)? as u32 as usize;
        let mut bytes = [0u8; OBS * 4 + ACTIONS];
        let (floats, flags) = bytes.split_at_mut(OBS * 4);
        for (slot, x) in floats.as_chunks_mut::<4>().0.iter_mut().zip(obs) {
            *slot = x.to_le_bytes();
        }
        for (slot, &l) in flags.iter_mut().zip(legal) {
            *slot = u8::from(l);
        }
        self.memory
            .write(&mut self.store, at, &bytes)
            .map_err(|e| Fault::Trap(format!("arena_input: {e}")))?;
        self.decide.call(&mut self.store, u).map_err(fault)
    }
}

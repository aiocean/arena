use std::fmt;

use sha2::{Digest, Sha256};
use wasmtime::{Engine, ExternType, FuncType, Module, ValType};

use crate::{LIMITS, WasmBot};

/// The ABI version a module must answer from `arena_abi`.
const ABI: i32 = 1;

/// Why a module is not a bot.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    TooBig {
        bytes: usize,
    },
    /// Not valid WebAssembly in the season's accepted set, or it does not instantiate.
    Invalid(String),
    /// `module::name` of the first import: a bot has none.
    Import(String),
    Export {
        name: String,
        why: String,
    },
    Abi(i32),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::TooBig { bytes } => write!(
                f,
                "module is {bytes} bytes, over the {} limit",
                LIMITS.module
            ),
            Refusal::Invalid(why) => write!(f, "not an accepted module: {why}"),
            Refusal::Import(name) => write!(f, "imports {name}, a bot has no imports"),
            Refusal::Export { name, why } => write!(f, "export {name}: {why}"),
            Refusal::Abi(v) => write!(f, "arena_abi returned {v}, this arena speaks {ABI}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// The module's sha256, lowercase hex.
pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The module, if it is a bot: in budget, valid in the season's set, no imports, every export of the ABI, and an
/// `arena_abi` that answers 1.
pub fn check(engine: &Engine, bytes: &[u8]) -> Result<Module, Refusal> {
    if bytes.len() > LIMITS.module {
        return Err(Refusal::TooBig { bytes: bytes.len() });
    }
    let module = Module::new(engine, bytes).map_err(|e| Refusal::Invalid(format!("{e:#}")))?;
    if let Some(import) = module.imports().next() {
        return Err(Refusal::Import(format!(
            "{}::{}",
            import.module(),
            import.name()
        )));
    }
    export_memory(&module, "memory")?;
    export_func(engine, &module, "arena_abi", &[], &[ValType::I32])?;
    export_func(engine, &module, "arena_input", &[], &[ValType::I32])?;
    export_func(engine, &module, "arena_start", &[ValType::I32], &[])?;
    export_func(engine, &module, "arena_round", &[], &[])?;
    export_func(
        engine,
        &module,
        "arena_decide",
        &[ValType::F32],
        &[ValType::I32],
    )?;
    let mut bot = WasmBot::new(engine, &module).map_err(|f| Refusal::Invalid(f.to_string()))?;
    match bot.abi().map_err(|f| Refusal::Invalid(f.to_string()))? {
        ABI => Ok(module),
        other => Err(Refusal::Abi(other)),
    }
}

fn refuse(name: &str, why: &str) -> Refusal {
    Refusal::Export {
        name: name.to_string(),
        why: why.to_string(),
    }
}

fn export_memory(module: &Module, name: &str) -> Result<(), Refusal> {
    match module.get_export(name) {
        Some(ExternType::Memory(_)) => Ok(()),
        Some(_) => Err(refuse(name, "not a memory")),
        None => Err(refuse(name, "missing")),
    }
}

fn export_func(
    engine: &Engine,
    module: &Module,
    name: &str,
    params: &[ValType],
    results: &[ValType],
) -> Result<(), Refusal> {
    let want = FuncType::new(engine, params.iter().cloned(), results.iter().cloned());
    match module.get_export(name) {
        Some(ExternType::Func(got)) if FuncType::eq(&got, &want) => Ok(()),
        Some(ExternType::Func(_)) => Err(refuse(name, "wrong signature")),
        Some(_) => Err(refuse(name, "not a function")),
        None => Err(refuse(name, "missing")),
    }
}

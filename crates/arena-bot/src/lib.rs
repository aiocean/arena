//! A bot as a WebAssembly module, the `arena-bot-1` ABI (`docs/bot.md`, "The bot module";
//! `docs/specs/2026-11-16-arena-wasm-bots-design.md`). `admit` takes a player's module and returns the one the arena
//! runs from then on: validated against the season's features and budgets, its memory capped, and metered by a fuel
//! counter written into its own code, so the same decision runs out of fuel in wasmtime and in a browser. Its hash is
//! the bot's identity. With the `host` feature, `host` runs an admitted module in a duel seat.

use sha2::{Digest, Sha256};
use wasm_encoder::reencode::{Error as ReencodeError, Reencode, RoundtripReencoder};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, ExportKind, ExportSection, GlobalSection, GlobalType,
    Instruction, MemoryType, SectionId, ValType,
};
use wasmparser::{
    CompositeInnerType, ExternalKind, FuncType, FunctionBody, Operator, Parser, Payload, Validator,
    WasmFeatures,
};

#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "onnx")]
pub mod onnx;

/// The ABI version `arena_abi` returns.
pub const ABI: i32 = 1;
/// The largest module a player may submit.
pub const MAX_BYTES: usize = 32 << 20;
/// The most linear memory a bot may grow to, in 64 KiB pages (256 MiB).
pub const MAX_PAGES: u64 = 4096;
/// Fuel for one `arena_decide`.
pub const DECIDE_FUEL: i64 = 20_000_000;
/// Fuel for one `arena_start` or `arena_round`, and for the module's own start function.
pub const SETUP_FUEL: i64 = 200_000_000;
/// The global the arena adds and exports: what is left of the current call's fuel.
pub const FUEL_EXPORT: &str = "arena_fuel";
/// The observation's length (`pixfx_mind::policy::OBS`) and the number of acts (`ACTIONS`).
pub const OBS: usize = 354;
pub const ACTIONS: usize = 30;

/// The features a bot may use: the ones that give the same bits on wasmtime and in Chrome, Firefox and Safari
/// without the deterministic profile (the spec's "Determinism"). Relaxed SIMD, GC, memory64, multiple memories,
/// threads and every phase < 4 proposal are refused.
pub fn features() -> WasmFeatures {
    WasmFeatures::MUTABLE_GLOBAL
        | WasmFeatures::SATURATING_FLOAT_TO_INT
        | WasmFeatures::SIGN_EXTENSION
        | WasmFeatures::REFERENCE_TYPES
        | WasmFeatures::MULTI_VALUE
        | WasmFeatures::BULK_MEMORY
        | WasmFeatures::SIMD
        | WasmFeatures::TAIL_CALL
        | WasmFeatures::FLOATS
        | WasmFeatures::EXCEPTIONS
        | WasmFeatures::EXTENDED_CONST
        | WasmFeatures::FUNCTION_REFERENCES
        | WasmFeatures::GC_TYPES
}

/// A module the arena accepted: the metered bytes it runs, and their SHA-256 (hex), the bot's identity.
#[derive(Clone, Debug, PartialEq)]
pub struct Admitted {
    pub bytes: Vec<u8>,
    pub hash: String,
}

/// Why a module was refused.
#[derive(Clone, Debug, PartialEq)]
pub enum Refusal {
    TooLarge(usize),
    Invalid(String),
    Import(String),
    MissingExport(&'static str),
    WrongExport(&'static str, String),
    Reserved(&'static str),
    Memory(String),
    Rewrite(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::TooLarge(n) => write!(f, "the module is {n} bytes; at most {MAX_BYTES}"),
            Refusal::Invalid(e) => write!(f, "not a valid module with the accepted features: {e}"),
            Refusal::Import(name) => write!(f, "it imports {name}; a bot imports nothing"),
            Refusal::MissingExport(name) => write!(f, "it does not export {name}"),
            Refusal::WrongExport(name, want) => write!(f, "its export {name} is not {want}"),
            Refusal::Reserved(name) => write!(f, "it exports {name}, which the arena adds"),
            Refusal::Memory(e) => write!(f, "its memory: {e}"),
            Refusal::Rewrite(e) => write!(f, "metering it failed: {e}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// The functions a bot exports, with their signatures (params, results).
const EXPORTS: [(&str, &[wasmparser::ValType], &[wasmparser::ValType]); 5] = {
    use wasmparser::ValType::{F32, I32};
    [
        ("arena_abi", &[], &[I32]),
        ("arena_input", &[], &[I32]),
        ("arena_start", &[I32], &[]),
        ("arena_round", &[], &[]),
        ("arena_decide", &[F32], &[I32]),
    ]
};

/// Check `bytes` against the ABI, the features and the budgets, and return the metered module.
pub fn admit(bytes: &[u8]) -> Result<Admitted, Refusal> {
    if bytes.len() > MAX_BYTES {
        return Err(Refusal::TooLarge(bytes.len()));
    }
    Validator::new_with_features(features())
        .validate_all(bytes)
        .map_err(|e| Refusal::Invalid(e.to_string()))?;
    let shape = Shape::read(bytes)?;
    shape.check()?;
    let mut meter = Meter {
        fuel: shape.globals,
        globals_done: false,
    };
    let mut module = wasm_encoder::Module::new();
    meter
        .parse_core_module(&mut module, Parser::new(0), bytes)
        .map_err(|e| Refusal::Rewrite(format!("{e:?}")))?;
    let metered = module.finish();
    // what the arena runs must itself be a valid module of the accepted features
    Validator::new_with_features(features())
        .validate_all(&metered)
        .map_err(|e| Refusal::Rewrite(e.to_string()))?;
    let hash = Sha256::digest(&metered)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Ok(Admitted {
        bytes: metered,
        hash,
    })
}

/// What `admit` reads of a module before it rewrites it.
#[derive(Default)]
struct Shape {
    types: Vec<Option<FuncType>>,
    funcs: Vec<u32>,
    memories: Vec<wasmparser::MemoryType>,
    globals: u32,
    exports: Vec<(String, ExternalKind, u32)>,
}

impl Shape {
    fn read(bytes: &[u8]) -> Result<Shape, Refusal> {
        let invalid = |e: wasmparser::BinaryReaderError| Refusal::Invalid(e.to_string());
        let mut s = Shape::default();
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.map_err(invalid)? {
                Payload::ImportSection(r) => {
                    if let Some(i) = r.into_imports().next() {
                        let i = i.map_err(invalid)?;
                        return Err(Refusal::Import(format!("{}::{}", i.module, i.name)));
                    }
                }
                Payload::TypeSection(r) => {
                    for group in r {
                        for t in group.map_err(invalid)?.into_types() {
                            s.types.push(match t.composite_type.inner {
                                CompositeInnerType::Func(f) => Some(f),
                                _ => None,
                            });
                        }
                    }
                }
                Payload::FunctionSection(r) => {
                    for f in r {
                        s.funcs.push(f.map_err(invalid)?);
                    }
                }
                Payload::MemorySection(r) => {
                    for m in r {
                        s.memories.push(m.map_err(invalid)?);
                    }
                }
                Payload::GlobalSection(r) => s.globals = r.count(),
                Payload::ExportSection(r) => {
                    for e in r {
                        let e = e.map_err(invalid)?;
                        s.exports.push((e.name.to_string(), e.kind, e.index));
                    }
                }
                _ => {}
            }
        }
        Ok(s)
    }

    fn check(&self) -> Result<(), Refusal> {
        let export = |name: &str| self.exports.iter().find(|(n, ..)| n == name);
        if export(FUEL_EXPORT).is_some() {
            return Err(Refusal::Reserved(FUEL_EXPORT));
        }
        match export("memory") {
            Some((_, ExternalKind::Memory, 0)) => {}
            Some(_) => return Err(Refusal::WrongExport("memory", "the module's memory".into())),
            None => return Err(Refusal::MissingExport("memory")),
        }
        let Some(memory) = self.memories.first() else {
            return Err(Refusal::MissingExport("memory"));
        };
        if memory.initial > MAX_PAGES {
            return Err(Refusal::Memory(format!(
                "it starts at {} pages; at most {MAX_PAGES} (256 MiB)",
                memory.initial
            )));
        }
        for (name, params, results) in EXPORTS {
            let want = format!("a function {params:?} -> {results:?}");
            let Some((_, kind, index)) = export(name) else {
                return Err(Refusal::MissingExport(name));
            };
            let ty = (*kind == ExternalKind::Func)
                .then(|| self.funcs.get(*index as usize))
                .flatten()
                .and_then(|&t| self.types.get(t as usize))
                .and_then(|t| t.as_ref());
            if !ty.is_some_and(|t| t.params() == params && t.results() == results) {
                return Err(Refusal::WrongExport(name, want));
            }
        }
        Ok(())
    }
}

/// The rewriter: adds the fuel global (exported), caps the memory, and charges each straight run of code its length
/// at its start, so every loop iteration and every call pays and a bot that never ends traps.
struct Meter {
    /// The fuel global's index: after the module's own globals (a bot imports none).
    fuel: u32,
    globals_done: bool,
}

impl Meter {
    fn fuel_global(&self, globals: &mut GlobalSection) {
        globals.global(
            GlobalType {
                val_type: ValType::I64,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i64_const(SETUP_FUEL),
        );
    }

    /// `fuel -= n; if fuel < 0 { unreachable }`
    fn charge(&self, f: &mut wasm_encoder::Function, n: usize) {
        let n = i64::try_from(n).unwrap_or(i64::MAX);
        f.instruction(&Instruction::GlobalGet(self.fuel))
            .instruction(&Instruction::I64Const(n))
            .instruction(&Instruction::I64Sub)
            .instruction(&Instruction::GlobalSet(self.fuel))
            .instruction(&Instruction::GlobalGet(self.fuel))
            .instruction(&Instruction::I64Const(0))
            .instruction(&Instruction::I64LtS)
            .instruction(&Instruction::If(BlockType::Empty))
            .instruction(&Instruction::Unreachable)
            .instruction(&Instruction::End);
    }
}

/// Whether control may leave or enter the code at this instruction: a straight run ends after it.
fn ends_run(op: &Operator<'_>) -> bool {
    matches!(
        op,
        Operator::Block { .. }
            | Operator::Loop { .. }
            | Operator::If { .. }
            | Operator::Else
            | Operator::End
            | Operator::Br { .. }
            | Operator::BrIf { .. }
            | Operator::BrTable { .. }
            | Operator::BrOnNull { .. }
            | Operator::BrOnNonNull { .. }
            | Operator::Return
            | Operator::Call { .. }
            | Operator::CallIndirect { .. }
            | Operator::CallRef { .. }
            | Operator::ReturnCall { .. }
            | Operator::ReturnCallIndirect { .. }
            | Operator::ReturnCallRef { .. }
            | Operator::TryTable { .. }
            | Operator::Throw { .. }
            | Operator::ThrowRef
            | Operator::Unreachable
    )
}

impl Reencode for Meter {
    type Error = std::convert::Infallible;

    fn memory_type(
        &mut self,
        m: wasmparser::MemoryType,
    ) -> Result<MemoryType, ReencodeError<Self::Error>> {
        let mut m = RoundtripReencoder.memory_type(m)?;
        m.maximum = Some(m.maximum.map_or(MAX_PAGES, |max| max.min(MAX_PAGES)));
        Ok(m)
    }

    fn parse_global_section(
        &mut self,
        globals: &mut GlobalSection,
        section: wasmparser::GlobalSectionReader<'_>,
    ) -> Result<(), ReencodeError<Self::Error>> {
        wasm_encoder::reencode::utils::parse_global_section(self, globals, section)?;
        self.fuel_global(globals);
        self.globals_done = true;
        Ok(())
    }

    fn intersperse_section_hook(
        &mut self,
        module: &mut wasm_encoder::Module,
        _after: Option<SectionId>,
        before: Option<SectionId>,
    ) -> Result<(), ReencodeError<Self::Error>> {
        // a module without globals gets a section holding only the fuel, where the global section goes
        let past_globals = matches!(
            before,
            None | Some(
                SectionId::Export
                    | SectionId::Start
                    | SectionId::Element
                    | SectionId::DataCount
                    | SectionId::Code
                    | SectionId::Data
            )
        );
        if !self.globals_done && past_globals {
            let mut globals = GlobalSection::new();
            self.fuel_global(&mut globals);
            module.section(&globals);
            self.globals_done = true;
        }
        Ok(())
    }

    fn parse_export_section(
        &mut self,
        exports: &mut ExportSection,
        section: wasmparser::ExportSectionReader<'_>,
    ) -> Result<(), ReencodeError<Self::Error>> {
        wasm_encoder::reencode::utils::parse_export_section(self, exports, section)?;
        exports.export(FUEL_EXPORT, ExportKind::Global, self.fuel);
        Ok(())
    }

    fn parse_function_body(
        &mut self,
        code: &mut CodeSection,
        func: FunctionBody<'_>,
    ) -> Result<(), ReencodeError<Self::Error>> {
        let mut f = self.new_function_with_parsed_locals(&func)?;
        let mut ops = Vec::new();
        let mut reader = func.get_operators_reader()?;
        while !reader.eof() {
            ops.push(reader.read()?);
        }
        let mut at = 0;
        while at < ops.len() {
            let len = ops[at..]
                .iter()
                .position(ends_run)
                .map_or(ops.len() - at, |i| i + 1);
            self.charge(&mut f, len);
            for op in &ops[at..at + len] {
                f.instruction(&self.instruction(op.clone())?);
            }
            at += len;
        }
        code.function(&f);
        Ok(())
    }
}

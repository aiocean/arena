//! Admission: what a module must be to enter, and what the arena makes of it.

use arena_bot::{FUEL_EXPORT, MAX_PAGES, Refusal, admit};
use wasmparser::{Parser, Payload};

/// A bot that waits, with `extra` spliced into the module.
fn bot(extra: &str) -> Vec<u8> {
    wat(&format!(
        r#"(module
  (memory (export "memory") 1)
  (func (export "arena_abi") (result i32) i32.const 6)
  (func (export "arena_input") (result i32) i32.const 1024)
  (func (export "arena_start") (param i32))
  (func (export "arena_round"))
  (func (export "arena_decide") (param f32) (result i32) i32.const 0)
  {extra})"#
    ))
}

fn wat(text: &str) -> Vec<u8> {
    wat::parse_str(text).unwrap_or_default()
}

#[test]
fn a_minimal_bot_is_admitted_metered_and_hashed() {
    let a = admit(&bot(""));
    assert!(a.is_ok(), "{a:?}");
    let Ok(a) = a else { return };
    assert_eq!(a.hash.len(), 64);
    assert_eq!(
        admit(&bot("")).map(|b| b.hash),
        Ok(a.hash.clone()),
        "the same bytes, the same identity"
    );
    let exports: Vec<String> = Parser::new(0)
        .parse_all(&a.bytes)
        .filter_map(|p| match p {
            Ok(Payload::ExportSection(r)) => Some(
                r.into_iter()
                    .filter_map(|e| e.ok())
                    .map(|e| e.name.to_string())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    assert!(exports.iter().any(|e| e == FUEL_EXPORT), "{exports:?}");
}

#[test]
fn a_bot_with_globals_of_its_own_keeps_them() {
    assert!(
        admit(&bot(
            r#"(global $g (mut i32) (i32.const 7)) (func (export "g") (result i32) global.get $g)"#
        ))
        .is_ok()
    );
}

#[test]
fn memory_is_capped_and_a_larger_start_is_refused() {
    let Ok(a) = admit(&bot("")) else { return };
    let max = Parser::new(0).parse_all(&a.bytes).find_map(|p| match p {
        Ok(Payload::MemorySection(r)) => r
            .into_iter()
            .next()
            .and_then(|m| m.ok())
            .and_then(|m| m.maximum),
        _ => None,
    });
    assert_eq!(max, Some(MAX_PAGES));
    let big = wat(&format!(
        r#"(module (memory (export "memory") {})
  (func (export "arena_abi") (result i32) i32.const 6)
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32)) (func (export "arena_round"))
  (func (export "arena_decide") (param f32) (result i32) i32.const 0))"#,
        MAX_PAGES + 1
    ));
    assert!(matches!(admit(&big), Err(Refusal::Memory(_))));
}

#[test]
fn imports_are_refused() {
    let text = r#"(module (import "env" "now" (func)) (memory (export "memory") 1)
  (func (export "arena_abi") (result i32) i32.const 6)
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32)) (func (export "arena_round"))
  (func (export "arena_decide") (param f32) (result i32) i32.const 0))"#;
    assert_eq!(admit(&wat(text)), Err(Refusal::Import("env::now".into())));
}

#[test]
fn a_missing_or_mistyped_export_is_named() {
    let missing = wat(r#"(module (memory (export "memory") 1)
  (func (export "arena_abi") (result i32) i32.const 6)
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32)) (func (export "arena_round")))"#);
    assert_eq!(admit(&missing), Err(Refusal::MissingExport("arena_decide")));
    let mistyped = wat(r#"(module (memory (export "memory") 1)
  (func (export "arena_abi") (result i32) i32.const 6)
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32)) (func (export "arena_round"))
  (func (export "arena_decide") (param f64) (result i32) i32.const 0))"#);
    assert!(matches!(
        admit(&mistyped),
        Err(Refusal::WrongExport("arena_decide", _))
    ));
}

#[test]
fn the_fuel_export_is_reserved() {
    assert_eq!(
        admit(&bot(r#"(global (export "arena_fuel") i64 (i64.const 0))"#)),
        Err(Refusal::Reserved(FUEL_EXPORT))
    );
}

#[test]
fn refused_features_are_refused() {
    // relaxed SIMD gives different bits on different CPUs
    let relaxed = bot(
        r#"(func (export "r") (result v128) (f32x4.relaxed_madd (v128.const i32x4 0 0 0 0) (v128.const i32x4 0 0 0 0) (v128.const i32x4 0 0 0 0)))"#,
    );
    assert!(matches!(admit(&relaxed), Err(Refusal::Invalid(_))));
    let shared = wat(r#"(module (memory (export "memory") 1 1 shared)
  (func (export "arena_abi") (result i32) i32.const 6)
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32)) (func (export "arena_round"))
  (func (export "arena_decide") (param f32) (result i32) i32.const 0))"#);
    assert!(matches!(admit(&shared), Err(Refusal::Invalid(_))));
    // plain SIMD is in
    assert!(admit(&bot(r#"(func (export "s") (result v128) (f32x4.add (v128.const i32x4 0 0 0 0) (v128.const i32x4 0 0 0 0)))"#)).is_ok());
}

#[test]
fn garbage_and_oversize_are_refused() {
    assert!(matches!(admit(b"not wasm"), Err(Refusal::Invalid(_))));
    assert!(matches!(
        admit(&vec![0; arena_bot::MAX_BYTES + 1]),
        Err(Refusal::TooLarge(_))
    ));
}

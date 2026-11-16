//! Modules written in WAT: what the check refuses and accepts, and how a checked bot runs.

use arena_bot::{LIMITS, Refusal, WasmBot, check, engine};
use pixfx_duel::seat::{Decider, Fault};
use pixfx_mind::policy::{ACTIONS, OBS};

type Test = Result<(), Box<dyn std::error::Error>>;

/// A module with the ABI's exports; `abi` is what `arena_abi` answers, `decide` the body of `arena_decide` (the
/// input lives at 0, 354 f32 then 30 bytes).
fn bot(abi: i32, decide: &str) -> String {
    format!(
        r#"(module
  (memory (export "memory") 1)
  (global $n (mut i32) (i32.const 0))
  (func (export "arena_abi") (result i32) i32.const {abi})
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32))
  (func (export "arena_round"))
  (func (export "arena_decide") (param f32) (result i32) {decide}))"#
    )
}

fn refusal(text: &str) -> Result<Refusal, Box<dyn std::error::Error>> {
    match check(&engine()?, &wat::parse_str(text)?) {
        Ok(_) => Err("the module was accepted".into()),
        Err(r) => Ok(r),
    }
}

fn run(text: &str) -> Result<WasmBot, Box<dyn std::error::Error>> {
    let engine = engine()?;
    let module = check(&engine, &wat::parse_str(text)?)?;
    ok(WasmBot::new(&engine, &module))
}

fn ok<T>(r: Result<T, Fault>) -> Result<T, Box<dyn std::error::Error>> {
    r.map_err(|f| f.to_string().into())
}

fn decide(bot: &mut WasmBot) -> Result<i32, Fault> {
    bot.decide(&[0.0; OBS], &[true; ACTIONS], 0.0)
}

#[test]
fn refuses_an_import() -> Test {
    let text = bot(1, "i32.const 0").replace("(memory", "(import \"env\" \"f\" (func))\n  (memory");
    assert_eq!(refusal(&text)?, Refusal::Import("env::f".into()));
    Ok(())
}

#[test]
fn refuses_gc() -> Test {
    let text = bot(1, "i32.const 0").replace("(memory", "(type (struct (field i32)))\n  (memory");
    assert!(matches!(refusal(&text)?, Refusal::Invalid(_)));
    Ok(())
}

#[test]
fn refuses_shared_memory() -> Test {
    let text = bot(1, "i32.const 0").replace("1)\n  (global", "1 1 shared)\n  (global");
    assert!(matches!(refusal(&text)?, Refusal::Invalid(_)));
    Ok(())
}

#[test]
fn refuses_a_missing_export() -> Test {
    let text = bot(1, "i32.const 0").replace("\"arena_round\"", "\"arena_rond\"");
    let r = refusal(&text)?;
    assert!(
        matches!(&r, Refusal::Export { name, .. } if name == "arena_round"),
        "{r}"
    );
    assert!(r.to_string().contains("arena_round"));
    Ok(())
}

#[test]
fn refuses_a_wrong_signature() -> Test {
    let text = bot(1, "i32.const 0").replace("(param f32)", "(param i32)");
    let r = refusal(&text)?;
    assert!(
        matches!(&r, Refusal::Export { name, why } if name == "arena_decide" && why == "wrong signature"),
        "{r}"
    );
    Ok(())
}

#[test]
fn refuses_abi_2() -> Test {
    assert_eq!(refusal(&bot(2, "i32.const 0"))?, Refusal::Abi(2));
    Ok(())
}

#[test]
fn refuses_33_mib() -> Test {
    let r = check(&engine()?, &vec![0u8; 33 << 20]).err();
    assert_eq!(r, Some(Refusal::TooBig { bytes: 33 << 20 }));
    Ok(())
}

#[test]
fn accepts_relaxed_simd_memory64_and_two_memories() -> Test {
    let text = r#"(module
  (memory (export "memory") i64 1)
  (memory 1)
  (func (export "arena_abi") (result i32) i32.const 1)
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32))
  (func (export "arena_round"))
  (func (export "arena_decide") (param f32) (result i32)
    v128.const f32x4 1 2 3 4
    v128.const f32x4 5 6 7 8
    v128.const f32x4 9 10 11 12
    f32x4.relaxed_madd
    drop
    i32.const 7))"#;
    assert_eq!(ok(decide(&mut run(text)?))?, 7);
    Ok(())
}

#[test]
fn a_loop_runs_out_of_fuel() -> Test {
    let mut b = run(&bot(1, "(loop $l br $l) i32.const 0"))?;
    assert_eq!(decide(&mut b), Err(Fault::OutOfFuel));
    Ok(())
}

#[test]
fn unreachable_is_a_trap() -> Test {
    let mut b = run(&bot(1, "unreachable"))?;
    assert!(matches!(decide(&mut b), Err(Fault::Trap(_))));
    Ok(())
}

#[test]
fn grow_past_256_mib_across_two_memories_returns_minus_one() -> Test {
    assert_eq!(LIMITS.memory, 256 << 20);
    // 3000 pages is 187 MiB: one memory fits, the second one's grow would put the sum past 256 MiB
    let text = bot(
        1,
        "i32.const 3000 memory.grow 0 drop i32.const 3000 memory.grow 1",
    )
    .replace("(memory (export", "(memory 1)\n  (memory (export");
    // memory 0 is the extra one, 1 is exported as "memory"
    let mut b = run(&text)?;
    assert_eq!(ok(decide(&mut b))?, -1);
    // and the bot keeps running
    assert_eq!(ok(decide(&mut b))?, -1);
    Ok(())
}

#[test]
fn two_instances_keep_their_own_memory() -> Test {
    let engine = engine()?;
    let module = check(
        &engine,
        &wat::parse_str(bot(
            1,
            "global.get $n i32.const 1 i32.add global.set $n global.get $n",
        ))?,
    )?;
    let mut a = ok(WasmBot::new(&engine, &module))?;
    let mut b = ok(WasmBot::new(&engine, &module))?;
    for count in 1..=3 {
        assert_eq!(ok(decide(&mut a))?, count);
        assert_eq!(ok(decide(&mut b))?, count);
    }
    Ok(())
}

#[test]
fn a_trap_in_start_is_a_fault() -> Test {
    // the module's own start section, then the ABI's arena_start
    let engine = engine()?;
    let text = bot(1, "i32.const 0").replace(
        "(memory",
        "(start $boom)\n  (func $boom unreachable)\n  (memory",
    );
    let module = engine_module(&engine, &text)?;
    assert!(matches!(
        WasmBot::new(&engine, &module).err(),
        Some(Fault::Trap(_))
    ));

    let text = bot(1, "i32.const 0").replace(
        "(func (export \"arena_start\") (param i32))",
        "(func (export \"arena_start\") (param i32) unreachable)",
    );
    let mut b = run(&text)?;
    assert!(matches!(b.start(7), Err(Fault::Trap(_))));
    Ok(())
}

/// A module compiled without the check (the check would instantiate it and refuse).
fn engine_module(
    engine: &wasmtime::Engine,
    text: &str,
) -> Result<wasmtime::Module, Box<dyn std::error::Error>> {
    Ok(wasmtime::Module::new(engine, wat::parse_str(text)?)?)
}

#[test]
fn the_input_reaches_the_bot() -> Test {
    let mut obs = [0.0f32; OBS];
    obs[3] = 0.75;
    let mut legal = [false; ACTIONS];
    legal[5] = true;
    legal[9] = true;

    // trunc(obs[3] * 10)
    let mut b = run(&bot(
        1,
        "i32.const 12 f32.load f32.const 10 f32.mul i32.trunc_f32_s",
    ))?;
    assert_eq!(ok(b.decide(&obs, &legal, 0.0))?, 7);

    // the first legal index
    let first = "(local $i i32)
      (loop $next
        (if (i32.load8_u (i32.add (i32.const 1416) (local.get $i)))
          (then (return (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        br $next)
      i32.const -1";
    let mut b = run(&bot(1, first))?;
    assert_eq!(ok(b.decide(&obs, &legal, 0.0))?, 5);

    // the dice
    let mut b = run(&bot(1, "local.get 0 f32.const 100 f32.mul i32.trunc_f32_s"))?;
    assert_eq!(ok(b.decide(&obs, &legal, 0.5))?, 50);
    Ok(())
}

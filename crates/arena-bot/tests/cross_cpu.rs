//! One module, the same bits on every CPU: a bot that runs every relaxed SIMD instruction and every NaN case, hashes
//! each result, and answers hash % 30. The golden pins its acts, the fuel left after each call and the final hash;
//! CI runs it on x86_64 and arm64 against the one file. A change meant to alter it rewrites it: delete the file and
//! run the test once.

use arena_bot::{WasmBot, check, engine};
use pixfx_duel::seat::Decider;
use pixfx_mind::policy::{ACTIONS, OBS};
use std::fmt::Write as _;

type Test = Result<(), Box<dyn std::error::Error>>;

const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens/cross_cpu.json");

const DECISIONS: usize = 2000;

/// u = 1 and u = 2 make arena_decide answer the hash's low and high 32 bits without folding anything.
const BOT: &str = r#"(module
  (memory (export "memory") 1)
  (global $h (mut i64) (i64.const 0xcbf29ce484222325))
  (global $n (mut i32) (i32.const 0))
  (func $fold (param i64)
    global.get $h i64.const 13 i64.rotl local.get 0 i64.xor i64.const 0x100000001b3 i64.mul global.set $h)
  (func $foldv (param v128)
    local.get 0 i64x2.extract_lane 0 call $fold
    local.get 0 i64x2.extract_lane 1 call $fold)
  (func (export "arena_abi") (result i32) i32.const 1)
  (func (export "arena_input") (result i32) i32.const 0)
  (func (export "arena_start") (param i32))
  (func (export "arena_round"))
  (func (export "arena_decide") (param f32) (result i32)
    (local $x f32) (local $a v128) (local $bits i32)
    local.get 0 f32.const 1 f32.eq if global.get $h i32.wrap_i64 return end
    local.get 0 f32.const 2 f32.eq if global.get $h i64.const 32 i64.shr_u i32.wrap_i64 return end
    global.get $n i32.const 1 i32.add global.set $n
    global.get $n f32.convert_i32_s f32.const 0.0001234 f32.mul f32.const 1.0002441 f32.add local.set $x
    local.get $x f32x4.splat local.set $a
    local.get $x i32.reinterpret_f32 local.set $bits

    ;; fused or not: products that need more than 24 bits
    local.get $a local.get $a v128.const f32x4 -1.0004883 -1.0004883 -1.0004883 -1.0004883 f32x4.relaxed_madd call $foldv
    local.get $a local.get $a v128.const f32x4 1.0004883 1.0004883 1.0004883 1.0004883 f32x4.relaxed_nmadd call $foldv
    local.get $a f64x2.promote_low_f32x4 local.get $a f64x2.promote_low_f32x4 v128.const f64x2 -1.0004883 -1.0004883 f64x2.relaxed_madd call $foldv

    ;; min and max on the two zeros and on NaN
    v128.const f32x4 0 -0 nan 1 v128.const f32x4 -0 0 1 nan f32x4.relaxed_min call $foldv
    v128.const f32x4 0 -0 nan 1 v128.const f32x4 -0 0 1 nan f32x4.relaxed_max call $foldv
    v128.const f64x2 0 nan v128.const f64x2 -0 1 f64x2.relaxed_min call $foldv
    v128.const f64x2 nan -0 v128.const f64x2 1 0 f64x2.relaxed_max call $foldv

    ;; truncation past the i32 range, of NaN and of negatives
    v128.const f32x4 3e9 -3e9 nan 1e10 local.get $a f32x4.mul i32x4.relaxed_trunc_f32x4_s call $foldv
    v128.const f32x4 3e9 -3e9 nan -1 local.get $a f32x4.mul i32x4.relaxed_trunc_f32x4_u call $foldv
    v128.const f64x2 4e9 nan i32x4.relaxed_trunc_f64x2_s_zero call $foldv
    v128.const f64x2 -1 5e9 i32x4.relaxed_trunc_f64x2_u_zero call $foldv

    ;; lane select with masks that are neither all ones nor all zeros in a lane
    local.get $bits i8x16.splat local.get $bits i32.const -1 i32.xor i8x16.splat
    v128.const i8x16 0x80 0x7f 0x01 0xfe 0 0xff 0x55 0xaa 0x80 0x7f 0x01 0xfe 0 0xff 0x55 0xaa
    i8x16.relaxed_laneselect call $foldv
    local.get $bits i16x8.splat v128.const i16x8 1 2 3 4 5 6 7 8
    v128.const i16x8 0x8000 0x7fff 0xff00 0x00ff 0x8001 0 0xffff 0x1234
    i16x8.relaxed_laneselect call $foldv
    local.get $bits i32x4.splat v128.const i32x4 1 2 3 4
    v128.const i32x4 0x80000000 0x7fffffff 0xff00ff00 0x00ff00ff
    i32x4.relaxed_laneselect call $foldv
    local.get $bits i64.extend_i32_u i64x2.splat v128.const i64x2 1 2
    v128.const i64x2 0x8000000000000000 0x7fffffffffffffff
    i64x2.relaxed_laneselect call $foldv

    ;; dot products with the second operand's top bits set, which i7 leaves undefined
    v128.const i8x16 -128 -128 127 127 -1 -1 1 1 -128 127 -128 127 0 -1 5 -5
    v128.const i8x16 -128 -1 127 -128 -1 127 -128 127 -2 -3 -4 -5 100 90 -90 -100
    i16x8.relaxed_dot_i8x16_i7x16_s call $foldv
    v128.const i8x16 -128 -128 127 127 -128 -128 127 127 -128 -128 127 127 -128 -128 127 127
    v128.const i8x16 -128 -1 127 -128 -128 -1 127 -128 -128 -1 127 -128 -128 -1 127 -128
    local.get $bits i32x4.splat
    i32x4.relaxed_dot_i8x16_i7x16_add_s call $foldv
    v128.const i16x8 -32768 -32768 32767 -32768 1 2 3 4
    v128.const i16x8 -32768 32767 -32768 -32768 5 6 7 8
    i16x8.relaxed_q15mulr_s call $foldv

    ;; NaNs made by 0/0, inf-inf and sqrt(-1); one stored and reloaded as bits
    i32.const 4096 f32.const 0 f32.const 0 f32.div f32.store
    i32.const 4096 i32.load i64.extend_i32_u call $fold
    f64.const 0 f64.const 0 f64.div i64.reinterpret_f64 call $fold
    f32.const inf f32.const inf f32.sub i32.reinterpret_f32 i64.extend_i32_u call $fold
    f32.const -1 f32.sqrt i32.reinterpret_f32 i64.extend_i32_u call $fold

    global.get $h i64.const 30 i64.rem_u i32.wrap_i64))"#;

fn list(xs: impl Iterator<Item = String>) -> String {
    xs.collect::<Vec<_>>().join(",")
}

/// The golden's text: acts, fuel left after each decision, and the hash from the two probes.
fn run() -> Result<String, Box<dyn std::error::Error>> {
    let engine = engine()?;
    let module = check(&engine, &wat::parse_str(BOT)?)?;
    let mut bot = WasmBot::new(&engine, &module).map_err(|f| f.to_string())?;
    let obs = [0.0_f32; OBS];
    let legal = [true; ACTIONS];
    let mut acts = Vec::with_capacity(DECISIONS);
    let mut fuel = Vec::with_capacity(DECISIONS);
    for _ in 0..DECISIONS {
        acts.push(bot.decide(&obs, &legal, 0.0).map_err(|f| f.to_string())?);
        fuel.push(bot.fuel_left());
    }
    let low = bot.decide(&obs, &legal, 1.0).map_err(|f| f.to_string())? as u32;
    let high = bot.decide(&obs, &legal, 2.0).map_err(|f| f.to_string())? as u32;
    let mut out = String::new();
    writeln!(
        out,
        "{{\n  \"acts\": [{}],",
        list(acts.iter().map(i32::to_string))
    )?;
    writeln!(
        out,
        "  \"fuel_left\": [{}],",
        list(fuel.iter().map(u64::to_string))
    )?;
    writeln!(out, "  \"hash\": \"{high:08x}{low:08x}\"\n}}")?;
    Ok(out)
}

#[test]
fn one_module_gives_the_same_bits_and_fuel_on_every_cpu() -> Test {
    let got = run()?;
    match std::fs::read_to_string(FILE) {
        Ok(want) => assert_eq!(
            got, want,
            "this CPU differs from the golden: if that was meant, delete {FILE} and run again"
        ),
        Err(_) => {
            std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/goldens"))?;
            std::fs::write(FILE, &got)?;
            eprintln!("wrote {FILE}: the first run pins the results");
        }
    }
    Ok(())
}

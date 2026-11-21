//! Running a module: fuel, faults, memory that lasts a match.
#![cfg(feature = "host")]

use arena_bot::host::Runner;
use arena_bot::{ACTIONS, OBS, admit};
use arena_duel::seat::Decider;

/// A bot whose `arena_decide` is `body` (an `i32` from the param `$u`), with a counter at address 0 it may use.
fn runner(body: &str) -> Runner {
    let text = format!(
        r#"(module
  (memory (export "memory") 1)
  (func (export "arena_abi") (result i32) i32.const 6)
  (func (export "arena_input") (result i32) i32.const 1024)
  (func (export "arena_start") (param i32))
  (func (export "arena_round"))
  (func (export "arena_decide") (param $u f32) (result i32) {body}))"#
    );
    let bytes = wat::parse_str(&text).unwrap_or_default();
    let admitted = admit(&bytes);
    assert!(admitted.is_ok(), "{admitted:?}");
    let runner = admitted.ok().and_then(|a| Runner::new(&a).ok());
    assert!(runner.is_some());
    runner.unwrap_or_else(|| unreachable!())
}

const OBS0: [f32; OBS] = [0.0; OBS];
const LEGAL: [bool; ACTIONS] = [true; ACTIONS];

#[test]
fn memory_lasts_across_calls_and_the_input_is_written() {
    // count calls at address 0; answer the count plus the mask byte of act 2 (1) read from the input
    let r = runner(
        "(i32.store (i32.const 0) (i32.add (i32.load (i32.const 0)) (i32.const 1)))
         (i32.add (i32.load (i32.const 0)) (i32.load8_u (i32.const 10222)))",
    );
    let Ok(mut bot) = r.instance() else { return };
    assert_eq!(bot.abi(), Ok(6));
    assert_eq!(bot.decide(&OBS0, &LEGAL, 0.5), Ok(2));
    assert_eq!(bot.start(9), Ok(()));
    assert_eq!(
        bot.decide(&OBS0, &LEGAL, 0.5),
        Ok(3),
        "the memory outlives a new fight"
    );
    let Ok(mut fresh) = r.instance() else { return };
    assert_eq!(
        fresh.decide(&OBS0, &LEGAL, 0.5),
        Ok(2),
        "a new instance starts clean"
    );
}

#[test]
fn a_bot_that_never_ends_runs_out_of_fuel_every_time() {
    let r = runner("(loop $l (br $l)) i32.const 0");
    let Ok(mut bot) = r.instance() else { return };
    for _ in 0..3 {
        assert!(bot.decide(&OBS0, &LEGAL, 0.5).is_err());
    }
    assert_eq!(bot.faults.len(), 3);
}

#[test]
fn fuel_runs_out_on_the_same_decision_every_time() {
    // each decision loops as many times as decisions so far, times 100,000: out of fuel on the same one
    let r = runner(
        "(local $n i32)
         (i32.store (i32.const 0) (i32.add (i32.load (i32.const 0)) (i32.const 1)))
         (local.set $n (i32.mul (i32.load (i32.const 0)) (i32.const 100000)))
         (loop $l (local.set $n (i32.sub (local.get $n) (i32.const 1))) (br_if $l (local.get $n)))
         i32.const 0",
    );
    let first_out = || {
        let mut bot = r.instance().ok()?;
        (1..100).find(|_| bot.decide(&OBS0, &LEGAL, 0.5).is_err())
    };
    let a = first_out();
    assert!(a.is_some());
    assert_eq!(a, first_out());
}

#[test]
fn a_trap_leaves_the_instance_playable() {
    // trap when u < 0.5, else answer 4
    let r = runner("(if (f32.lt (local.get $u) (f32.const 0.5)) (then unreachable)) i32.const 4");
    let Ok(mut bot) = r.instance() else { return };
    assert!(bot.decide(&OBS0, &LEGAL, 0.1).is_err());
    assert_eq!(bot.decide(&OBS0, &LEGAL, 0.9), Ok(4));
}

#[test]
fn a_module_of_another_abi_does_not_start() {
    // an ABI 5 module (2055 inputs) is refused, not fed ABI 6's 2299
    let text = r#"(module
  (memory (export "memory") 1)
  (func (export "arena_abi") (result i32) i32.const 5)
  (func (export "arena_input") (result i32) i32.const 1024)
  (func (export "arena_start") (param i32))
  (func (export "arena_round"))
  (func (export "arena_decide") (param $u f32) (result i32) i32.const 0))"#;
    let runner = wat::parse_str(text)
        .ok()
        .and_then(|b| admit(&b).ok())
        .and_then(|a| Runner::new(&a).ok());
    assert!(
        runner.is_some(),
        "it is admitted: the ABI is checked when it runs"
    );
    let Some(r) = runner else { return };
    match r.instance() {
        Ok(_) => unreachable!("an arena-bot-5 module started"),
        Err(f) => assert_eq!(
            f.0,
            "arena_abi() is 5, not 6: the module is not arena-bot-6"
        ),
    }
}

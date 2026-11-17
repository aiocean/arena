//! The arena's commands: a fight kept as an act log plays again, and a log that is not of these modules is refused.

use std::cell::RefCell;
use std::rc::Rc;

use arena_bot::run::{
    Decision, LIMIT, Mismatch, ROUNDS, SEASON, Verdict, decisions, entry, fight, play, verify,
};
use arena_bot::{WasmBot, check, engine, hash};
use pixfx_duel::seat::{BotSeat, Brain, Decider, Fault};
use pixfx_duel::style::Style;
use pixfx_mind::policy::{ACTIONS, OBS};

type Test = Result<(), Box<dyn std::error::Error>>;

const HOUSE: &[u8] = include_bytes!("../../pixfx-duel/weights/house.wasm");

#[test]
fn fight_then_verify() -> Test {
    let engine = engine()?;
    let log = fight(&engine, HOUSE, HOUSE, 3, ROUNDS, LIMIT)?;
    assert_eq!(log.season, SEASON);
    assert!(log.acts.iter().all(|acts| !acts.is_empty()));
    assert_eq!(verify(&engine, &log, HOUSE, HOUSE), Ok(()));
    Ok(())
}

#[test]
fn verify_refuses_another_season() -> Test {
    let engine = engine()?;
    let mut log = fight(&engine, HOUSE, HOUSE, 3, ROUNDS, LIMIT)?;
    log.season = "arena-s0".into();
    assert_eq!(
        verify(&engine, &log, HOUSE, HOUSE),
        Err(Mismatch::Season {
            want: "arena-s0".into(),
            got: SEASON.into()
        })
    );
    Ok(())
}

#[test]
fn verify_refuses_another_module() -> Test {
    let engine = engine()?;
    let log = fight(&engine, HOUSE, HOUSE, 3, ROUNDS, LIMIT)?;
    let other = wat::parse_str("(module)")?;
    assert_eq!(
        verify(&engine, &log, HOUSE, &other),
        Err(Mismatch::Bot {
            corner: 1,
            want: log.bots[1].clone(),
            got: arena_bot::hash(&other),
        })
    );
    Ok(())
}

#[test]
fn verify_finds_a_changed_act() -> Test {
    let engine = engine()?;
    let mut log = fight(&engine, HOUSE, HOUSE, 3, ROUNDS, LIMIT)?;
    let (at, other) = (log.acts[1][2].0, log.acts[1][3].1.clone());
    assert_ne!(log.acts[1][2].1, other, "pick two different acts");
    log.acts[1][2].1 = other;
    assert_eq!(
        verify(&engine, &log, HOUSE, HOUSE),
        Err(Mismatch::Acts { corner: 1, at })
    );
    Ok(())
}

#[test]
fn entry_passes_the_house_bot() -> Test {
    let engine = engine()?;
    let verdict = entry(&engine, HOUSE, 1..11, &[]);
    assert!(matches!(verdict, Verdict::Pass { .. }), "{verdict}");
    Ok(())
}

/// The house bot's calls against the brain on seed 1, kept.
struct Recorder {
    inner: WasmBot,
    calls: Rc<RefCell<Vec<Decision>>>,
}

impl Decider for Recorder {
    fn start(&mut self, seed: u32) -> Result<(), Fault> {
        self.inner.start(seed)
    }
    fn round(&mut self) -> Result<(), Fault> {
        self.inner.round()
    }
    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        self.calls.borrow_mut().push(Decision {
            obs: *obs,
            legal: *legal,
            u,
        });
        self.inner.decide(obs, legal, u)
    }
}

struct Faulty(Fault);

impl Decider for Faulty {
    fn start(&mut self, _: u32) -> Result<(), Fault> {
        Err(self.0.clone())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Err(self.0.clone())
    }
    fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        Err(self.0.clone())
    }
}

fn record() -> Test {
    let engine = engine()?;
    let module = check(&engine, HOUSE)?;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let seen = calls.clone();
    let new: Rc<dyn Fn() -> Box<dyn Decider>> =
        Rc::new(move || match WasmBot::new(&engine, &module) {
            Ok(inner) => Box::new(Recorder {
                inner,
                calls: seen.clone(),
            }),
            Err(fault) => Box::new(Faulty(fault)),
        });
    let seat = BotSeat::make(new, Some(Style::Balanced));
    play([seat, Brain::make()], 1, ROUNDS, LIMIT);
    let bytes: Vec<u8> = calls
        .borrow()
        .iter()
        .take(64)
        .flat_map(Decision::encode)
        .collect();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/entry_decisions.bin");
    if std::fs::metadata(path)?.len() == 0 {
        std::fs::write(path, &bytes)?;
    }
    assert_eq!(
        std::fs::read(path)?,
        bytes,
        "the kept decisions are the house bot's against the brain"
    );
    assert_eq!(decisions(&bytes).len(), 64);
    Ok(())
}

#[test]
fn entry_decisions_are_the_house_bots() -> Test {
    record()
}

#[test]
fn entry_refuses_a_known_module() -> Test {
    let engine = engine()?;
    let id = hash(HOUSE);
    assert_eq!(
        entry(&engine, HOUSE, 1..11, std::slice::from_ref(&id)),
        Verdict::Known(id.clone())
    );
    assert!(Verdict::Known(id.clone()).to_string().contains(&id));
    Ok(())
}

#[test]
fn entry_refuses_a_bot_that_runs_out_of_fuel_on_decision_10() -> Test {
    let engine = engine()?;
    let bot = wat::parse_str(
        r#"(module
          (memory (export "memory") 1)
          (global $n (mut i32) (i32.const 0))
          (func (export "arena_abi") (result i32) (i32.const 1))
          (func (export "arena_input") (result i32) (i32.const 0))
          (func (export "arena_start") (param i32))
          (func (export "arena_round"))
          (func (export "arena_decide") (param f32) (result i32)
            (global.set $n (i32.add (global.get $n) (i32.const 1)))
            (if (i32.eq (global.get $n) (i32.const 10)) (then (loop $l (br $l))))
            (i32.const 0)))"#,
    )?;
    let verdict = entry(&engine, &bot, 1..11, &[]);
    assert_eq!(verdict, Verdict::Faulted(Fault::OutOfFuel), "{verdict}");
    Ok(())
}

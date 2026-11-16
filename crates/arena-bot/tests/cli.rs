//! The arena's commands: a fight kept as an act log plays again, and a log that is not of these modules is refused.

use arena_bot::engine;
use arena_bot::run::{LIMIT, Mismatch, ROUNDS, SEASON, Verdict, entry, fight, verify};

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
    let verdict = entry(&engine, HOUSE, 1..11);
    assert!(matches!(verdict, Verdict::Pass { .. }), "{verdict}");
    Ok(())
}

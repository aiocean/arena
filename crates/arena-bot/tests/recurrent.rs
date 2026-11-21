//! A recurrent ONNX policy (`train/net.py`, `Policy(recurrent=...)`): tract runs it with its state, the state moves
//! with what it sees, and its seat clears it at each fight and round, as `bots/onnx` does.
#![cfg(feature = "onnx")]

use arena_bot::onnx::{Onnx, OnnxBot};
use arena_bot::{ACTIONS, OBS};
use arena_duel::seat::Decider;

/// A tiny recurrent policy (width 8, a state of 4) whose state weighs on its logits.
const MODEL: &[u8] = include_bytes!("fixtures/recurrent.onnx");

fn obs(k: usize) -> [f32; OBS] {
    std::array::from_fn(|i| ((i * 7 + k * 13) % 17) as f32 / 17.0 - 0.5)
}

#[test]
fn its_state_moves_and_changes_what_it_answers() -> Result<(), String> {
    let onnx = Onnx::load(MODEL).map_err(|e| e.0)?;
    assert_eq!(onnx.state(), 4);
    let mut h = vec![0.0; 4];
    let first = onnx.step(&obs(0), &mut h).map_err(|e| e.0)?;
    assert!(h.iter().any(|&x| x != 0.0), "the state moved: {h:?}");
    let again = onnx.step(&obs(0), &mut h).map_err(|e| e.0)?;
    assert_ne!(first, again, "the same observation from another state");
    assert_eq!(
        onnx.logits(&obs(0)).map_err(|e| e.0)?,
        first,
        "logits read from a fresh state"
    );
    Ok(())
}

#[test]
fn its_seat_clears_the_state_each_round() -> Result<(), String> {
    let onnx = Onnx::load(MODEL).map_err(|e| e.0)?;
    let legal = [true; ACTIONS];
    let mut bot = OnnxBot::new(onnx);
    assert!(!bot.stateless(), "a recurrent seat is never shared");
    let mut play = |bot: &mut OnnxBot| -> Result<Vec<i32>, String> {
        (0..12)
            .map(|k| {
                bot.decide(&obs(k), &legal, (k * 37 % 100) as f32 / 100.0)
                    .map_err(|e| e.0)
            })
            .collect()
    };
    bot.start(1).map_err(|e| e.0)?;
    assert!(bot.state().iter().all(|&x| x == 0.0));
    let a = play(&mut bot)?;
    assert!(
        bot.state().iter().any(|&x| x != 0.0),
        "it remembers within a round"
    );
    bot.round().map_err(|e| e.0)?;
    assert!(
        bot.state().iter().all(|&x| x == 0.0),
        "and forgets at the next"
    );
    assert_eq!(play(&mut bot)?, a, "a new round plays as the first did");
    bot.start(2).map_err(|e| e.0)?;
    assert!(
        bot.state().iter().all(|&x| x == 0.0),
        "a new fight forgets too"
    );
    Ok(())
}

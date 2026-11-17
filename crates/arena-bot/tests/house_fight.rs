//! The house bot module's fights, pinned (`goldens/house_fight.json`): the module `site/build.sh` builds from
//! `weights/policy.onnx` (`site/dist/bots/policy.wasm`), one instance in each corner, seeds 1-3, 60 s at 60 Hz, then
//! the round, wins and each fighter's hp, qi, x and air.
#![cfg(feature = "host")]

use arena_bot::admit;
use arena_bot::host::Runner;
use arena_duel::data::UNIT;
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::seat::BotSeat;
use serde_json::{Value, json};

const MODULE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../site/dist/bots/policy.wasm"
);
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/goldens/house_fight.json"
);

#[test]
fn the_house_modules_fights_are_pinned() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(MODULE)
        .map_err(|e| format!("{MODULE}: {e}; build it with site/build.sh"))?;
    let runner = Runner::new(&admit(&bytes)?).map_err(|e| e.0)?;
    let (left, right) = (runner.decider(), runner.decider());
    let effects: Vec<String> = arena_duel::effects::all();
    let mut all = serde_json::Map::new();
    for seed in 1..=3_u32 {
        let config = DuelConfig {
            seed,
            left: 0.0,
            right: 220.0 * UNIT,
            ground: 150.0 * UNIT,
            sky: 96.0 * UNIT,
            effects: effects.clone(),
        };
        let mut duel = Duel::new(
            config,
            [
                BotSeat::make(left.clone(), None),
                BotSeat::make(right.clone(), None),
            ],
        );
        for _ in 0..60 * 60 {
            duel.step(TICK);
        }
        let s = &duel.state();
        let fighters: Vec<Value> = s
            .fighters
            .iter()
            .map(|f| json!([f.hp(), f.qi(), f.body.x, f.body.air]))
            .collect();
        all.insert(
            seed.to_string(),
            json!({ "time": s.time, "round": s.round, "wins": s.wins, "fighters": fighters }),
        );
    }
    let got = Value::Object(all);
    match std::fs::read_to_string(GOLDEN) {
        Ok(text) => {
            let want: Value = serde_json::from_str(&text)?;
            assert_eq!(
                got, want,
                "the house module's fights changed: if that was meant, delete {GOLDEN} and run again"
            );
        }
        Err(_) => {
            std::fs::write(GOLDEN, serde_json::to_string_pretty(&got)? + "\n")?;
            eprintln!("wrote {GOLDEN}: the first run pins it");
        }
    }
    Ok(())
}

//! The arena's duel in a window: `arena [--seed N] [--left BOT] [--right BOT]`, a bot being `house`, `brain`,
//! `first` or a bot module (`.wasm`).

use arena_game::bots::Corner;
use arena_game::{Start, app};
use std::sync::Arc;

fn corner(arg: &str) -> Result<Corner, String> {
    Ok(match arg {
        "house" => Corner::House,
        "brain" => Corner::Brain,
        "first" => Corner::First,
        path => Corner::Module {
            name: path.rsplit('/').next().unwrap_or(path).to_string(),
            bytes: Arc::new(std::fs::read(path).map_err(|e| format!("{path}: {e}"))?),
        },
    })
}

fn main() -> Result<(), String> {
    let mut start = Start::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let value = args.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--seed" => {
                start.seed = Some(value.parse().map_err(|_| format!("not a seed: {value}"))?)
            }
            "--left" => start.corners[0] = corner(&value)?,
            "--right" => start.corners[1] = corner(&value)?,
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    app(start).run();
    Ok(())
}

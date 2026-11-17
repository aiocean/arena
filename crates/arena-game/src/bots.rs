//! Who sits in each corner. A corner holds a bot (a player's module, or the house AI) or one of the rules' own minds.
//! Natively a module runs in wasmtime with its fuel metered as the tournament's host runs it, and the house AI from
//! its ONNX model; in the browser the page's WebAssembly runs the module (`web`), the house AI being the module
//! built from the same model. Each corner gets its own instance, which keeps its memory for the corner's whole match.

use bevy::prelude::*;
use arena_duel::first_mind::FirstMind;
use arena_duel::seat::BotSeat;
use arena_duel::seat::{Brain, MakeSeat, SharedDecider};
use std::rc::Rc;
use std::sync::Arc;

/// What a corner holds.
#[derive(Clone, Debug)]
pub enum Corner {
    /// The house AI, the trained policy the arena ships.
    House,
    /// The rules' utility brain.
    Brain,
    /// The rules' first mind.
    First,
    /// A player's bot module, named for the status line.
    Module { name: String, bytes: Arc<Vec<u8>> },
}

impl Corner {
    pub fn name(&self) -> String {
        match self {
            Corner::House => "house bot".into(),
            Corner::Brain => "brain".into(),
            Corner::First => "first mind".into(),
            Corner::Module { name, .. } => name.clone(),
        }
    }
}

/// A corner ready to fight: the decider of its bot (none for a rules' mind), or the mind itself.
#[derive(Clone)]
pub enum Seated {
    Bot(SharedDecider),
    Brain,
    First,
}

impl Seated {
    pub fn make(&self) -> MakeSeat {
        match self {
            Seated::Bot(d) => BotSeat::make(Rc::clone(d), None),
            Seated::Brain => Brain::make(),
            Seated::First => FirstMind::make(),
        }
    }
}

/// The corners as they are seated, the seed to fight on, and a count bumped each time they change (the fight starts
/// over). `None` while a corner is still being made (a module compiling in the browser).
#[derive(Default)]
pub struct Seating {
    pub seated: Option<[Seated; 2]>,
    pub names: [String; 2],
    pub seed: u32,
    pub version: u32,
    /// Why the last corners asked for could not be seated.
    pub error: Option<String>,
}

/// Ask for `corners` on `seed`: natively they are seated at once; in the browser once their modules are compiled.
#[derive(Message, Clone, Debug)]
pub struct Seat {
    pub corners: [Corner; 2],
    pub seed: u32,
}

pub struct BotsPlugin;

impl Plugin for BotsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(Seating::default())
            .add_message::<Seat>()
            .add_systems(Startup, first_seating)
            .add_systems(Update, seat.in_set(crate::Frame::Pace));
    }
}

/// A seed drawn from the clock, when none was asked for.
pub fn any_seed() -> u32 {
    let now = web_time_ms();
    1 + (now as u64 % 999_999) as u32
}

#[cfg(not(target_arch = "wasm32"))]
fn web_time_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(1.0)
}

#[cfg(target_arch = "wasm32")]
fn web_time_ms() -> f64 {
    js_sys::Date::now()
}

fn first_seating(start: Res<crate::Start>, mut ask: MessageWriter<Seat>) {
    ask.write(Seat {
        corners: start.corners.clone(),
        seed: start.seed.unwrap_or_else(any_seed),
    });
}

fn seat(mut asked: MessageReader<Seat>, mut seating: NonSendMut<Seating>) {
    let Some(ask) = asked.read().last().cloned() else {
        return;
    };
    #[cfg(not(target_arch = "wasm32"))]
    match native::seat(&ask.corners) {
        Ok(seated) => {
            seating.seated = Some(seated);
            seating.names = ask.corners.clone().map(|c| c.name());
            seating.seed = ask.seed;
            seating.version += 1;
            seating.error = None;
        }
        Err(e) => {
            error!("a corner could not be seated: {e}");
            seating.error = Some(e);
        }
    }
    #[cfg(target_arch = "wasm32")]
    crate::web::seat(ask, &mut seating);
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::{Corner, Seated};
    use arena_bot::host::Runner;
    use arena_bot::onnx::Onnx;

    /// Each corner made: a module admitted and instantiated (an instance a corner), the house AI from its model.
    pub fn seat(corners: &[Corner; 2]) -> Result<[Seated; 2], String> {
        let one = |c: &Corner| -> Result<Seated, String> {
            Ok(match c {
                Corner::House => Seated::Bot(Onnx::house().map_err(|e| e.0)?.decider()),
                Corner::Brain => Seated::Brain,
                Corner::First => Seated::First,
                Corner::Module { name, bytes } => {
                    let admitted = arena_bot::admit(bytes).map_err(|e| format!("{name}: {e}"))?;
                    let runner = Runner::new(&admitted).map_err(|e| format!("{name}: {}", e.0))?;
                    Seated::Bot(runner.decider())
                }
            })
        };
        Ok([one(&corners[0])?, one(&corners[1])?])
    }
}

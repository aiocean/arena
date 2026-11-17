//! The arena's commands as functions: a fight with its act log, checking a log, and a module's entry into the season.

use std::cell::RefCell;
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

use pixfx_duel::duel::{Duel, DuelConfig, TICK};
use pixfx_duel::ladder::{Bot, EntryBar, Match, entry_bar};
use pixfx_duel::log::{ActLog, recorded};
use pixfx_duel::seat::{Decider, Fault, MakeSeat};
use pixfx_mind::policy::{ACTIONS, OBS};
use wasmtime::{Engine, Module};

use crate::{Refusal, WasmBot, check, hash};

/// The season these rules and this wasmtime make.
pub const SEASON: &str = "arena-s1";

/// Rounds a fight lasts and the most seconds one may last: the ladder's.
pub const ROUNDS: u32 = 3;
pub const LIMIT: f64 = 120.0;

/// What a seat holds when its module will not instantiate: every call faults.
struct Broken(Fault);

impl Decider for Broken {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        Err(self.0.clone())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Err(self.0.clone())
    }
    fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        Err(self.0.clone())
    }
}

/// A checked module as a contestant: a fresh instance for each fight.
pub fn bot(engine: &Engine, module: &Module) -> Bot {
    let (engine, module) = (engine.clone(), module.clone());
    Bot::Decider(Rc::new(move || match WasmBot::new(&engine, &module) {
        Ok(bot) => Box::new(bot),
        Err(fault) => Box::new(Broken(fault)),
    }))
}

pub fn effects() -> Vec<String> {
    pixfx_effects::EFFECTS
        .iter()
        .map(|e| e.meta().id.to_string())
        .collect()
}

/// Plays `seats` on `seed` until `rounds` are decided, a round passes `limit` seconds, or a seat faults; the faults
/// of the two seats.
pub fn play(seats: [MakeSeat; 2], seed: u32, rounds: u32, limit: f64) -> [Option<Fault>; 2] {
    let config = DuelConfig {
        seed,
        left: 0.0,
        right: 220.0,
        ground: 150.0,
        sky: 96.0,
        effects: effects(),
    };
    let mut duel = Duel::new(config, seats);
    let (mut started, mut round) = (duel.state.time, duel.state.round);
    loop {
        duel.step(TICK);
        let faults = [0, 1].map(|side| duel.seat(side).fault().cloned());
        let s = &duel.state;
        if faults.iter().any(Option::is_some) || s.wins[0] + s.wins[1] >= rounds {
            return faults;
        }
        if s.round != round {
            (started, round) = (s.time, s.round);
        }
        if s.winner.is_none() && s.time - started > limit {
            return faults;
        }
    }
}

/// The fight of `seed` between modules `a` (Thunder) and `b` (Frost), with the acts each chose.
pub fn fight(
    engine: &Engine,
    a: &[u8],
    b: &[u8],
    seed: u32,
    rounds: u32,
    limit: f64,
) -> Result<ActLog, Refusal> {
    fought(engine, a, b, seed, rounds, limit).map(|(log, _)| log)
}

/// `fight`, and the fault that ended it for each corner.
pub fn fought(
    engine: &Engine,
    a: &[u8],
    b: &[u8],
    seed: u32,
    rounds: u32,
    limit: f64,
) -> Result<(ActLog, [Option<Fault>; 2]), Refusal> {
    let (ma, mb) = (check(engine, a)?, check(engine, b)?);
    let log = Rc::new(RefCell::new(ActLog {
        season: SEASON.to_string(),
        seed,
        bots: [hash(a), hash(b)],
        ..ActLog::default()
    }));
    let seats = [bot(engine, &ma).maker(), bot(engine, &mb).maker()]
        .map(|make| recorded(make, log.clone()));
    let faults = play(seats, seed, rounds, limit);
    Ok((log.take(), faults))
}

/// Why a log is not the fight of these modules.
#[derive(Clone, Debug, PartialEq)]
pub enum Mismatch {
    Season {
        want: String,
        got: String,
    },
    /// Corner 0 is Thunder, 1 Frost; the hashes are the log's (`want`) and the module's (`got`).
    Bot {
        corner: usize,
        want: String,
        got: String,
    },
    /// A module that is not a bot cannot play the fight again.
    Refused(Refusal),
    /// The corner's first act that differs, by the index of the `think` call it was made at.
    Acts {
        corner: usize,
        at: u32,
    },
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mismatch::Season { want, got } => {
                write!(f, "log is of season {want}, this arena is {got}")
            }
            Mismatch::Bot { corner, want, got } => {
                write!(f, "corner {corner}: log has module {want}, given {got}")
            }
            Mismatch::Refused(why) => write!(f, "{why}"),
            Mismatch::Acts { corner, at } => write!(f, "corner {corner}: acts differ at call {at}"),
        }
    }
}

impl std::error::Error for Mismatch {}

/// Plays `log`'s fight again with modules `a` and `b`: the same season, the same modules, and the same acts.
pub fn verify(engine: &Engine, log: &ActLog, a: &[u8], b: &[u8]) -> Result<(), Mismatch> {
    if log.season != SEASON {
        return Err(Mismatch::Season {
            want: log.season.clone(),
            got: SEASON.to_string(),
        });
    }
    for (corner, bytes) in [a, b].into_iter().enumerate() {
        let got = hash(bytes);
        if got != log.bots[corner] {
            return Err(Mismatch::Bot {
                corner,
                want: log.bots[corner].clone(),
                got,
            });
        }
    }
    let again = fight(engine, a, b, log.seed, ROUNDS, LIMIT).map_err(Mismatch::Refused)?;
    for corner in 0..2 {
        let (want, got) = (&log.acts[corner], &again.acts[corner]);
        let at = want
            .iter()
            .zip(got)
            .find(|(w, g)| w != g)
            .map(|(w, _)| w.0)
            .or_else(|| want.get(got.len()).or(got.get(want.len())).map(|x| x.0));
        if let Some(at) = at {
            return Err(Mismatch::Acts { corner, at });
        }
    }
    Ok(())
}

/// The parts of the entry bar: each one's name, what the module won, and what it had to.
pub fn rows(bar: &EntryBar) -> [(&'static str, f64, f64); 4] {
    let brain = &bar.vs_brain;
    [
        ("vs brain", brain.win_rate(), EntryBar::BRAIN),
        (
            "vs brain as Thunder",
            brain.as_school(0),
            EntryBar::BRAIN_EACH_SCHOOL,
        ),
        (
            "vs brain as Frost",
            brain.as_school(1),
            EntryBar::BRAIN_EACH_SCHOOL,
        ),
        ("vs first mind", bar.vs_first.win_rate(), EntryBar::FIRST),
    ]
}

/// Whether a module enters the season.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Every check passed; the module's hash and what it won at the bar.
    Pass {
        hash: String,
        bar: EntryBar,
    },
    Refused(Refusal),
    /// The arena has this module already.
    Known(String),
    /// It faulted answering the recorded decisions.
    Faulted(Fault),
    /// Fights the bar played ended by a fault.
    FaultedFights(u32),
    /// Two fresh instances answered the recorded decisions differently: the call where they first differ.
    Unstable(usize),
    /// It won too little at the bar.
    Below(EntryBar),
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pct = |x: f64| x * 100.0;
        match self {
            Verdict::Pass { hash, bar } => {
                for (name, rate, min) in rows(bar) {
                    writeln!(f, "{name:<28} {:>5.1}% (bar {:.0}%)", pct(rate), pct(min))?;
                }
                write!(f, "enters the season as {hash}")
            }
            Verdict::Refused(why) => write!(f, "{why}"),
            Verdict::Known(hash) => write!(f, "module {hash} is already entered"),
            Verdict::Faulted(fault) => write!(f, "faulted on the recorded decisions: {fault}"),
            Verdict::FaultedFights(n) => write!(f, "{n} fights of the bar ended by a fault"),
            Verdict::Unstable(at) => write!(
                f,
                "two fresh instances differ at decision {at} (act or fuel left)"
            ),
            Verdict::Below(bar) => {
                let missed: Vec<String> = rows(bar)
                    .into_iter()
                    .filter(|(_, rate, min)| rate < min)
                    .map(|(name, rate, min)| {
                        format!("{name}: {:.1}%, the bar is {:.0}%", pct(rate), pct(min))
                    })
                    .collect();
                write!(f, "{}", missed.join("; "))
            }
        }
    }
}

/// One call of `arena_decide`.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub obs: [f32; OBS],
    pub legal: [bool; ACTIONS],
    pub u: f32,
}

const DECISION_BYTES: usize = OBS * 4 + ACTIONS + 4;

/// The seed the recorded decisions are answered from.
const REPLAY_SEED: u32 = 1;

/// The recorded decisions the entry replays: 64 of the house bot against the brain, seed 1 (`tests/cli.rs` writes
/// the file when it is empty).
const DECISIONS: &[u8] = include_bytes!("entry_decisions.bin");

impl Decision {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(DECISION_BYTES);
        out.extend(self.obs.iter().flat_map(|x| x.to_le_bytes()));
        out.extend(self.legal.iter().map(|&l| u8::from(l)));
        out.extend(self.u.to_le_bytes());
        out
    }

    fn decode(bytes: &[u8]) -> Decision {
        let mut obs = [0.0; OBS];
        let (floats, rest) = bytes.split_at(OBS * 4);
        for (x, c) in obs.iter_mut().zip(floats.as_chunks::<4>().0.iter()) {
            *x = f32::from_le_bytes(*c);
        }
        let mut legal = [false; ACTIONS];
        for (l, &b) in legal.iter_mut().zip(rest) {
            *l = b != 0;
        }
        let u = rest
            .get(ACTIONS..ACTIONS + 4)
            .map_or(0.0, |c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]));
        Decision { obs, legal, u }
    }
}

/// The decisions of a recording.
pub fn decisions(bytes: &[u8]) -> Vec<Decision> {
    bytes
        .as_chunks::<DECISION_BYTES>()
        .0
        .iter()
        .map(|c| Decision::decode(c))
        .collect()
}

/// What a fresh instance answers the decisions: the act and the fuel left after each call.
fn answers(
    engine: &Engine,
    module: &Module,
    decisions: &[Decision],
) -> Result<Vec<(i32, u64)>, Fault> {
    let mut bot = WasmBot::new(engine, module)?;
    bot.start(REPLAY_SEED)?;
    decisions
        .iter()
        .map(|d| {
            let act = bot.decide(&d.obs, &d.legal, d.u)?;
            Ok((act, bot.fuel_left()))
        })
        .collect()
}

/// The submission checks: the module is not `known`, it is a bot, two fresh instances answer the recorded decisions
/// alike within fuel, and it clears the bar of `docs/tournament.md` over `seeds`, both ways.
pub fn entry(engine: &Engine, bytes: &[u8], seeds: Range<u32>, known: &[String]) -> Verdict {
    let id = hash(bytes);
    if known.contains(&id) {
        return Verdict::Known(id);
    }
    let module = match check(engine, bytes) {
        Ok(module) => module,
        Err(why) => return Verdict::Refused(why),
    };
    let recorded = decisions(DECISIONS);
    let twice = [0, 1].map(|_| answers(engine, &module, &recorded));
    let [first, second] = twice;
    let (first, second) = match (first, second) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(fault), _) | (_, Err(fault)) => return Verdict::Faulted(fault),
    };
    if let Some(at) = first.iter().zip(&second).position(|(a, b)| a != b) {
        return Verdict::Unstable(at);
    }
    let m = Match {
        seeds,
        rounds: ROUNDS,
        limit: LIMIT,
        effects: effects(),
    };
    let bar = entry_bar(&bot(engine, &module), &m);
    let faulted = bar.vs_brain.faults[0] + bar.vs_first.faults[0];
    if faulted > 0 {
        return Verdict::FaultedFights(faulted);
    }
    if bar.passed() {
        Verdict::Pass { hash: id, bar }
    } else {
        Verdict::Below(bar)
    }
}

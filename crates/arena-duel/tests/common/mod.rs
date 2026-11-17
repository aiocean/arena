//! What the tests share: a duel's state built as `Duel::new` builds it (each fighter's random loadout, then three
//! draws each), without its minds or its first round's bodies, duels to run, loadouts to fight with, and `pinned`,
//! which checks an output against its golden file.
#![allow(dead_code)]

use arena_calc::loadout::{Loadout, Slot as LoadoutSlot};
use arena_duel::bar::draw;
use arena_duel::fighter::{Fighter, Slot, State};

pub fn arena(seed: u32) -> State {
    let [a, b] = arena_duel::duel::loadouts(seed);
    let mut state = State::new(
        seed,
        0.0,
        880.0,
        600.0,
        384.0,
        Vec::new(),
        [Fighter::new(0, a), Fighter::new(1, b)],
    );
    // startRound's places: 22% and 78% across the arena
    let span = state.right - state.left;
    for (i, at) in [(0, 0.22), (1, 0.78)] {
        state.fighters[i].body.start_round(state.left + span * at);
    }
    for f in 0..2 {
        for i in 0..3 {
            draw(&mut state, f, i, None);
        }
    }
    state
}

/// A loadout of `actives` (with no supports; padded from a fixed list to eight), with no keystones.
pub fn loadout(actives: &[&str]) -> Loadout {
    let pad = [
        "bolt", "palm", "fireball", "quake", "heal", "ward", "spark", "seal", "jet", "orb",
    ];
    let mut skills: Vec<&str> = actives.to_vec();
    for p in pad {
        if skills.len() >= arena_calc::loadout::ACTIVES {
            break;
        }
        if !skills.contains(&p) {
            skills.push(p);
        }
    }
    Loadout {
        actives: skills
            .into_iter()
            .map(|s| LoadoutSlot {
                skill: s.to_string(),
                supports: Vec::new(),
                fuse: None,
            })
            .collect(),
        keystones: Vec::new(),
        root: None,
    }
}

/// A plain bar slot holding `spell`.
pub fn common(spell: &'static str) -> Slot {
    Slot::Held { spell, idle: 0.0 }
}

pub fn empty(left: f64) -> Slot {
    Slot::Empty { left, full: left }
}

/// The skills on fighter `f`'s bar.
pub fn on_bar(f: &Fighter) -> Vec<&'static str> {
    f.bar.iter().filter_map(Slot::spell).collect()
}

/// A duel of `seats` with `loadouts`, on the landing arena with the whole registry.
pub fn duel_of(
    seed: u32,
    seats: [arena_duel::seat::MakeSeat; 2],
    loadouts: [Loadout; 2],
) -> arena_duel::duel::Duel {
    match arena_duel::duel::Duel::with_loadouts(config(seed, all_effects()), seats, loadouts) {
        Ok(d) => d,
        Err(e) => unreachable!("an illegal test loadout: {e:?}"),
    }
}

/// Every number as an f64, so 1 and 1.0 compare equal.
pub fn norm(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Number(n) => serde_json::json!(n.as_f64().unwrap_or(f64::NAN)),
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), norm(v))).collect()),
        other => other.clone(),
    }
}

/// Every effect in the engine's registry, as the page has it.
pub fn all_effects() -> Vec<String> {
    arena_duel::effects::all()
}

/// Whether effect `fx` loops (and so must be stopped).
pub fn loops(fx: &str) -> bool {
    arena_duel::effects::loops(fx)
}

/// The landing arena (0 to 880, ground 600) with effects `effects`.
pub fn config(seed: u32, effects: Vec<String>) -> arena_duel::duel::DuelConfig {
    arena_duel::duel::DuelConfig {
        seed,
        left: 0.0,
        right: 880.0,
        ground: 600.0,
        sky: 384.0,
        effects,
    }
}

/// A duel of two brains with the whole registry.
pub fn fight(seed: u32) -> arena_duel::duel::Duel {
    arena_duel::duel::Duel::with_brains(config(seed, all_effects()))
}

/// A seat that never acts.
pub fn idle() -> arena_duel::seat::MakeSeat {
    Box::new(|_, _| Box::new(arena_duel::seat::Script(Box::new(|_, _| None))))
}

/// A seat that casts `skill` once, then never acts again.
pub fn caster(skill: &'static str) -> arena_duel::seat::MakeSeat {
    Box::new(move |_, _| {
        let mut cast = false;
        Box::new(arena_duel::seat::Script(Box::new(move |_, _| {
            if cast {
                return None;
            }
            cast = true;
            Some(arena_mind::Choice::Cast {
                skill: skill.to_string(),
                target: None,
            })
        })))
    })
}

/// A seat that casts `skill` every tick it is asked.
pub fn always(skill: &'static str) -> arena_duel::seat::MakeSeat {
    Box::new(move |_, _| {
        Box::new(arena_duel::seat::Script(Box::new(move |_, _| {
            Some(arena_mind::Choice::Cast {
                skill: skill.to_string(),
                target: None,
            })
        })))
    })
}

/// Step `duel` `seconds` at 60 Hz; returns everything it emitted.
pub fn run(duel: &mut arena_duel::duel::Duel, seconds: f64) -> Vec<arena_duel::out::Out> {
    let mut out = Vec::new();
    for _ in 0..arena_duel::js::js_round(seconds * 60.0) as usize {
        out.extend(duel.step(1.0 / 60.0));
    }
    out
}

/// `got` against `tests/goldens/<name>.json`. A change that alters it fails; when the change is meant, delete the file
/// and run the test once: a missing file is written from `got`, and the test says so.
pub fn pinned(name: &str, got: &serde_json::Value) {
    let file = format!("{}/tests/goldens/{name}.json", env!("CARGO_MANIFEST_DIR"));
    match std::fs::read_to_string(&file) {
        Ok(text) => {
            let want: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
            assert_eq!(
                norm(got),
                norm(&want),
                "{name} changed: if that was meant, delete {file} and run again"
            );
        }
        Err(_) => {
            let text = serde_json::to_string_pretty(got).unwrap_or_default() + "\n";
            assert!(
                std::fs::write(&file, text).is_ok(),
                "could not write {file}"
            );
            eprintln!("wrote {file}: the first run pins it");
        }
    }
}

/// A bot that plays any legal act, picked by the seat's draw: a decider with no model, for the seat's own tests.
pub struct Pick;

impl arena_duel::seat::Decider for Pick {
    fn start(&mut self, _seed: u32) -> Result<(), arena_duel::seat::Fault> {
        Ok(())
    }
    fn round(&mut self) -> Result<(), arena_duel::seat::Fault> {
        Ok(())
    }
    fn decide(
        &mut self,
        _obs: &[f32; arena_mind::policy::OBS],
        legal: &[bool; arena_mind::policy::ACTIONS],
        u: f32,
    ) -> Result<i32, arena_duel::seat::Fault> {
        let ok: Vec<usize> = (0..legal.len()).filter(|&i| legal[i]).collect();
        let at = ((u * ok.len() as f32) as usize).min(ok.len().saturating_sub(1));
        Ok(ok.get(at).map_or(0, |&i| i as i32))
    }
}

/// A fresh `Pick` for a seat.
pub fn pick() -> arena_duel::seat::SharedDecider {
    std::rc::Rc::new(std::cell::RefCell::new(Pick))
}

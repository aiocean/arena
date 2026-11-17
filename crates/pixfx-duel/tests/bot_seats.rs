//! Bots over a `Decider`, and fights kept as act logs: a bot running the shipped net plays the policy's fight, a log
//! replays its fight, a bad act is a wait, a fault stops the seat.

mod common;

use common::{all_effects, config, run};
use pixfx_duel::duel::Duel;
use pixfx_duel::log::{ActLog, recorded, replay};
use pixfx_duel::out::Out;
use pixfx_duel::seat::{BotSeat, Decider, Fault, MakeSeat, PolicySeat, shipped};
use pixfx_mind::policy::{ACTIONS, Net, OBS, sample};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What `PolicySeat` does with its net: the logits on the observation, sampled on `u`.
struct NetDecider(Rc<Net>);

impl Decider for NetDecider {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        Ok(())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Ok(())
    }
    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        Ok(sample(&self.0.logits(obs), legal, u) as i32)
    }
}

/// A decider that answers `act` every time.
struct Fixed(i32);

impl Decider for Fixed {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        Ok(())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Ok(())
    }
    fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        Ok(self.0)
    }
}

fn bot(new: impl Fn() -> Box<dyn Decider> + 'static) -> MakeSeat {
    BotSeat::make(Rc::new(new), None)
}

fn fixed_bot(act: i32) -> MakeSeat {
    bot(move || Box::new(Fixed(act)))
}

fn brain() -> MakeSeat {
    pixfx_duel::seat::Brain::make()
}

fn shipped_net() -> Result<Rc<Net>, String> {
    shipped().map_err(|e| format!("{e:?}"))
}

/// Every `Out` of a fight as one string, to compare fights whole.
fn stream(outs: &[Out]) -> String {
    outs.iter().map(|o| format!("{o:?}\n")).collect()
}

fn play(seats: [MakeSeat; 2], seed: u32, seconds: f64) -> String {
    stream(&run(
        &mut Duel::new(config(seed, all_effects()), seats),
        seconds,
    ))
}

#[test]
fn bot_seat_plays_the_policys_fight() -> Result<(), String> {
    let net = shipped_net()?;
    for seed in 1..=5 {
        let policy = play([PolicySeat::make(net.clone()), brain()], seed, 60.0);
        let n = net.clone();
        let botted = play(
            [bot(move || Box::new(NetDecider(n.clone()))), brain()],
            seed,
            60.0,
        );
        assert_eq!(botted, policy, "seed {seed}");
    }
    Ok(())
}

#[test]
fn a_log_replays_its_fight() -> Result<(), String> {
    let net = shipped_net()?;
    let log = Rc::new(RefCell::new(ActLog::default()));
    let corner = |net: &Rc<Net>| {
        let net = net.clone();
        recorded(bot(move || Box::new(NetDecider(net.clone()))), log.clone())
    };
    let played = play([corner(&net), corner(&net)], 7, 120.0);
    let log = Rc::new(log.borrow().clone());
    assert!(log.acts.iter().all(|a| !a.is_empty()), "both corners acted");
    assert_eq!(play(replay(log), 7, 120.0), played);
    Ok(())
}

#[test]
fn an_act_out_of_range_is_a_wait() {
    let wait = play([fixed_bot(0), brain()], 3, 30.0);
    for act in [-1, 30, i32::MAX] {
        assert_eq!(play([fixed_bot(act), brain()], 3, 30.0), wait, "act {act}");
    }
}

/// Answers 0 twice, then runs out of fuel, counting every call.
struct Failing(Rc<Cell<u32>>);

impl Decider for Failing {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        Ok(())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Ok(())
    }
    fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        self.0.set(self.0.get() + 1);
        if self.0.get() == 3 {
            Err(Fault::OutOfFuel)
        } else {
            Ok(0)
        }
    }
}

#[test]
fn a_fault_stops_the_seat() {
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    let mut duel = Duel::new(
        config(1, all_effects()),
        [bot(move || Box::new(Failing(c.clone()))), brain()],
    );
    assert_eq!(duel.seat(0).fault(), None);
    run(&mut duel, 30.0);
    assert_eq!(duel.seat(0).fault(), Some(&Fault::OutOfFuel));
    assert_eq!(calls.get(), 3, "decide is not called after the fault");
}

/// Logs `start` seeds and counts `round`s.
struct Counting {
    starts: Rc<RefCell<Vec<u32>>>,
    rounds: Rc<Cell<u32>>,
}

impl Decider for Counting {
    fn start(&mut self, seed: u32) -> Result<(), Fault> {
        self.starts.borrow_mut().push(seed);
        Ok(())
    }
    fn round(&mut self) -> Result<(), Fault> {
        self.rounds.set(self.rounds.get() + 1);
        Ok(())
    }
    fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        Ok(0)
    }
}

#[test]
fn start_then_round() {
    let starts = Rc::new(RefCell::new(Vec::new()));
    let rounds = Rc::new(Cell::new(0));
    let (s, r) = (starts.clone(), rounds.clone());
    let mut duel = Duel::new(
        config(9, all_effects()),
        [
            bot(move || {
                Box::new(Counting {
                    starts: s.clone(),
                    rounds: r.clone(),
                })
            }),
            brain(),
        ],
    );
    // two idle-ish brains finish rounds well inside 300 s
    run(&mut duel, 300.0);
    assert!(duel.state.round >= 2, "the fight reached a later round");
    assert_eq!(*starts.borrow(), vec![9_u32.wrapping_mul(2) ^ 0x9e37_79b9]);
    assert_eq!(u64::from(rounds.get()), u64::from(duel.state.round) - 1);
}

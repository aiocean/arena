//! A bot's seat: a seed replays the same fight, one instance plays a whole match, and a fault gives up its round.

mod common;

use arena_mind::policy::{ACTIONS, OBS};
use common::{all_effects, config, pick};
use arena_duel::duel::{Duel, TICK};
use arena_duel::ladder::{Bot, Match, bout};
use arena_duel::seat::{BotSeat, Brain, Decider, Fault, MakeSeat, SharedDecider};
use std::cell::RefCell;
use std::rc::Rc;

/// A bot that faults where told: in `start`, in the `round`s listed (1 is the first), and in every decision.
#[derive(Default)]
struct Faulty {
    start: bool,
    rounds: Vec<u32>,
    decide: bool,
    round: u32,
}

impl Decider for Faulty {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        if self.start {
            Err(Fault("start".into()))
        } else {
            Ok(())
        }
    }
    fn round(&mut self) -> Result<(), Fault> {
        self.round += 1;
        if self.rounds.contains(&self.round) {
            Err(Fault("round".into()))
        } else {
            Ok(())
        }
    }
    fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
        if self.decide {
            Err(Fault("decide".into()))
        } else {
            Ok(0)
        }
    }
}

fn shared(d: impl Decider + 'static) -> SharedDecider {
    Rc::new(RefCell::new(d))
}

/// Runs a duel for `seconds` and returns each tick's lives and wins.
fn trace(seats: [MakeSeat; 2], seed: u32, seconds: f64) -> Vec<([f64; 2], [u32; 2])> {
    let mut duel = Duel::new(config(seed, all_effects()), seats);
    let mut out = Vec::new();
    while duel.state().time < seconds {
        duel.step(TICK);
        out.push((
            duel.state().fighters.each_ref().map(|f| f.hp()),
            duel.state().wins,
        ));
    }
    out
}

#[test]
fn a_bot_seat_replays_the_same_fight_from_a_seed() {
    for seed in [3, 18] {
        let fight = || trace([BotSeat::make(pick(), None), Brain::make()], seed, 60.0);
        let first = fight();
        assert_eq!(first, fight(), "seed {seed}");
        assert!(
            first.iter().any(|(hp, _)| hp[1] < 100.0),
            "seed {seed}: it fights"
        );
    }
}

/// The ticks until each of the first `rounds` rounds was won, and by whom.
fn rounds(bot: Faulty, rounds: u32) -> Vec<usize> {
    let mut duel = Duel::new(
        config(5, all_effects()),
        [BotSeat::make(shared(bot), None), Brain::make()],
    );
    let mut winners = Vec::new();
    let mut last = [0, 0];
    while duel.state().wins[0] + duel.state().wins[1] < rounds && duel.state().time < 600.0 {
        duel.step(TICK);
        if duel.state().wins != last {
            winners.push(usize::from(duel.state().wins[1] != last[1]));
            last = duel.state().wins;
        }
    }
    winners
}

#[test]
fn a_bot_that_faults_every_decision_loses_every_round_at_once() {
    let mut duel = Duel::new(
        config(5, all_effects()),
        [
            BotSeat::make(
                shared(Faulty {
                    decide: true,
                    ..Faulty::default()
                }),
                None,
            ),
            Brain::make(),
        ],
    );
    let mut first = None;
    while duel.state().wins[1] < 3 && duel.state().time < 60.0 {
        duel.step(TICK);
        if first.is_none() && duel.state().wins[1] == 1 {
            first = Some(duel.state().time);
        }
    }
    assert_eq!(duel.state().wins, [0, 3]);
    // its first decision comes 0.4-0.7 s into the round
    assert!(first.is_some_and(|t| t < 1.0), "{first:?}");
}

#[test]
fn a_fault_in_start_gives_up_only_the_first_round() {
    let winners = rounds(
        Faulty {
            start: true,
            ..Faulty::default()
        },
        1,
    );
    assert_eq!(winners, [1]);
    let mut duel = Duel::new(
        config(5, all_effects()),
        [
            BotSeat::make(
                shared(Faulty {
                    start: true,
                    ..Faulty::default()
                }),
                None,
            ),
            Brain::make(),
        ],
    );
    duel.step(TICK);
    assert_eq!(duel.state().wins, [0, 1], "lost on the first tick");
    while duel.state().round < 2 {
        duel.step(TICK);
    }
    assert!(!duel.seat(0).forfeit(), "the second round is played");
}

#[test]
fn a_fault_in_round_gives_up_that_round() {
    let mut duel = Duel::new(
        config(5, all_effects()),
        [
            BotSeat::make(
                shared(Faulty {
                    rounds: vec![2],
                    ..Faulty::default()
                }),
                None,
            ),
            Brain::make(),
        ],
    );
    assert!(!duel.seat(0).forfeit());
    while duel.state().round < 2 {
        duel.step(TICK);
    }
    let before = duel.state().wins;
    duel.step(TICK);
    assert_eq!(duel.state().wins[1], before[1] + 1);
}

#[test]
fn a_module_bot_keeps_one_instance_for_a_whole_match() {
    let made = Rc::new(RefCell::new(0));
    let starts = Rc::new(RefCell::new(0));
    struct Counting(Rc<RefCell<u32>>);
    impl Decider for Counting {
        fn start(&mut self, _: u32) -> Result<(), Fault> {
            *self.0.borrow_mut() += 1;
            Ok(())
        }
        fn round(&mut self) -> Result<(), Fault> {
            Ok(())
        }
        fn decide(&mut self, _: &[f32; OBS], _: &[bool; ACTIONS], _: f32) -> Result<i32, Fault> {
            Ok(0)
        }
    }
    let (m, s) = (made.clone(), starts.clone());
    let bot = Bot::Decider(Rc::new(move || {
        *m.borrow_mut() += 1;
        shared(Counting(s.clone()))
    }));
    let quick = Match {
        seeds: 1..3,
        rounds: 1,
        limit: 5.0,
        effects: all_effects(),
    };
    bout(&bot, &Bot::Turtle, &quick);
    assert_eq!(*made.borrow(), 1, "one instance for the match");
    assert_eq!(
        *starts.borrow(),
        4,
        "started once a fight: 2 seeds, both ways"
    );
}

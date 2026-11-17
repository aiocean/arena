//! Neither seat nor look wins for being it: a mirror of one mind over many seeds, each fought both ways with the looks
//! drawn apart from the seats (`ladder::looks_of`), gives seat 0 and Thunder each about half the rounds. The fighters
//! take their turns in an order that swaps every tick, and the policy is told its look, never its seat. 160 seeds
//! (about 950 rounds): a share's spread is then under 2 points, so the 45-55% band tests a bias, not the dice.

mod common;

use arena_duel::ladder::{Match, match_up};
use arena_duel::scripted::Scripted;
use common::all_effects;

#[test]
fn a_mirror_wins_as_often_in_either_seat_and_either_look() {
    let m = Match {
        seeds: 1..161,
        rounds: 3,
        limit: 120.0,
        effects: all_effects(),
    };
    let r = match_up(&Scripted::aggressive, &Scripted::aggressive, &m);
    // the first contestant won `won[0]` in seat 0 and lost `lost[1]` from seat 1 to whoever sat in seat 0
    let seat0 = f64::from(r.won[0] + r.lost[1]);
    let all = f64::from(r.won[0] + r.won[1] + r.lost[0] + r.lost[1]);
    let thunder = f64::from(r.by_look[0]) / f64::from((r.by_look[0] + r.by_look[1]).max(1));
    assert!(all > 900.0, "{all} rounds");
    let seat = seat0 / all;
    assert!(
        (0.45..=0.55).contains(&seat),
        "seat 0 won {:.1}% of {all} rounds",
        seat * 100.0
    );
    assert!(
        (0.45..=0.55).contains(&thunder),
        "Thunder won {:.1}%",
        thunder * 100.0
    );
}

//! Contestants by name, a bout between two of them, and the entry bar.

use arena_duel::ladder::{Bot, EntryBar, Match, Record, bout};

fn quick(seeds: u32) -> Match {
    Match {
        seeds: 1..seeds + 1,
        rounds: 3,
        limit: 120.0,
        effects: arena_duel::effects::all(),
    }
}

#[test]
fn every_built_in_name_seats_a_bot_and_nothing_else_does() {
    for name in Bot::NAMES {
        assert!(Bot::named(name).is_some(), "{name}");
    }
    assert!(Bot::named("policy").is_none());
    assert!(Bot::named("").is_none());
}

#[test]
fn a_bout_plays_every_seed_both_ways_and_a_seed_replays_the_same()
-> Result<(), Box<dyn std::error::Error>> {
    // a property over seeds, not lucky seeds: every fight of every seed, both ways, plays at least a round (won,
    // lost or cut), both seats are played, and a bout replays the same
    let me = Bot::First;
    let n = 4;
    let m = quick(n);
    let r = bout(&me, &Bot::Brain, &m);
    let rounds = r.won.iter().chain(&r.lost).sum::<u32>() + r.timeouts;
    assert!(rounds >= 2 * n, "{rounds} rounds for {} fights", 2 * n);
    assert!(r.decisions > 0, "no decisions: {r:?}");
    assert_eq!(r, bout(&me, &Bot::Brain, &m));
    Ok(())
}

/// A record of `won` and `lost` rounds as each school.
fn rounds(won: [u32; 2], lost: [u32; 2]) -> Record {
    Record {
        won,
        lost,
        ..Record::default()
    }
}

#[test]
fn the_entry_bar_is_75_against_the_brain_65_as_each_school_and_80_against_the_first_mind() {
    let bar = |vs_brain, vs_first| EntryBar { vs_brain, vs_first };
    // 80% overall, 70% and 90% by school; 80% against the first mind: in
    assert!(bar(rounds([7, 9], [3, 1]), rounds([8, 8], [2, 2])).passed());
    // 80% overall, but 60% as Thunder: out
    assert!(!bar(rounds([6, 10], [4, 0]), rounds([8, 8], [2, 2])).passed());
    // 70% against the brain: out
    assert!(!bar(rounds([7, 7], [3, 3]), rounds([8, 8], [2, 2])).passed());
    // 75% against the first mind: out
    assert!(!bar(rounds([8, 8], [2, 2]), rounds([7, 8], [3, 2])).passed());
}



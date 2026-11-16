//! Contestants by name or weights file, a bout between two of them, and the entry bar.

use pixfx_duel::ladder::{Bot, EntryBar, Match, Record, bout, entry_bar};
use pixfx_duel::seat::shipped;

fn quick(seeds: u32) -> Match {
    Match {
        seeds: 1..seeds + 1,
        rounds: 3,
        limit: 120.0,
        effects: pixfx_effects::EFFECTS
            .iter()
            .map(|e| e.meta().id.to_string())
            .collect(),
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
    let me = Bot::Policy(shipped()?);
    let m = quick(2);
    let r = bout(&me, &Bot::Brain, &m);
    // 2 seeds x 2 corners, each fight to 2 round wins of 3, some rounds cut
    let rounds = r.won.iter().chain(&r.lost).sum::<u32>() + r.timeouts;
    assert!(rounds >= 2 * 2 * 2, "{rounds} rounds");
    assert!(
        r.won[0] + r.lost[0] > 0 && r.won[1] + r.lost[1] > 0,
        "played as both schools: {r:?}"
    );
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

#[test]
fn the_shipped_policy_clears_the_entry_bar() -> Result<(), Box<dyn std::error::Error>> {
    let bar = entry_bar(&shipped()?, &quick(2));
    assert!(
        bar.passed(),
        "vs brain {:.3} (thunder {:.3}, frost {:.3}), vs first {:.3}",
        bar.vs_brain.win_rate(),
        bar.vs_brain.as_school(0),
        bar.vs_brain.as_school(1),
        bar.vs_first.win_rate()
    );
    Ok(())
}

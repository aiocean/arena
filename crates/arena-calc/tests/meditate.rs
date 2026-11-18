//! Meditation's extra qi comes in blocks of sitting: nothing before the first full second, then the blocks in it at
//! once, then one block per further half second; the normal regen runs on all along.

use arena_calc::cultivator::{BASE, Blocks, Cultivator, Sitting};

const BLOCKS: Blocks = Blocks {
    block: 0.5,
    first: 1.0,
};
const DT: f64 = 1.0 / 60.0;

/// Qi a fighter with room for it gains over `secs` seconds of sitting, in ticks of `DT`.
fn sat_for(secs: f64) -> f64 {
    let mut c = Cultivator::new();
    c.qi_max = 10_000.0;
    c.qi = 0.0;
    c.sitting = Some(Sitting::new(BLOCKS));
    let ticks = (secs / DT + 0.5) as u32;
    for _ in 0..ticks {
        c.tick(DT, false);
    }
    c.qi
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn block() -> f64 {
    (BASE.meditate_regen - BASE.qi_regen) * BLOCKS.block
}

#[test]
fn nothing_extra_before_the_first_second() {
    for secs in [0.1, 0.5, 0.9, 59.0 / 60.0] {
        let qi = sat_for(secs);
        assert!(close(qi, BASE.qi_regen * secs), "{secs}: {qi}");
    }
}

#[test]
fn the_first_second_pays_two_blocks_at_once() {
    let qi = sat_for(1.0);
    assert!(close(qi, BASE.qi_regen + 2.0 * block()), "{qi}");
    // a block still filling pays nothing
    let qi = sat_for(1.4);
    assert!(close(qi, BASE.qi_regen * 1.4 + 2.0 * block()), "{qi}");
}

#[test]
fn then_a_block_each_half_second() {
    let qi = sat_for(1.5);
    assert!(close(qi, BASE.qi_regen * 1.5 + 3.0 * block()), "{qi}");
    let qi = sat_for(3.0);
    assert!(close(qi, BASE.qi_regen * 3.0 + 6.0 * block()), "{qi}");
}

#[test]
fn standing_up_loses_the_block_in_progress_and_starts_over() {
    let mut c = Cultivator::new();
    c.qi_max = 10_000.0;
    c.qi = 0.0;
    c.sitting = Some(Sitting::new(BLOCKS));
    for _ in 0..84 {
        c.tick(DT, false);
    }
    c.sitting = None;
    let stood = c.qi;
    assert!(close(stood, BASE.qi_regen * 1.4 + 2.0 * block()), "{stood}");
    // sitting again counts from nothing
    c.sitting = Some(Sitting::new(BLOCKS));
    for _ in 0..30 {
        c.tick(DT, false);
    }
    assert!(close(c.qi, stood + BASE.qi_regen * 0.5), "{}", c.qi);
}

#[test]
fn choked_it_gains_nothing_and_the_block_is_lost() {
    let mut c = Cultivator::new();
    c.qi_max = 10_000.0;
    c.qi = 0.0;
    c.sitting = Some(Sitting::new(BLOCKS));
    c.choke = 5.0;
    for _ in 0..60 {
        c.tick(DT, false);
    }
    assert_eq!(c.qi, 0.0);
    assert_eq!(c.sitting.map(|s| s.paid), Some(2));
}

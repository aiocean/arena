//! The brain with Cổ Thần Quyết (`Keystone::AncientGod`: a qi bar of 10 000, no recovery skills) fills its qi to what
//! a fighter usually holds and then fights, instead of meditating out of reach toward a share of a bar it would take
//! minutes to fill.

mod common;

use arena_calc::keystone::Keystone;
use arena_duel::duel::{Duel, TICK, loadouts};
use arena_duel::pose::Pose;
use arena_duel::seat::Brain;
use arena_duel::view::view;
use common::{all_effects, config};

#[test]
fn a_brain_with_a_bottomless_qi_bar_fights() {
    // the first seeds whose draw gives a fighter Cổ Thần Quyết
    let found: Vec<(u32, usize)> = (1..2000)
        .filter_map(|seed| {
            let l = loadouts(seed);
            (0..2)
                .find(|&i| l[i].keystones.contains(&Keystone::AncientGod))
                .map(|i| (seed, i))
        })
        .take(4)
        .collect();
    assert!(!found.is_empty(), "no seed draws Cổ Thần Quyết");
    for (seed, me) in found {
        let mut duel = Duel::new(config(seed, all_effects()), [Brain::make(), Brain::make()]);
        let (mut ticks, mut meditating) = (0_u32, 0_u32);
        while duel.state().time < 60.0 && duel.state().winner.is_none() {
            duel.step(TICK);
            ticks += 1;
            if view(duel.state()).fighters[me].pose == Pose::Meditate {
                meditating += 1;
            }
        }
        let share = f64::from(meditating) / f64::from(ticks.max(1));
        assert!(
            share < 0.5,
            "seed {seed}: the Cổ Thần Quyết fighter (seat {me}) meditated {:.0}% of {:.0} s",
            share * 100.0,
            f64::from(ticks) * TICK
        );
    }
}

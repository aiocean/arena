//! The rotating bar: three slots drawn from the loadout's actives cast by hand, spent by a cast, refilled after a
//! wait, never a skill twice; the basics and the ultimate always at hand. No tiers, traits, luck or evolving pairs.

mod common;

use common::{arena, common as held, on_bar, pinned};
use pixfx_duel::bar::{draw, skill_bar, spend};
use pixfx_duel::data::{BAR, RAGE};
use pixfx_duel::fighter::Slot;
use pixfx_duel::kit::Place;
use serde_json::json;

#[test]
fn a_fresh_round_draws_three_different_skills_from_the_loadouts_actives() {
    for seed in 1..=30 {
        let s = arena(seed);
        for f in &s.fighters {
            let bar = on_bar(f);
            assert_eq!(bar.len(), BAR.slots, "seed {seed}");
            for name in &bar {
                assert!(f.pool.contains(name), "seed {seed}: {name}");
                assert_eq!(bar.iter().filter(|n| *n == name).count(), 1);
            }
        }
    }
}

#[test]
fn the_pool_is_the_actives_cast_by_hand() {
    for seed in 1..=30 {
        let s = arena(seed);
        for f in &s.fighters {
            let hand: Vec<&str> = f
                .kit
                .skills
                .iter()
                .filter(|k| matches!(k.place, Place::Active(_)) && !k.triggered)
                .map(|k| k.gem.id)
                .collect();
            assert_eq!(f.pool, hand);
            assert!(f.pool.len() <= arena_calc::loadout::ACTIVES);
        }
    }
}

#[test]
fn a_spent_skill_empties_its_slot_for_1_5_to_2_5_s() {
    for seed in 1..=20 {
        let mut s = arena(seed);
        spend(&mut s, 0, 1);
        let Slot::Empty { left, full } = s.fighters[0].bar[1] else {
            unreachable!("a spent slot is empty")
        };
        assert_eq!(left, full);
        assert!((BAR.wait..=BAR.wait + BAR.spread).contains(&left));
    }
}

#[test]
fn a_draw_never_repeats_a_skill_on_the_bar_nor_brings_back_the_one_that_left() {
    for seed in 1..=30 {
        let mut s = arena(seed);
        for _ in 0..20 {
            let gone = s.fighters[0].bar[2].spell();
            draw(&mut s, 0, 2, gone);
            let bar = on_bar(&s.fighters[0]);
            let mut dedup = bar.clone();
            dedup.dedup();
            assert_eq!(bar.len(), dedup.len());
            if s.fighters[0].pool.len() > BAR.slots {
                assert_ne!(s.fighters[0].bar[2].spell(), gone);
            }
        }
    }
}

#[test]
fn with_no_skill_left_to_draw_a_slot_waits() {
    let mut s = arena(1);
    s.fighters[0].pool = vec!["bolt", "palm"];
    s.fighters[0].bar = vec![held("bolt"), held("palm"), held("palm")];
    draw(&mut s, 0, 2, None);
    assert!(matches!(s.fighters[0].bar[2], Slot::Empty { .. }));
}

#[test]
fn the_skill_bar_shows_the_ultimate_last_charged_by_rage_and_uses_stored() {
    let mut s = arena(3);
    let f = &mut s.fighters[0];
    let bar = skill_bar(f);
    assert_eq!(bar.len(), 4);
    assert!(bar[3].ultimate);
    assert_eq!(bar[3].spell, Some(f.loadout.ultimate.as_str()));
    f.rage = RAGE / 2.0;
    assert_eq!(skill_bar(f)[3].charge, 0.5);
    for v in &skill_bar(f)[..3] {
        assert!(v.max_uses >= 1 && v.uses == v.max_uses, "{v:?}");
        assert!(v.left.is_finite() && v.full.is_finite());
    }
}

#[test]
fn loadouts_and_opening_bars_are_pinned_for_each_seed() {
    let got: Vec<_> = (1..=20)
        .map(|seed| {
            let s = arena(seed);
            json!({
                "seed": seed,
                "fighters": s.fighters.iter().map(|f| json!({
                    "actives": f.loadout.actives.iter().map(|a| json!({
                        "skill": a.skill, "supports": a.supports, "fuse": a.fuse,
                    })).collect::<Vec<_>>(),
                    "ultimate": f.loadout.ultimate,
                    "keystones": f.loadout.keystones.iter().map(|k| k.id()).collect::<Vec<_>>(),
                    "root": f.loadout.root.map(|r| r.name()),
                    "bar": on_bar(f),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    pinned("arenas", &json!(got));
}

//! Each fighter's style: drawn from the seed on dice of its own, so the rules' dice and fights stay as they were.

mod common;

use arena_duel::style::Style;
use arena_duel::view::view;
use common::fight;

#[test]
fn styles_are_drawn_from_the_seed() {
    let styles = |seed| fight(seed).state().fighters.clone().map(|f| f.style);
    assert_eq!(styles(4), styles(4));
    let all: Vec<Style> = (1..=40).flat_map(styles).collect();
    for s in Style::ALL {
        assert!(all.contains(&s), "{s:?} drawn in 40 seeds");
    }
    let differs = (1..=40).filter(|&s| styles(s)[0] != styles(s)[1]).count();
    assert!(differs > 10, "the two corners draw on their own: {differs}");
}

#[test]
fn a_style_lasts_the_whole_fight_and_shows_in_the_view() {
    // the first of ten seeds whose first round ends within two minutes (how long a round lasts is not what this is about)
    let ended = (1..=10).find_map(|seed| {
        let mut duel = fight(seed);
        let before = duel.state().fighters.clone().map(|f| f.style);
        while duel.state().round == 1 && duel.state().time < 120.0 {
            duel.step(1.0 / 60.0);
        }
        (duel.state().round > 1).then_some((duel, before))
    });
    let Some((duel, before)) = ended else {
        unreachable!("no round ended in ten seeds")
    };
    assert_eq!(duel.state().fighters.clone().map(|f| f.style), before);
    assert_eq!(view(duel.state()).fighters.map(|f| f.style), before);
}

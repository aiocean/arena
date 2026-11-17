//! A blow plays out before the next: while a fighter's bolt flies, its seeker circles or its field turns, it casts no
//! other skill that binds (`Gem::binds`); a curse, an aura, a blessing, a move, a guard, a heal or a stance it casts
//! whenever it is free, and a unit's blows never hold it.

mod common;

use arena_duel::act::begin;
use arena_duel::bar::ready;
use arena_duel::duel::{Duel, TICK};
use common::{common, duel_of, idle, loadout};

/// Fighter 0 holds `bar` on its bar, rich enough for anything; nobody's seat acts.
fn duel(bar: [&'static str; 3]) -> Duel {
    let mut d = duel_of(
        1,
        [idle(), idle()],
        [loadout(&bar, "tribulation"), loadout(&[], "tribulation")],
    );
    let s = d.state_mut();
    s.fighters[0].bar = bar.iter().map(|&n| common(n)).collect();
    s.fighters[0].c.qi = 1e6;
    s.fighters[0].c.souls = 30.0;
    d
}

/// Steps until fighter 0's cast is over.
fn until_free(d: &mut Duel) {
    for _ in 0..600 {
        if d.state().fighters[0].act.is_none() {
            return;
        }
        d.step(TICK);
    }
    assert!(d.state().fighters[0].act.is_none(), "the cast never ended");
}

#[test]
fn a_fighter_whose_orb_still_circles_casts_no_other_blow_but_heals() {
    let mut d = duel(["orb", "bolt", "heal"]);
    assert!(ready(&d.state().fighters[0], "bolt"));
    begin(d.state_mut(), 0, "orb", None);
    until_free(&mut d);
    let f = &d.state().fighters[0];
    assert!(d.state().shots.iter().any(|s| s.owner == 0 && s.binds));
    assert!(f.bound);
    assert!(!ready(f, "bolt"));
    assert!(ready(f, "heal"));
    // a cast asked for anyway is not begun
    begin(d.state_mut(), 0, "bolt", None);
    assert!(d.state().fighters[0].act.is_none());
}

#[test]
fn once_its_last_blow_is_over_a_fighter_strikes_again() {
    let mut d = duel(["orb", "bolt", "heal"]);
    begin(d.state_mut(), 0, "orb", None);
    until_free(&mut d);
    let mut t = 0.0;
    while d.state().shots.iter().any(|s| s.owner == 0) {
        d.step(TICK);
        t += TICK;
        assert!(t < 30.0, "the orb never ended");
    }
    let f = &d.state().fighters[0];
    assert!(!f.bound);
    assert!(ready(f, "bolt"));
}

#[test]
fn a_heal_binds_nobody() {
    let mut d = duel(["heal", "bolt", "orb"]);
    begin(d.state_mut(), 0, "heal", None);
    until_free(&mut d);
    let f = &d.state().fighters[0];
    assert!(!f.bound);
    assert!(ready(f, "bolt"));
}

#[test]
fn a_totems_blows_never_hold_its_owner() {
    let mut d = duel(["arsenal", "bolt", "heal"]);
    d.state_mut().fighters[1].body.x = d.state().fighters[0].body.x + 400.0;
    begin(d.state_mut(), 0, "arsenal", None);
    until_free(&mut d);
    let mut shot = false;
    for _ in 0..600 {
        d.step(TICK);
        let s = d.state();
        if s.shots.iter().any(|s| s.payload.unit.is_some()) {
            shot = true;
            assert!(s.shots.iter().all(|s| s.payload.unit.is_none() || !s.binds));
            assert!(!s.fighters[0].bound);
        }
    }
    assert!(shot, "the totem never cast");
}

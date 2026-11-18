//! Traps, mines, totems and minions on the field, and projectiles among several bodies.

use arena_calc::ailment::Ailment;
use arena_calc::device::{Kind, MINE_ARM, TRAP_ARM};
use arena_calc::dot::Dot;
use arena_calc::gem::GEMS;
use arena_calc::loadout::{self, BASICS, Loadout, Slot};
use arena_calc::stat::Happened;
use arena_sim::fight::Fight;
use arena_sim::{DT, Rng};

/// A legal loadout holding `first` (skill, supports), filled with the cheapest plain damaging skills.
fn holding(first: &[(&str, &[&str])]) -> Loadout {
    let mut actives: Vec<Slot> = first
        .iter()
        .map(|(id, sup)| Slot {
            skill: id.to_string(),
            supports: sup.iter().map(|s| s.to_string()).collect(),
            fuse: None,
        })
        .collect();
    let mut fill: Vec<_> = GEMS
        .iter()
        .filter(|g| {
            g.deals_damage()
                && !BASICS.contains(&g.id)
                && g.uses.is_none()
                && !first.iter().any(|f| f.0 == g.id)
        })
        .collect();
    fill.sort_by_key(|g| (g.tier.points(), g.id));
    for g in fill.into_iter().take(8 - actives.len()) {
        actives.push(Slot {
            skill: g.id.to_string(),
            supports: Vec::new(),
            fuse: None,
        });
    }
    let l = Loadout {
        actives,
        keystones: Vec::new(),
        root: None,
    };
    assert_eq!(loadout::check(&l), Ok(()));
    l
}

fn random(seed: u64) -> Loadout {
    let mut r = Rng::salted(seed, 42);
    loadout::random(&mut || r.next_f64()).unwrap_or_default()
}

fn slot_of(f: &Fight, me: usize, id: &str) -> usize {
    f.f[me]
        .slots
        .iter()
        .position(|s| s.gem.id == id)
        .unwrap_or(usize::MAX)
}

/// Both fighters held still (the brains never act), seat 0 at `a`, seat 1 at `b`.
fn still(f: &mut Fight, a: f64, b: f64) {
    f.f[0].x = a;
    f.f[1].x = b;
    f.f[0].stun = 1e9;
    f.f[1].stun = 1e9;
}

/// Seat `me` casts `id` now, then is held still again.
fn cast(f: &mut Fight, me: usize, id: &str) {
    let slot = slot_of(f, me, id);
    f.f[me].stun = 0.0;
    assert!(f.begin_cast(me, slot, 1.0), "{id} did not start");
    f.f[me].stun = 1e9;
}

#[test]
fn a_totem_casts_on_cadence_and_its_damage_lands() {
    // Pháp Đàn on Lôi: a totem casting every 1.3 s (the bolt's cooldown) for 8 s
    let mut f = Fight::new(1, &holding(&[("bolt", &["totem"])]), &random(2), false);
    still(&mut f, 3.0, 5.5);
    cast(&mut f, 0, "bolt");
    let slot = slot_of(&f, 0, "bolt");
    let mut times = Vec::new();
    let mut last = 0;
    let mut stood = false;
    for _ in 0..(6.0 / DT) as usize {
        f.step();
        stood |= f.units_of(0, Kind::Totem).count() == 1;
        let n = f.f[0].report.skills[slot].device_blows;
        if n > last {
            times.push(f.time);
            last = n;
        }
    }
    assert!(stood);
    assert!(times.len() >= 3, "{times:?}");
    assert!(f.f[0].report.skills[slot].damage > 0.0);
    // the same gap between every two casts, to a tick
    let gaps: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    for g in &gaps {
        assert!((g - gaps[0]).abs() <= DT + 1e-9, "{gaps:?}");
        assert!(*g > 0.1);
    }
}

#[test]
fn a_trap_springs_only_after_arming() {
    let mut f = Fight::new(3, &holding(&[("plumblossom", &[])]), &random(4), false);
    still(&mut f, 3.0, 5.0);
    cast(&mut f, 0, "plumblossom");
    let slot = slot_of(&f, 0, "plumblossom");
    // the trap lands on the foe
    let mut placed = None;
    for _ in 0..120 {
        f.step();
        if f.units_of(0, Kind::Trap).count() == 1 {
            placed = Some(f.time);
            break;
        }
    }
    assert!(placed.is_some(), "the trap was never laid");
    let t0 = placed.unwrap_or_default();
    let life = f.c[1].life + f.c[1].shield;
    while f.time < t0 + TRAP_ARM - 2.0 * DT {
        f.step();
        assert_eq!(
            f.f[0].report.skills[slot].device_blows, 0,
            "sprang at {}",
            f.time
        );
        assert_eq!(f.units_of(0, Kind::Trap).count(), 1);
    }
    for _ in 0..6 {
        f.step();
    }
    assert_eq!(f.units_of(0, Kind::Trap).count(), 0);
    assert_eq!(f.f[0].report.skills[slot].device_blows, 1);
    assert!(f.c[1].life + f.c[1].shield < life);
}

#[test]
fn mines_detonate_when_the_foe_is_near() {
    let mut f = Fight::new(5, &holding(&[("fireball", &["mine"])]), &random(6), false);
    still(&mut f, 3.0, 6.0);
    cast(&mut f, 0, "fireball");
    let slot = slot_of(&f, 0, "fireball");
    let mut laid = false;
    for _ in 0..((1.0 + MINE_ARM) / DT) as usize {
        f.step();
        laid |= f.units_of(0, Kind::Mine).count() == 1;
    }
    assert!(laid);
    assert_eq!(f.units_of(0, Kind::Mine).count(), 0, "the mine went off");
    assert!(f.f[0].report.skills[slot].device_blows >= 1);
    assert!(f.f[0].report.skills[slot].damage > 0.0);
}

#[test]
fn a_minion_closes_in_and_hits() {
    let mut f = Fight::new(7, &holding(&[("ghostsummon", &[])]), &random(8), false);
    still(&mut f, 2.0, 10.0);
    cast(&mut f, 0, "ghostsummon");
    let slot = slot_of(&f, 0, "ghostsummon");
    let mut nearest = f64::INFINITY;
    for _ in 0..(6.0 / DT) as usize {
        f.step();
        for u in f.units_of(0, Kind::Minion) {
            nearest = nearest.min((u.x - f.f[1].x).abs());
        }
    }
    assert!(nearest <= 1.0, "the minion stayed {nearest} T away");
    assert!(f.f[0].report.skills[slot].device_blows >= 2);
    assert!(f.f[0].report.skills[slot].damage > 0.0);
}

#[test]
fn a_minion_dying_fires_minion_died() {
    let mut f = Fight::new(9, &holding(&[("ghostsummon", &[])]), &random(10), false);
    still(&mut f, 2.0, 10.0);
    cast(&mut f, 0, "ghostsummon");
    let mut seen = false;
    for _ in 0..120 {
        f.step();
        if let Some(u) = f.units.iter_mut().find(|u| u.kind == Kind::Minion) {
            seen = true;
            u.c.life = 1.0;
            u.c.ailments.lay(Dot::new(Ailment::Burn, 1000.0, 2.0));
        }
        if seen && f.units.is_empty() {
            break;
        }
    }
    assert!(seen && f.units.is_empty());
    for _ in 0..2 {
        f.step();
    }
    assert!(f.c[0].recent.since(Happened::MinionDied) < 0.2);
}

#[test]
fn a_chain_jumps_from_the_foe_to_its_minion() {
    let mut f = Fight::new(
        11,
        &holding(&[("bolt", &["chain"])]),
        &holding(&[("ghostsummon", &[])]),
        false,
    );
    still(&mut f, 3.0, 7.0);
    cast(&mut f, 1, "ghostsummon");
    // the minion comes out, then is held behind its owner, away from the shot
    for _ in 0..60 {
        f.step();
        for u in f.units.iter_mut() {
            u.x = 9.0;
        }
    }
    assert_eq!(f.units_of(1, Kind::Minion).count(), 1);
    let full = f.units[0].c.life;
    let foe = f.c[1].life + f.c[1].shield;
    cast(&mut f, 0, "bolt");
    let slot = slot_of(&f, 0, "bolt");
    for _ in 0..90 {
        f.step();
        for u in f.units.iter_mut() {
            u.x = 9.0;
        }
    }
    assert!(
        f.c[1].life + f.c[1].shield < foe,
        "the foe was struck first"
    );
    assert!(
        f.units.first().is_none_or(|u| u.c.life < full),
        "the chain went on to the minion"
    );
    assert!(f.f[0].report.skills[slot].unit_damage > 0.0);
}

/// Lôi Bạo Trễ as the duel has it: a projectile a ward stops stays as its caster's mine, which goes off on the foe
/// standing in it; with no ward the bolt lands as before and leaves nothing.
#[test]
fn a_warded_delayed_thunder_bolt_stays_as_a_mine_that_goes_off() {
    let run = |supports: &'static [&'static str], ward: u8| {
        let mut f = Fight::new(5, &holding(&[("fireball", supports)]), &random(6), false);
        still(&mut f, 3.0, 6.0);
        f.f[1].ward = ward;
        f.f[1].ward_left = 10.0;
        cast(&mut f, 0, "fireball");
        let slot = slot_of(&f, 0, "fireball");
        let mut laid = 0;
        for _ in 0..(2.0 / DT) as usize {
            f.step();
            laid = laid.max(f.units_of(0, Kind::Mine).count());
        }
        let line = &f.f[0].report.skills[slot];
        (
            laid,
            line.device_blows,
            line.damage,
            f.units_of(0, Kind::Mine).count(),
        )
    };
    let (laid, blows, damage, left) = run(&["delayedthunder"], 1);
    assert_eq!(laid, 1, "the warded bolt stays as a mine");
    assert_eq!(left, 0, "and goes off");
    assert!(blows >= 1 && damage > 0.0);
    let (laid, blows, damage, _) = run(&[], 1);
    assert_eq!(
        (laid, blows),
        (0, 0),
        "a plain bolt into a ward leaves nothing"
    );
    assert_eq!(damage, 0.0);
    let (laid, _, damage, _) = run(&["delayedthunder"], 0);
    assert_eq!(laid, 0, "an unstopped bolt leaves nothing");
    assert!(damage > 0.0);
}

/// A projectile spent on a minion stays as a mine where the minion stood.
#[test]
fn a_delayed_thunder_bolt_spent_on_a_minion_stays_as_a_mine() {
    let mut f = Fight::new(
        5,
        &holding(&[("fireball", &["delayedthunder"])]),
        &holding(&[("ghostsummon", &[])]),
        false,
    );
    still(&mut f, 3.0, 9.0);
    cast(&mut f, 1, "ghostsummon");
    while f.units_of(1, Kind::Minion).count() == 0 {
        f.step();
    }
    cast(&mut f, 0, "fireball");
    let mut at = None;
    for _ in 0..(1.0 / DT) as usize {
        let minion = f.units_of(1, Kind::Minion).next().map(|u| u.x);
        f.step();
        if at.is_none()
            && let Some(m) = f.units_of(0, Kind::Mine).next()
        {
            at = Some((m.x, minion));
        }
    }
    let (mine, minion) = at.unwrap_or_else(|| unreachable!("the bolt stays as a mine"));
    assert!(
        minion.is_some_and(|x| (x - mine).abs() < 0.5),
        "{mine} {minion:?}"
    );
}

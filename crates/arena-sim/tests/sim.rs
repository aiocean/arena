//! The simulator: determinism, ending, reach, damage over time, triggers, mirror fairness and the balance run.

use arena_calc::ailment::Ailment;
use arena_calc::dot::Dot;
use arena_calc::gem::{GEMS, Gem};
use arena_calc::loadout::{self, BASICS, Loadout, Slot};
use arena_calc::tag::Tag;
use arena_sim::balance::{self, Config};
use arena_sim::delivery::REACH_NEAR;
use arena_sim::fight::Fight;
use arena_sim::report::Place;
use arena_sim::{MAX_ROUNDS, ROUND_TIME, Rng, play_match};

fn random(seed: u64) -> Loadout {
    let mut r = Rng::salted(seed, 42);
    loadout::random(&mut || r.next_f64()).unwrap_or_default()
}

fn slot(id: &str, supports: &[&str]) -> Slot {
    Slot {
        skill: id.to_string(),
        supports: supports.iter().map(|s| s.to_string()).collect(),
        fuse: None,
    }
}

/// The cheapest `n` actives passing `keep`, with `first` in front.
fn cheapest(keep: impl Fn(&Gem) -> bool, n: usize) -> Vec<&'static Gem> {
    let mut g: Vec<&Gem> = GEMS
        .iter()
        .filter(|g| !BASICS.contains(&g.id) && g.uses.is_none() && keep(g))
        .collect();
    g.sort_by_key(|g| (g.tier.points(), g.id));
    g.into_iter().take(n).collect()
}

#[test]
fn the_same_seed_and_loadouts_give_the_same_match_bit_for_bit() {
    for seed in [1, 2, 3] {
        let (a, b) = (random(seed), random(seed + 100));
        let x = play_match(seed, &a, &b);
        let y = play_match(seed, &a, &b);
        assert_eq!(x, y);
        assert_eq!(format!("{x:?}"), format!("{y:?}"));
        let lx = Fight::new(seed, &a, &b, true).play().log;
        let ly = Fight::new(seed, &a, &b, true).play().log;
        assert_eq!(lx, ly);
    }
}

#[test]
fn a_fight_always_ends() {
    for seed in 0..30 {
        let m = play_match(seed, &random(seed), &random(seed + 1000));
        assert!(!m.rounds.is_empty() && m.rounds.len() <= MAX_ROUNDS as usize);
        for r in &m.rounds {
            assert!(r.seconds <= ROUND_TIME + 0.02, "round of {} s", r.seconds);
        }
    }
}

#[test]
fn a_melee_only_loadout_closes_in_to_hit() {
    let melee = cheapest(
        |g| {
            let t = g.tags();
            t.has(Tag::Near)
                && t.any(&[Tag::Melee, Tag::Strike, Tag::Slam, Tag::Nova])
                && !t.any(&[Tag::Area, Tag::Projectile])
        },
        8,
    );
    assert_eq!(melee.len(), 8);
    let l = Loadout {
        actives: melee.iter().map(|g| slot(g.id, &[])).collect(),
        keystones: Vec::new(),
        root: None,
    };
    assert_eq!(loadout::check(&l), Ok(()));
    let m = play_match(5, &l, &l);
    let mut landed = 0;
    for side in &m.sides {
        for s in side
            .skills
            .iter()
            .filter(|s| matches!(s.place, Place::Active(_)))
        {
            landed += s.landed;
            if s.landed > 0 {
                // the fighters start 6 T apart: every hit came after closing in
                assert!(
                    s.dist_hit / f64::from(s.landed) <= REACH_NEAR + 1e-9,
                    "{}",
                    s.id
                );
            }
        }
    }
    assert!(landed > 0);
}

#[test]
fn damage_over_time_kills_over_time() {
    let l = random(9);
    let mut f = Fight::new(9, &l, &l, false);
    // nobody acts: a long stun on both
    f.f[0].stun = 1e9;
    f.f[1].stun = 1e9;
    f.f[0].last_burn = Some(3);
    f.c[1].life = 200.0;
    f.c[1].ailments.lay(Dot::new(Ailment::Burn, 100.0, 4.0));
    let mut ticks = 0;
    while f.c[1].life > 0.0 && ticks < 600 {
        let before = f.c[1].life;
        f.step();
        assert!(f.c[1].life <= before);
        ticks += 1;
    }
    assert!(f.c[1].life <= 0.0);
    // about 2 s of a 100-a-second burn, less fire resistance (none)
    assert!((100..=140).contains(&ticks), "{ticks} ticks");
    assert!(f.f[0].report.dot > 0.0);
    assert!(f.f[0].report.skills[3].damage > 0.0);
}

#[test]
fn a_trigger_fires_through_the_queue() {
    // Thụ Thương Phản Chú on a spell: every 300 damage taken casts it
    let mut actives: Vec<Slot> = cheapest(|g| g.deals_damage() && g.id != "bolt", 7)
        .iter()
        .map(|g| slot(g.id, &[]))
        .collect();
    actives.push(slot("bolt", &["hurtcast"]));
    let l = Loadout {
        actives,
        keystones: Vec::new(),
        root: None,
    };
    assert_eq!(loadout::check(&l), Ok(()));
    let foe = random(11);
    let m = play_match(11, &l, &foe);
    let bolt = m.sides[0].skills.iter().find(|s| s.id == "bolt");
    // never cast by hand, cast by its trigger, and its blows came out
    assert!(
        bolt.is_some_and(|b| b.casts == 0 && b.triggered > 0 && b.landed + b.missed > 0),
        "{bolt:?}"
    );
}

/// Mirror fights: the same loadout on both seats, a different one each seed. Seat 0's share of the 50 matches must
/// lie within 0.5 ± 0.2 (the binomial standard error of 50 matches is about 0.07, so this is about 3 of them).
#[test]
fn mirror_loadouts_come_close_to_even() {
    let mut wins = 0.0;
    for seed in 0..50 {
        let l = random(seed + 500);
        wins += match play_match(seed, &l, &l).winner {
            Some(0) => 1.0,
            Some(_) => 0.0,
            None => 0.5,
        };
    }
    let share = wins / 50.0;
    assert!((0.3..=0.7).contains(&share), "seat 0 won {share}");
}

#[test]
fn the_balance_run_works_on_a_tiny_input() {
    let t = balance::run(Config {
        seeds: 1,
        pairs: 2,
        seed: 3,
        threads: 2,
    });
    assert_eq!(t.matches, 4);
    assert!(t.rows.iter().any(|r| r.games > 0 && r.tier != '-'));
    let v: serde_json::Value = serde_json::from_str(&t.json()).unwrap_or_default();
    assert_eq!(v["matches"], 4);
    assert!(t.text().contains("tier"));
    // the same run on one thread gives the same table
    let one = balance::run(Config {
        threads: 1,
        ..t.config
    });
    assert_eq!(
        (one.matches, one.seat0_wins, &one.rows),
        (t.matches, t.seat0_wins, &t.rows)
    );
}

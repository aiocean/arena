//! The field behaviours (`docs/specs/2026-11-17-damage-system-design.md`, 17.8): projectiles that pierce, fork and
//! chain, knockback, areas and durations, stored cooldown uses, traps, mines, totems and minions, and run speed,
//! each on concrete geometry.

use arena_calc::ailment::{AIL, Ailment};
use arena_calc::build::{Built, Situation, build};
use arena_calc::charge::Uses;
use arena_calc::cultivator::Cultivator;
use arena_calc::device::{
    self as dv, Kind, MINE_STAGGER, Placed, TOTEM_LIFETIME, TRAP_ARM, TRAP_RADIUS, spawn,
};
use arena_calc::gem::{GEMS, Gem, gem};
use arena_calc::hit::{Rolls, resolve};
use arena_calc::motion::{
    dash_reach, knockback, knockback_distance, run_speed, run_speed_of, step_x,
};
use arena_calc::play::{Field, Point, Shape, Target, area_factor, duration, radius};
use arena_calc::projectile::{
    CHAIN_RANGE, FORK_ANGLE, Flight, Powers, SPREAD, Shot, Strike, Via, flight_time, trace,
};
use arena_calc::sheet::{EMPTY, Sheet};
use arena_calc::stat::{Mod, Source, Stat, Table};
use arena_calc::support::{self as sp, Support, support};

const SRC: Source = Source::Other("test");

const NO_CRIT: Rolls = Rolls {
    evade: 1.0,
    crit: 1.0,
    ..Rolls::NONE
};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn pool(id: &str) -> &'static Gem {
    gem(id).unwrap_or(&GEMS[0])
}

fn sup(id: &str) -> &'static Support {
    support(id).unwrap_or(&Support::NONE)
}

/// `gem` at level 1 with `supports`, by a fresh fighter wearing `sheet`.
fn cast(gem: &str, supports: &[&str], sheet: &Sheet) -> Built {
    let sups: Vec<&Support> = supports.iter().map(|s| sup(s)).collect();
    build(
        pool(gem),
        1,
        &sups,
        &Cultivator::new(),
        sheet,
        &Situation::default(),
    )
}

fn table(mods: &[Mod]) -> Table {
    Table::gather(
        mods.iter(),
        &arena_calc::stat::Ctx::alone(&Cultivator::new()),
    )
}

/// A shot along the floor from x = 0 at height `y`, 10 T a second, 20 T far.
fn shot_at(y: f64) -> Shot {
    Shot {
        origin: Point::new(0.0, y),
        direction: Point::new(1.0, 0.0),
        speed: 10.0,
        range: 20.0,
    }
}

fn ids(s: &[Strike]) -> Vec<u32> {
    s.iter().map(|s| s.target).collect()
}

fn powers(pierce: u32, fork: u32, chain: u32) -> Powers {
    Powers {
        pierce,
        fork,
        chain,
        ..Powers::NONE
    }
}

// ---- projectiles ----

#[test]
fn a_shot_strikes_the_first_target_on_its_path_and_ends() {
    let targets = [
        Target::new(2, 7.0, 0.5, 0.3),
        Target::new(1, 3.0, 0.5, 0.3),
        Target::new(3, 3.0, 4.0, 0.3), // off the path
    ];
    let s = trace(&shot_at(0.5), &Powers::NONE, &targets);
    assert_eq!(ids(&s), [1]);
    close(s[0].time, 0.27);
    close(s[0].at.x, 2.7);
    close(s[0].travelled, 2.7);
    assert_eq!(s[0].via, Via::Direct);
    close(s[0].factor, 1.0);
}

#[test]
fn three_in_a_line_pierce_one_then_stop() {
    let targets = [
        Target::new(1, 3.0, 0.5, 0.3),
        Target::new(2, 5.0, 0.5, 0.3),
        Target::new(3, 7.0, 0.5, 0.3),
    ];
    let s = trace(&shot_at(0.5), &powers(1, 0, 0), &targets);
    assert_eq!(ids(&s), [1, 2]);
    assert_eq!(s[1].via, Via::Pierce);
    close(s[1].time, 0.47);
    // Xuyên Thấu pierces two: all three
    let b = cast("fireball", &["piercing"], &EMPTY);
    let p = Powers::of_built(&b);
    assert_eq!(p.pierce, 2);
    assert_eq!(ids(&trace(&shot_at(0.5), &p, &targets)), [1, 2, 3]);
    // a dead one and one struck before are passed by
    let mut gone = targets;
    gone[0].alive = false;
    gone[1].hit_before = true;
    assert_eq!(ids(&trace(&shot_at(0.5), &Powers::NONE, &gone)), [3]);
}

/// A scattered field: the shot strikes A on the floor line; B is 2.5 T from A, C 3.2 T from A and 3.54 from B, D
/// more than `CHAIN_RANGE` from both B and C.
fn scattered() -> [Target; 4] {
    [
        Target::new(10, 3.0, 0.5, 0.3),  // A
        Target::new(11, 4.5, 2.5, 0.3),  // B
        Target::new(12, 1.0, 3.0, 0.3),  // C
        Target::new(13, 10.0, 3.0, 0.3), // D
    ]
}

#[test]
fn chain_two_goes_nearest_first_never_twice_and_stops_out_of_range() {
    let targets = scattered();
    let s = trace(&shot_at(0.5), &powers(0, 0, 2), &targets);
    // A, then B (2.5 T, nearer than C's 3.2), then C (from B, A struck already; D 5.5 T away is out of range)
    assert_eq!(ids(&s), [10, 11, 12]);
    assert_eq!(
        s.iter().map(|s| s.via).collect::<Vec<_>>(),
        [Via::Direct, Via::Chain, Via::Chain]
    );
    assert!(s.windows(2).all(|w| w[0].time < w[1].time));
    // the hop to B: from the strike on A's edge (2.7, 0.5) to B's edge
    let hop = Point::new(2.7, 0.5).distance(Point::new(4.5, 2.5)) - 0.3;
    close(s[1].travelled, 2.7 + hop);
    close(s[1].time, (2.7 + hop) / 10.0);
    // more chains than targets in range: still three, none twice
    let s = trace(&shot_at(0.5), &powers(0, 0, 5), &targets);
    assert_eq!(ids(&s), [10, 11, 12]);
    // nothing in range of A: the shot ends at A
    let lone = [targets[0], targets[3]];
    assert!(lone[0].at().distance(lone[1].at()) > CHAIN_RANGE);
    assert_eq!(ids(&trace(&shot_at(0.5), &powers(0, 0, 2), &lone)), [10]);
}

#[test]
fn a_fork_splits_at_plus_and_minus_its_angle_and_never_strikes_its_target_again() {
    // A on the line at y = 2; B on the +30° ray 3 T from the strike, C on the −30° ray; E straight behind A
    let hit = Point::new(4.7, 2.0);
    let up = Point::new(1.0, 0.0).rotated(FORK_ANGLE);
    let down = Point::new(1.0, 0.0).rotated(-FORK_ANGLE);
    let b = hit.plus(up, 3.0);
    let c = hit.plus(down, 3.0);
    let targets = [
        Target::new(1, 5.0, 2.0, 0.3),
        Target::new(2, b.x, b.y, 0.3),
        Target::new(3, c.x, c.y, 0.3),
        Target::new(4, 8.0, 2.0, 0.3),
    ];
    let s = trace(&shot_at(2.0), &powers(0, 1, 0), &targets);
    let mut got = ids(&s);
    got.sort_unstable();
    assert_eq!(got, [1, 2, 3]);
    assert_eq!(s.iter().filter(|s| s.target == 1).count(), 1);
    let on = |id: u32| s.iter().find(|s| s.target == id).copied();
    let (sb, sc) = (on(2), on(3));
    assert_eq!(sb.map(|s| (s.branch, s.via)), Some((1, Via::Fork)));
    assert_eq!(sc.map(|s| (s.branch, s.via)), Some((2, Via::Fork)));
    // each half flew 3 − 0.3 T from the strike
    close(sb.map_or(0.0, |s| s.travelled), 4.7 + 2.7);
    // the halves' first step from the strike, with A all around them
    let mut f = Flight::launch(&shot_at(2.0), &powers(0, 1, 0)).remove(0);
    let st = f.step(f64::INFINITY, &targets[..1]);
    assert!(f.done);
    assert_eq!(st.forks.len(), 2);
    close(st.forks[0].dir.x, libm::cos(FORK_ANGLE));
    close(st.forks[0].dir.y, libm::sin(FORK_ANGLE));
    close(st.forks[1].dir.y, -libm::sin(FORK_ANGLE));
    assert!(st.forks.iter().all(|h| h.struck == [1]));
}

#[test]
fn pierce_then_fork_then_chain() {
    // A and B on the line; the fork after B sends its upper half to C; C chains to D
    let after_b = Point::new(5.7, 2.0);
    let c = after_b.plus(Point::new(1.0, 0.0).rotated(FORK_ANGLE), 2.0);
    let targets = [
        Target::new(1, 3.0, 2.0, 0.3),
        Target::new(2, 6.0, 2.0, 0.3),
        Target::new(3, c.x, c.y, 0.3),
        Target::new(4, 8.5, 5.0, 0.3),
    ];
    let s = trace(&shot_at(2.0), &powers(1, 1, 1), &targets);
    assert_eq!(ids(&s), [1, 2, 3, 4]);
    assert_eq!(
        s.iter().map(|s| s.via).collect::<Vec<_>>(),
        [Via::Direct, Via::Pierce, Via::Fork, Via::Chain]
    );
    assert_eq!(s[3].branch, 1);
}

#[test]
fn extra_projectiles_fan_out_around_the_aim() {
    let p = Powers {
        extra: 2,
        ..Powers::NONE
    };
    let fan = Flight::launch(&shot_at(2.0), &p);
    assert_eq!(fan.len(), 3);
    for (k, f) in fan.iter().enumerate() {
        let a = (k as f64 - 1.0) * SPREAD;
        close(f.dir.x, libm::cos(a));
        close(f.dir.y, libm::sin(a));
        assert_eq!(f.projectile, k as u32);
    }
    // a target 6 T down each ray: each struck once, by its own projectile
    let o = Point::new(0.0, 2.0);
    let targets: Vec<Target> = fan
        .iter()
        .enumerate()
        .map(|(k, f)| {
            let p = o.plus(f.dir, 6.0);
            Target::new(k as u32, p.x, p.y, 0.3)
        })
        .collect();
    let s = trace(&shot_at(2.0), &p, &targets);
    assert_eq!(s.len(), 3);
    for x in &s {
        assert_eq!(x.target, x.projectile);
    }
    // Tiểu Liên Châu: one more
    let b = cast("fireball", &["lessermultiple"], &EMPTY);
    assert_eq!(Powers::of_built(&b).extra, 1);
}

#[test]
fn faster_projectiles_reach_earlier() {
    let targets = [Target::new(1, 6.0, 0.5, 0.5)];
    let plain = trace(&shot_at(0.5), &Powers::NONE, &targets);
    let b = cast("fireball", &["fasterprojectiles"], &EMPTY);
    let p = Powers::of_built(&b);
    close(p.speed, 1.0 + sp::FASTER_SHOTS);
    let fast = trace(&shot_at(0.5), &p, &targets);
    close(plain[0].time, 0.55);
    close(fast[0].time, 0.55 / 1.5);
    close(flight_time(20.0, 10.0, &p), 20.0 / 15.0);
    let slow = Powers::of_built(&cast("fireball", &["slowerprojectiles"], &EMPTY));
    assert!(trace(&shot_at(0.5), &slow, &targets)[0].time > plain[0].time);
}

#[test]
fn stepping_a_frame_at_a_time_strikes_as_the_trace_does() {
    let targets = scattered();
    let whole = trace(&shot_at(0.5), &powers(0, 0, 2), &targets);
    let mut f = Flight::launch(&shot_at(0.5), &powers(0, 0, 2)).remove(0);
    let mut stepped = Vec::new();
    for _ in 0..200 {
        stepped.extend(f.step(1.0 / 60.0, &targets).strikes);
        if f.done {
            break;
        }
    }
    assert_eq!(ids(&stepped), ids(&whole));
    for (a, b) in stepped.iter().zip(&whole) {
        close(a.time, b.time);
    }
    // the flight ends once the range is spent
    let mut f = Flight::launch(&shot_at(0.5), &Powers::NONE).remove(0);
    for _ in 0..3 {
        f.step(1.0, &[]);
    }
    assert!(f.done);
    close(f.travelled, 20.0);
}

#[test]
fn a_strike_carries_its_factor_as_one_more() {
    let b = cast("fireball", &[], &EMPTY);
    let s = Strike {
        target: 0,
        time: 0.0,
        factor: 0.5,
        projectile: 0,
        branch: 0,
        via: Via::Chain,
        at: Point::default(),
        travelled: 0.0,
    };
    let mut t1 = Cultivator::new();
    let mut t2 = Cultivator::new();
    let full = resolve(&b.hit, None, &mut t1, NO_CRIT).total();
    let half = resolve(&s.blow(&b.hit), None, &mut t2, NO_CRIT).total();
    close(half, full * 0.5);
}

// ---- knockback and motion ----

#[test]
fn knockback_is_held_by_the_wall() {
    let field = Field { width: 10.0 };
    // pushed right from the left, 2 T, stops at the wall less its body
    close(knockback(9.0, 0.3, 8.0, 2.0, &field), 9.7);
    close(knockback(5.0, 0.3, 4.0, 2.0, &field), 7.0);
    // pushed left
    close(knockback(1.0, 0.3, 2.0, 5.0, &field), 0.3);
    // the distance from the striker's table: 1.5 T flat, 20% increased
    let t = table(&[
        Mod::flat(Stat::Knockback, 1.5, SRC),
        Mod::inc(Stat::Knockback, 0.2, SRC),
    ]);
    close(knockback_distance(0.0, &t), 1.8);
    close(knockback_distance(1.0, &Table::default()), 1.0);
    // a run step is held too
    close(step_x(9.5, 0.3, 3.0, 1.0, &field), 9.7);
    close(step_x(5.0, 0.3, -3.0, 0.5, &field), 3.5);
}

#[test]
fn heavy_shortens_a_dash() {
    close(dash_reach(4.0, 3.0), 4.0 * (1.0 - 3.0 * AIL.heavy_reach));
    close(dash_reach(4.0, 20.0), 0.0);
}

#[test]
fn run_speed_from_bind_stacks_and_maim_and_zero_when_rooted() {
    let mut target = Cultivator::new();
    target.ailments.add(Ailment::Bind, 2.0, 1.0);
    let base = 3.0;
    // 2 binds: 16% reduced
    close(
        run_speed_of(&target, &EMPTY, base, false, false),
        base * (1.0 - 2.0 * AIL.bind_slow),
    );
    // Phế Cước's maim, laid by a blow that lands: 30% reduced more
    let striker = Cultivator::new();
    let b = build(
        pool("palm"),
        1,
        &[sup("maim")],
        &striker,
        &EMPTY,
        &Situation {
            target: Some(&target),
            ..Situation::default()
        },
    );
    resolve(&b.hit, Some(&striker), &mut target, NO_CRIT);
    let binds = target.ailments.stacks(Ailment::Bind);
    close(
        run_speed_of(&target, &EMPTY, base, false, false),
        base * (1.0 - binds * AIL.bind_slow + sp::MAIM_SLOW),
    );
    // rooted: none
    close(run_speed_of(&target, &EMPTY, base, false, true), 0.0);
    // never below zero
    let t = table(&[Mod::inc(Stat::MoveSpeed, -3.0, SRC)]);
    close(run_speed(base, &t, false), 0.0);
}

// ---- areas and durations ----

#[test]
fn area_scales_the_radius_by_its_square_root() {
    let b = cast("collapse", &["widearea"], &EMPTY);
    close(area_factor(&b.table), 1.0 + sp::WIDE_AREA);
    close(radius(2.0, &b.table), 2.0 * libm::sqrt(1.4));
    // 44% increased: 1.2 times the radius; Tụ Lực's 30% less: √0.7
    close(radius(1.0, &table(&[Mod::inc(Stat::Area, 0.44, SRC)])), 1.2);
    let b = cast("collapse", &["concentrated"], &EMPTY);
    close(radius(1.0, &b.table), libm::sqrt(0.7));
    close(radius(1.5, &Table::default()), 1.5);
}

#[test]
fn who_is_inside_a_circle_or_a_band() {
    let targets = [
        Target::new(1, 2.0, 0.5, 0.3),
        Target::new(2, 3.1, 0.5, 0.3), // its body overlaps the circle's edge
        Target::new(3, 5.0, 0.5, 0.3),
        Target {
            alive: false,
            ..Target::new(4, 2.0, 0.5, 0.3)
        },
    ];
    let circle = Shape::Circle {
        centre: Point::new(2.0, 0.5),
        radius: 0.9,
    };
    assert_eq!(circle.inside(&targets), [1, 2]);
    let band = Shape::Band {
        left: 4.6,
        right: 6.0,
        bottom: 0.0,
        top: 1.0,
    };
    assert_eq!(band.inside(&targets), [3]);
    let high = Shape::Band {
        left: 0.0,
        right: 10.0,
        bottom: 2.0,
        top: 3.0,
    };
    assert!(high.inside(&targets).is_empty());
}

#[test]
fn skill_duration_scales_zones_buffs_and_curses() {
    let b = cast("collapse", &["efficacy"], &EMPTY);
    close(duration(5.0, &b.table), 5.0 * (1.0 + sp::EFFICACY_DURATION));
    let t = table(&[
        Mod::inc(Stat::SkillDuration, 0.5, SRC),
        Mod::more(Stat::SkillDuration, -0.2, SRC),
    ]);
    close(duration(4.0, &t), 4.0 * 1.5 * 0.8);
    close(duration(4.0, &Table::default()), 4.0);
}

// ---- cooldown uses ----

#[test]
fn two_uses_spend_both_and_recharge_one_at_a_time() {
    let sheet = Sheet {
        extra: vec![Mod::flat(Stat::ExtraCooldownUse, 1.0, SRC)],
        ..Sheet::default()
    };
    let b = cast("fireball", &[], &sheet);
    let mut u = Uses::of(&b);
    assert_eq!((u.max, u.stored), (2, 2));
    close(u.cooldown, 1.5);
    assert!(u.try_use());
    assert!(u.try_use());
    assert!(!u.try_use());
    u.tick(1.0);
    assert_eq!(u.stored, 0);
    u.tick(0.5);
    assert_eq!(u.stored, 1);
    close(u.recharge_left, 1.5);
    // the second recharges after the first, not beside it
    u.tick(1.0);
    assert_eq!(u.stored, 1);
    u.tick(0.5);
    assert_eq!(u.stored, 2);
    close(u.recharge_left, 0.0);
    // a long tick runs into the next
    assert!(u.try_use() && u.try_use());
    u.tick(3.0);
    assert_eq!(u.stored, 2);
    // without the stat, one
    assert_eq!(Uses::of(&cast("fireball", &[], &EMPTY)).max, 1);
}

// ---- devices and minions ----

#[test]
fn a_trap_arms_then_springs_on_an_enemy_in_reach() {
    let b = cast("fireball", &["trap"], &EMPTY);
    let s = spawn(&b).unwrap_or_else(|| unreachable!());
    assert_eq!(s.kind, Kind::Trap);
    assert_eq!(s.cap, 1);
    close(s.arm, TRAP_ARM);
    close(s.life, dv::DEVICE_LIFE);
    let mut p = Placed::new(s);
    p.place(Point::new(5.0, 0.0));
    let near = [Target::new(7, 5.8, 0.5, 0.3)]; // body 0.64 T from the trap
    let far = [Target::new(8, 5.0 + TRAP_RADIUS + 1.0, 0.0, 0.3)];
    assert!(p.spring(&near).is_empty(), "not armed yet");
    p.tick(0.25);
    assert!(p.spring(&near).is_empty());
    p.tick(0.25);
    assert!(p.spring(&far).is_empty());
    assert_eq!(p.spring(&near), [(0, 7)]);
    assert!(p.standing.is_empty());
}

#[test]
fn mines_go_off_on_command_the_oldest_first_and_the_cap_removes_the_oldest() {
    let sheet = Sheet {
        extra: vec![Mod::flat(Stat::MineCount, 2.0, SRC)],
        ..Sheet::default()
    };
    let b = cast("fireball", &["mine"], &sheet);
    let s = spawn(&b).unwrap_or_else(|| unreachable!());
    assert_eq!((s.kind, s.cap), (Kind::Mine, 3));
    let mut p = Placed::new(s);
    for x in [1.0, 2.0, 3.0] {
        assert!(p.place(Point::new(x, 0.0)).removed.is_empty());
    }
    p.tick(dv::MINE_ARM);
    // the fourth removes the first
    let placed = p.place(Point::new(4.0, 0.0));
    assert_eq!((placed.made, placed.removed), (vec![3], vec![0]));
    // 1 and 2 are armed, 3 not yet
    let boom = p.detonate();
    assert_eq!(
        boom.iter().map(|b| (b.0, b.1)).collect::<Vec<_>>(),
        [(1, 0.0), (2, MINE_STAGGER)]
    );
    close(boom[0].2.x, 2.0);
    assert_eq!(p.standing.iter().map(|d| d.id).collect::<Vec<_>>(), [3]);
}

#[test]
fn a_totem_casts_on_its_cadence_and_the_cap_removes_the_oldest() {
    let b = cast("fireball", &["totem"], &EMPTY);
    let s = spawn(&b).unwrap_or_else(|| unreachable!());
    assert_eq!((s.kind, s.cap), (Kind::Totem, 1));
    // the first cast after its windup (paced), then every max(windup + recover, cooldown) = 1.5 s
    let first = 0.26 * arena_calc::build::PACE.windup;
    close(s.first_cast, first);
    close(s.cast_interval, 1.5);
    close(s.life, dv::TOTEM_LIFE);
    let mut p = Placed::new(s);
    p.place(Point::new(2.0, 0.0));
    let mut casts = Vec::new();
    let mut clock = 0.0;
    for _ in 0..81 {
        let t = p.tick(0.1);
        casts.extend(t.casts.iter().map(|c| clock + c.1));
        clock += 0.1;
    }
    // over its 8 s lifetime (81 ticks of 0.1: the sum of 80 falls a hair short of 8): 0.416, 1.916, 3.416, 4.916,
    // 6.416, 7.916, then it falls apart
    assert_eq!(casts.len(), 6);
    for (k, at) in casts.iter().enumerate() {
        close(*at, first + 1.5 * k as f64);
    }
    assert!(clock >= TOTEM_LIFETIME && p.standing.is_empty());
    // a second totem past the cap of one removes the first
    let mut p = Placed::new(s);
    p.place(Point::new(1.0, 0.0));
    let second = p.place(Point::new(2.0, 0.0));
    assert_eq!((second.made, second.removed), (vec![1], vec![0]));
    // a totem broken stops casting
    assert!(p.hurt(1, dv::TOTEM_LIFE));
    assert!(p.tick(1.0).casts.is_empty());
}

#[test]
fn minions_take_their_life_and_damage_from_minion_life_and_minion_damage() {
    let b = cast("ghostsummon", &["miniondamage", "minionlife"], &EMPTY);
    let s = spawn(&b).unwrap_or_else(|| unreachable!());
    assert_eq!(s.kind, Kind::Minion);
    close(s.life, dv::MINION_LIFE * (1.0 + sp::MINION_LIFE));
    close(s.damage, dv::MINION_SHARE * (1.0 + sp::MINION_DAMAGE));
    assert_eq!(s.lifetime, Some(dv::MINION_LIFETIME));
    assert_eq!(s.cap, dv::MINION_CAP as u32);
    let plain = spawn(&cast("ghostsummon", &[], &EMPTY)).unwrap_or_else(|| unreachable!());
    close(plain.life, dv::MINION_LIFE);
    close(plain.damage, dv::MINION_SHARE);
    // a third past the cap of two removes the first; the two left fall apart at their lifetime
    let mut p = Placed::new(s);
    for _ in 0..3 {
        p.place(Point::new(1.0, 0.0));
    }
    assert_eq!(p.standing.iter().map(|d| d.id).collect::<Vec<_>>(), [1, 2]);
    assert!(p.tick(dv::MINION_LIFETIME - 0.5).expired.is_empty());
    assert_eq!(p.tick(1.0).expired, [1, 2]);
    assert!(p.standing.is_empty());
    // a skill that makes nothing
    assert_eq!(spawn(&cast("fireball", &[], &EMPTY)), None);
}

// ---- determinism ----

#[test]
fn the_same_shot_gives_the_same_bits() {
    let targets = scattered();
    let p = Powers {
        pierce: 1,
        fork: 2,
        chain: 3,
        extra: 4,
        speed: 1.37,
    };
    let shot = Shot {
        origin: Point::new(0.1, 0.7),
        direction: Point::new(3.0, 0.4),
        speed: 9.3,
        range: 17.0,
    };
    let a = trace(&shot, &p, &targets);
    let b = trace(&shot, &p, &targets);
    assert!(!a.is_empty());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(x.target, y.target);
        assert_eq!(x.time.to_bits(), y.time.to_bits());
        assert_eq!(x.at.x.to_bits(), y.at.x.to_bits());
        assert_eq!(x.at.y.to_bits(), y.at.y.to_bits());
    }
}

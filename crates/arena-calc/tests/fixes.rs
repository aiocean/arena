//! What the simulator (`arena-sim`) and the duel's port found wrong in the damage system, each pinned once fixed:
//! souls had no income, a totem whose skill's cooldown outlasts it cast once, Liên Hoàn Suy Yếu aimed its linked
//! attack at the guard it hangs on (no damage), and Quá Tải looked dead in a small balance sample.

use arena_calc::build::{Situation, Skill, build_skill};
use arena_calc::cultivator::{Cultivator, RES};
use arena_calc::device::{self as dv, Kind, Placed, TOTEM_CADENCE_SHARE, spawn};
use arena_calc::element::Element;
use arena_calc::event::{
    CAST_ON_SHIELD_BREAK, Dice, Effect, Event, EventKind, GUARD_BREAK_SHARE, Queue, Seat,
};
use arena_calc::gem::{self, Gem};
use arena_calc::hit::{Hit, Rolls, resolve};
use arena_calc::play::Point;
use arena_calc::sheet::EMPTY;
use arena_calc::support::{self, Support};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn pool(id: &str) -> &'static Gem {
    gem::gem(id).unwrap_or(&gem::GEMS[0])
}

fn sup(id: &str) -> &'static Support {
    support::support(id).unwrap_or(&Support::NONE)
}

fn built(id: &str, supports: &[&str]) -> arena_calc::build::Built {
    let sups: Vec<&Support> = supports.iter().map(|s| sup(s)).collect();
    let me = Cultivator::new();
    build_skill(
        &Skill::of(pool(id), gem::BASE_LEVEL, me.might),
        &sups,
        &me,
        &EMPTY,
        &Situation::default(),
    )
}

#[test]
fn soul_damage_earns_a_soul_each_hundred() {
    let me = Cultivator::new();
    let mut foe = Cultivator::new();
    let out = resolve(
        &Hit::split(250.0, &[(Element::Soul, 1.0)]),
        Some(&me),
        &mut foe,
        Rolls::NONE,
    );
    let dealt = out.dealt[Element::Soul.index()];
    assert!(dealt > 0.0);
    close(out.gains.souls, dealt / RES.souls_per_damage);
    // a blow with no soul in it earns none
    let mut foe = Cultivator::new();
    let out = resolve(
        &Hit::split(250.0, &[(Element::Fire, 1.0)]),
        Some(&me),
        &mut foe,
        Rolls::NONE,
    );
    close(out.gains.souls, 0.0);
}

#[test]
fn a_kill_earns_a_soul_unless_the_target_has_none() {
    let me = Cultivator::new();
    let mut foe = Cultivator::new();
    foe.life = 10.0;
    foe.shield = 0.0;
    let out = resolve(
        &Hit::split(100.0, &[(Element::Fire, 1.0)]),
        Some(&me),
        &mut foe,
        Rolls::NONE,
    );
    close(out.gains.souls, RES.souls_kill);
    let mut puppet = Cultivator::new();
    puppet.life = 10.0;
    puppet.shield = 0.0;
    puppet.soulless_body = true;
    let out = resolve(
        &Hit::split(100.0, &[(Element::Fire, 1.0)]),
        Some(&me),
        &mut puppet,
        Rolls::NONE,
    );
    close(out.gains.souls, 0.0);
}

#[test]
fn a_minion_dying_near_gives_a_soul_up_to_the_cap() {
    let mut c = Cultivator::new();
    close(c.minion_died(RES.souls_reach + 0.1), 0.0);
    close(c.minion_died(-RES.souls_reach), RES.souls_minion);
    close(c.souls, RES.souls_minion);
    c.souls = RES.souls_max - 0.5;
    close(c.minion_died(0.0), 0.5);
    close(c.souls, RES.souls_max);
    c.gain_souls(5.0);
    close(c.souls, RES.souls_max);
}

#[test]
fn soul_cost_skills_become_castable_from_their_income() {
    // Tát Đậu Thành Binh costs 2 souls: four 50-point soul blows pay for it
    let cost = pool("beans")
        .costs
        .iter()
        .find(|c| c.0 == gem::Resource::Souls)
        .map_or(0.0, |c| c.1);
    assert!(cost > 0.0);
    let mut me = Cultivator::new();
    let mut foe = Cultivator::new();
    foe.sense = 1e9;
    foe.sense_max = 1e9;
    while me.souls < cost {
        let out = resolve(
            &Hit::split(100.0, &[(Element::Soul, 1.0)]),
            Some(&me),
            &mut foe,
            Rolls::NONE,
        );
        assert!(out.gains.souls > 0.0);
        me.gain_souls(out.gains.souls);
    }
}

#[test]
fn a_totem_casts_several_times_whatever_its_cooldown() {
    let b = built("arsenal", &[]);
    assert!(
        b.cooldown > dv::TOTEM_LIFETIME,
        "arsenal's cooldown outlasts a totem"
    );
    let s = spawn(&b).unwrap_or_else(|| unreachable!());
    assert_eq!(s.kind, Kind::Totem);
    close(s.cast_interval, dv::TOTEM_LIFETIME * TOTEM_CADENCE_SHARE);
    let mut p = Placed::new(s);
    p.place(Point::new(2.0, 0.0));
    let mut casts = 0;
    for _ in 0..90 {
        casts += p.tick(0.1).casts.len();
    }
    assert!(casts >= 4, "{casts} casts");
    // a quick skill keeps its cooldown's pace (design 17.7, case 21)
    let quick = spawn(&built("fireball", &["totem"])).unwrap_or_else(|| unreachable!());
    close(quick.cast_interval, 1.5);
}

#[test]
fn chainweaken_fires_its_linked_attack_for_what_the_shield_absorbed() {
    let guard = Skill::of(pool("ward"), gem::BASE_LEVEL, 400.0);
    let Some(t) = sup("chainweaken").trigger_for(&guard) else {
        unreachable!("chainweaken is a trigger")
    };
    let mut q = Queue::new();
    let mut e = Event::new(EventKind::ShieldBroke, 0).against(1);
    e.damage = 480.0;
    q.push(e);
    let triggers = [t];
    let seats = [
        Seat {
            triggers: &triggers,
            cursed: false,
            moving: false,
            distance: 1.0,
        },
        Seat {
            triggers: &[],
            cursed: false,
            moving: false,
            distance: 1.0,
        },
    ];
    let mut cs = [Cultivator::new(), Cultivator::new()];
    let out = q.run(&mut cs, &seats, &mut Dice::new(&[0.0; 4]));
    assert_eq!(out.casts.len(), 1);
    let cast = out.casts[0];
    let Effect::Cast(linked) = CAST_ON_SHIELD_BREAK.does else {
        unreachable!()
    };
    assert_eq!(cast.skill, linked.skill);
    assert_ne!(cast.skill, "ward");
    close(cast.base.unwrap_or(0.0), 480.0 * GUARD_BREAK_SHARE);
    // built as the fight builds it, the linked attack deals damage
    let palm = pool(cast.skill);
    let mut hit = built(palm.id, &[]).hit;
    hit.base = Hit::split(cast.base.unwrap_or(0.0), palm.split).base;
    let mut foe = Cultivator::new();
    let landed = resolve(&hit, Some(&cs[0]), &mut foe, Rolls::NONE);
    assert!(landed.total() > 0.0);
}

#[test]
fn overcharge_adds_thunder_damage_and_jolt() {
    // it was dead in the balance table only because its one pick hung on a skill that sample never cast
    let plain = built("bolt", &[]);
    let over = built("bolt", &["overcharge"]);
    let land = |h: &Hit| {
        let mut foe = Cultivator::new();
        resolve(h, Some(&Cultivator::new()), &mut foe, Rolls::NONE)
    };
    let (a, b) = (land(&plain.hit), land(&over.hit));
    close(b.total(), a.total() * (1.0 + support::OVERCHARGE_MORE));
    assert!(
        b.stacks[arena_calc::ailment::Ailment::Jolt.index()]
            > a.stacks[arena_calc::ailment::Ailment::Jolt.index()]
    );
}

/// Lôi Bạo Trễ's mine: a share of the stopped blow, armed after the mine's arming, at most its cap standing, and
/// the support carries the behaviour the fight reads.
#[test]
fn a_stopped_shots_mine_arms_caps_and_carries_a_share() {
    assert!(
        sup("delayedthunder")
            .behaviours
            .contains(&support::Behaviour::MineOnBlock)
    );
    let s = dv::STOPPED_MINE;
    assert_eq!(s.kind, Kind::Mine);
    assert!(s.damage > 0.0 && s.damage < 1.0);
    let mut p = Placed::new(s);
    for i in 0..5 {
        let made = p.place(Point::new(f64::from(i), 0.0)).made;
        assert_eq!(made.len(), 1);
    }
    assert_eq!(p.standing.len(), dv::STOPPED_CAP as usize);
    // the oldest went: the three standing are the last placed
    let at: Vec<f64> = p.standing.iter().map(|d| d.at.x).collect();
    assert_eq!(at, vec![2.0, 3.0, 4.0]);
    // not armed yet: nothing goes off
    assert!(p.detonate().is_empty());
    p.tick(dv::MINE_ARM);
    assert_eq!(p.detonate().len(), dv::STOPPED_CAP as usize);
    assert!(p.standing.is_empty());
}

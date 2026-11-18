//! Events and triggers (`event`): a blow's outcome to its events, the `Recent` marks, every trigger case, internal
//! cooldowns, chance rolls in order, the loop caps and determinism.

use arena_calc::ailment::Ailment;
use arena_calc::cultivator::Cultivator;
use arena_calc::element::{Element, N};
use arena_calc::event::{
    self, Blow, CAST_ON_CRIT, CAST_ON_KILL, CAST_ON_SHIELD_BREAK, CAST_WHEN_HURT, CHAIN_DEPTH,
    Dice, Effect, Event, EventKind, Filter, HERALD_OF_ASH, LifeMark, ON_BURN_FULL,
    ON_HIT_VS_CURSED, Output, Queue, Scale, Seat, TRIGGERS_PER_TICK, Trigger, TriggeredCast,
    events_of,
};
use arena_calc::hit::{Hit, Outcome, Rolls, resolve};
use arena_calc::stat::{Condition, Happened, Source};
use arena_calc::tag::{Tag, Tags};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn two() -> Vec<Cultivator> {
    vec![Cultivator::new(), Cultivator::new()]
}

fn seats(t0: &[Trigger], t1: &[Trigger]) -> Vec<Seat<'static>> {
    // the triggers used here are all consts or leaked test data, so 'static is fine
    let a: &'static [Trigger] = Box::leak(t0.to_vec().into_boxed_slice());
    let b: &'static [Trigger] = Box::leak(t1.to_vec().into_boxed_slice());
    vec![
        Seat {
            triggers: a,
            ..Seat::default()
        },
        Seat {
            triggers: b,
            ..Seat::default()
        },
    ]
}

fn run_one(trigger: Trigger, e: Event) -> Output {
    let mut f = two();
    let s = seats(&[trigger], &[]);
    let mut q = Queue::new();
    q.push(e);
    q.run(&mut f, &s, &mut Dice::new(&[]))
}

fn attack() -> Tags {
    Tags::of(&[Tag::Attack, Tag::Melee, Tag::Force])
}

// ---- the outcome -> events mapping

fn kinds(es: &[Event]) -> Vec<(EventKind, usize)> {
    es.iter().map(|e| (e.kind, e.who)).collect()
}

#[test]
fn a_plain_blow_gives_hit_then_been_hit() {
    let mut t = Cultivator::new();
    t.shield = 0.0;
    let before = (t.life, t.shield);
    let o = resolve(
        &Hit::split(10.0, &[(Element::Force, 1.0)]),
        None,
        &mut t,
        Rolls {
            evade: 1.0,
            crit: 1.0,
            ..Rolls::NONE
        },
    );
    let blow = Blow {
        by: Some(0),
        on: 1,
        skill: Some("slash"),
        tags: attack(),
        depth: 0,
        life_before: before.0,
        shield_before: before.1,
    };
    let es = events_of(&o, &blow, &t);
    let ks: Vec<_> = kinds(&es)
        .into_iter()
        .filter(|(k, _)| !matches!(k, EventKind::AilmentLaid(_)))
        .collect();
    assert_eq!(ks, vec![(EventKind::Hit, 0), (EventKind::BeenHit, 1)]);
    assert_eq!(es[0].other, Some(1));
    assert_eq!(es[1].other, Some(0));
    assert_eq!(es[0].skill, Some("slash"));
    close(es[0].damage, o.total());
}

#[test]
fn a_killing_crit_through_the_shield_gives_every_event_in_order() {
    let mut o = Outcome {
        crit: true,
        to_shield: 50.0,
        to_life: 900.0,
        ..Outcome::default()
    };
    o.dealt[Element::Fire.index()] = 950.0;
    o.stacks[Ailment::Burn.index()] = 3.0;
    o.stacks[Ailment::Bleed.index()] = 1.0;
    o.full.push(Ailment::Burn);
    let mut t = Cultivator::new();
    t.life = 0.0;
    t.shield = 0.0;
    let blow = Blow {
        by: Some(0),
        on: 1,
        skill: Some("fireball"),
        tags: Tags::of(&[Tag::Spell, Tag::Fire]),
        depth: 0,
        life_before: t.life_max,
        shield_before: 50.0,
    };
    let es = events_of(&o, &blow, &t);
    assert_eq!(
        kinds(&es),
        vec![
            (EventKind::Hit, 0),
            (EventKind::BeenHit, 1),
            (EventKind::Crit, 0),
            (EventKind::BeenCrit, 1),
            (EventKind::ShieldBroke, 1),
            (EventKind::AilmentLaid(Ailment::Bleed), 0),
            (EventKind::AilmentLaid(Ailment::Burn), 0),
            (EventKind::AilmentFull(Ailment::Burn), 0),
            (EventKind::LowLife(LifeMark::Half), 1),
            (EventKind::LowLife(LifeMark::Low), 1),
            (EventKind::Kill, 0),
        ]
    );
    close(es[4].damage, 50.0);
    close(es[6].stacks, 3.0);
    assert!(es.iter().all(|e| e.crit));
}

#[test]
fn a_dodge_gives_evaded_alone_and_no_striker_gives_only_the_target_side() {
    let o = Outcome {
        evaded: true,
        ..Outcome::default()
    };
    let t = Cultivator::new();
    let blow = Blow {
        by: Some(0),
        on: 1,
        skill: None,
        tags: attack(),
        depth: 0,
        life_before: t.life,
        shield_before: t.shield,
    };
    assert_eq!(
        kinds(&events_of(&o, &blow, &t)),
        vec![(EventKind::Evaded, 1)]
    );

    let mut o = Outcome {
        crit: true,
        ..Outcome::default()
    };
    o.stacks[Ailment::Burn.index()] = 1.0;
    let lava = Blow { by: None, ..blow };
    assert_eq!(
        kinds(&events_of(&o, &lava, &t)),
        vec![(EventKind::BeenHit, 1), (EventKind::BeenCrit, 1)]
    );
}

#[test]
fn a_low_life_line_is_crossed_only_going_down() {
    let o = Outcome::default();
    let mut t = Cultivator::new();
    t.life = 0.4 * t.life_max;
    let blow = Blow {
        by: Some(0),
        on: 1,
        skill: None,
        tags: attack(),
        depth: 0,
        life_before: 0.45 * t.life_max,
        shield_before: 0.0,
    };
    // already under half before: only nothing new; 0.4 is above 0.35
    assert!(
        events_of(&o, &blow, &t)
            .iter()
            .all(|e| !matches!(e.kind, EventKind::LowLife(_)))
    );
    t.life = 0.3 * t.life_max;
    let lows: Vec<_> = events_of(&o, &blow, &t)
        .into_iter()
        .filter(|e| matches!(e.kind, EventKind::LowLife(_)))
        .map(|e| e.kind)
        .collect();
    assert_eq!(lows, vec![EventKind::LowLife(LifeMark::Low)]);
}

// ---- Recent marks

#[test]
fn every_event_marks_its_happened_on_its_fighter() {
    let mut f = two();
    let mut q = Queue::new();
    q.extend([
        Event::new(EventKind::Hit, 0).against(1),
        Event::new(EventKind::Crit, 0).against(1),
        Event::new(EventKind::BeenHit, 1).against(0),
        Event::new(EventKind::ShieldBroke, 1),
        Event::new(EventKind::AilmentLaid(Ailment::Burn), 0),
        Event::new(EventKind::Cast, 1).with_skill("ward", Tags::of(&[Tag::Guard])),
        Event::new(EventKind::Cast, 0).with_skill("blink", Tags::of(&[Tag::Movement])),
        Event::new(EventKind::Blocked, 1),
    ]);
    q.extend(event::curse_events(0, 1, "hex", Tags::of(&[Tag::Curse]), 0));
    q.run(&mut f, &seats(&[], &[]), &mut Dice::new(&[]));
    let on = |c: &Cultivator| -> Vec<Happened> {
        Happened::ALL
            .into_iter()
            .filter(|&h| c.recent.since(h) == 0.0)
            .collect()
    };
    assert_eq!(
        on(&f[0]),
        vec![
            Happened::Hit,
            Happened::Crit,
            Happened::UsedMovementSkill,
            Happened::CursedSomeone,
            Happened::LaidAilment
        ]
    );
    assert_eq!(
        on(&f[1]),
        vec![
            Happened::BeenHit,
            Happened::Guarded,
            Happened::UsedGuardSkill,
            Happened::ShieldBroke,
            Happened::Cursed
        ]
    );
    assert_eq!(f[1].recent.since(Happened::Kill), f64::INFINITY);
}

// ---- each trigger case

#[test]
fn cast_on_crit_fires_on_an_attack_crit_only() {
    let crit = Event::new(EventKind::Crit, 0)
        .against(1)
        .with_skill("slash", attack());
    let out = run_one(CAST_ON_CRIT, crit);
    assert_eq!(out.casts.len(), 1);
    let c = out.casts[0];
    assert_eq!((c.by, c.at, c.skill, c.depth), (0, Some(1), "jithunder", 1));
    close(c.factor, event::CRIT_CAST_FACTOR);
    assert_eq!(c.base, None);

    let spell = Event::new(EventKind::Crit, 0).with_skill("bolt", Tags::of(&[Tag::Spell]));
    assert!(run_one(CAST_ON_CRIT, spell).casts.is_empty());
    let triggered = Event::new(EventKind::Crit, 0).with_skill("slash", attack().with(Tag::Trigger));
    assert!(run_one(CAST_ON_CRIT, triggered).casts.is_empty());
    let hit = Event::new(EventKind::Hit, 0).with_skill("slash", attack());
    assert!(run_one(CAST_ON_CRIT, hit).casts.is_empty());
}

#[test]
fn cast_when_hurt_adds_damage_up_to_its_threshold_then_starts_over() {
    let mut f = two();
    let s = seats(&[CAST_WHEN_HURT], &[]);
    let mut q = Queue::new();
    let hurt = |d: f64| {
        Event::new(EventKind::BeenHit, 0)
            .against(1)
            .with_damage(d, [0.0; N])
    };
    // Thụ Thương Phản Pháp fires once 1500 taken have added up
    assert_eq!(arena_calc::event::HURT_CAST_THRESHOLD, 1500.0);
    q.push(hurt(1200.0));
    assert!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.is_empty());
    close(q.summed(0, "castwhenhurt"), 1200.0);
    q.push(hurt(350.0));
    let out = q.run(&mut f, &s, &mut Dice::new(&[]));
    assert_eq!(out.casts.len(), 1);
    assert_eq!(out.casts[0].skill, "quake");
    close(q.summed(0, "castwhenhurt"), 0.0);
    // a hit dealt does not count
    q.tick(1.0);
    q.push(Event::new(EventKind::Hit, 0).with_damage(1000.0, [0.0; N]));
    assert!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.is_empty());
}

#[test]
fn cast_on_kill_fires_on_a_kill_not_from_a_triggered_blow() {
    let kill = Event::new(EventKind::Kill, 0)
        .against(1)
        .with_skill("slash", attack());
    let out = run_one(CAST_ON_KILL, kill);
    assert_eq!(out.casts.len(), 1);
    assert_eq!(out.casts[0].skill, "fireball");
    let triggered =
        Event::new(EventKind::Kill, 0).with_skill("bolt", Tags::of(&[Tag::Spell, Tag::Trigger]));
    assert!(run_one(CAST_ON_KILL, triggered).casts.is_empty());
    assert!(
        run_one(CAST_ON_KILL, Event::new(EventKind::Hit, 0))
            .casts
            .is_empty()
    );
}

#[test]
fn lien_hoan_casts_with_what_the_shield_absorbed() {
    let broke = Event::new(EventKind::ShieldBroke, 1)
        .against(0)
        .with_damage(120.0, [0.0; N]);
    let mut f = two();
    let s = seats(&[], &[CAST_ON_SHIELD_BREAK]);
    let mut q = Queue::new();
    q.push(broke);
    let out = q.run(&mut f, &s, &mut Dice::new(&[]));
    assert_eq!(out.casts.len(), 1);
    let c = out.casts[0];
    assert_eq!((c.by, c.at, c.skill), (1, Some(0), "palm"));
    assert_eq!(c.base, Some(120.0 * event::GUARD_BREAK_SHARE));
    // the shield of the other fighter breaking is not this fighter's event
    let out = run_one(CAST_ON_SHIELD_BREAK, Event::new(EventKind::ShieldBroke, 1));
    assert!(out.casts.is_empty());
}

#[test]
fn on_hit_vs_cursed_needs_a_cursed_target_and_its_roll() {
    let hit = Event::new(EventKind::Hit, 0).against(1);
    let mut s = seats(&[ON_HIT_VS_CURSED], &[]);

    // not cursed: no roll drawn, nothing
    let mut f = two();
    let mut q = Queue::new();
    let mut d = Dice::new(&[0.0]);
    q.push(hit);
    let out = q.run(&mut f, &s, &mut d);
    assert!(out.fired.is_empty());
    assert_eq!(d.used(), 0);

    s[1].cursed = true;
    // cursed, a bad roll
    let mut d = Dice::new(&[0.9]);
    q.push(hit);
    assert!(q.run(&mut f, &s, &mut d).fired.is_empty());
    assert_eq!(d.used(), 1);
    assert!(f[0].effects.is_empty());

    // cursed, a good roll: the buff goes on
    let mut d = Dice::new(&[0.1]);
    q.push(hit);
    let out = q.run(&mut f, &s, &mut d);
    assert_eq!(out.fired.len(), 1);
    assert!(out.casts.is_empty());
    assert_eq!(f[0].effects.len(), 1);
    assert_eq!(f[0].effects[0].source, Source::Other("cursedhunt"));
    assert_eq!(f[0].effects[0].left, Some(event::CURSED_HUNT_SECONDS));

    // out of dice: a roll of 1, nothing
    let mut d = Dice::new(&[]);
    q.push(hit);
    assert!(q.run(&mut f, &s, &mut d).fired.is_empty());
}

#[test]
fn burn_full_gives_qi_and_another_full_does_not() {
    let mut f = two();
    f[0].qi = 10.0;
    let s = seats(&[ON_BURN_FULL], &[]);
    let mut q = Queue::new();
    q.push(Event::new(EventKind::AilmentFull(Ailment::Bleed), 0));
    q.run(&mut f, &s, &mut Dice::new(&[]));
    close(f[0].qi, 10.0);
    q.push(Event::new(EventKind::AilmentFull(Ailment::Burn), 0));
    q.run(&mut f, &s, &mut Dice::new(&[]));
    close(f[0].qi, 10.0 + event::BURN_FULL_QI);
    // capped at the bar
    q.tick(10.0);
    f[0].qi = f[0].qi_max - 1.0;
    q.push(Event::new(EventKind::AilmentFull(Ailment::Burn), 0));
    q.run(&mut f, &s, &mut Dice::new(&[]));
    close(f[0].qi, f[0].qi_max);
}

#[test]
fn the_herald_blasts_on_killing_a_burning_foe_by_its_stacks() {
    let mut kill = Event::new(EventKind::Kill, 0).against(1);
    assert!(run_one(HERALD_OF_ASH, kill).casts.is_empty());
    kill.ailments[Ailment::Burn.index()] = 5.0;
    let out = run_one(HERALD_OF_ASH, kill);
    assert_eq!(out.casts.len(), 1);
    assert_eq!(out.casts[0].skill, "flameorigin");
    assert_eq!(out.casts[0].at, Some(1));
    close(
        out.casts[0].factor,
        event::HERALD_FACTOR * (1.0 + 5.0 * event::HERALD_PER_STACK),
    );
    // another ailment is not a burn
    let mut bleeding = Event::new(EventKind::Kill, 0).against(1);
    bleeding.ailments[Ailment::Bleed.index()] = 5.0;
    assert!(run_one(HERALD_OF_ASH, bleeding).casts.is_empty());
}

#[test]
fn every_case_is_listed() {
    assert_eq!(event::ALL.len(), 8);
    let mut ids: Vec<_> = event::ALL.iter().map(|t| t.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 8);
}

// ---- cooldowns, chance, caps

#[test]
fn the_internal_cooldown_runs_down_with_dt() {
    let mut f = two();
    let s = seats(&[CAST_ON_CRIT], &[]);
    let mut q = Queue::new();
    let crit = Event::new(EventKind::Crit, 0).with_skill("slash", attack());
    q.push(crit);
    assert_eq!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.len(), 1);
    close(q.cooldown(0, "castoncrit"), event::CRIT_CAST_COOLDOWN);
    q.push(crit);
    assert!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.is_empty());
    q.tick(0.1);
    q.push(crit);
    assert!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.is_empty());
    q.tick(0.2);
    close(q.cooldown(0, "castoncrit"), 0.0);
    q.push(crit);
    assert_eq!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.len(), 1);
    // one fighter's cooldown is not the other's
    let s2 = seats(&[CAST_ON_CRIT], &[CAST_ON_CRIT]);
    q.push(Event::new(EventKind::Crit, 1).with_skill("slash", attack()));
    assert_eq!(q.run(&mut f, &s2, &mut Dice::new(&[])).casts.len(), 1);
}

#[test]
fn chance_rolls_are_drawn_in_fighter_then_event_order() {
    let mut f = two();
    let mut s = seats(&[ON_HIT_VS_CURSED], &[ON_HIT_VS_CURSED]);
    s[0].cursed = true;
    s[1].cursed = true;
    let mut q = Queue::new();
    // pushed fighter 1 first: fighter 0 still draws first
    q.push(Event::new(EventKind::Hit, 1).against(0));
    q.push(Event::new(EventKind::Hit, 0).against(1));
    q.push(Event::new(EventKind::Hit, 0).against(1));
    let mut d = Dice::new(&[0.9, 0.1, 0.2]);
    let out = q.run(&mut f, &s, &mut d);
    assert_eq!(d.used(), 3);
    let by: Vec<_> = out.fired.iter().map(|x| x.by).collect();
    // fighter 0's first roll 0.9 misses, its second 0.1 hits; fighter 1's 0.2 hits
    assert_eq!(by, vec![0, 1]);
}

const SELF_LOOP: Trigger = Trigger {
    id: "selfloop",
    name: "Vòng Lặp",
    on: EventKind::Cast,
    when: Condition::Always,
    filter: Filter {
        skill: Some("bolt"),
        ..Filter::ANY
    },
    chance: 1.0,
    cooldown: 0.0,
    threshold: 0.0,
    does: Effect::Cast(TriggeredCast {
        skill: "bolt",
        level: 1,
        factor: 1.0,
        scale: Scale::Skill,
    }),
};

#[test]
fn a_trigger_that_triggers_itself_stops_at_the_chain_depth() {
    let mut f = two();
    let s = seats(&[SELF_LOOP], &[]);
    let mut q = Queue::new();
    q.push(Event::new(EventKind::Cast, 0).with_skill("bolt", Tags::of(&[Tag::Spell])));
    let mut casts = Vec::new();
    let mut too_deep = 0;
    for _ in 0..100 {
        let out = q.run(&mut f, &s, &mut Dice::new(&[]));
        too_deep += out.too_deep;
        if out.casts.is_empty() && q.pending().is_empty() {
            break;
        }
        for c in out.casts {
            // the fight would build and resolve it; here it only reports the cast
            q.push(
                Event::new(EventKind::Cast, c.by)
                    .with_skill(c.skill, Tags::of(&[Tag::Spell, Tag::Trigger]))
                    .at_depth(c.depth),
            );
            casts.push(c);
        }
    }
    assert_eq!(casts.len(), usize::from(CHAIN_DEPTH));
    let depths: Vec<_> = casts.iter().map(|c| c.depth).collect();
    assert_eq!(depths, (1..=CHAIN_DEPTH).collect::<Vec<_>>());
    assert_eq!(too_deep, 1);
}

#[test]
fn a_fighter_fires_at_most_the_per_tick_cap() {
    const ON_HIT: Trigger = Trigger {
        id: "onhit",
        on: EventKind::Hit,
        ..SELF_LOOP
    };
    let on_hit = Trigger {
        filter: Filter::ANY,
        ..ON_HIT
    };
    let mut f = two();
    let s = seats(&[on_hit], &[on_hit]);
    let mut q = Queue::new();
    for _ in 0..20 {
        q.push(Event::new(EventKind::Hit, 0));
    }
    q.push(Event::new(EventKind::Hit, 1));
    let out = q.run(&mut f, &s, &mut Dice::new(&[]));
    let by0 = out.casts.iter().filter(|c| c.by == 0).count();
    assert_eq!(by0 as u32, TRIGGERS_PER_TICK);
    assert_eq!(out.capped, 20 - TRIGGERS_PER_TICK);
    // the other fighter has its own count
    assert_eq!(out.casts.iter().filter(|c| c.by == 1).count(), 1);
    // still capped in a second run of the same tick, free after the tick
    q.push(Event::new(EventKind::Hit, 0));
    assert!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.is_empty());
    q.tick(0.016);
    q.push(Event::new(EventKind::Hit, 0));
    assert_eq!(q.run(&mut f, &s, &mut Dice::new(&[])).casts.len(), 1);
}

// ---- determinism

fn scenario() -> (Vec<Output>, Vec<Cultivator>, Queue) {
    let mut f = two();
    let mut s = seats(
        &[CAST_ON_CRIT, ON_HIT_VS_CURSED, HERALD_OF_ASH, ON_BURN_FULL],
        &[CAST_WHEN_HURT, CAST_ON_SHIELD_BREAK, CAST_ON_KILL],
    );
    s[1].cursed = true;
    let rolls = [0.3, 0.7, 0.1, 0.45, 0.05, 0.99];
    let mut d = Dice::new(&rolls);
    let mut q = Queue::new();
    let mut outs = Vec::new();
    for step in 0..6u32 {
        let mut kill = Event::new(EventKind::Kill, 0)
            .against(1)
            .with_skill("slash", attack())
            .with_damage(400.0, [0.0; N]);
        kill.ailments[Ailment::Burn.index()] = f64::from(step);
        q.extend([
            Event::new(EventKind::Hit, 0)
                .against(1)
                .with_skill("slash", attack()),
            Event::new(EventKind::BeenHit, 1)
                .against(0)
                .with_damage(130.0, [0.0; N]),
            Event::new(EventKind::Crit, 0)
                .against(1)
                .with_skill("slash", attack()),
            Event::new(EventKind::ShieldBroke, 1).with_damage(60.0, [0.0; N]),
            Event::new(EventKind::AilmentFull(Ailment::Burn), 0),
            kill,
        ]);
        outs.push(q.run(&mut f, &s, &mut d));
        q.tick(0.1);
        for c in &mut f {
            c.recent.tick(0.1);
        }
    }
    (outs, f, q)
}

#[test]
fn the_same_inputs_give_the_same_output_twice() {
    let a = scenario();
    let b = scenario();
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    assert_eq!(a.2, b.2);
    assert!(a.0.iter().any(|o| !o.casts.is_empty()));
}

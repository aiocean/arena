//! What the policy sees: every value finite and bounded, its tag at the end, at most four threats.

mod common;

use arena_mind::policy::{BLOCKS, OBS, TAG, UNITS, observe};
use arena_mind::*;
use common::*;

fn slots() -> Vec<Option<usize>> {
    vec![Some(0), None, Some(2), Some(1), Some(3), Some(6)]
}

fn bolt_at(x: f64) -> Threat {
    Threat {
        age: 0.5,
        damage: 13.0,
        shape: ThreatShape::Projectile {
            x,
            y: 20.0,
            vx: -230.0,
        },
    }
}

#[test]
fn every_observation_value_is_finite_and_bounded() {
    let kit = kit();
    let tag = [1.0, 0.0, 0.0, 1.0, 0.0];
    let doings = [
        Doing::Idle,
        Doing::Move { dir: -1.0 },
        Doing::Guard,
        blade_windup(0.1),
        Doing::Act {
            kind: SkillKind::Blink,
            phase: ActPhase::Recover,
            t: 9.0,
            windup: 0.0,
            recover: 0.0,
            reach: f64::NAN,
            damage: 1e9,
        },
    ];
    for (mx, fx) in [(0.0, 220.0), (220.0, 0.0), (110.0, 110.0), (-50.0, 400.0)] {
        for doing in &doings {
            let mut s = sense(3.0, me(mx), foe(fx));
            s.slots = slots();
            s.me.vx = 1e6;
            s.slotx[1] = Some(SlotX {
                damage: f64::NAN,
                cooldown: f64::INFINITY,
                inflict: [1e9; AILMENTS],
                supports: u128::MAX,
                ..SlotX::default()
            });
            s.effects[0].ailments[5] = AilmentOn {
                stacks: 5.0,
                cap: 0.0,
                left: f64::INFINITY,
                strength: -1e9,
            };
            s.units = vec![UnitSeen {
                mine: false,
                kind: UnitKind::Trap,
                x: f64::NAN,
                y: 1e9,
                hp: 1.0,
                left: Some(f64::NAN),
                armed: false,
                next: 0.0,
            }];
            s.me_act = Some(ActFrames {
                hitbox: Some([f64::NAN, 0.0, 1e9, -1e9]),
                to_window: f64::INFINITY,
                ..ActFrames::default()
            });
            let mut f = foe(fx);
            f.doing = doing.clone();
            f.out_for = 50.0;
            let threats = vec![
                bolt_at(mx + 30.0),
                Threat {
                    age: 0.0,
                    damage: f64::NAN,
                    shape: ThreatShape::Area {
                        x: fx,
                        radius: f64::NAN,
                        reach: 0.0,
                        lands_in: -3.0,
                        grounded: true,
                    },
                },
            ];
            let o = observe(&s, &f, &threats, &kit, &tag);
            assert_eq!(o.len(), OBS);
            for (i, v) in o.iter().enumerate() {
                assert!(
                    v.is_finite() && (-4.0..=4.0).contains(v),
                    "obs[{i}] = {v} at me {mx} foe {fx} doing {doing:?}"
                );
            }
        }
    }
}

#[test]
fn the_tag_lands_where_abi_2_ended_unchanged() {
    let tag = [0.0, 1.0, 0.0, 0.0, 1.0];
    let s = sense(1.0, me(40.0), foe(150.0));
    let o = observe(&s, &foe(150.0), &[], &kit(), &tag);
    assert_eq!(&o[TAG_AT..TAG_AT + TAG], &tag);
    assert!(
        o[TAG_AT + TAG..].iter().all(|&v| v == 0.0),
        "nothing else sensed"
    );
}

#[test]
fn the_view_is_the_same_from_either_side() {
    // a fighter on the left facing a foe on its right sees what its mirror image sees
    let kit = kit();
    let tag = [0.0; TAG];
    let a = observe(
        &sense(1.0, me(40.0), foe(150.0)),
        &foe(150.0),
        &[bolt_at(100.0)],
        &kit,
        &tag,
    );
    let mut m = me(180.0);
    m.face = -1.0;
    let mut b_threat = bolt_at(120.0);
    if let ThreatShape::Projectile { vx, .. } = &mut b_threat.shape {
        *vx = 230.0;
    }
    let b = observe(
        &sense(1.0, m, foe(70.0)),
        &foe(70.0),
        &[b_threat],
        &kit,
        &tag,
    );
    assert_eq!(a, b);
}

#[test]
fn threats_past_the_fourth_nearest_are_dropped_and_missing_ones_are_zero() {
    let kit = kit();
    let tag = [0.0; TAG];
    let s = sense(1.0, me(40.0), foe(150.0));
    let none = observe(&s, &foe(150.0), &[], &kit, &tag);
    let near: Vec<Threat> = [60.0, 70.0, 80.0, 90.0]
        .iter()
        .map(|&x| bolt_at(x))
        .collect();
    let mut more = near.clone();
    more.push(bolt_at(200.0));
    more.insert(0, bolt_at(210.0));
    let four = observe(&s, &foe(150.0), &near, &kit, &tag);
    let six = observe(&s, &foe(150.0), &more, &kit, &tag);
    assert_eq!(four, six, "the two far bolts are not seen");
    assert_ne!(none, four);
    let one = observe(&s, &foe(150.0), &near[..1], &kit, &tag);
    let differing = one.iter().zip(&none).filter(|(a, b)| a != b).count();
    assert!(
        differing > 0 && differing <= 12,
        "one threat fills one threat's group only: {differing}"
    );
}

// The documented layout (`observe.rs`'s header, `docs/bot.md`): ABI 2's me 17, foe 37, threats 4 x 12, slots 6 x 33,
// tag 5, whose indices never move; then ABI 3's two casts 11 each, slot builds 6 x 148, effects 2 x 113, units
// 8 x 12, the foe's slots 6 x 34; then ABI 4's why 6 x 9, the foe's bands 6 x 4, buffs 4 x 5, in play 10 x 5.
const MY_WINDUP: usize = 14;
const MY_RECOVER: usize = 15;
const MY_LEFT: usize = 16;
const FOE_LEFT: usize = 17 + 26;
const FOE_SPAN: usize = 17 + 27;
const SLOTS_AT: usize = 17 + 37 + 4 * 12;
const SLOT: usize = 33;
const SLOT_STUN: usize = 21;
const SLOT_CHILL: usize = 27;

const TAG_AT: usize = 300;
const ME_ACT: usize = 305;
const FOE_ACT: usize = 316;
const SLOTX_AT: usize = 327;
const SLOTX: usize = 148;
const EFFECTS_AT: usize = 1215;
const EFFECT: usize = 113;
const UNITS_AT: usize = 1441;
const UNIT: usize = 12;
const FOE_SLOTS_AT: usize = 1537;
const FOE_SLOT: usize = 34;
const WHY_AT: usize = 1741;
const WHY: usize = 9;
const FOE_BANDS_AT: usize = 1795;
const BUFFS_AT: usize = 1819;
const IN_PLAY_AT: usize = 1839;
const IN_PLAY: usize = 5;
const MY_ACTS_LEN: usize = 3 * 30;
const FOE_ACTS_LEN: usize = 4 * 19;

#[test]
fn obs_is_the_documented_sum() {
    assert_eq!(
        TAG_AT,
        17 + 37 + 4 * 12 + 6 * 33,
        "ABI 2's inputs keep their places"
    );
    assert_eq!(
        OBS,
        TAG_AT
            + TAG
            + 2 * 11
            + 6 * SLOTX
            + 2 * EFFECT
            + UNITS * UNIT
            + 6 * FOE_SLOT
            + 6 * WHY
            + 6 * 4
            + 4 * 5
            + 10 * IN_PLAY
            + MY_ACTS_LEN
            + FOE_ACTS_LEN
            + 5 * 30
            + 4 * 19
            + 9 * 2
    );
    assert_eq!(WHY_AT, 1741, "ABI 3's inputs keep their places");
    assert_eq!(
        WHY_AT + 6 * WHY + 6 * 4 + 4 * 5 + 10 * IN_PLAY,
        1889,
        "ABI 4's inputs keep their places"
    );
    // ABI 5 appends what it remembers: 3 of its own acts (29 acts + age), 4 of the foe's (19 each)
    assert_eq!(
        1889 + 3 * 30 + 4 * 19,
        2055,
        "ABI 5's inputs keep their places"
    );
    // ABI 6 appends 5 older acts of its own, 4 older starts of the foe's, and the foe's 9 kinds (seen, age)
    assert_eq!(OBS, 2055 + 5 * 30 + 4 * 19 + 9 * 2);
    let starts: Vec<usize> = BLOCKS
        .iter()
        .scan(0, |at, b| {
            let start = *at;
            *at += b.1;
            Some(start)
        })
        .collect();
    assert_eq!(
        starts,
        [
            0,
            17,
            54,
            102,
            TAG_AT,
            ME_ACT,
            FOE_ACT,
            SLOTX_AT,
            EFFECTS_AT,
            UNITS_AT,
            FOE_SLOTS_AT,
            WHY_AT,
            FOE_BANDS_AT,
            BUFFS_AT,
            IN_PLAY_AT,
            1889,
            1889 + MY_ACTS_LEN,
            2055,
            2205,
            2281
        ]
    );
    assert_eq!(BLOCKS.iter().map(|b| b.1).sum::<usize>(), OBS);
}

/// `docs/bot.md`'s table of what a bot sees: each row's range and its block's name (the first bold words).
fn documented() -> Vec<(usize, usize, String)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/bot.md");
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let (a, b) = cells.get(1)?.split_once("..")?;
            let name = cells.get(3)?.strip_prefix("**")?.split("**").next()?;
            Some((
                a.parse().ok()?,
                b.parse().ok()?,
                name.trim_end_matches('.').to_string(),
            ))
        })
        .collect()
}

#[test]
fn the_documented_table_is_the_layout() {
    let mut at = 0;
    let want: Vec<(usize, usize, String)> = BLOCKS
        .iter()
        .map(|&(name, len)| {
            at += len;
            (at - len, at, name.to_string())
        })
        .collect();
    assert_eq!(documented(), want, "docs/bot.md, \"What it sees\"");
    assert_eq!(want.last().map(|w| w.1), Some(OBS));
}

#[test]
fn the_foes_seconds_left_fall_as_its_windup_runs() {
    let s = sense(1.0, me(40.0), foe(150.0));
    let at = |t: f64| {
        let mut f = foe(150.0);
        f.doing = blade_windup(t);
        observe(&s, &f, &[], &kit(), &[0.0; TAG])
    };
    let (early, late) = (at(0.05), at(0.15));
    assert!(
        (v(&early, FOE_LEFT) - 0.15).abs() < 1e-6,
        "{}",
        v(&early, FOE_LEFT)
    );
    assert!(
        (v(&late, FOE_LEFT) - 0.05).abs() < 1e-6,
        "{}",
        v(&late, FOE_LEFT)
    );
    assert!((v(&early, FOE_SPAN) - 0.2).abs() < 1e-6);
}

#[test]
fn its_own_recover_flag_is_set_while_it_recovers() {
    let mut m = me(40.0);
    let idle = observe(
        &sense(1.0, m.clone(), foe(150.0)),
        &foe(150.0),
        &[],
        &kit(),
        &[0.0; TAG],
    );
    assert_eq!(
        [v(&idle, MY_WINDUP), v(&idle, MY_RECOVER), v(&idle, MY_LEFT)],
        [0.0, 0.0, 0.0]
    );
    m.act = Some(Acting {
        phase: ActPhase::Recover,
        t: 0.1,
        windup: 0.2,
        recover: 0.32,
    });
    let o = observe(
        &sense(1.0, m, foe(150.0)),
        &foe(150.0),
        &[],
        &kit(),
        &[0.0; TAG],
    );
    assert_eq!(v(&o, MY_WINDUP), 0.0);
    assert_eq!(v(&o, MY_RECOVER), 1.0);
    assert!((v(&o, MY_LEFT) - 0.22).abs() < 1e-6, "{}", v(&o, MY_LEFT));
}

#[test]
fn a_slot_of_a_stunning_skill_carries_its_stun() {
    let mut kit = kit();
    kit[4].inflicts.stun = 0.6;
    kit[3].inflicts.chill = 1.0;
    kit[3].inflicts.chill_max = 3.0;
    let mut s = sense(1.0, me(40.0), foe(150.0));
    s.slots = slots();
    s.stacks = vec![1.0, 1.0, 1.0, 2.0];
    let o = observe(&s, &foe(150.0), &[], &kit, &[0.0; TAG]);
    // slot 4 holds skill 3 (the falling area), slot 6 skill 6; skill 4 (the barrier) is in no slot
    assert!((v(&o, SLOTS_AT + 4 * SLOT + SLOT_CHILL) - 2.0 / 3.0).abs() < 1e-6);
    s.slots[1] = Some(4);
    let o = observe(&s, &foe(150.0), &[], &kit, &[0.0; TAG]);
    assert!((v(&o, SLOTS_AT + SLOT + SLOT_STUN) - 0.6).abs() < 1e-6);
    assert_eq!(v(&o, SLOTS_AT + SLOT_STUN), 0.0, "the blade does not stun");
}

fn observed(s: &Sense) -> [f32; OBS] {
    observe(s, &s.foe, &[], &kit(), &[0.0; TAG])
}

#[test]
fn a_bleed_on_the_foe_shows_in_its_effects() {
    let mut s = sense(1.0, me(40.0), foe(150.0));
    let none = observed(&s);
    s.effects[1].ailments[0] = AilmentOn {
        stacks: 2.0,
        cap: 8.0,
        left: 3.0,
        strength: 0.02,
    };
    s.effects[1].element = Some(2);
    let o = observed(&s);
    let foe = EFFECTS_AT + EFFECT;
    assert_eq!(
        [v(&o, foe), v(&o, foe + 1), v(&o, foe + 2), v(&o, foe + 3)],
        [0.25, 0.8, 0.3, 0.2]
    );
    assert_eq!(v(&o, foe + EFFECT - 7 + 2), 1.0, "its root");
    let differing = o.iter().zip(&none).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 5, "only the foe's bleed and root");
    assert_eq!(v(&o, EFFECTS_AT), 0.0, "not on me");
}

#[test]
fn units_show_nearest_first_from_my_side() {
    let unit = |x: f64, mine: bool| UnitSeen {
        mine,
        kind: UnitKind::Minion,
        x,
        y: 24.0,
        hp: 0.5,
        left: None,
        armed: true,
        next: 0.4,
    };
    let mut s = sense(1.0, me(40.0), foe(150.0));
    // nine units: the farthest is not seen
    s.units = (0..9)
        .map(|i| unit(200.0 - 10.0 * i as f64, i == 8))
        .collect();
    let o = observed(&s);
    let first = UNITS_AT;
    assert_eq!(v(&o, first), 1.0);
    assert_eq!(v(&o, first + 1), 1.0, "the nearest is mine");
    assert_eq!(&o[first + 2..first + 6], &[0.0, 0.0, 0.0, 1.0], "a minion");
    assert!(
        (v(&o, first + 6) - 0.8).abs() < 1e-6,
        "{}",
        v(&o, first + 6)
    );
    assert!((v(&o, first + 7) - 0.48).abs() < 1e-6);
    assert_eq!(v(&o, first + 9), 4.0, "it lasts: 40 s");
    let last = UNITS_AT + (UNITS - 1) * UNIT;
    assert!((v(&o, last + 6) - 1.5).abs() < 1e-6, "the eighth: 190");
    assert_eq!(v(&o, UNITS_AT + UNITS * UNIT), 0.0, "the next block");
}

#[test]
fn a_slot_shows_its_cooldown_and_its_build() {
    let mut s = sense(1.0, me(40.0), foe(150.0));
    s.slots = slots();
    let plain = observed(&s);
    let x = SlotX {
        cooldown_left: 1.2,
        cooldown: 1.5,
        stored: 0.0,
        max_uses: 1.0,
        qi: 25.0,
        damage: 20.0,
        pierce: 2.0,
        inflict: [0.5; AILMENTS],
        supports: 1 << 3 | 1 << 118,
        ..SlotX::default()
    };
    s.slotx[2] = Some(x);
    let o = observed(&s);
    let at = SLOTX_AT + 2 * SLOTX;
    assert_eq!([v(&o, at), v(&o, at + 1)], [1.2, 0.15]);
    assert_eq!(v(&o, at + 14), 0.5, "pierce / 4");
    assert_eq!(v(&o, at + 18), 0.5, "the first ailment");
    assert_eq!(v(&o, at + 29 + 3), 1.0);
    assert_eq!(v(&o, at + 29 + 118), 1.0);
    assert_eq!(v(&o, at + 29 + 4), 0.0);
    // the ABI 2 slot reads the built cost and damage
    let slot = SLOTS_AT + 2 * SLOT;
    assert_eq!([v(&o, slot + 14), v(&o, slot + 15)], [0.5, 0.4]);
    assert_ne!(v(&plain, slot + 14), 0.5);
}

#[test]
fn the_foes_cast_shows_once_it_is_seen() {
    let mut s = sense(1.0, me(40.0), foe(150.0));
    s.foe.doing = blade_windup(0.05);
    s.foe_act = Some(ActFrames {
        to_window: 0.15,
        window: 0.2,
        to_punish: 0.15,
        recover: 0.32,
        hitbox: Some([120.0, 10.0, 145.0, 30.0]),
        ..ActFrames::default()
    });
    let mut idle = s.foe.clone();
    idle.doing = Doing::Idle;
    let hidden = observe(&s, &idle, &[], &kit(), &[0.0; TAG]);
    assert!(
        hidden[FOE_ACT..FOE_ACT + 11].iter().all(|&x| x == 0.0),
        "not seen yet"
    );
    let o = observed(&s);
    assert_eq!(v(&o, FOE_ACT), 1.0);
    assert!((v(&o, FOE_ACT + 2) - 0.15).abs() < 1e-6);
    assert_eq!(v(&o, FOE_ACT + 6), 1.0, "a hitbox");
    assert!(
        (v(&o, FOE_ACT + 7) - 0.8).abs() < 1e-6,
        "near edge 80 ahead"
    );
    assert!((v(&o, FOE_ACT + 8) - 1.05).abs() < 1e-6);
    assert!((v(&o, FOE_ACT + 10) - 0.6).abs() < 1e-6);
    assert!(
        o[ME_ACT..FOE_ACT].iter().all(|&x| x == 0.0),
        "I do not cast"
    );
}

#[test]
fn the_foes_slots_show_its_kind_and_build() {
    let mut s = sense(1.0, me(40.0), foe(150.0));
    s.foe_slots[4] = Some(FoeSlot {
        kind: SkillKind::Bolt,
        ready: true,
        reach: 200.0,
        built: SlotX {
            cooldown_left: 0.5,
            stored: 2.0,
            windup: 0.3,
            ..SlotX::default()
        },
        band: None,
        in_band: true,
    });
    let o = observed(&s);
    let at = FOE_SLOTS_AT + 4 * FOE_SLOT;
    assert_eq!(&o[at..at + 4], &[1.0, 1.0, 0.5, 0.5]);
    assert_eq!(v(&o, at + 4 + 4), 1.0, "a bolt");
    assert!((v(&o, at + 16) - 0.3).abs() < 1e-6);
    assert_eq!(v(&o, at + 22), 2.0, "reach / 100");
    assert!(o[FOE_SLOTS_AT..at].iter().all(|&x| x == 0.0));
}

#[test]
fn a_slot_says_its_band_and_why_it_cannot_be_cast() {
    let mut s = sense(1.0, me(40.0), foe(150.0));
    s.slots = slots();
    s.why[2] = Some(SlotWhy {
        band: Some(Band {
            min: 56.0,
            max: 160.0,
        }),
        in_band: false,
        too_near: true,
        cooling: true,
        ..SlotWhy::default()
    });
    s.foe_slots[1] = Some(FoeSlot {
        kind: SkillKind::Blast,
        ready: false,
        reach: 26.0,
        built: SlotX::default(),
        band: Some(Band {
            min: 0.0,
            max: 48.0,
        }),
        in_band: false,
    });
    let o = observed(&s);
    let at = WHY_AT + 2 * WHY;
    assert_eq!(
        &o[at..at + WHY],
        &[1.0, 0.56, 1.6, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
        "has a band, 56..160, out of it, cooling, too near"
    );
    assert!(
        o[WHY_AT..at].iter().all(|&x| x == 0.0),
        "the others say nothing"
    );
    let at = FOE_BANDS_AT + 4;
    assert_eq!(&o[at..at + 4], &[1.0, 0.0, 0.48, 0.0]);
}

#[test]
fn what_is_in_play_shows_how_long_it_lasts() {
    let mut s = sense(1.0, me(40.0), foe(150.0));
    s.lasting = Lasting {
        buffs: vec![
            BuffSeen {
                kind: BuffKind::Blessing,
                left: 8.0,
            },
            BuffSeen {
                kind: BuffKind::Stance,
                left: 3.0,
            },
        ],
        domains: vec![
            InPlay {
                mine: false,
                left: Some(6.0),
                works_in: 0.0,
                x: Some(140.0),
            },
            InPlay {
                mine: true,
                left: Some(12.0),
                works_in: 0.0,
                x: Some(40.0),
            },
        ],
        fields: Vec::new(),
        auras: vec![InPlay {
            mine: true,
            left: None,
            works_in: 0.5,
            x: None,
        }],
    };
    let o = observed(&s);
    // the stance ends first
    assert_eq!(&o[BUFFS_AT..BUFFS_AT + 5], &[1.0, 1.0, 0.0, 0.0, 0.3]);
    assert_eq!(&o[BUFFS_AT + 5..BUFFS_AT + 10], &[1.0, 0.0, 1.0, 0.0, 0.8]);
    // my domain first, under me; the foe's 100 ahead
    assert_eq!(&o[IN_PLAY_AT..IN_PLAY_AT + 5], &[1.0, 1.0, 1.2, 0.0, 0.0]);
    assert_eq!(
        &o[IN_PLAY_AT + 5..IN_PLAY_AT + 10],
        &[1.0, 0.0, 0.6, 0.0, 1.0]
    );
    // no field; my aura held, working in half a second
    let auras = IN_PLAY_AT + 6 * IN_PLAY;
    assert!(o[IN_PLAY_AT + 10..auras].iter().all(|&x| x == 0.0));
    assert_eq!(&o[auras..auras + 5], &[1.0, 1.0, 4.0, 0.5, 0.0]);
}

/// Value `i` of an observation, NaN past its end.
fn v(o: &[f32], i: usize) -> f32 {
    o.get(i).copied().unwrap_or(f32::NAN)
}

/// Where a named block starts (`OBS` for none).
fn block_at(name: &str) -> usize {
    let mut at = 0;
    for (n, len) in BLOCKS {
        if n == name {
            return at;
        }
        at += len;
    }
    OBS
}

#[test]
fn it_remembers_its_last_acts_and_the_foes_newest_first_with_their_age() {
    use arena_mind::policy::{ACTIONS, Act, FoeAct, Memory, observe_with};
    let s = sense(3.0, me(40.0), foe(150.0));
    let mut m = Memory::default();
    let none = observe_with(&s, &foe(150.0), &[], &kit(), &[0.0; TAG], &m);
    assert_eq!(
        none,
        observe(&s, &foe(150.0), &[], &kit(), &[0.0; TAG]),
        "an empty memory reads all zero"
    );
    let mine = block_at("My acts");
    assert!(none[mine..].iter().all(|&x| x == 0.0));
    m.chose(Act::Duck.index(), 2.0);
    m.chose(Act::Toward.index(), 2.5);
    m.saw(FoeAct::Idle, 1.0);
    m.saw(FoeAct::Idle, 1.5); // the same thing again is not a new start
    m.saw(FoeAct::Guard, 2.2);
    let o = observe_with(&s, &foe(150.0), &[], &kit(), &[0.0; TAG], &m);
    let entry = ACTIONS + 1;
    // newest first: walking toward 0.5 s ago, then the duck 1 s ago, then nothing
    assert_eq!(o[mine + Act::Toward.index()], 1.0);
    assert!((o[mine + ACTIONS] - 0.25).abs() < 1e-6);
    assert_eq!(o[mine + entry + Act::Duck.index()], 1.0);
    assert!((o[mine + entry + ACTIONS] - 0.5).abs() < 1e-6);
    assert!(
        o[mine + 2 * entry..mine + 3 * entry]
            .iter()
            .all(|&x| x == 0.0)
    );
    let theirs = block_at("Foe's acts");
    let entry = 6 + 12 + 1;
    assert_eq!(o[theirs + 3], 1.0, "guarding, newest");
    assert_eq!(o[theirs + entry], 1.0, "idle before it");
    assert!(
        (o[theirs + entry + entry - 1] - 1.0).abs() < 1e-6,
        "idle began 2 s ago"
    );
    assert!(o[theirs + 2 * entry..].iter().all(|&x| x == 0.0));
    assert_eq!(theirs + 4 * entry, 2055);
}

#[test]
fn it_remembers_further_back_and_when_the_foe_last_used_each_kind() {
    use arena_mind::SkillKind;
    use arena_mind::policy::{ACTIONS, Act, FoeAct, Memory, observe_with};
    let s = sense(8.5, me(40.0), foe(150.0));
    let mut m = Memory::default();
    // nine acts: the newest three are ABI 5's block, the next five ABI 6's, the oldest forgotten
    for k in 0..9 {
        m.chose(Act::Toward.index(), f64::from(k));
    }
    m.saw(FoeAct::Skill(SkillKind::Bolt), 4.0);
    m.saw(FoeAct::Idle, 5.0);
    m.saw(FoeAct::Skill(SkillKind::Blast), 6.0);
    m.saw(FoeAct::Idle, 7.0);
    m.saw(FoeAct::Skill(SkillKind::Bolt), 8.0);
    let o = observe_with(&s, &foe(150.0), &[], &kit(), &[0.0; TAG], &m);
    let entry = ACTIONS + 1;
    let older = block_at("My older acts");
    // my fourth newest, chosen at 5 s: 3.5 s ago / 2
    assert_eq!(o[older + Act::Toward.index()], 1.0);
    assert!((o[older + ACTIONS] - 1.75).abs() < 1e-6);
    // the fifth older one is the eighth newest, at 1 s; the ninth (at 0 s) is gone
    assert!((o[older + 4 * entry + ACTIONS] - 3.75).abs() < 1e-6);
    // five starts: the newest four in ABI 5's block, the first bolt (4.5 s ago) the newest of the older ones
    let foe_older = block_at("Foe's older acts");
    let foe_entry = 6 + 12 + 1;
    assert_eq!(
        o[foe_older + 6 + 3 + 1],
        1.0,
        "a bolt: the second kind, after 6 doings and 3 reserved"
    );
    assert!((o[foe_older + foe_entry - 1] - 2.25).abs() < 1e-6);
    assert!(
        o[foe_older + foe_entry..block_at("Foe's kinds")]
            .iter()
            .all(|&x| x == 0.0)
    );
    let kinds = block_at("Foe's kinds");
    // Blast is the first kind, Bolt the second: the blast 2.5 s ago, the last bolt 0.5 s ago (each / 10)
    assert_eq!(o[kinds], 1.0);
    assert!((o[kinds + 1] - 0.25).abs() < 1e-6);
    assert_eq!(o[kinds + 2], 1.0);
    assert!((o[kinds + 3] - 0.05).abs() < 1e-6);
    assert!(
        o[kinds + 4..].iter().all(|&x| x == 0.0),
        "kinds it never saw"
    );
    assert_eq!(kinds + 18, OBS);
}

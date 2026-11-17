//! The daos (rules 6.1 to 6.8) and curses (rules 6.9, research 6.1), and the rows of rules 10 that touch them.

use arena_calc::ailment::{Ailment, Control};
use arena_calc::cultivator::{Cultivator, RES, Star};
use arena_calc::curse::{
    self, Boon, Conduits, Curse, Curses, Event, Kind, Lay, Misfire, NUMBERS, Organ, RULES, Reach,
};
use arena_calc::dao::{self, *};
use arena_calc::element::Element;
use arena_calc::gem::Resource;
use arena_calc::grade::{Grade, Rank};
use arena_calc::hit::{Hit, ILLUSION_TRUE_PEN, Rolls, resolve};
use arena_calc::tag::{Tag, Tags};

const EPS: f64 = 1e-9;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn yin(h: &Hit) -> f64 {
    h.base[Element::Yin.index()]
}

// ----- Sinh tử ---------------------------------------------------------------------------------------------------

#[test]
fn life_mark_stores_heal_up_to_a_cap_and_death_triggers_two_and_a_half_times() {
    let mut t = Cultivator::new();
    t.life = 1000.0;
    let mut ld = LifeDeath::default();
    let s = ld.sow(&mut t, 400.0);
    assert!(near(s.healed, 400.0));
    assert!(near(s.stored, 400.0));
    // a second sowing adds, capped at 30% of full life (900)
    let s = ld.sow(&mut t, 600.0);
    assert!(near(s.stored, DAO.mark_cap * t.life_max));
    let h = ld.trigger().map(|h| yin(&h));
    assert_eq!(h, Some(DAO.death_trigger * 900.0));
    // the mark is gone
    assert!(ld.trigger().is_none());
    assert!(sows(Tags::of(&[Tag::Life])) && triggers(Tags::of(&[Tag::Death])));
    assert!(!triggers(Tags::of(&[Tag::Life])));
}

#[test]
fn life_mark_runs_out_after_five_seconds() {
    let mut t = Cultivator::new();
    let mut ld = LifeDeath::default();
    ld.sow(&mut t, 100.0);
    ld.tick(DAO.mark_time - 0.1);
    assert!(ld.mark.is_some());
    ld.tick(0.2);
    assert!(ld.mark.is_none());
    assert!(ld.trigger().is_none());
}

#[test]
fn life_mark_stores_the_heal_even_on_a_full_target() {
    // the opponent at full life still carries the seed for a Death blow
    let mut t = Cultivator::new();
    let mut ld = LifeDeath::default();
    let s = ld.sow(&mut t, 300.0);
    assert!(near(s.healed, 0.0));
    assert!(near(s.stored, 300.0));
}

#[test]
fn sowing_life_on_oneself_is_allowed_and_the_opponents_death_triggers_it() {
    // rules 10: Sinh tử: gieo Sinh lên chính mình
    let mut me = Cultivator::new();
    me.life = 2000.0;
    let mut mine = LifeDeath::default();
    let s = mine.sow(&mut me, 500.0);
    assert!(near(s.healed, 500.0));
    let blow = mine.trigger();
    let Some(blow) = blow else {
        unreachable!("the mark is there")
    };
    let before = me.life;
    let out = resolve(&blow, None, &mut me, Rolls::default());
    assert!(out.to_life > 0.0 && me.life < before);
    // yin passes the shield
    assert!(near(out.to_shield, 0.0));
}

#[test]
fn rebirth_follows_life_sown_within_a_second_of_a_death_trigger() {
    let mut t = Cultivator::new();
    let mut hist = History::new();
    for i in 0..120 {
        hist.record(Sample {
            x: f64::from(i),
            y: 0.0,
            life: 3000.0 - f64::from(i) * 10.0,
            shield: 600.0,
        });
    }
    let mut ld = LifeDeath::default();
    ld.sow(&mut t, 100.0);
    assert!(ld.trigger().is_some());
    ld.tick(0.5);
    let s = ld.sow(&mut t, 100.0);
    assert!(s.rebirth);
    t.life = 100.0;
    t.shield = 0.0;
    let r = rebirth(&mut t, &hist, true);
    let Some(r) = r else {
        unreachable!("the buffer is full")
    };
    // one second back is 60 samples before the newest (index 119)
    assert!(near(r.at.x, 59.0));
    assert!(near(t.life, 3000.0 - 590.0));
    // life only: the shield stays
    assert!(near(t.shield, 0.0));
    assert!(r.cancelled && r.refunded.is_empty());
}

#[test]
fn no_rebirth_past_the_window() {
    let mut t = Cultivator::new();
    let mut ld = LifeDeath::default();
    ld.sow(&mut t, 100.0);
    ld.trigger();
    ld.tick(DAO.rebirth_window + 0.1);
    assert!(!ld.sow(&mut t, 100.0).rebirth);
    // and sowing with no trigger before never rebirths
    let mut fresh = LifeDeath::default();
    assert!(!fresh.sow(&mut t, 100.0).rebirth);
}

#[test]
fn below_ten_percent_a_life_death_cultivator_pays_life_in_death_qi() {
    let mut c = Cultivator::new();
    // above the line: life
    assert_eq!(pay_life_cost(&mut c, true, 300.0), Paid::Life(300.0));
    c.life = 200.0;
    assert!(death_open(&c));
    // an empty bar cannot pay
    assert_eq!(pay_life_cost(&mut c, true, 300.0), Paid::Short);
    // yin dealt fills it: 1 point per 1% of full life
    let got = fill_death_qi(&mut c, true, 600.0);
    assert!(near(got, 20.0));
    // capped at RES.death_max
    fill_death_qi(&mut c, true, 1e6);
    assert!(near(c.death_qi, RES.death_max));
    let life = c.life;
    assert_eq!(pay_life_cost(&mut c, true, 300.0), Paid::DeathQi(10.0));
    assert!(near(c.life, life));
    assert!(near(c.death_qi, RES.death_max - 10.0));
    // a cultivator outside the dao pays life as ever, and fills nothing
    let mut other = Cultivator::new();
    other.life = 200.0;
    assert!(matches!(
        pay_life_cost(&mut other, false, 100.0),
        Paid::Life(_)
    ));
    assert!(near(fill_death_qi(&mut other, false, 600.0), 0.0));
}

#[test]
fn death_qi_fills_only_while_the_bar_is_open() {
    let mut c = Cultivator::new();
    assert!(!death_open(&c));
    assert!(near(fill_death_qi(&mut c, true, 600.0), 0.0));
}

// ----- Nhân quả --------------------------------------------------------------------------------------------------

#[test]
fn a_karmic_thread_echoes_a_share_of_the_blow_with_the_same_kind_and_rank() {
    let th = Thread::new(0, 7, Rank::Rule, 5.0);
    let mut blow = Hit::split(1000.0, &[(Element::Fire, 0.6), (Element::Soul, 0.4)]);
    blow.grade = Grade::Ranked(Rank::Rule);
    blow.crit = 1.0;
    blow.evadable = true;
    let Some((to, echo)) = th.echo(0, &blow) else {
        unreachable!("0 is an end")
    };
    assert_eq!(to, 7);
    assert!(near(
        echo.base[Element::Fire.index()],
        1000.0 * 0.6 * DAO.echo
    ));
    assert!(near(
        echo.base[Element::Soul.index()],
        1000.0 * 0.4 * DAO.echo
    ));
    assert_eq!(echo.grade, Grade::Ranked(Rank::Rule));
    assert!(echo.crit == 0.0 && !echo.evadable);
    // both ways
    assert_eq!(th.echo(7, &blow).map(|e| e.0), Some(0));
    // not an end
    assert!(th.echo(3, &blow).is_none());
}

#[test]
fn a_karmic_thread_runs_out() {
    let mut th = Thread::new(0, 1, Rank::Art, 1.0);
    assert!(th.tick(0.5));
    assert!(!th.tick(0.6));
    assert!(
        th.echo(0, &Hit::split(100.0, &[(Element::Force, 1.0)]))
            .is_none()
    );
}

#[test]
fn karma_qi_sends_back_half_the_blow_that_broke_the_shield() {
    let blow = Hit::split(800.0, &[(Element::Metal, 1.0)]);
    let r = rebound(true, true, &blow);
    assert_eq!(
        r.map(|h| h.base[Element::Metal.index()]),
        Some(800.0 * DAO.rebound)
    );
    assert!(rebound(false, true, &blow).is_none());
    assert!(rebound(true, false, &blow).is_none());
}

#[test]
fn the_tether_holds_the_cursed_within_its_length() {
    let t = Tether::new(0, 1, Rank::Divine);
    assert_eq!(t.hold((0.0, 0.0), (3.0, 0.0)), (3.0, 0.0));
    let (x, y) = t.hold((0.0, 0.0), (6.0, 8.0));
    assert!(near(x, 6.0 * DAO.tether_length / 10.0) && near(y, 8.0 * DAO.tether_length / 10.0));
}

#[test]
fn the_tether_shows_for_half_a_second_every_three() {
    let mut t = Tether::new(0, 1, Rank::Divine);
    assert!(!t.exposed());
    t.tick(2.4);
    assert!(!t.exposed());
    t.tick(0.2);
    assert!(t.exposed());
    t.tick(0.5);
    assert!(!t.exposed());
    t.tick(2.5);
    assert!(t.exposed());
}

#[test]
fn only_a_sword_of_its_rank_or_more_cuts_a_showing_tether() {
    let mut t = Tether::new(0, 1, Rank::Rule);
    let sword = Tags::of(&[Tag::Sword, Tag::Attack]);
    assert!(!t.cut_by(sword, Grade::Ranked(Rank::Faith)), "hidden");
    t.tick(2.75);
    assert!(t.cut_by(sword, Grade::Ranked(Rank::Rule)));
    assert!(t.cut_by(sword, Grade::Ranked(Rank::Origin)));
    assert!(!t.cut_by(sword, Grade::Ranked(Rank::Divine)), "lower rank");
    assert!(
        !t.cut_by(Tags::of(&[Tag::Fist]), Grade::Ranked(Rank::Faith)),
        "not a sword"
    );
    assert!(
        !t.cut_by(sword, Grade::Restriction),
        "a restriction has no rank"
    );
}

#[test]
fn the_curser_dying_costs_the_tethered_half_its_current_life() {
    // rules 10: Người nguyền chết, nguyền còn: Tử Ấn applies 6.2
    let mut cursed = Cultivator::new();
    cursed.life = 2000.0;
    let mut on = Curses::new(1, false);
    assert_eq!(
        on.lay(Curse::new(Kind::DeathSeal, Rank::Divine, 0, 1)),
        Lay::Added
    );
    let lost = on.curser_died(0, &mut cursed);
    assert!(near(lost, 1000.0) && near(cursed.life, 1000.0));
    assert!(on.on.is_empty());
}

#[test]
fn other_curses_just_fall_when_the_curser_dies() {
    let mut cursed = Cultivator::new();
    let mut on = Curses::new(1, true);
    on.lay(Curse::new(Kind::Aging, Rank::Art, 0, 1));
    on.lay(Curse::new(Kind::DreamDao, Rank::Art, 5, 1));
    assert!(near(on.curser_died(0, &mut cursed), 0.0));
    assert!(near(cursed.life, cursed.life_max));
    assert_eq!(on.on.len(), 1);
    assert_eq!(on.on[0].by, 5);
}

#[test]
fn tam_menh_revival_cuts_the_tether() {
    // rules 10: Tam Mệnh kích khi đang bị Tử Ấn
    let mut on = Curses::new(1, false);
    on.lay(Curse::new(Kind::DeathSeal, Rank::Origin, 0, 1));
    assert!(on.tether().is_some());
    assert_eq!(on.sever(), 1);
    assert!(on.tether().is_none());
}

#[test]
fn a_showing_tether_cut_by_a_sword_ends_the_death_seal() {
    let mut on = Curses::new(1, false);
    on.lay(Curse::new(Kind::DeathSeal, Rank::Divine, 0, 1));
    let sword = Tags::of(&[Tag::Sword]);
    assert!(!on.cut_tether(sword, Grade::Ranked(Rank::Divine)));
    let mut cursed = Cultivator::new();
    on.tick(2.6, &mut cursed, false);
    assert!(on.tether().is_some_and(Tether::exposed));
    assert!(on.cut_tether(sword, Grade::Ranked(Rank::Divine)));
    assert!(on.on.is_empty());
}

// ----- Chân giả --------------------------------------------------------------------------------------------------

#[test]
fn an_illusion_deals_half_and_lays_nothing_until_believed() {
    let mut ill = Illusion::new();
    let mut h = Hit::split(1000.0, &[(Element::Force, 1.0)]);
    ill.shape(&mut h);
    assert!(h.illusory);
    // a plain attack is no answer
    assert!(!ill.answer(Tags::of(&[Tag::Attack]), false));
    ill.tick(1.0);
    assert!(ill.answer(Tags::of(&[Tag::Guard]), false));
    ill.shape(&mut h);
    assert!(!h.illusory);
    assert!(near(h.armour_pen, ILLUSION_TRUE_PEN));
}

#[test]
fn a_top_rank_skill_answers_an_illusion() {
    let mut ill = Illusion::new();
    assert!(ill.answer(Tags::of(&[Tag::Spell]), true));
}

#[test]
fn an_illusion_answered_too_late_or_after_landing_stays_false() {
    let mut ill = Illusion::new();
    ill.tick(DAO.believe_window + 0.01);
    assert!(!ill.answer(Tags::of(&[Tag::Guard]), true));
    let mut landed = Illusion::new();
    landed.landed = true;
    assert!(!landed.answer(Tags::of(&[Tag::Guard]), true));
}

#[test]
fn a_movement_answer_makes_it_true_but_a_dodge_is_still_a_dodge() {
    // rules 10: Đối thủ "tin" ảo bằng cách dùng Độn
    let mut ill = Illusion::new();
    assert!(ill.answer(Tags::of(&[Tag::Movement]), false));
    let mut h = Hit::split(1000.0, &[(Element::Force, 1.0)]);
    h.evadable = true;
    ill.shape(&mut h);
    let mut t = Cultivator::new();
    t.evasion = 1.0;
    let out = resolve(&h, None, &mut t, Rolls::default());
    assert!(out.evaded);
}

#[test]
fn glare_or_an_own_true_false_skill_sees_through() {
    let mut c = Cultivator::new();
    assert!(!sees_through(&c, false));
    assert!(sees_through(&c, true));
    c.ailments.add(Ailment::Glare, 1.0, 1.0);
    assert!(sees_through(&c, false));
}

#[test]
fn two_true_false_skills_within_half_a_second_clash_for_a_tenth_of_full_life_as_soul() {
    assert!(clash(10.0, 10.4));
    assert!(!clash(10.0, 10.6));
    let c = Cultivator::new();
    let h = clash_hit(&c);
    assert!(near(h.base[Element::Soul.index()], DAO.clash * c.life_max));
    assert!(h.instant && !h.evadable);
}

// ----- Tuế nguyệt ------------------------------------------------------------------------------------------------

#[test]
fn slow_scales_speed_and_a_still_tree_watcher_is_immune() {
    assert!(near(slowed(0.3, false), 0.7));
    assert!(near(slowed(1.5, false), 0.0));
    assert!(near(slowed(0.3, true), 1.0));
    assert!(needs_lifespan(Tags::of(&[Tag::Time])));
    assert!(!needs_lifespan(Tags::of(&[Tag::Space])));
}

#[test]
fn the_buffer_keeps_120_samples() {
    let mut h = History::new();
    assert!(h.back(0.0).is_none());
    for i in 0..300 {
        h.record(Sample {
            x: f64::from(i),
            ..Sample::default()
        });
    }
    assert_eq!(h.samples.len(), DAO.history);
    assert_eq!(h.back(0.0).map(|s| s.x), Some(299.0));
    assert_eq!(h.back(1.0).map(|s| s.x), Some(239.0));
    // past the buffer: the oldest
    assert_eq!(h.back(5.0).map(|s| s.x), Some(180.0));
}

fn filled() -> History {
    let mut h = History::new();
    for i in 0..120 {
        h.record(Sample {
            x: f64::from(i),
            y: 1.0,
            life: 1000.0 + f64::from(i),
            shield: f64::from(i),
        });
    }
    h
}

#[test]
fn rewind_restores_position_life_and_shield_but_not_spent_resources() {
    let h = filled();
    let mut t = Cultivator::new();
    t.life = 10.0;
    t.shield = 0.0;
    t.qi = 5.0;
    let r = rewind(&mut t, &h, 0.5, None, false);
    let Some(r) = r else {
        unreachable!("the buffer is full")
    };
    assert!(near(r.at.x, 89.0));
    assert!(near(t.life, 1089.0) && near(t.shield, 89.0));
    assert!(near(t.qi, 5.0));
    assert!(!r.cancelled);
}

#[test]
fn rewind_misses_a_still_tree_watcher() {
    let h = filled();
    let mut t = Cultivator::new();
    t.life = 10.0;
    let mut still = Stillness::default();
    still.tick(0.4, false);
    assert!(!still.immune(true));
    still.tick(0.1, false);
    assert!(still.immune(true));
    assert!(!still.immune(false), "only with the keystone");
    assert!(rewind(&mut t, &h, 1.0, None, still.immune(true)).is_none());
    assert!(near(t.life, 10.0));
    still.tick(0.016, true);
    assert!(!still.immune(true));
}

#[test]
fn rewinding_a_self_destruct_windup_cancels_it_and_refunds_its_cost() {
    // rules 12: quay lui chặn tự bạo và trả chi phí
    let h = filled();
    let mut t = Cultivator::new();
    t.qi = 10.0;
    t.souls = 2.0;
    let w = Windup {
        self_destruct: true,
        age: 1.5,
        paid: vec![
            (Resource::Qi, 40.0),
            (Resource::Souls, 5.0),
            (Resource::Life, 300.0),
            (Resource::Star(Star::Body), 2.0),
        ],
    };
    let r = rewind(&mut t, &h, 1.0, Some(&w), false);
    let Some(r) = r else {
        unreachable!("the buffer is full")
    };
    assert!(r.cancelled);
    assert!(near(t.qi, 50.0) && near(t.souls, 7.0));
    assert!(near(t.stars[Star::Body as usize], 2.0));
    // the cast is older than the rewind: its life comes back on top of the rewound life
    assert!(near(t.life, 1059.0 + 300.0));
    assert_eq!(r.refunded.len(), 4);
}

#[test]
fn a_self_destruct_cast_inside_the_rewound_span_does_not_get_its_life_twice() {
    let h = filled();
    let mut t = Cultivator::new();
    let w = Windup {
        self_destruct: true,
        age: 0.3,
        paid: vec![(Resource::Life, 300.0), (Resource::Qi, 20.0)],
    };
    t.qi = 0.0;
    let r = rewind(&mut t, &h, 1.0, Some(&w), false);
    assert!(near(t.life, 1059.0));
    assert!(near(t.qi, 20.0));
    assert_eq!(r.map(|r| r.refunded), Some(vec![(Resource::Qi, 20.0)]));
}

#[test]
fn rewinding_an_ordinary_windup_cancels_nothing_and_refunds_nothing() {
    let h = filled();
    let mut t = Cultivator::new();
    t.qi = 0.0;
    let w = Windup {
        self_destruct: false,
        age: 1.5,
        paid: vec![(Resource::Qi, 40.0)],
    };
    let r = rewind(&mut t, &h, 1.0, Some(&w), false);
    assert_eq!(r.map(|r| r.cancelled), Some(false));
    assert!(near(t.qi, 0.0));
}

#[test]
fn refund_respects_caps() {
    let mut c = Cultivator::new();
    refund(
        &mut c,
        &[
            (Resource::Qi, 500.0),
            (Resource::Sense, 500.0),
            (Resource::Intent, 500.0),
            (Resource::DeathQi, 500.0),
        ],
    );
    assert!(near(c.qi, c.qi_max) && near(c.sense, c.sense_max));
    assert!(near(c.intent, RES.intent_max) && near(c.death_qi, RES.death_max));
}

// ----- Không gian ------------------------------------------------------------------------------------------------

#[test]
fn a_blink_lock_bars_movement_except_through_time() {
    let mut l = BlinkLock::default();
    let blink = Tags::of(&[Tag::Movement, Tag::Body]);
    let quy_sat = Tags::of(&[Tag::Movement, Tag::Time]);
    assert!(l.allows(blink));
    l.lock(2.0);
    l.lock(1.0);
    assert!(near(l.left, 2.0));
    assert!(!l.allows(blink));
    assert!(l.allows(quy_sat));
    assert!(l.allows(Tags::of(&[Tag::Spell])));
    l.tick(2.0);
    assert!(l.allows(blink));
}

#[test]
fn a_blink_ending_in_a_rift_tears_for_a_fifth_of_full_life_through_armour() {
    let mut r = Rift::new(5.0, 0.0);
    assert!(r.catches(5.5, 0.0, true));
    assert!(!r.catches(5.5, 0.0, false), "walking through is safe");
    assert!(!r.catches(7.0, 0.0, true), "outside");
    let mut t = Cultivator::new();
    t.shield = 0.0;
    let h = Rift::tear(&t);
    assert!(near(h.armour_pen, 1.0));
    let out = resolve(&h, None, &mut t, Rolls::default());
    assert!(near(out.to_life, DAO.rift * t.life_max));
    assert!(r.tick(0.5));
    assert!(!r.tick(0.6));
    assert!(!r.catches(5.0, 0.0, true));
}

#[test]
fn a_blink_passes_flying_blows_but_not_instants() {
    assert!(blink_passes(Tags::of(&[Tag::Projectile])));
    assert!(!blink_passes(Tags::of(&[Tag::Instant])));
}

// ----- Duyên -----------------------------------------------------------------------------------------------------

#[test]
fn bond_cuts_go_outside_buffs_then_treasures_then_minions() {
    let mut owner = Cultivator::new();
    let mut b = Bonds::new();
    b.tie(Tie::Minion, 30);
    b.tie(Tie::Treasure, 20);
    b.tie(Tie::Outside, 10);
    b.tie(Tie::Zone, 40);
    assert_eq!(b.cut(None, &mut owner), BondCut::Thing(Tie::Outside, 10));
    assert_eq!(b.cut(None, &mut owner), BondCut::Thing(Tie::Treasure, 20));
    assert_eq!(b.cut(None, &mut owner), BondCut::Thing(Tie::Minion, 30));
    assert_eq!(b.cut(None, &mut owner), BondCut::Thing(Tie::Zone, 40));
    assert!(near(owner.lifespan, 100.0));
}

#[test]
fn a_picked_bond_is_cut_first() {
    let mut owner = Cultivator::new();
    let mut b = Bonds::new();
    b.tie(Tie::Outside, 10);
    b.tie(Tie::Minion, 30);
    assert_eq!(b.cut(Some(30), &mut owner), BondCut::Thing(Tie::Minion, 30));
}

#[test]
fn a_cut_thing_stays_out_until_the_round_ends() {
    let mut owner = Cultivator::new();
    let mut b = Bonds::new();
    b.tie(Tie::Treasure, 20);
    b.cut(None, &mut owner);
    assert!(b.is_cut(20));
    b.untie(20);
    // it stays on record and cannot come back
    assert!(!b.tie(Tie::Treasure, 20));
    assert!(b.is_cut(20));
    b.new_round();
    assert!(!b.is_cut(20));
    assert!(b.tie(Tie::Treasure, 20));
}

#[test]
fn cutting_bonds_of_a_fighter_with_no_outside_thing_drains_ten_lifespan() {
    // rules 10: Trảm duyên lên đấu sĩ không có thực thể ngoài
    let mut owner = Cultivator::new();
    let mut b = Bonds::new();
    assert_eq!(b.cut(None, &mut owner), BondCut::Lifespan(DAO.bond_drain));
    assert!(near(owner.lifespan, 100.0 - DAO.bond_drain));
    // a thing that ended by itself leaves no thread
    b.tie(Tie::Minion, 3);
    b.untie(3);
    assert_eq!(b.cut(None, &mut owner), BondCut::Lifespan(DAO.bond_drain));
    owner.lifespan = 4.0;
    assert_eq!(b.cut(None, &mut owner), BondCut::Lifespan(4.0));
    assert!(near(owner.lifespan, 0.0));
}

// ----- Mệnh ------------------------------------------------------------------------------------------------------

#[test]
fn a_fate_cut_takes_an_extra_life_first() {
    let mut t = Cultivator::new();
    let mut lives = 2u8;
    let buffs = [Held {
        grade: Grade::Ranked(Rank::Faith),
        hollow: false,
    }];
    assert_eq!(cut_fate(&mut lives, &buffs, &mut t), FateCut::ExtraLife);
    assert_eq!(lives, 1);
}

#[test]
fn a_fate_cut_then_takes_the_highest_buff_that_is_not_hollow() {
    let mut t = Cultivator::new();
    let mut lives = 0u8;
    let buffs = [
        Held {
            grade: Grade::Ranked(Rank::Divine),
            hollow: false,
        },
        Held {
            grade: Grade::Ranked(Rank::Origin),
            hollow: true,
        },
        Held {
            grade: Grade::Restriction,
            hollow: false,
        },
        Held {
            grade: Grade::Ranked(Rank::Rule),
            hollow: false,
        },
        Held {
            grade: Grade::Ranked(Rank::Rule),
            hollow: false,
        },
    ];
    assert_eq!(cut_fate(&mut lives, &buffs, &mut t), FateCut::Buff(3));
}

#[test]
fn a_fate_cut_with_nothing_else_drains_ten_lifespan() {
    let mut t = Cultivator::new();
    let mut lives = 0u8;
    let hollow = [Held {
        grade: Grade::Ranked(Rank::Origin),
        hollow: true,
    }];
    assert_eq!(
        cut_fate(&mut lives, &hollow, &mut t),
        FateCut::Lifespan(DAO.fate_drain)
    );
    assert!(near(t.lifespan, 100.0 - DAO.fate_drain));
}

#[test]
fn only_fate_or_time_drains_lifespan() {
    let mut t = Cultivator::new();
    assert!(near(
        drain_lifespan(Tags::of(&[Tag::Yin]), &mut t, 5.0),
        0.0
    ));
    assert!(near(
        drain_lifespan(Tags::of(&[Tag::Fate]), &mut t, 5.0),
        5.0
    ));
    assert!(near(
        drain_lifespan(Tags::of(&[Tag::Time]), &mut t, 5.0),
        5.0
    ));
    assert!(near(t.lifespan, 90.0));
}

// ----- Sát lục ---------------------------------------------------------------------------------------------------

#[test]
fn intent_rises_on_hits_and_kills_and_falls_at_rest() {
    let mut c = Cultivator::new();
    intent_on_hit(&mut c);
    intent_on_kill(&mut c);
    assert!(near(c.intent, RES.intent_hit + RES.intent_kill));
    intent_tick(&mut c, 1.0, true);
    assert!(near(c.intent, 14.0));
    intent_tick(&mut c, 1.0, false);
    assert!(near(c.intent, 14.0 - RES.intent_fall));
    // above 50, +1% a point
    c.intent = 70.0;
    assert!(near(c.power(), 1.2));
}

#[test]
fn full_intent_for_three_seconds_wakes_the_heart_demon() {
    let mut c = Cultivator::new();
    c.intent = 100.0;
    c.tick(3.0, false);
    assert!(c.demon);
    let before = c.life;
    c.tick(1.0, false);
    assert!(before - c.life >= RES.demon * c.life_max - EPS);
    c.intent = 79.0;
    c.tick(0.1, false);
    assert!(!c.demon);
}

// ----- Nguyền: the rules -----------------------------------------------------------------------------------------

#[test]
fn a_near_hit_takes_a_conduit_held_twenty_seconds_up_to_three() {
    let mut c = Conduits::new();
    c.on_hit(Tags::of(&[Tag::Far]));
    assert_eq!(c.count(), 0);
    for _ in 0..5 {
        c.on_hit(Tags::of(&[Tag::Near]));
    }
    assert_eq!(c.count(), RULES.conduits);
    c.tick(RULES.conduit_time - 0.1);
    assert_eq!(c.count(), 3);
    c.tick(0.2);
    assert_eq!(c.count(), 0);
}

#[test]
fn far_curses_need_a_talisman_or_spirit_tag_and_spend_a_conduit() {
    let mut c = Conduits::new();
    let talisman = Tags::of(&[Tag::Curse, Tag::Talisman]);
    let plain = Tags::of(&[Tag::Curse, Tag::Spell]);
    // no conduit: only Near
    assert_eq!(c.reach(talisman, true), Reach::Near);
    assert_eq!(c.reach(talisman, false), Reach::Out);
    c.on_hit(Tags::of(&[Tag::Near]));
    assert_eq!(c.reach(plain, false), Reach::Out);
    assert_eq!(c.count(), 1, "a failed reach spends nothing");
    assert_eq!(c.reach(Tags::of(&[Tag::Spirit]), false), Reach::Spent);
    assert_eq!(c.count(), 0);
}

#[test]
fn one_curse_a_fighter_replaced_only_by_an_equal_or_higher_rank() {
    let mut on = Curses::new(1, false);
    assert_eq!(
        on.lay(Curse::new(Kind::Aging, Rank::Rule, 0, 1)),
        Lay::Added
    );
    assert_eq!(
        on.lay(Curse::new(Kind::DreamDao, Rank::Divine, 0, 1)),
        Lay::Failed
    );
    assert_eq!(
        on.lay(Curse::new(Kind::DreamDao, Rank::Rule, 0, 1)),
        Lay::Replaced(Kind::Aging)
    );
    assert_eq!(on.on.len(), 1);
    // the same kind refreshes when not lower
    assert_eq!(
        on.lay(Curse::new(Kind::DreamDao, Rank::Origin, 0, 1)),
        Lay::Replaced(Kind::DreamDao)
    );
}

#[test]
fn vu_chu_carries_two_and_the_third_takes_the_lowest() {
    let mut on = Curses::new(1, true);
    assert_eq!(
        on.lay(Curse::new(Kind::Aging, Rank::Rule, 0, 1)),
        Lay::Added
    );
    assert_eq!(
        on.lay(Curse::new(Kind::Demonize, Rank::Divine, 0, 1)),
        Lay::Added
    );
    assert_eq!(
        on.lay(Curse::new(Kind::DreamDao, Rank::Art, 0, 1)),
        Lay::Failed
    );
    assert_eq!(
        on.lay(Curse::new(Kind::DreamDao, Rank::Divine, 0, 1)),
        Lay::Replaced(Kind::Demonize)
    );
    assert_eq!(on.on.len(), 2);
}

#[test]
fn a_curse_spreads_to_minions_at_half() {
    let c = Curse::new(Kind::ExposeMark(Element::Fire), Rank::Divine, 0, 1);
    let Some(m) = c.spread_to(9) else {
        unreachable!("a mark spreads")
    };
    assert!(near(m.potency, RULES.spread) && m.spread);
    let mut minion = Curses::new(9, false);
    minion.lay(m);
    assert!(near(minion.resist(Element::Fire), -NUMBERS.expose * 0.5));
    // a tether binds two fighters and does not spread
    assert!(
        Curse::new(Kind::DeathSeal, Rank::Divine, 0, 1)
            .spread_to(9)
            .is_none()
    );
}

#[test]
fn cursing_back_ones_curser_is_half_again_as_strong_and_the_first_curse_falls() {
    // X (0) curses Y (1); Y curses X back
    let mut on_x = Curses::new(0, false);
    let mut on_y = Curses::new(1, false);
    let first = Curse::new(Kind::ExposeMark(Element::Soul), Rank::Art, 0, 1);
    assert_eq!(curse::cast(first, &mut on_y, &mut on_x), Lay::Added);
    let back = Curse::new(Kind::ExposeMark(Element::Fire), Rank::Art, 1, 0);
    assert_eq!(curse::cast(back, &mut on_x, &mut on_y), Lay::Added);
    assert!(on_y.on.is_empty());
    assert!(near(on_x.on[0].potency, RULES.backlash));
    assert!(near(on_x.resist(Element::Fire), -NUMBERS.expose * 1.5));
}

#[test]
fn a_failed_curse_back_is_no_backlash() {
    let mut on_x = Curses::new(0, false);
    let mut on_y = Curses::new(1, false);
    on_x.lay(Curse::new(Kind::Aging, Rank::Origin, 7, 0));
    curse::cast(
        Curse::new(Kind::DreamDao, Rank::Art, 0, 1),
        &mut on_y,
        &mut on_x,
    );
    let lay = curse::cast(
        Curse::new(Kind::Demonize, Rank::Art, 1, 0),
        &mut on_x,
        &mut on_y,
    );
    assert_eq!(lay, Lay::Failed);
    assert_eq!(on_y.on.len(), 1);
}

#[test]
fn a_blessing_turns_a_curse_of_its_rank_or_lower_into_the_opposite_buff_for_three_seconds() {
    let mut on = Curses::new(1, false);
    on.lay(Curse::new(
        Kind::ExposeMark(Element::Water),
        Rank::Rule,
        0,
        1,
    ));
    assert!(on.bless(Rank::Divine).is_none(), "too low");
    let Some(b) = on.bless(Rank::Rule) else {
        unreachable!("rank 3 lifts rank 3")
    };
    assert!(on.on.is_empty());
    assert!(near(b.boon.left, RULES.boon_time));
    assert!(near(on.resist(Element::Water), NUMBERS.expose));
    let mut c = Cultivator::new();
    on.tick(RULES.boon_time + 0.01, &mut c, false);
    assert!(on.boons.is_empty());
}

#[test]
fn boons_mirror_doom_and_aging_and_give_more_damage_otherwise() {
    let doom = Boon {
        from: Kind::AncestorDoom,
        potency: 1.0,
        strength: 5.0,
        left: 3.0,
    };
    assert!(near(doom.taken(), 1.0 - 5.0 * NUMBERS.doom_step));
    let aging = Boon {
        from: Kind::Aging,
        ..doom
    };
    assert!(near(aging.haste(), NUMBERS.aging_slow) && near(aging.more(), 0.0));
    let dream = Boon {
        from: Kind::DreamDao,
        ..doom
    };
    assert!(near(dream.more(), NUMBERS.boon_more));
}

// ----- Nguyền: the list ------------------------------------------------------------------------------------------

fn bearer() -> (Curses, Cultivator) {
    (Curses::new(1, false), Cultivator::new())
}

#[test]
fn straw_knots_misfire_at_ten_hold_at_eleven_and_bleed_the_curser_from_seven() {
    let (mut on, _) = bearer();
    let mut curser = Cultivator::new();
    on.lay(Curse::new(Kind::StrawKnot, Rank::Divine, 0, 1));
    for _ in 0..6 {
        assert!(on.knot(0, &mut curser).is_empty());
    }
    assert!(near(curser.life, curser.life_max));
    on.knot(0, &mut curser);
    assert!(near(
        curser.life,
        curser.life_max * (1.0 - NUMBERS.straw_bleed)
    ));
    on.knot(0, &mut curser);
    on.knot(0, &mut curser);
    assert_eq!(on.misfire(0.0), Misfire::None, "nine knots");
    on.knot(0, &mut curser);
    assert_eq!(on.misfire(0.1), Misfire::Miss);
    assert_eq!(on.misfire(0.25), Misfire::SelfHarm);
    assert_eq!(on.misfire(0.5), Misfire::None);
    let ev = on.knot(0, &mut curser);
    assert!(ev.contains(&Event::Control {
        kind: Kind::StrawKnot,
        control: Control::Hold,
        seconds: NUMBERS.straw_hold
    }));
    assert!(on.on.is_empty());
    // only the doll's own curser ties it
    on.lay(Curse::new(Kind::StrawKnot, Rank::Divine, 0, 1));
    on.knot(5, &mut curser);
    assert_eq!(on.on[0].state.knots, 0);
}

#[test]
fn nine_deaths_beats_nine_times_heavier_for_each_guard_and_resists_lifting() {
    let (mut on, mut c) = bearer();
    on.lay(Curse::new(Kind::NineDeaths, Rank::Art, 0, 1));
    let ev = on.tick(1.0, &mut c, false);
    assert_eq!(
        ev,
        vec![Event::Strike {
            kind: Kind::NineDeaths,
            element: Element::Yin,
            share: NUMBERS.nine_damage
        }]
    );
    on.on_cast(Tags::of(&[Tag::Guard, Tag::Body]), false);
    on.on_cast(Tags::of(&[Tag::Guard, Tag::Body]), false);
    let ev = on.tick(1.0, &mut c, false);
    assert!(matches!(ev[0], Event::Strike { share, .. } if near(share, NUMBERS.nine_damage * 2.0)));
    assert!(on.dispel(Rank::Faith).is_none());
    assert!(on.bless(Rank::Faith).is_none());
    let ev = on.tick(7.0, &mut c, false);
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, Event::Strike { .. }))
            .count(),
        7
    );
    assert!(ev.contains(&Event::Ended(Kind::NineDeaths)));
    assert!(on.on.is_empty());
}

#[test]
fn nine_deaths_gives_way_to_another_curse() {
    let (mut on, _) = bearer();
    on.lay(Curse::new(Kind::NineDeaths, Rank::Art, 0, 1));
    assert_eq!(
        on.lay(Curse::new(Kind::Aging, Rank::Art, 0, 1)),
        Lay::Replaced(Kind::NineDeaths)
    );
}

#[test]
fn ancestor_doom_grows_with_each_blow_and_falls_only_at_rest() {
    let (mut on, mut c) = bearer();
    on.lay(Curse::new(Kind::AncestorDoom, Rank::Art, 0, 1));
    for _ in 0..3 {
        on.on_cast(Tags::of(&[Tag::Attack]), false);
    }
    on.on_cast(Tags::of(&[Tag::Guard, Tag::Body]), false);
    assert!(near(on.taken(), 1.0 + 3.0 * NUMBERS.doom_step));
    on.tick(1.0, &mut c, false);
    on.on_cast(Tags::of(&[Tag::Spell]), false);
    on.tick(1.0, &mut c, false);
    assert!(near(on.taken(), 1.0 + 4.0 * NUMBERS.doom_step));
    on.tick(NUMBERS.doom_rest * 2.0, &mut c, false);
    assert!(near(on.taken(), 1.0 + 2.0 * NUMBERS.doom_step));
    for _ in 0..20 {
        on.on_cast(Tags::of(&[Tag::Attack]), false);
    }
    assert!(near(on.taken(), 1.0 + NUMBERS.doom_max * NUMBERS.doom_step));
}

#[test]
fn five_organs_break_one_by_one_and_the_heart_is_a_heavy_blow() {
    let (mut on, mut c) = bearer();
    on.lay(Curse::new(Kind::FiveOrgans, Rank::Art, 0, 1));
    let heal = Tags::of(&[Tag::Recovery]);
    let shout = Tags::of(&[Tag::Warcry]);
    assert!(!on.barred(heal));
    assert_eq!(
        on.tick(2.0, &mut c, false),
        vec![Event::Organ(Organ::Liver)]
    );
    assert!(on.heal_blocked() && on.barred(heal) && !on.barred(shout));
    on.tick(2.0, &mut c, false);
    assert!(on.barred(shout) && on.barred(Tags::of(&[Tag::Sound])));
    on.tick(2.0, &mut c, false);
    assert!(on.qi_blocked() && !on.sense_blocked());
    on.tick(2.0, &mut c, false);
    assert!(on.sense_blocked());
    let ev = on.tick(2.0, &mut c, false);
    assert!(ev.contains(&Event::Organ(Organ::Heart)));
    assert!(ev.contains(&Event::Strike {
        kind: Kind::FiveOrgans,
        element: Element::Yin,
        share: NUMBERS.organs_heart
    }));
    assert!(ev.contains(&Event::Ended(Kind::FiveOrgans)));
    assert!(!on.heal_blocked());
}

#[test]
fn three_breaths_strikes_the_soul_when_it_runs_out_and_turns_on_the_diviner_when_broken() {
    let (mut on, mut c) = bearer();
    on.lay(Curse::new(Kind::ThreeBreaths, Rank::Divine, 0, 1));
    assert!(on.tick(2.9, &mut c, false).is_empty());
    let ev = on.tick(0.2, &mut c, false);
    assert_eq!(
        ev,
        vec![
            Event::Strike {
                kind: Kind::ThreeBreaths,
                element: Element::Soul,
                share: NUMBERS.breaths
            },
            Event::Ended(Kind::ThreeBreaths)
        ]
    );
    on.lay(Curse::new(Kind::ThreeBreaths, Rank::Divine, 4, 1));
    assert!(on.dispel(Rank::Art).is_none());
    let Some((kind, ev)) = on.dispel(Rank::Divine) else {
        unreachable!("rank 2 lifts rank 2")
    };
    assert_eq!(kind, Kind::ThreeBreaths);
    assert_eq!(
        ev[0],
        Event::Backfire {
            kind: Kind::ThreeBreaths,
            by: 4,
            element: Element::Soul,
            share: NUMBERS.breaths_backfire
        }
    );
    let h = curse::strike_hit(Element::Soul, NUMBERS.breaths, c.life_max);
    assert!(near(
        h.base[Element::Soul.index()],
        NUMBERS.breaths * c.life_max
    ));
}

#[test]
fn the_binding_word_cracks_the_heart_when_its_tag_is_used() {
    let (mut on, mut c) = bearer();
    on.lay(Curse::new(
        Kind::BindingWord(Tag::Movement),
        Rank::Art,
        0,
        1,
    ));
    assert!(on.on_cast(Tags::of(&[Tag::Attack]), false).is_empty());
    assert!(!on.cracked());
    assert_eq!(
        on.on_cast(Tags::of(&[Tag::Movement]), false),
        vec![Event::Cracked]
    );
    assert!(on.cracked());
    assert!(near(on.resist(Element::Soul), -NUMBERS.crack_soul));
    // the crack outlasts the word, then both go
    on.tick(NUMBERS.crack_time - 0.1, &mut c, false);
    assert!(on.cracked());
    on.tick(NUMBERS.word_time, &mut c, false);
    assert!(!on.cracked() && on.on.is_empty());
}

#[test]
fn the_dream_is_hidden_and_stalls_the_strongest_skill_once() {
    let (mut on, _) = bearer();
    on.lay(Curse::new(Kind::DreamDao, Rank::Art, 0, 1));
    assert_eq!(on.shown(false).count(), 0);
    assert_eq!(on.shown(true).count(), 1);
    assert!(on.on_cast(Tags::of(&[Tag::Spell]), false).is_empty());
    let ev = on.on_cast(Tags::of(&[Tag::Spell]), true);
    assert_eq!(
        ev,
        vec![
            Event::Stall(NUMBERS.dream_stall),
            Event::Ended(Kind::DreamDao)
        ]
    );
    assert!(on.on.is_empty());
}

#[test]
fn seal_god_locks_a_damage_tag_and_costs_the_curser_full_life() {
    let (mut on, mut c) = bearer();
    on.lay(Curse::new(Kind::SealGod(Tag::Fire), Rank::Art, 0, 1));
    assert!(on.barred(Tags::of(&[Tag::Spell, Tag::Fire])));
    assert!(!on.barred(Tags::of(&[Tag::Spell, Tag::Water])));
    on.tick(NUMBERS.god_time + 0.01, &mut c, false);
    assert!(!on.barred(Tags::of(&[Tag::Fire])));
    let mut curser = Cultivator::new();
    let paid = curse::seal_cost(&mut curser);
    assert!(near(paid, NUMBERS.god_cost * 3000.0));
    assert!(near(curser.life_max, 3000.0 - paid) && near(curser.life, curser.life_max));
}

#[test]
fn demonize_turns_immortal_buffs_and_pure_yang_thunder_drives_it_out() {
    let (mut on, _) = bearer();
    on.lay(Curse::new(Kind::Demonize, Rank::Art, 0, 1));
    assert!(on.turns(Tags::of(&[Tag::Blessing, Tag::Immortal])));
    assert!(!on.turns(Tags::of(&[Tag::Blessing, Tag::Ancient])));
    assert!(on.on_cast(Tags::of(&[Tag::Thunder]), false).is_empty());
    assert_eq!(
        on.on_cast(Tags::of(&[Tag::Thunder, Tag::Light]), false),
        vec![Event::Ended(Kind::Demonize)]
    );
    assert!(!on.turns(Tags::of(&[Tag::Immortal])));
}

#[test]
fn aging_slows_and_drains_lifespan_but_not_a_still_tree_watcher() {
    let (mut on, mut c) = bearer();
    on.lay(Curse::new(Kind::Aging, Rank::Art, 0, 1));
    assert!(near(on.speed(false), 1.0 - NUMBERS.aging_slow));
    assert!(near(on.speed(true), 1.0));
    on.tick(1.0, &mut c, false);
    assert!(near(c.lifespan, 100.0 - NUMBERS.aging_drain));
    on.tick(1.0, &mut c, true);
    assert!(near(c.lifespan, 100.0 - NUMBERS.aging_drain));
}

#[test]
fn the_expose_mark_shows_the_target_and_cuts_one_resistance() {
    let (mut on, _) = bearer();
    assert!(!on.exposed());
    on.lay(Curse::new(
        Kind::ExposeMark(Element::Thunder),
        Rank::Art,
        0,
        1,
    ));
    assert!(on.exposed());
    assert!(near(on.resist(Element::Thunder), -NUMBERS.expose));
    assert!(near(on.resist(Element::Fire), 0.0));
}

#[test]
fn every_curse_has_a_slot_a_name_and_a_life() {
    let all = [
        Kind::StrawKnot,
        Kind::NineDeaths,
        Kind::AncestorDoom,
        Kind::DeathSeal,
        Kind::FiveOrgans,
        Kind::ThreeBreaths,
        Kind::BindingWord(Tag::Guard),
        Kind::DreamDao,
        Kind::SealGod(Tag::Fire),
        Kind::Demonize,
        Kind::Aging,
        Kind::ExposeMark(Element::Fire),
    ];
    for (i, k) in all.iter().enumerate() {
        assert_eq!(k.index(), i);
        assert!(!k.name().is_empty());
        assert!(k.time() > 0.0);
    }
    // a tether curse ties its tether to the right two
    let c = Curse::new(Kind::DeathSeal, Rank::Rule, 0, 1);
    assert_eq!(
        c.state.tether.map(|t| (t.by, t.on, t.rank)),
        Some((0, 1, Rank::Rule))
    );
    // dao's own blows never crit or get dodged
    let h = dao::dao_hit(Element::Force, 10.0);
    assert!(h.instant && h.crit == 0.0);
}

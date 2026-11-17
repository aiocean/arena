//! What a brain is told: each skill as arena-mind reads it (`skill_of`), and what a fighter could perceive this tick
//! (`sense_of`). The minds think in the engine's pixels, the rules in the arena's (`UNIT` of them to one of the
//! engine's): every length and speed a mind is told is divided by it, and what it chooses multiplied back (`chosen`).
//!
//! The minds' view (and so the trained policy's inputs, `arena_mind::policy::observe`) keeps its shape: the damage
//! system's state is mapped onto the slots that fit it and the rest left at zero (design 16).

use crate::act::recover_of;
use crate::bar::{cooldown_left, ready};
use crate::data::{BAR, FRAMES, Kind, Spell, T, UNIT, spell};
use crate::fighter::{Fighter, Phase, Slot, State};
use crate::frames::{clip_for, strike_hitbox, window_of};
use crate::kit::Place;
use crate::shot::{ShotKind, orb_at};
use crate::unit::{FLOOR_Y, UNIT_Y};
use arena_calc::ailment::{Ailment, Control};
use arena_calc::aura::AuraKind;
use arena_calc::device;
use arena_calc::dot::{BURN_RATE, BURN_TIME};
use arena_calc::element::Element;
use arena_calc::gem::{self, Effect};
use arena_calc::keystone::Keystone;
use arena_calc::loadout::ROOTS;
use arena_calc::stat::{Source, Stat};
use arena_mind::policy::SLOTS;
use arena_mind::{
    ActFrames, ActPhase, Acting, AilmentOn, Arena, Choice, Doing, Effects, Foe, FoeSlot, Inflicts,
    Me, Sense, Skill, SkillKind, SlotX, Threat, ThreatShape, UnitKind, UnitSeen,
};

/// A length or speed in the arena's pixels, in the engine's.
fn seen(v: f64) -> f64 {
    v / UNIT
}

/// What a mind chose, in the arena's pixels: the height it flies at and the x it aims at.
pub fn chosen(c: Choice) -> Choice {
    match c {
        Choice::Fly { height } => Choice::Fly {
            height: height * UNIT,
        },
        Choice::Cast { skill, target } => Choice::Cast {
            skill,
            target: target.map(|x| x * UNIT),
        },
        other => other,
    }
}

/// Skill `name` as arena-mind reads it: what it does, never what it is called (a heal's `damage` is what it heals;
/// a blow's what it is expected to deal, in the old power, `data::POWER`).
pub fn skill_of(name: &str) -> Skill {
    let Some(s) = spell(name) else {
        return Skill {
            name: name.to_string(),
            kind: SkillKind::Melee,
            cost: 0.0,
            windup: 0.0,
            recover: 0.0,
            damage: 0.0,
            reach: 0.0,
            radius: 0.0,
            speed: 0.0,
            lands: 0.0,
            strikes: 1,
            grounded: false,
            inflicts: Inflicts::default(),
        };
    };
    let (kind, reach) = (kind_of(s), reach_of(s));
    let damage = if s.kind == Kind::Heal {
        s.amount
    } else {
        s.damage
    };
    let lands = if s.kind == Kind::Void {
        s.life.unwrap_or(f64::NAN)
    } else {
        s.sigil.unwrap_or(0.0) + s.impact.unwrap_or(0.0)
    };
    Skill {
        name: name.to_string(),
        kind,
        cost: s.cost,
        windup: s.windup,
        recover: s.recover,
        damage: damage.unwrap_or(0.0),
        reach: seen(reach),
        radius: seen(s.radius.unwrap_or(0.0)),
        speed: seen(s.speed.unwrap_or(0.0)),
        lands,
        strikes: s.strikes.map_or(1, |n| n as u32),
        grounded: s.grounded,
        inflicts: inflicts_of(s),
    }
}

/// What skill `s` does, as arena-mind knows skills.
fn kind_of(s: &Spell) -> SkillKind {
    if s.dive {
        return SkillKind::Dive;
    }
    match s.kind {
        Kind::Melee => SkillKind::Melee,
        Kind::Dash => SkillKind::Dash,
        Kind::Blast => SkillKind::Blast,
        Kind::Bolt => SkillKind::Bolt,
        Kind::Orb => SkillKind::Seeker,
        Kind::Zone => SkillKind::Area,
        Kind::Void => SkillKind::Field,
        Kind::Heal => SkillKind::Heal,
        Kind::Ward => SkillKind::Ward,
        Kind::Barrier => SkillKind::Barrier,
        Kind::Blink => SkillKind::Blink,
    }
}

/// How far skill `s` reaches, in the arena's pixels: a dash's or blink's distance, a blade's or blast's range, an
/// area's or field's reach.
fn reach_of(s: &Spell) -> f64 {
    match s.kind {
        Kind::Dash | Kind::Blink => s.distance.unwrap_or(f64::NAN),
        Kind::Melee | Kind::Blast => s.range.unwrap_or(f64::NAN),
        _ => s.reach.unwrap_or(0.0),
    }
}

/// What skill `s` leaves on the one it lands on (the bell: on the one striking it), mapped onto the minds' fixed
/// slots: a hold (Định thân) as a freeze, a stun, a knockdown or a jolt as a stun, a lost soul as a silence; Phá
/// (metal) as broken defence; Thiêu (fire) as a burn; Thấm (water) in the chill slots and Triền (wood) as their slow;
/// Tê (thunder) in the shock slots. Each by the share of its kind in the skill's damage.
fn inflicts_of(s: &Spell) -> Inflicts {
    let Some(g) = gem::gem(s.name) else {
        return Inflicts::default();
    };
    let share = |e: Element| -> f64 { g.split.iter().filter(|p| p.0 == e).map(|p| p.1).sum() };
    let mut i = Inflicts::default();
    for e in g.effects {
        if let Effect::Control(c, secs) = e {
            match c {
                Control::Hold => i.freeze = i.freeze.max(*secs),
                Control::Stun | Control::Knockdown | Control::Jolt => i.stun = i.stun.max(*secs),
                Control::Soulless => i.silence = i.silence.max(*secs),
                Control::Root => {}
            }
        }
    }
    if let Some(c) = s.counter {
        i.stun = i.stun.max(c.stun);
    }
    let metal = share(Element::Metal);
    if metal > 0.0 {
        i.def_down = Ailment::Sunder.data().time;
        i.def_loss = arena_calc::ailment::AIL.sunder * metal;
    }
    let fire = share(Element::Fire);
    if fire > 0.0 {
        i.burn = BURN_TIME;
        i.burn_rate = BURN_RATE * fire;
    }
    let water = share(Element::Water);
    i.chill = water;
    i.chill_max = if water > 0.0 {
        Ailment::Soak.data().cap
    } else {
        0.0
    };
    i.slow = arena_calc::ailment::AIL.bind_slow * share(Element::Wood);
    let thunder = share(Element::Thunder);
    i.shock = thunder;
    if thunder > 0.0 {
        i.shock_max = Ailment::Jolt.data().cap;
        i.shock_hold = Ailment::Jolt.data().seconds;
        i.shock_rate = arena_calc::ailment::AIL.jolt_taken;
    }
    i.pull = seen(s.pull.unwrap_or(0.0));
    i
}

/// The skill fighter `f` is using, as arena-mind reads it.
fn acting_of(f: &Fighter) -> Option<Acting> {
    let a = f.act?;
    let (windup, recover) = (a.windup, recover_of(&a));
    let phase = if a.phase == Phase::Windup {
        ActPhase::Windup
    } else {
        ActPhase::Recover
    };
    Some(Acting {
        phase,
        t: a.t,
        windup,
        recover,
    })
}

/// What fighter `f` is doing, as arena-mind reads it.
pub fn doing_of(f: &Fighter) -> Doing {
    if let (Some(a), Some(acting)) = (f.act, acting_of(f)) {
        let skill = skill_of(a.spell);
        return Doing::Act {
            kind: skill.kind,
            phase: acting.phase,
            t: acting.t,
            windup: acting.windup,
            recover: acting.recover,
            reach: skill.reach,
            damage: skill.damage,
        };
    }
    if f.guard > 0.0 {
        Doing::Guard
    } else if f.medit {
        Doing::Meditate
    } else if f.fly {
        Doing::Fly
    } else if f.intent != 0.0 {
        Doing::Move { dir: f.intent }
    } else {
        Doing::Idle
    }
}

/// The foe's shots that could reach fighter `me`, each with how long it has been there to see.
pub fn threats_of(state: &State, me: usize) -> Vec<Threat> {
    let mut out = Vec::new();
    for s in &state.shots {
        if s.owner == me {
            continue;
        }
        let sp = s.spell;
        let damage = sp.damage.unwrap_or(0.0);
        let threat = match &s.kind {
            ShotKind::Bolt { .. } => s.bolt_at(state).map(|(x, y, vx)| Threat {
                age: s.t,
                damage,
                shape: ThreatShape::Projectile {
                    x: seen(x),
                    y: seen(y),
                    vx: seen(vx),
                },
            }),
            ShotKind::Orb { .. } => {
                let (x, y) = orb_at(s);
                Some(Threat {
                    age: s.t,
                    damage,
                    shape: ThreatShape::Seeker {
                        x: seen(x),
                        y: seen(-y),
                        radius: seen(sp.radius.unwrap_or(f64::NAN)),
                    },
                })
            }
            ShotKind::Zone {
                fell: false,
                follow,
                x,
                lead,
                ..
            } => {
                let (follow, x, lead) = (*follow, *x, *lead);
                if follow && s.t < 0.0 {
                    // a barrage still falling: its next strike aims where I stand
                    let age = s.t + sp.every.unwrap_or(f64::NAN) + sp.aim_ahead.unwrap_or(f64::NAN);
                    Some(Threat {
                        age,
                        damage,
                        shape: ThreatShape::Barrage { lands_in: -s.t },
                    })
                } else {
                    // a sealed spell is seen from the moment its seal shows, a falling one once it trembles
                    let shape = ThreatShape::Area {
                        x: seen(x),
                        radius: seen(sp.radius.unwrap_or(f64::NAN)),
                        reach: seen(sp.reach.unwrap_or(0.0)),
                        lands_in: sp.impact.unwrap_or(f64::NAN) - s.t,
                        grounded: sp.grounded,
                    };
                    Some(Threat {
                        age: s.t + lead - sp.notice.unwrap_or(f64::NAN),
                        damage,
                        shape,
                    })
                }
            }
            ShotKind::Void { x, .. } => Some(Threat {
                age: s.t,
                damage,
                shape: ThreatShape::Field {
                    x: seen(*x),
                    reach: seen(sp.reach.unwrap_or(f64::NAN)),
                },
            }),
            ShotKind::Zone { .. } | ShotKind::Spray => None,
        };
        out.extend(threat);
    }
    out
}

/// What fighter `me` could perceive this tick, as arena-mind reads it: the true state; it sees late on its own.
pub fn sense_of(state: &State, me: usize) -> Sense {
    let f = &state.fighters[me];
    let foe = &state.fighters[1 - me];
    let b = &f.body;
    let slots = slots_of(f);
    let free = !(f.act.is_some()
        || b.reeling()
        || b.squat > 0.0
        || b.duck > 0.0
        || b.shocked > 0.0
        || !f.alive());
    Sense {
        time: state.time,
        me: Me {
            x: seen(b.x),
            air: seen(b.air),
            face: b.face,
            hp: f.hp(),
            qi: f.qi(),
            qi_max: f.c.qi_max,
            flying: f.fly,
            hover: seen(f.hover),
            vx: seen(b.vx),
            can_act: free,
            // its soul lost (thất hồn): only attacks answer
            silenced: f.silenced(),
            // no blinking: sealed, or rooted
            sealed: f.sealed > 0.0 || f.root > 0.0,
            barrier: f.barrier > 0.0,
            shield: f.charges > 0.0,
            seeker_out: state
                .shots
                .iter()
                .any(|s| matches!(s.kind, ShotKind::Orb { .. }) && s.owner == me),
            act: acting_of(f),
        },
        foe: Foe {
            x: seen(foe.body.x),
            air: seen(foe.body.air),
            flying: foe.fly,
            hp: foe.hp(),
            qi: foe.qi(),
            doing: doing_of(foe),
            out_for: foe.body.out_for(),
            open_in: foe.body.open_in(),
            // nothing freezes: a root (no walking) takes the freeze's place
            frozen: foe.root,
            shocked: foe.body.shocked,
            barrier: foe.barrier > 0.0,
            ready: foe.spells.iter().map(|n| ready(foe, n)).collect(),
        },
        arena: Arena {
            left: seen(state.left),
            right: seen(state.right),
            top: seen(state.top),
        },
        threats: threats_of(state, me),
        ready: f.spells.iter().map(|n| ready(f, n)).collect(),
        // every skill strikes at its own power (no bar tiers)
        power: Vec::new(),
        stacks: Vec::new(),
        slotx: slots_at(&slots, |k| slotx_of(f, k)),
        slots,
        me_act: act_frames_of(state, me),
        foe_act: act_frames_of(state, 1 - me),
        effects: [effects_of(f), effects_of(foe)],
        units: units_of(state, me),
        foe_slots: slots_at(&slots_of(foe), |k| {
            let ks = foe.kit.skills.get(k)?;
            Some(FoeSlot {
                kind: kind_of(ks.motion),
                ready: ready(foe, ks.gem.id),
                reach: seen(reach_of(ks.motion)),
                built: slotx_of(foe, k)?,
            })
        }),
    }
}

/// `at` of each of a fighter's `slots` (`slots_of`), `None` for an empty one.
fn slots_at<T>(slots: &[Option<usize>], at: impl Fn(usize) -> Option<T>) -> [Option<T>; SLOTS] {
    core::array::from_fn(|i| slots.get(i).copied().flatten().and_then(&at))
}

/// Fighter `f`'s cast under way as its clip times it, its hitbox in the minds' pixels.
fn act_frames_of(state: &State, f: usize) -> Option<ActFrames> {
    let fi = &state.fighters[f];
    let a = fi.act?;
    let recover = recover_of(&a);
    let (_, window) = spell(a.spell)
        .and_then(|s| window_of(fi.look, clip_for(s), a.windup, recover))
        .unwrap_or((0.0, 0.0));
    let windup = a.phase == Phase::Windup;
    let left = (a.windup - a.t).max(0.0);
    Some(ActFrames {
        in_window: !windup && a.t < window,
        to_window: if windup && window > 0.0 { left } else { 0.0 },
        window,
        to_punish: if windup { left } else { 0.0 },
        recover,
        hitbox: strike_hitbox(state, f).map(|b| b.map(seen)),
    })
}

/// Fighter `f`'s kit skill `k` as it builds now (`KitSkill::seen`), with its uses and its clip's window.
fn slotx_of(f: &Fighter, k: usize) -> Option<SlotX> {
    let ks = f.kit.skills.get(k)?;
    let (v, u) = (&ks.seen, &ks.uses);
    let (window_start, window) =
        window_of(f.look, clip_for(ks.motion), v.windup, v.recover).unwrap_or((0.0, 0.0));
    Some(SlotX {
        cooldown_left: cooldown_left(u),
        cooldown: u.cooldown,
        stored: f64::from(u.stored),
        max_uses: f64::from(u.max),
        windup: v.windup,
        recover: v.recover,
        blocked_extra: if FRAMES.unsafe_on_block.contains(&ks.gem.id) {
            FRAMES.blocked_recover
        } else {
            0.0
        },
        window_start,
        window,
        damage: v.damage,
        qi: v.qi,
        projectiles: f64::from(v.projectiles),
        strikes: f64::from(v.strikes),
        area: v.area,
        pierce: f64::from(v.pierce),
        chain: f64::from(v.chain),
        fork: f64::from(v.fork),
        inert: v.inert as f64,
        inflict: v.inflict,
        supports: v.supports,
    })
}

/// What lies on fighter `f`: its ailments, controls, curses, auras, its skills' buffs, its shields, its loadout's
/// keystones and its root.
fn effects_of(f: &Fighter) -> Effects {
    let c = &f.c;
    let mut e = Effects::default();
    let longest = |left: &mut dyn Iterator<Item = f64>| left.fold(0.0, f64::max);
    for a in Ailment::ALL {
        let i = a.index();
        let (left, strength) = match c.ailments.dots(a) {
            Some(d) => (
                longest(&mut d.0.iter().map(|x| x.left)),
                d.strongest()
                    .filter(|_| c.life_max > 0.0)
                    .map_or(0.0, |x| x.dps / c.life_max),
            ),
            None => (longest(&mut c.ailments.on[i].left.iter().copied()), 0.0),
        };
        e.ailments[i] = AilmentOn {
            stacks: c.ailments.stacks(a),
            cap: a.data().cap,
            left,
            strength,
        };
    }
    let b = &f.body;
    e.stun = b.stun;
    e.root = f.root;
    e.down = if b.down.is_some() { b.out_for() } else { 0.0 };
    e.shocked = b.shocked;
    e.soul_lost = c.soul_lost;
    e.sealed = f.sealed;
    e.no_guard = f.no_guard;
    e.choke = c.choke;
    e.rot = c.rot;
    for cu in &f.curses.on {
        if let Some(l) = e.curses.get_mut(cu.kind.index()) {
            *l = l.max(cu.left);
        }
    }
    for a in &f.auras.on {
        if let Some(i) = AuraKind::ALL.iter().position(|k| *k == a.kind) {
            e.auras[i] = true;
        }
    }
    for x in &c.effects {
        if let (Source::Skill(_), Some(left)) = (x.source, x.left) {
            e.buffs += 1.0;
            e.buff_left = e.buff_left.max(left);
            e.buff_size += x
                .mods
                .iter()
                .filter(|m| matches!(m.stat, Stat::Damage | Stat::Taken))
                .map(|m| m.value.abs())
                .sum::<f64>();
        }
    }
    e.shield = if c.shield_max > 0.0 {
        c.shield / c.shield_max
    } else {
        0.0
    };
    e.ward = f.charges;
    e.barrier = f.barrier;
    for (on, k) in e.keystones.iter_mut().zip(Keystone::ALL) {
        *on = f.has(k);
    }
    e.element = c.root.and_then(|r| ROOTS.iter().position(|o| *o == r));
    e
}

/// The traps, mines, totems and minions standing, as fighter `me` sees them: lengths in the minds' pixels, each
/// one's height its body's centre (`UNIT_Y`, or `FLOOR_Y` for what lies on the floor), its time left and arming from
/// its group's `device::Placed`.
fn units_of(state: &State, me: usize) -> Vec<UnitSeen> {
    state
        .units
        .iter()
        .map(|u| {
            let placed = state
                .groups
                .iter()
                .find(|g| g.owner == u.owner && g.skill == u.skill)
                .map(|g| &g.placed);
            let d = placed.and_then(|p| p.standing.iter().find(|d| d.id == u.device));
            let spawn = placed.map(|p| &p.spawn);
            let age = d.map_or(0.0, |d| d.age);
            let next = match u.kind {
                device::Kind::Minion => u.next_strike - state.time,
                device::Kind::Totem => d.map_or(0.0, |d| d.next_cast - d.age),
                device::Kind::Trap | device::Kind::Mine => 0.0,
            };
            UnitSeen {
                mine: u.owner == me,
                kind: match u.kind {
                    device::Kind::Trap => UnitKind::Trap,
                    device::Kind::Mine => UnitKind::Mine,
                    device::Kind::Totem => UnitKind::Totem,
                    device::Kind::Minion => UnitKind::Minion,
                },
                x: seen(u.x),
                y: seen(if u.on_floor() { FLOOR_Y } else { UNIT_Y } * T),
                hp: u.hp() / 100.0,
                left: spawn.and_then(|s| s.lifetime).map(|l| (l - age).max(0.0)),
                armed: spawn.is_none_or(|s| age >= s.arm),
                next: next.max(0.0),
            }
        })
        .collect()
}

/// Fighter `f`'s castable slots for a policy: its three basics, then its bar's slots, each the index of
/// the skill in `f.spells` (`None` for an empty bar slot or a skill it cannot cast this fight).
pub fn slots_of(f: &Fighter) -> Vec<Option<usize>> {
    let at = |name: &str| f.spells.iter().position(|n| *n == name);
    let basics = f
        .kit
        .skills
        .iter()
        .filter(|k| k.place == Place::Basic)
        .map(|k| at(k.gem.id));
    let basics: Vec<Option<usize>> = basics.chain(core::iter::repeat(None)).take(3).collect();
    let bar = (0..BAR.slots).map(|i| f.bar.get(i).and_then(Slot::spell).and_then(at));
    basics.into_iter().chain(bar).collect()
}

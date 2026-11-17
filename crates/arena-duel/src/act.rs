//! Casting and blows: a cast begins (paid, a stored use spent), gathers, lands (`release`, then `deliver` by how the
//! skill works: `kit::Way`) and recovers; a blow is guarded, turned aside by a barrier, erased by a ward, dodged or
//! taken (`strike::hurt`); knockouts; and the effect commands a cast plays. Each draws its dice in a fixed order.

use crate::bar::{ready, spend};
use crate::body::{BodySpec, Doing};
use crate::data::{
    CAST_CIRCLE, CHIP, CLASH, FRAMES, KNOCKBACK, Kind, RING, Spell, T, WARD, block_chance, look,
    motion, spell, tint,
};
use crate::fighter::{Act, Phase, State};
use crate::js::{js_max, js_min, js_sign, or_one};
use crate::kit::Way;
use crate::out::{Note, Out, PlayOpts, Stop};
use crate::shot::{Shot, ShotKind, launch_bolt};
use crate::strike::{
    Origin, PULSE, Payload, Pending, Target, buff, build, can_pay, hurt, lay_curse, pay, payload,
    scale_hit,
};
use crate::unit::{UNIT_RADIUS, enemies_near, hurt_unit, in_front, place};
use arena_calc::build::Built;
use arena_calc::domain;
use arena_calc::event::{Event, EventKind};
use arena_calc::gem;
use arena_calc::hit::{Defence, Rolls, resolve_with};
use arena_calc::motion as calc_motion;
use arena_calc::support::Behaviour;

/// JS `clamp(v, lo, hi)`: `Math.max(lo, Math.min(hi, v))`.
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    js_max(lo, js_min(hi, v))
}

/// A die: `Math.floor(rng() * n)`.
fn roll_index(state: &mut State, n: usize) -> usize {
    let i = libm::floor(state.rng.next() * n as f64) as usize;
    i.min(n.saturating_sub(1))
}

/// A seed for an effect: `Math.floor(rng() * 1e6)`.
fn roll_seed(state: &mut State) -> u32 {
    libm::floor(state.rng.next() * 1e6) as u32
}

/// Start effect `fx` at (`x`, `y`); returns its key.
pub fn play(state: &mut State, fx: &'static str, x: f64, y: f64, opts: PlayOpts) -> u32 {
    state.key += 1;
    let key = state.key;
    state.emit(Out::Play {
        key,
        fx,
        x,
        y,
        opts,
    });
    key
}

/// Play `fx` for fighter `me`'s skill, in its look's colours: `{ ...tint, seed: roll, ...opts }`.
pub fn cast(state: &mut State, me: usize, fx: &'static str, x: f64, y: f64, opts: PlayOpts) -> u32 {
    let base = tint(state.fighters[me].look, fx).copied();
    let seed = roll_seed(state);
    let merged = PlayOpts {
        palette: opts.palette.or(base.and_then(|t| t.palette)),
        accent: opts.accent.or(base.and_then(|t| t.accent)),
        stars: opts.stars.or(base.and_then(|t| t.stars)),
        fire: opts.fire.or(base.and_then(|t| t.fire)),
        glow: opts.glow.or(base.and_then(|t| t.glow)),
        width: opts.width.or(base.and_then(|t| t.width)),
        height: opts.height.or(base.and_then(|t| t.height)),
        flip: opts.flip,
        seed: opts.seed.or(Some(seed)),
    };
    play(state, fx, x, y, merged)
}

/// `cast`, for an effect nobody keeps to stop: a looping one is stopped as it starts, so it shows a moment and fades
/// instead of playing for the rest of the fight (a buff or a curse that borrows a bolt's motion shows the bolt's
/// looping effect once). Every play whose key is dropped goes through here.
pub fn cast_once(
    state: &mut State,
    me: usize,
    fx: &'static str,
    x: f64,
    y: f64,
    opts: PlayOpts,
) -> u32 {
    let key = cast(state, me, fx, x, y, opts);
    if crate::effects::loops(fx) {
        state.emit(Out::Stop { key });
    }
    key
}

/// A streak of `me`'s qi from where it casts to (`x`, `y` up from the ground).
pub fn streak(state: &mut State, me: usize, x: f64, y: f64) {
    let f = &state.fighters[me];
    let (x0, y0) = (f.body.x + f.body.face * 24.0, f.chest());
    state.emit(Out::Streak {
        by: me,
        x0,
        y0,
        x1: x,
        y1: y,
    });
}

/// A spent skill's burst where it ended, when its effect is in the registry.
pub fn burst(state: &mut State, fx: Option<&'static str>, x: f64, y: f64) {
    if let Some(fx) = fx
        && state.has(fx)
    {
        let seed = roll_seed(state);
        play(
            state,
            fx,
            x,
            y,
            PlayOpts {
                seed: Some(seed),
                ..PlayOpts::default()
            },
        );
    }
}

/// Settle fighter `f`'s pose from what the rules have it doing; the body decides when it is off its feet or reeling.
pub fn pose(state: &mut State, f: usize) {
    let me = &mut state.fighters[f];
    let melee = me
        .act
        .is_some_and(|a| spell(a.spell).is_some_and(|s| s.kind == Kind::Melee));
    let doing = match me.act {
        Some(Act {
            phase: Phase::Windup,
            ..
        }) => Doing::Windup { melee },
        Some(Act {
            phase: Phase::Recover,
            ..
        }) => Doing::Recover { melee },
        None if me.guard > 0.0 => Doing::Guard,
        None if me.medit => Doing::Meditate,
        None if me.fly => Doing::Fly,
        None if me.intent != 0.0 => Doing::Walk,
        None => Doing::Idle,
    };
    let alive = me.alive();
    let pose = me.body.pose_of(doing, alive);
    me.body.set_pose(pose);
}

/// Fighter `me` begins casting `name` at `target`: built for the situation, paid, a stored use spent, its bar slot
/// spent, its guard dropped; it gathers for the windup it was built with.
pub fn begin(state: &mut State, me: usize, name: &'static str, target: Option<f64>) {
    let f = &state.fighters[me];
    let Some(k) = f.kit.index(name) else { return };
    if !ready(f, name) {
        return;
    }
    let Some(built) = build(state, me, k, None) else {
        return;
    };
    if !can_pay(&state.fighters[me].c, &built.costs) {
        return;
    }
    let now = state.time;
    let tags = built.hit.tags;
    let (qi, rejection) = {
        let f = &mut state.fighters[me];
        let qi = pay(&mut f.c, &built);
        f.c.on_cast();
        f.c.meditating = false;
        let ks = &mut f.kit.skills[k];
        let fresh = arena_calc::charge::Uses::of(&built);
        ks.uses.max = fresh.max;
        ks.uses.cooldown = fresh.cooldown;
        ks.uses.stored = ks.uses.stored.min(ks.uses.max);
        ks.uses.try_use();
        ks.count += 1;
        if let Some(n) = &mut ks.card {
            *n = n.saturating_sub(1);
        }
        let rejection = ks.supports.iter().find_map(|x| {
            x.behaviours.iter().find_map(|b| match b {
                Behaviour::Rejection { chance, share } => Some((*chance, *share)),
                _ => None,
            })
        });
        (qi, rejection)
    };
    state.fighters[me].history.push((now, 0, 0, qi, 0));
    // a bar skill's cast empties its slot
    if let Some(at) = state.fighters[me]
        .bar
        .iter()
        .position(|s| s.spell() == Some(name))
    {
        spend(state, me, at);
    }
    // Dung's backlash: a share of the blow on the caster
    if let Some((chance, share)) = rejection
        && state.rng.next() < chance
    {
        let amount = built.hit.base.iter().sum::<f64>() * share;
        state.fighters[me].c.hurt(amount);
    }
    let stall: f64 = state.fighters[me]
        .curses
        .on_cast(tags, false)
        .iter()
        .map(|e| match e {
            arena_calc::curse::Event::Stall(s) => *s,
            _ => 0.0,
        })
        .sum();
    state
        .queue
        .push(Event::new(EventKind::Cast, me).with_skill(name, tags));
    state.fighters[me].act = Some(Act {
        spell: name,
        phase: Phase::Windup,
        t: 0.0,
        windup: built.windup + stall,
        recover: built.recover,
        target,
        circle: None,
        blocked: false,
    });
    // a fighter casting is not guarding, as its pose shows
    state.fighters[me].guard = 0.0;
    set_meditating(state, me, false);
    // longer casts stand in a rune circle while they gather
    let kind = state.fighters[me].kit.skills[k].motion.kind;
    if !matches!(kind, Kind::Melee | Kind::Blink | Kind::Dash | Kind::Barrier)
        && state.has(CAST_CIRCLE)
    {
        let f = &state.fighters[me];
        let opts = PlayOpts {
            palette: Some(&look(f.look).circle),
            seed: Some(me as u32 + 1),
            ..PlayOpts::default()
        };
        let (x, g) = (f.body.x, state.ground);
        let key = play(state, CAST_CIRCLE, x, g, opts);
        if let Some(a) = state.fighters[me].act.as_mut() {
            a.circle = Some(key);
        }
    }
}

/// Fighter `me`'s cast moves on by `dt`: at the end of its windup it lands, at the end of its recovery it is over.
/// Then its pose follows.
pub fn advance(state: &mut State, me: usize, dt: f64) {
    if let Some(mut act) = state.fighters[me].act {
        act.t += dt;
        state.fighters[me].act = Some(act);
        if act.phase == Phase::Windup && act.t >= act.windup {
            release(state, me);
            // a blow rung back by a golden bell breaks the striker's own action
            if let Some(a) = state.fighters[me].act {
                state.fighters[me].act = Some(Act {
                    phase: Phase::Recover,
                    t: 0.0,
                    ..a
                });
                if let Some(key) = a.circle {
                    state.emit(Out::Stop { key });
                }
            }
        } else if act.phase == Phase::Recover && act.t >= recover_of(&act) {
            state.fighters[me].act = None;
        }
    }
    pose(state, me);
}

/// The whole span of `act`'s phase: its wind-up, or its recovery as the rules keep it (`recover_of`); what the drawn
/// body stretches the phase's frames over (`pick::appearance`).
pub fn phase_span(act: &Act) -> f64 {
    match act.phase {
        Phase::Windup => act.windup,
        Phase::Recover => recover_of(act),
    }
}

/// How long `act` recovers: an unsafe skill whose blow was guarded recovers longer.
pub fn recover_of(act: &Act) -> f64 {
    if act.blocked && FRAMES.unsafe_on_block.contains(&act.spell) {
        act.recover + FRAMES.blocked_recover
    } else {
        act.recover
    }
}

/// Fighter `me`'s blow was guarded: its act remembers it.
fn guarded(state: &mut State, me: usize) {
    if let Some(a) = state.fighters[me].act.as_mut() {
        a.blocked = true;
    }
}

/// The moment fighter `me`'s skill lands: built again for the situation now, it does what it does.
pub fn release(state: &mut State, me: usize) {
    let Some(act) = state.fighters[me].act else {
        return;
    };
    let Some(k) = state.fighters[me].kit.index(act.spell) else {
        return;
    };
    let Some(built) = build(state, me, k, None) else {
        return;
    };
    deliver(state, me, k, built, act.target, 0);
}

/// How far a skill of motion `sp` reaches, in the arena's pixels: its range, its reach or its distance, else a
/// middle reach (design 7: 4 T).
fn reach_of(sp: &Spell) -> f64 {
    sp.range
        .or(sp.reach)
        .or(sp.distance)
        .filter(|r| *r > 0.0)
        .unwrap_or(4.0 * T)
}

/// Seat `me`'s kit skill `k`, built as `built`, does what it does (`kit::Way`): at the end of a windup, or at once
/// for a triggered cast (chain `depth` above 0).
pub fn deliver(
    state: &mut State,
    me: usize,
    k: usize,
    built: Built,
    target: Option<f64>,
    depth: u8,
) {
    let ks = &state.fighters[me].kit.skills[k];
    let (g, way, sp) = (ks.gem, ks.way, ks.motion);
    let foe = 1 - me;
    let near = (state.fighters[foe].body.x - state.fighters[me].body.x).abs()
        <= reach_of(sp) + state.fighters[foe].body.spec.half_width;
    let (mx, mair) = (state.fighters[me].body.x, state.fighters[me].body.air);
    let g_ = state.ground;
    match way {
        Way::Aura => {
            if let Some(kind) = gem::aura_of(g) {
                let f = &mut state.fighters[me];
                let _ = f
                    .auras
                    .light_with(kind, built.rank, true, &mut f.c, built.reservation);
            }
            show(state, me, sp, mx, g_ - mair);
        }
        Way::Domain { damaging } => {
            if let Some(kind) = gem::domain_of(g) {
                let pos = [
                    state.to_t(state.fighters[0].body.x),
                    state.to_t(state.fighters[1].body.x),
                ];
                let mid = state.width_t() / 2.0;
                let centre = if domain::can_raise(kind, pos[me], mid) {
                    pos[me]
                } else {
                    mid
                };
                let _ = state.field.raise(me, kind, built.rank, centre, pos, mid);
            }
            if damaging {
                let p = payload(state, me, k, &built, depth);
                strikes(state, me, k, &built, p, target);
            } else {
                show(state, me, sp, mx, g_ - mair);
            }
        }
        Way::Curse => {
            if near {
                lay_curse(state, me, k, &built, depth);
            }
            show(
                state,
                me,
                sp,
                state.fighters[foe].body.x,
                g_ - state.fighters[foe].body.air,
            );
        }
        Way::Buff => {
            buff(state, me, g.id, &built);
            show(state, me, sp, mx, g_ - mair);
        }
        Way::Device(_) => {
            place(state, me, k, &built, target);
        }
        Way::Motion => {
            if gem::curse_of(g).is_some() && near {
                lay_curse(state, me, k, &built, depth);
            }
            match sp.kind {
                Kind::Heal => heal(state, me, sp),
                Kind::Ward => ward(state, me, sp),
                Kind::Barrier => barrier(state, me, k),
                Kind::Blink => blink(state, me, sp, target),
                _ => {
                    let p = payload(state, me, k, &built, depth);
                    strikes(state, me, k, &built, p, target);
                }
            }
        }
    }
}

/// A skill's own effect on whoever it acts on, for a skill that strikes nothing (an intent, a buff, a curse).
fn show(state: &mut State, me: usize, sp: &'static Spell, x: f64, y: f64) {
    if let Some(fx) = sp.fx
        && state.has(fx)
    {
        cast_once(state, me, fx, x, y, PlayOpts::default());
    }
}

/// How many blows a cast of `g` moving as `sp` lands, besides what the motion lands on its own: its `Hits` times its
/// repeats, except a barrage or a field, whose strikes and ticks are its hits already.
fn blows_of(g: &gem::Gem, sp: &Spell, built: &Built) -> f64 {
    let own = matches!(sp.kind, Kind::Void)
        || (sp.kind == Kind::Zone && sp.strikes.is_some_and(|n| n > 1.0));
    let hits = if own { 1.0 } else { g.hits() };
    (hits * f64::from(built.strikes)).max(0.0)
}

/// A striking cast's blows: the first now, the rest `PULSE` apart; a fraction of a blow lands last at that fraction.
fn strikes(state: &mut State, me: usize, k: usize, built: &Built, p: Payload, target: Option<f64>) {
    let ks = &state.fighters[me].kit.skills[k];
    let (g, sp) = (ks.gem, ks.motion);
    let total = blows_of(g, sp, built);
    let count = (libm::ceil(total - 1e-9).max(1.0)) as usize;
    let frac = total - (count as f64 - 1.0);
    let origin = Origin::of(state, me);
    for i in 0..count {
        let mut p = p.clone();
        if i + 1 == count && frac < 1.0 - 1e-9 {
            scale_hit(&mut p.hit, frac.max(0.0));
        }
        if i == 0 {
            strike(state, sp, p, origin, target, true);
        } else {
            let at = state.time + i as f64 * PULSE;
            state.pending.push(Pending {
                at,
                payload: p,
                motion: sp,
                origin,
                aim: target,
                target: None,
                retry: false,
            });
        }
    }
}

/// One blow of motion `sp` from `o`: as a blade, a blast, a dash, a bolt, an orb, a zone or a field. `first`: the
/// cast's first blow, which moves the caster (a dash, a dive).
pub fn strike(
    state: &mut State,
    sp: &'static Spell,
    p: Payload,
    o: Origin,
    target: Option<f64>,
    first: bool,
) {
    let me = p.by;
    let foe = 1 - me;
    let g = state.ground;
    let (mx, mair, face) = (o.x, o.air, o.face);
    let caster = o.unit.is_none();
    let mspec = state.fighters[me].body.spec;
    let fspec = state.fighters[foe].body.spec;
    match sp.kind {
        Kind::Melee => {
            if caster && first {
                state.fighters[me].body.vx += face * 200.0;
                if sp.dive {
                    state.fighters[me].fly = false;
                    // a dive that ends on the ground cracks it
                    if let Some(slam) = sp.slam_fx
                        && mair < 8.0
                        && state.has(slam)
                    {
                        cast_once(state, me, slam, mx + face * 24.0, g, PlayOpts::default());
                        state.emit(Out::Land {
                            x: mx,
                            strength: 1.0,
                        });
                    }
                }
            }
            let range = sp.range.unwrap_or(0.0);
            match in_front(state, me, mx, mair, face, range, mspec.chest + 16.0) {
                Some(Target::Unit(u)) => {
                    hurt_unit(state, u, &p);
                }
                Some(Target::Fighter(_)) => {
                    let fx = state.fighters[foe].body.x;
                    let fair = state.fighters[foe].body.air;
                    let slash_windup = spell("slash").map_or(0.0, |s| s.windup);
                    let crossing = caster
                        && state.fighters[foe].act.is_some_and(|a| {
                            a.spell == "slash"
                                && a.phase == Phase::Windup
                                && a.t > slash_windup * 0.6
                        });
                    let _ = fair;
                    if blocks(state, foe, mx, &p) {
                        if caster {
                            guarded(state, me);
                        }
                        clash(state, (mx + fx) / 2.0, g - mspec.chest - mair, &[me, foe]);
                    } else if crossing {
                        // both blades swing at once: they meet
                        state.note(Note::Stopped {
                            by: me,
                            spell: p.skill,
                            how: Stop::Clash,
                            chip: 0.0,
                        });
                        state.note(Note::Broken {
                            who: foe,
                            spell: "slash",
                        });
                        clash(state, (mx + fx) / 2.0, g - mspec.chest - mair, &[me, foe]);
                        state.fighters[foe].act = None;
                    } else {
                        hurt(state, foe, &p, face);
                    }
                }
                // a fighter's blade that missed tries again next tick while its clip's active window lasts (its blade
                // still out ahead as the frames go by), one hit a swing (`frames::in_window`)
                None if caster && crate::frames::in_window(state, me) => {
                    state.pending.push(Pending {
                        at: state.time + crate::duel::TICK,
                        payload: p,
                        motion: sp,
                        origin: o,
                        aim: target,
                        target: None,
                        retry: true,
                    });
                }
                None => {}
            }
        }
        Kind::Bolt => {
            launch_bolt(state, sp, p, o);
        }
        Kind::Orb => {
            let x = mx + face * 40.0;
            let key = play(
                state,
                sp.fx.unwrap_or(""),
                x,
                g,
                PlayOpts {
                    seed: Some(me as u32 + 3),
                    ..PlayOpts::default()
                },
            );
            push(state, sp, p, Some(key), ShotKind::Orb { x });
        }
        Kind::Zone => {
            let x = clamp(
                target.unwrap_or(state.fighters[foe].body.x),
                state.left + 24.0,
                state.right - 24.0,
            );
            // a unit's zone lands no farther than the skill reaches from it
            let x = if caster {
                x
            } else {
                let r = reach_of(sp);
                mx + (x - mx).clamp(-r, r)
            };
            // what hangs in the air (a collapse) gathers at the foe's height; what falls hits the floor
            let air = if sp.grounded {
                0.0
            } else {
                state.fighters[foe].body.air
            };
            // a sealed skill first marks the spot with the caster's seal, which the foe can see and leave
            let sigil = sp.sigil.unwrap_or(0.0);
            let mark = if sigil > 0.0 && state.has(CAST_CIRCLE) {
                let opts = PlayOpts {
                    palette: Some(&look(state.fighters[me].look).circle),
                    seed: Some(me as u32 + 11),
                    ..PlayOpts::default()
                };
                Some(play(state, CAST_CIRCLE, x, g - air, opts))
            } else {
                None
            };
            // a barrage falls strike after strike, each where the foe will be when it lands
            let strikes = sp.strikes.unwrap_or(1.0);
            let mut n = 0.0;
            while n < strikes {
                let kind = ShotKind::Zone {
                    x,
                    air,
                    lead: sigil,
                    follow: n > 0.0,
                    aimed: false,
                    shown: false,
                    fell: false,
                };
                let key = if n == 0.0 { mark } else { None };
                let id = push(state, sp, p.clone(), key, kind);
                if let Some(s) = state.shots.iter_mut().find(|s| s.id == id) {
                    s.t = -n * sp.every.unwrap_or(0.0) - sigil;
                }
                n += 1.0;
            }
            if caster {
                streak(state, me, x, air + zone_height(sp, &fspec));
            }
        }
        Kind::Blast => {
            let range = sp.range.unwrap_or(0.0);
            let x = mx + face * range * 0.7;
            // a spray keeps the effect and stops it when it ends; a plain blast plays it once
            let play_blast = if sp.spray.is_some() { cast } else { cast_once };
            let key = play_blast(
                state,
                me,
                sp.fx.unwrap_or(""),
                x,
                g - mspec.chest - mair,
                PlayOpts::default(),
            );
            if sp.spray.is_some() {
                let mut q = p.clone();
                q.hit = arena_calc::hit::Hit::default();
                push(state, sp, q, Some(key), ShotKind::Spray);
            }
            for t in enemies_near(state, me, mx, mair, range, fspec.body) {
                match t {
                    Target::Unit(u) => {
                        hurt_unit(state, u, &p);
                    }
                    // the blast goes off ahead of its caster: a foe behind it, or above or below its blast, is not caught
                    Target::Fighter(_)
                        if !crate::unit::strike_hits(
                            state, me, mx, mair, face, range, fspec.body, false,
                        ) => {}
                    Target::Fighter(_) => {
                        let fx = state.fighters[foe].body.x;
                        let fair = state.fighters[foe].body.air;
                        if blocks(state, foe, mx, &p) {
                            if caster {
                                guarded(state, me);
                            }
                            clash(state, fx - face * 16.0, g - fspec.chest - fair, &[foe]);
                        } else {
                            hurt(state, foe, &p, face);
                        }
                    }
                }
            }
        }
        Kind::Void => {
            let x = clamp(
                state.fighters[foe].body.x,
                state.left + 40.0,
                state.right - 40.0,
            );
            let key = cast(
                state,
                me,
                sp.fx.unwrap_or(""),
                x,
                g,
                PlayOpts {
                    seed: Some(me as u32 + 5),
                    ..PlayOpts::default()
                },
            );
            push(state, sp, p, Some(key), ShotKind::Void { x, tick: 0.0 });
            if caster {
                let height = sp.height.unwrap_or(0.0);
                streak(state, me, x, if height != 0.0 { height } else { 16.0 });
            }
        }
        Kind::Dash if caster && first => {
            // gone in a flash and back a stride past the foe, cutting through it on the way
            let from = mx;
            let reach =
                calc_motion::dash_reach_of(&state.fighters[me].c, sp.distance.unwrap_or(0.0) / T)
                    * T;
            let to = clamp(
                mx + face * reach,
                state.left + mspec.half_width,
                state.right - mspec.half_width,
            );
            let fx = sp.fx.unwrap_or("");
            cast_once(state, me, fx, from, g - mair, PlayOpts::default());
            state.fighters[me].body.x = to;
            cast_once(state, me, fx, to, g - mair, PlayOpts::default());
            let y = mair + mspec.chest;
            state.emit(Out::Streak {
                by: me,
                x0: from,
                y0: y,
                x1: to,
                y1: y,
            });
            let lo = js_min(from, to);
            let hi = js_max(from, to);
            for t in crate::unit::enemies_between(state, me, lo, hi, mair, fspec.body) {
                match t {
                    Target::Unit(u) => {
                        hurt_unit(state, u, &p);
                    }
                    Target::Fighter(_) => {
                        let (fx, fair) = (state.fighters[foe].body.x, state.fighters[foe].body.air);
                        if blocks(state, foe, from, &p) {
                            guarded(state, me);
                            clash(state, fx, g - fspec.chest - fair, &[foe]);
                        } else {
                            hurt(state, foe, &p, face);
                        }
                    }
                }
            }
        }
        // a dash's later blows, or a unit's: what stands close in front
        Kind::Dash => {
            let reach = mspec.half_width * 3.0;
            match in_front(state, me, mx, mair, face, reach, fspec.body) {
                Some(Target::Unit(u)) => {
                    hurt_unit(state, u, &p);
                }
                Some(Target::Fighter(_)) if !blocks(state, foe, mx, &p) => {
                    hurt(state, foe, &p, face);
                }
                _ => {}
            }
        }
        Kind::Heal | Kind::Ward | Kind::Barrier | Kind::Blink => {}
    }
}

/// A heal: life back by its share of full life, and every ailment washed off.
fn heal(state: &mut State, me: usize, sp: &'static Spell) {
    let g = state.ground;
    let f = &mut state.fighters[me];
    f.c.ailments = arena_calc::ailment::Ailments::default();
    let amount = sp.amount.unwrap_or(0.0) / 100.0 * f.c.life_max;
    let gained = f.c.heal(amount);
    let share = gained / f.c.life_max.max(1.0) * 100.0;
    state.note(Note::Healed { by: me, share });
    let f = &state.fighters[me];
    let (x, y, air, body) = (f.body.x, f.chest(), f.body.air, f.body.spec.body);
    state.emit(Out::Heal {
        by: me,
        amount: libm::round(gained),
        x,
        y,
    });
    cast_once(
        state,
        me,
        sp.fx.unwrap_or(""),
        x,
        g - body - air,
        PlayOpts::default(),
    );
}

/// A ward: every ailment washed off, and charges that each erase a whole blow.
fn ward(state: &mut State, me: usize, sp: &'static Spell) {
    let g = state.ground;
    state.fighters[me].c.ailments = arena_calc::ailment::Ailments::default();
    let charges = 1.0 + roll_index(state, WARD.charges as usize) as f64;
    let f = &mut state.fighters[me];
    f.charges = charges;
    f.shield_t = WARD.time;
    let (x, y, air, body) = (f.body.x, f.chest(), f.body.air, f.body.spec.body);
    state.emit(Out::Ward {
        by: me,
        amount: charges,
        x,
        y,
    });
    let lift = if sp.feet { 0.0 } else { body };
    cast_once(
        state,
        me,
        sp.fx.unwrap_or(""),
        x,
        g - lift - air,
        PlayOpts::default(),
    );
}

/// A barrier: it turns blows aside for its time.
fn barrier(state: &mut State, me: usize, k: usize) {
    let g = state.ground;
    let (id, sp) = {
        let ks = &state.fighters[me].kit.skills[k];
        (ks.gem.id, ks.motion)
    };
    drop_barrier(state, me);
    state.fighters[me].barrier = sp.time.unwrap_or(0.0);
    state.fighters[me].barrier_spell = Some(id);
    let (mx, mair) = (state.fighters[me].body.x, state.fighters[me].body.air);
    let key = cast(
        state,
        me,
        sp.fx.unwrap_or(""),
        mx,
        g - mair,
        PlayOpts {
            seed: Some(me as u32 + 21),
            ..PlayOpts::default()
        },
    );
    state.fighters[me].barrier_key = Some(key);
    let y = state.fighters[me].chest();
    state.emit(Out::Barrier { by: me, x: mx, y });
}

/// A blink: to its target, or trading places with the foe; shortened by Trọng (`motion::dash_reach_of`).
fn blink(state: &mut State, me: usize, sp: &'static Spell, target: Option<f64>) {
    let g = state.ground;
    let foe = 1 - me;
    let (mx, mair) = (state.fighters[me].body.x, state.fighters[me].body.air);
    let spec = state.fighters[me].body.spec;
    let fx = sp.fx.unwrap_or("");
    cast_once(state, me, fx, mx, g - mair, PlayOpts::default());
    let to = if sp.swap {
        let at = state.fighters[foe].body.x;
        state.fighters[foe].body.x = mx;
        at
    } else {
        let want = target.unwrap_or(mx);
        let full = sp.distance.unwrap_or(f64::INFINITY);
        let reach = calc_motion::dash_reach_of(&state.fighters[me].c, full / T) * T;
        let at = mx + (want - mx).clamp(-reach, reach);
        clamp(
            at,
            state.left + spec.half_width,
            state.right - spec.half_width,
        )
    };
    state.fighters[me].body.x = to;
    cast_once(state, me, fx, to, g - mair, PlayOpts::default());
    state.queue.push(Event::new(EventKind::Blinked, me));
}

/// Put a new shot of seat `p.by` in flight; returns its id.
pub fn push(
    state: &mut State,
    sp: &'static Spell,
    p: Payload,
    key: Option<u32>,
    kind: ShotKind,
) -> u32 {
    state.shot_id += 1;
    let id = state.shot_id;
    // a field's tick strikes for its share of the motion's whole blow
    let m = gem::gem(p.skill).and_then(|g| motion(g.spell));
    let tick = m.map_or(0.0, |m| {
        let whole = m.damage.unwrap_or(0.0);
        if whole > 0.0 {
            m.tick.unwrap_or(0.0) / whole
        } else {
            0.0
        }
    });
    let binds = p.binds();
    if binds {
        state.fighters[p.by].bound = true;
    }
    state.shots.push(Shot {
        binds,
        id,
        owner: p.by,
        name: p.skill,
        spell: sp,
        t: 0.0,
        key,
        tick,
        payload: p,
        kind,
    });
    id
}

/// How high a zone strikes: what hangs in the air (a collapse) at the foe's chest, what falls at the floor.
pub fn zone_height(sp: &Spell, foe: &BodySpec) -> f64 {
    if sp.grounded { 0.0 } else { foe.chest }
}

/// Does fighter `f` block blow `p` from `from_x`? A block costs qi and still lets a share through (chip); with too
/// little qi left the guard breaks and the blow lands; a blow too heavy for the guard breaks through whole.
pub fn blocks(state: &mut State, f: usize, from_x: f64, p: &Payload) -> bool {
    let me = &mut state.fighters[f];
    if me.guard == 0.0 || me.body.reeling() || js_sign(from_x - me.body.x) != me.body.face {
        return false;
    }
    if me.c.qi < crate::data::GUARD_COST {
        me.guard = 0.0;
        return false;
    }
    me.c.qi -= crate::data::GUARD_COST;
    if state.rng.next() >= block_chance(p.power) {
        return false;
    }
    // what gets through a guard is a share of the blow, through the damage system like any blow
    let mut hit = p.hit.clone();
    scale_hit(&mut hit, CHIP);
    let by = p.by;
    let distance = state.distance_t();
    let out = {
        let [a, b] = &mut state.fighters;
        let (striker, target) = if by == 0 { (&*a, b) } else { (&*b, a) };
        let def = Defence {
            sheet: &target.sheet,
            distance,
            moving: target.moving,
            striker_cursed: !striker.curses.on.is_empty(),
        };
        let s = p.unit.is_none().then_some(&striker.c);
        resolve_with(&hit, s, &mut target.c, &def, Rolls::NONE)
    };
    let chip = out.to_life / state.fighters[f].c.life_max.max(1.0) * 100.0;
    state.note(Note::Stopped {
        by,
        spell: p.skill,
        how: Stop::Guard,
        chip,
    });
    true
}

/// Does fighter `f`'s barrier turn blow `p` aside? A golden bell rings back at a blade,
/// blast or dash that struck it from close by: the striker takes the bell's own blow, is stunned and thrown off.
pub fn turns_aside(state: &mut State, f: usize, dir: f64, p: &Payload) -> bool {
    let s = spell(p.skill);
    let me = &state.fighters[f];
    if me.barrier == 0.0 {
        return false;
    }
    let (x, air, spec) = (me.body.x, me.body.air, me.body.spec);
    state.note(Note::Stopped {
        by: p.by,
        spell: p.skill,
        how: Stop::Barrier,
        chip: 0.0,
    });
    turn_aside(state, f, x - dir * 32.0, state.ground - spec.chest - air);
    let me = &state.fighters[f];
    let Some(barrier_spell) = me.barrier_spell else {
        return true;
    };
    let counter = spell(barrier_spell).and_then(|b| b.counter);
    let by = p.by;
    if p.unit.is_some() {
        return true;
    }
    let other = &state.fighters[by];
    let close = (other.body.x - x).abs() < 240.0 && (other.body.air - air).abs() < spec.body;
    let struck_close = s.is_some_and(|s| matches!(s.kind, Kind::Melee | Kind::Blast | Kind::Dash));
    if let Some(counter) = counter
        && close
        && struck_close
    {
        let (x1, y1) = (other.body.x, other.chest());
        let y0 = state.fighters[f].chest();
        state.emit(Out::Streak {
            by: f,
            x0: x,
            y0,
            x1,
            y1,
        });
        // the bell's blow: its power in force, as the bell's owner strikes
        let mut hit = arena_calc::hit::Hit::split(
            counter.damage * crate::data::POWER,
            &[(arena_calc::element::Element::Force, 1.0)],
        );
        hit.tags = gem::gem(barrier_spell).map_or(hit.tags, |g| g.all_tags());
        let q = Payload {
            by: f,
            skill: barrier_spell,
            hit,
            control: 1.0,
            knockback: 0.0,
            depth: 1,
            unit: None,
            power: counter.damage,
            powers: arena_calc::projectile::Powers::NONE,
        };
        if hurt(state, by, &q, or_one(js_sign(x1 - x))) {
            let t = state.fighters[by].c.control(counter.stun, false);
            crate::strike::control(state, by, arena_calc::ailment::Control::Stun, t);
        }
    }
    true
}

/// Fighter `f`'s barrier rings where a blow struck it, at (`x`, `y`) on screen.
pub fn turn_aside(state: &mut State, f: usize, x: f64, y: f64) {
    state.fighters[f].rang = RING;
    let (fx, fy) = (state.fighters[f].body.x, state.fighters[f].chest());
    state.emit(Out::Block {
        by: f,
        x: fx,
        y: fy,
    });
    if state.has(CLASH) {
        cast_once(
            state,
            f,
            CLASH,
            x,
            y + 120.0,
            PlayOpts {
                height: Some(120.0),
                seed: Some(f as u32 + 31),
                ..PlayOpts::default()
            },
        );
    }
}

/// Fighter `f`'s barrier falls, and its dome goes.
pub fn drop_barrier(state: &mut State, f: usize) {
    if let Some(key) = state.fighters[f].barrier_key {
        state.emit(Out::Stop { key });
    }
    state.fighters[f].barrier = 0.0;
    state.fighters[f].barrier_key = None;
}

/// Two blows meet at (`x`, `y` on screen): a clash, its spark, and the fighters in it pushed apart.
pub fn clash(state: &mut State, x: f64, y: f64, fighters: &[usize]) {
    let up = state.ground - y;
    state.emit(Out::Clash { x, y: up });
    if state.has(CLASH) {
        let seed = roll_seed(state);
        play(
            state,
            CLASH,
            x,
            y + 120.0,
            PlayOpts {
                height: Some(120.0),
                seed: Some(seed),
                ..PlayOpts::default()
            },
        );
    }
    for &f in fighters {
        let body = &mut state.fighters[f].body;
        body.vx += js_sign(or_one(body.x - x)) * KNOCKBACK;
    }
}

/// Fighter `f` is knocked out: the other wins the round, and both stop what they were doing.
pub fn knock_out(state: &mut State, f: usize) {
    let other = 1 - f;
    let (x, y) = (state.fighters[f].body.x, state.fighters[f].chest());
    state.emit(Out::Ko { x, y });
    // the knockout throws the loser off its feet, if the blow has not already
    let ox = state.fighters[other].body.x;
    state.fighters[f].body.knock_out(ox);
    state.fighters[f].fly = false;
    state.fighters[other].fly = false;
    state.winner = Some(other);
    state.wins[other] += 1;
    break_act(state, f);
    set_meditating(state, f, false);
    break_act(state, other);
    set_meditating(state, other, false);
}

/// Fighter `f` stops what it was doing: a cast still gathering loses its rune circle.
pub fn break_act(state: &mut State, f: usize) {
    if let Some(a) = state.fighters[f].act
        && a.phase == Phase::Windup
        && let Some(key) = a.circle
    {
        state.emit(Out::Stop { key });
    }
    state.fighters[f].act = None;
    // its blade's retries go with the swing
    state
        .pending
        .retain(|d| !(d.retry && d.payload.by == f && d.origin.unit.is_none()));
}

/// Between rounds: the loser falls and lies where it lands; the winner comes down and stands over it.
pub fn settle(state: &mut State, f: usize, dt: f64) {
    let foe_x = state.fighters[1 - f].body.x;
    let won = state.winner == Some(f);
    let arena = state.arena();
    let State { rng, fighters, .. } = state;
    let me = &mut fighters[f];
    let alive = me.alive();
    let events = me
        .body
        .settle(dt, won, alive, foe_x, &arena, &mut || rng.next());
    for e in events {
        state.emit(e.into());
    }
}

/// Fighter `f` sits down to meditate, or gets up (the page draws it outlined while it sits).
pub fn set_meditating(state: &mut State, f: usize, on: bool) {
    state.fighters[f].medit = on;
    state.fighters[f].c.meditating = on;
}

/// A blow that was due (`strike::Pending`) lands now: a stored force on what it was stored for, or a skill's repeat
/// struck again from where it began.
pub fn due(state: &mut State, d: Pending) {
    match d.target {
        Some(Target::Fighter(f)) => {
            let dir = or_one(js_sign(
                state.fighters[f].body.x - state.fighters[d.payload.by].body.x,
            ));
            hurt(state, f, &d.payload, dir);
        }
        Some(Target::Unit(u)) => {
            hurt_unit(state, u, &d.payload);
        }
        None => {
            // a repeat strikes from where its caster (or unit) stands now
            let o = match d.origin.unit {
                Some(u) => match state.units.iter().find(|x| x.id == u) {
                    Some(unit) => Origin {
                        x: unit.x,
                        air: 0.0,
                        face: d.origin.face,
                        unit: Some(u),
                    },
                    None => return,
                },
                None => {
                    if !state.fighters[d.payload.by].alive() {
                        return;
                    }
                    // a missed blade's retry, its window shut (or its act broken) since: the swing is over
                    if d.retry && !crate::frames::in_window(state, d.payload.by) {
                        return;
                    }
                    Origin::of(state, d.payload.by)
                }
            };
            strike(state, d.motion, d.payload, o, d.aim, false);
        }
    }
}

/// The radius of a unit's body, in the arena's pixels, re-exported for the shots.
pub const UNIT_BODY: f64 = UNIT_RADIUS * T;

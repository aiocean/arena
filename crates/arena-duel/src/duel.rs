//! The duel: two fighters and their seats, stepped in whole ticks, round after round.

use crate::act::{advance, begin, break_act, drop_barrier, get_up, knock_out, settle, sit_down};
use crate::bar::draw;
use crate::body::{Drive, Motion};
use crate::data::{BAR, DECOR, FLY, JUMP_QI, Look, QI_REGEN, ROUND_PAUSE, STALE};
use crate::fighter::{Fighter, Sit, Slot, State, Told};
use crate::js::{js_max, js_min};
use crate::seat::{Brain, MakeSeat, Seat};
use crate::shot::{shot, stop_shot};
use crate::strike::run_queue;
use crate::unit;
use arena_calc::ailment::AIL;
use arena_calc::aura;
use arena_calc::keystone::Keystone;
use arena_calc::loadout::{self, Loadout, LoadoutError};
use arena_mind::{Choice, Rng};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{ScheduleLabel, SingleThreadedExecutor};

/// One tick of the rules, in seconds: a seed is one fight at any frame rate.
pub const TICK: f64 = 1.0 / 60.0;
/// The most one step runs, in seconds; the rest is dropped, as after a stall.
pub const STEP_MAX: f64 = 0.25;

/// Where a duel is fought: the arena from `left` to `right` (game pixels) standing on `ground`, how high a fighter
/// may fly (`sky`, below whatever hangs over the arena), and the effects in the registry.
#[derive(Clone, Debug, PartialEq)]
pub struct DuelConfig {
    pub seed: u32,
    pub left: f64,
    pub right: f64,
    pub ground: f64,
    pub sky: f64,
    pub effects: Vec<String>,
}

/// The two seats that decide for the fighters. They hold minds and bot instances that stay on one thread, so the
/// world keeps them as a non-send resource.
pub struct Seats(pub [Box<dyn Seat>; 2]);

/// Whether this tick is fought (no winner yet when it began) or spent in the pause between rounds.
#[derive(Resource, Clone, Copy, Debug, Default)]
struct Fighting(bool);

/// The schedule of one tick of the rules.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tick;

/// The salt of the loadouts' dice, so they never share a stream with the rules' (`Rng::new(seed)`) or the styles'.
const LOADOUT_SALT: u32 = 0x10ad_0a75;

/// Both fighters' loadouts for a fight of `seed`, legal and at random (`loadout::random`), on dice of their own: drawing
/// them never moves a fight's dice. A seat whose draw fails (never, for the pool as it is) fights with the basics.
pub fn loadouts(seed: u32) -> [Loadout; 2] {
    let mut rng = Rng::new(seed ^ LOADOUT_SALT);
    let mut one = || loadout::random(&mut || rng.next()).unwrap_or_default();
    [one(), one()]
}

/// A duel: its state and its two seats in one world, stepped in whole ticks of one single-threaded schedule, its
/// systems chained in the rules' order (so a seed is one fight on every machine). Traps, mines, totems and minions
/// are entities of that world (`unit::Unit`).
pub struct Duel {
    world: World,
    tick: Schedule,
}

impl Duel {
    /// A new duel with a random legal loadout for each fighter (`loadouts`), each fighter's seat made by `seats` once
    /// both fighters exist, its first round begun.
    pub fn new(config: DuelConfig, seats: [MakeSeat; 2]) -> Duel {
        Duel::new_with_looks(config, seats, [Look::of_seat(0), Look::of_seat(1)])
    }

    /// As [`Duel::new`], fighter `i` looking as `looks[i]`: its art, so its frames, its hit boxes and where its hands
    /// are, and its colours; what the policy reads as who it is (`style::tag`). `new` gives each fighter its seat's
    /// look; a ladder, the trainer or a server draws looks apart from seats (`ladder::looks_of`), so that neither seat
    /// keeps one look's art.
    pub fn new_with_looks(config: DuelConfig, seats: [MakeSeat; 2], looks: [Look; 2]) -> Duel {
        let l = loadouts(config.seed);
        Duel::build(config, seats, l, looks)
    }

    /// As [`Duel::new`], the looks drawn from the seed apart from the seats (`ladder::looks_of`): a live match as the
    /// server fights it, and every replay of one (the page, `arena-watch`). The replay must build its duel here too,
    /// or an odd seed's fighters wear each other's art, hit boxes and hands, and the fight it steps is another.
    pub fn new_drawn(config: DuelConfig, seats: [MakeSeat; 2]) -> Duel {
        let looks = crate::ladder::looks_of(config.seed);
        Duel::new_with_looks(config, seats, looks)
    }

    /// A new duel of the two `loadouts` (seat 0's, seat 1's), each checked (`loadout::check`).
    pub fn with_loadouts(
        config: DuelConfig,
        seats: [MakeSeat; 2],
        loadouts: [Loadout; 2],
    ) -> Result<Duel, Vec<LoadoutError>> {
        let mut errs = Vec::new();
        for l in &loadouts {
            if let Err(e) = loadout::check(l) {
                errs.extend(e);
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Duel::build(
            config,
            seats,
            loadouts,
            [Look::of_seat(0), Look::of_seat(1)],
        ))
    }

    fn build(
        config: DuelConfig,
        seats: [MakeSeat; 2],
        loadouts: [Loadout; 2],
        looks: [Look; 2],
    ) -> Duel {
        let top = js_min(FLY.top, config.sky);
        let [la, lb] = loadouts;
        let mut state = State::new(
            config.seed,
            config.left,
            config.right,
            config.ground,
            top,
            config.effects,
            [Fighter::new(0, la), Fighter::new(1, lb)],
        );
        for ((f, style), look) in state
            .fighters
            .iter_mut()
            .zip(crate::style::draw(config.seed))
            .zip(looks)
        {
            f.style = style;
            f.look = look;
        }
        // a skill whose effect is not in the registry is never cast
        let effects = state.effects.clone();
        for f in &mut state.fighters {
            for k in &mut f.kit.skills {
                k.usable = k.motion.fx.is_none_or(|fx| effects.iter().any(|e| e == fx));
            }
        }
        let [make0, make1] = seats;
        let seats = Seats([make0(&state, 0), make1(&state, 1)]);
        state.decor = DECOR.iter().any(|d| state.has(d));
        let mut world = World::new();
        world.insert_resource(state);
        world.insert_non_send(seats);
        world.insert_resource(Fighting(false));
        let mut duel = Duel {
            world,
            tick: tick_schedule(),
        };
        duel.world.run_system_cached(start_round).ok();
        duel
    }

    /// A duel of two brains.
    pub fn with_brains(config: DuelConfig) -> Duel {
        Duel::new(config, [Brain::make(), Brain::make()])
    }

    /// The duel's state.
    pub fn state(&self) -> &State {
        self.world.resource::<State>()
    }

    /// The duel's state, to change by hand (tests and tools; the rules change it only in a tick).
    pub fn state_mut(&mut self) -> &mut State {
        self.world.resource_mut::<State>().into_inner()
    }

    /// The world the duel runs in: its `State`, its `Seats` and its units.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// The world, to read with queries (its units).
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Fighter `me`'s seat.
    pub fn seat(&self, me: usize) -> &dyn Seat {
        self.world.non_send::<Seats>().0[me].as_ref()
    }

    /// Fighter `me`'s seat thinks nothing for `seconds`.
    pub fn hold(&mut self, me: usize, seconds: f64) {
        self.world.non_send_mut::<Seats>().0[me].hold(seconds);
    }

    /// Advance by `dt` seconds (at most `STEP_MAX`; a step that is not a positive number moves nothing); returns the
    /// effect commands and events of this step. The rules run in whole ticks: what is left of `dt` waits for the next
    /// call.
    pub fn step(&mut self, dt: f64) -> Vec<crate::out::Out> {
        {
            let mut state = self.world.resource_mut::<State>();
            state.out.clear();
            state.notes.clear();
            let dt = if dt > 0.0 { js_min(dt, STEP_MAX) } else { 0.0 };
            state.carry += dt;
        }
        // a hair under a whole tick counts as one, so 1/60 steps at 60 Hz never drift a tick behind
        while self.state().carry >= TICK - 1e-9 {
            self.tick.run(&mut self.world);
            let state = self.state_mut();
            state.carry = js_max(0.0, state.carry - TICK);
        }
        core::mem::take(&mut self.state_mut().out)
    }
}

/// One tick of the rules as a schedule: the units gathered in id order, the clock, the pause between rounds, the
/// domains and intents, each fighter's turn (seat 0's before seat 1's), the bodies kept apart, the shots in flight,
/// the blows due, the units, time wearing everyone, the knock-outs, what the seats are told, the units written back.
pub fn tick_schedule() -> Schedule {
    let mut tick = Schedule::new(Tick);
    tick.set_executor(SingleThreadedExecutor::new());
    let fighting = |f: Res<Fighting>| f.0;
    tick.add_systems(
        (
            unit::gather,
            clock,
            between_rounds.run_if(not(fighting)),
            (
                fields, turns, keep_apart, shots, pending, units, wear, knock_outs, hold, flush,
            )
                .chain()
                .run_if(fighting),
            unit::scatter,
        )
            .chain(),
    );
    tick
}

/// Hand each seat what the rules told it, in order.
fn flush(mut state: ResMut<State>, mut seats: NonSendMut<Seats>) {
    tell_seats(&mut state, &mut seats);
}

fn tell_seats(state: &mut State, seats: &mut Seats) {
    for (me, told) in core::mem::take(&mut state.told) {
        match told {
            Told::Outcome(o) => seats.0[me].tell(&o),
            Told::Reset => seats.0[me].reset(),
        }
    }
}

/// The clock moves on, and whether this tick is fought is settled for all of it.
fn clock(mut state: ResMut<State>, mut fighting: ResMut<Fighting>) {
    state.time += TICK;
    state.ticks += 1;
    fighting.0 = state.winner.is_none();
}

/// Between rounds the fallen settle; after the pause the next round begins.
fn between_rounds(mut state: ResMut<State>, mut seats: NonSendMut<Seats>) {
    state.pause += TICK;
    for f in 0..2 {
        settle(&mut state, f, TICK);
    }
    if state.pause >= ROUND_PAUSE {
        begin_round(&mut state, &mut seats);
    }
}

fn start_round(mut state: ResMut<State>, mut seats: NonSendMut<Seats>) {
    begin_round(&mut state, &mut seats);
}

fn begin_round(state: &mut State, seats: &mut Seats) {
    let span = state.right - state.left;
    state.round += 1;
    state.pause = 0.0;
    state.winner = None;
    for s in core::mem::take(&mut state.shots) {
        stop_shot(state, &s);
    }
    for u in core::mem::take(&mut state.units) {
        state.emit(crate::out::Out::UnitDie { id: u.id, x: u.x });
    }
    unit::clear(state);
    state.pending.clear();
    state.queue = arena_calc::event::Queue::new();
    state.field.new_round();
    for f in 0..2 {
        // what the last round left standing goes with it
        break_act(state, f);
        drop_barrier(state, f);
        state.fighters[f].refresh();
        state.tell(f, Told::Reset);
        state.fighters[f].bar.clear();
        let at = if f == 0 { 0.22 } else { 0.78 };
        state.fighters[f].body.start_round(state.left + span * at);
    }
    for f in 0..2 {
        crate::strike::see(state, f);
    }
    for f in 0..2 {
        for i in 0..BAR.slots {
            draw(state, f, i, None);
        }
    }
    tell_seats(state, seats);
}

/// The domains run down and put their rules on the fighters; each fighter's intents age and press (positions in T).
fn fields(mut state: ResMut<State>) {
    let state = state.as_mut();
    let pos = [
        state.to_t(state.fighters[0].body.x),
        state.to_t(state.fighters[1].body.x),
    ];
    state.field.tick(crate::duel::TICK, pos);
    let moving = [state.fighters[0].moving, state.fighters[1].moving];
    {
        let State {
            fighters, field, ..
        } = state;
        let [a, b] = fighters;
        field.sync([&mut a.c, &mut b.c], pos, moving);
    }
    for me in 0..2 {
        let [a, b] = &mut state.fighters;
        let (mine, theirs) = if me == 0 { (a, b) } else { (b, a) };
        let calm = mine.has(Keystone::GreyHeart);
        mine.auras.tick(TICK, &mut mine.c);
        aura::sync(
            &mine.auras,
            &mine.sheet,
            &mut mine.c,
            pos[me],
            &mut theirs.c,
            pos[1 - me],
            calm,
        );
    }
}

/// Both fighters' turns, the first of them swapping every tick: seat 0 first on an even tick, seat 1 on an odd one.
/// The fighter who goes first strikes first on a tick both strike, and the one who goes second sees the other's move
/// before it thinks; swapping gives each seat each edge half the time, so neither seat wins more for being it.
fn turns(mut state: ResMut<State>, mut seats: NonSendMut<Seats>) {
    let state = state.as_mut();
    let first = (state.ticks % 2) as usize;
    for me in [first, 1 - first] {
        turn(state, &mut seats, me);
    }
}

/// Fighter `me`'s turn: its seat hears what happened, thinks, and its fighter does what it chose and moves; then the
/// damage system's triggers answer what happened.
fn turn(state: &mut State, seats: &mut Seats, me: usize) {
    tell_seats(state, seats);
    if let Some(choice) = seats.0[me].think(state, me, TICK) {
        apply(state, me, &choice);
    }
    body(state, me, TICK);
    run_queue(state);
}

/// Two living bodies never overlap: their hurtboxes are kept `collide::body_gap` apart.
fn keep_apart(mut state: ResMut<State>) {
    let state = state.as_mut();
    let alive = state.fighters[0].alive() && state.fighters[1].alive();
    if alive {
        let (dir, overlap) = crate::collide::bodies_overlap(state);
        let arena = crate::body::Arena {
            left: state.left,
            right: state.right,
            top: state.top,
        };
        let [a, b] = &mut state.fighters;
        crate::body::separate(&mut a.body, &mut b.body, &arena, overlap, dir);
    }
}

/// The shots in flight at the start of the tick run, but not one another shot ended this tick.
fn shots(mut state: ResMut<State>) {
    let state = state.as_mut();
    let ids: Vec<u32> = state.shots.iter().map(|s| s.id).collect();
    for id in ids {
        shot(state, id, TICK);
    }
    run_queue(state);
}

/// A fighter whose last blow is over (no shot of its own in flight, no repeat due) may cast a skill that binds again.
fn hold(mut state: ResMut<State>) {
    let state = state.as_mut();
    for f in 0..2 {
        let bound = state.shots.iter().any(|s| s.binds && s.owner == f)
            || state
                .pending
                .iter()
                .any(|p| p.payload.by == f && p.payload.binds());
        state.fighters[f].bound = bound;
    }
}

/// The blows due by now land, in the order they were made.
fn pending(mut state: ResMut<State>) {
    let state = state.as_mut();
    let now = state.time + 1e-9;
    let (due, wait): (Vec<_>, Vec<_>) = core::mem::take(&mut state.pending)
        .into_iter()
        .partition(|p| p.at <= now);
    state.pending = wait;
    for d in due {
        crate::act::due(state, d);
    }
    run_queue(state);
}

/// The traps, mines, totems and minions act.
fn units(mut state: ResMut<State>) {
    let state = state.as_mut();
    unit::act(state, TICK);
    run_queue(state);
}

/// Time wears the fighters (curses, damage over time, regen, timers) and the units.
fn wear(mut state: ResMut<State>) {
    let state = state.as_mut();
    crate::strike::wear(state, TICK);
    unit::wear(state, TICK);
    run_queue(state);
}

/// A life at 0 ends the round unless a keystone gives a spare one; so does a seat that gives it up (a bot module's
/// fault).
fn knock_outs(mut state: ResMut<State>, seats: NonSend<Seats>) {
    let state = state.as_mut();
    for f in 0..2 {
        if !state.fighters[f].alive() && crate::strike::spare(state, f) {
            continue;
        }
        let out = !state.fighters[f].alive() || seats.0[f].forfeit();
        if out && state.winner.is_none() {
            knock_out(state, f);
        }
    }
}

/// One step of fighter `me`: its body's clocks, the rules' own timers and its stored uses, its bar, then its body
/// moves as the rules drive it, and its cast moves on.
fn body(state: &mut State, me: usize, dt: f64) {
    let foe = 1 - me;
    {
        let f = &mut state.fighters[me];
        let alive = f.alive();
        f.body.tick(dt, alive);
        f.sealed = js_max(0.0, f.sealed - dt);
        f.root = js_max(0.0, f.root - dt);
        f.no_guard = js_max(0.0, f.no_guard - dt);
        f.dodged = js_max(0.0, f.dodged - dt);
        f.shield_t = js_max(0.0, f.shield_t - dt);
        if f.shield_t == 0.0 {
            f.charges = 0.0;
        }
        f.rang = js_max(0.0, f.rang - dt);
        // choked (Ngạt), cooldowns run at half speed
        let choke = if f.c.choke > 0.0 {
            AIL.choke_cooldown
        } else {
            1.0
        };
        for k in &mut f.kit.skills {
            k.uses.tick(dt * choke);
        }
    }
    if state.fighters[me].barrier > 0.0 {
        let f = &mut state.fighters[me];
        f.barrier = js_max(0.0, f.barrier - dt);
        if f.barrier == 0.0 {
            drop_barrier(state, me);
        }
    }
    {
        let f = &mut state.fighters[me];
        f.guard = js_max(0.0, f.guard - dt);
    }
    for i in 0..state.fighters[me].bar.len() {
        let f = &mut state.fighters[me];
        let cooling = f.bar[i]
            .spell()
            .and_then(|n| f.kit.get(n))
            .is_some_and(|k| !k.uses.ready());
        let redraw = match &mut f.bar[i] {
            Slot::Held { .. } if cooling => None,
            Slot::Held { spell, idle } => {
                *idle += dt;
                (*idle >= STALE).then_some(Some(*spell))
            }
            Slot::Empty { left, .. } => {
                *left = js_max(0.0, *left - dt);
                (*left == 0.0).then_some(None)
            }
        };
        if let Some(gone) = redraw {
            draw(state, me, i, gone);
        }
    }
    let f = &mut state.fighters[me];
    // flying stops the qi's regen (`wear` gives it back) and costs its own
    if f.fly {
        f.c.qi = js_max(0.0, f.c.qi - (FLY.cost + QI_REGEN) * dt);
    }
    // out of qi, or knocked out of the sky: flight ends and it falls
    if f.fly && (f.qi() <= 1.0 || f.body.reeling()) {
        f.fly = false;
    }
    let foe_x = state.fighters[foe].body.x;
    let f = &state.fighters[me];
    let motion = if f.fly {
        Motion::Fly { hover: f.hover }
    } else {
        Motion::Gravity
    };
    // rooted, it cannot walk; Triền and Phế Cước slow it (its run speed through `MoveSpeed`)
    let speed =
        arena_calc::motion::run_speed(1.0, &f.c.bearer_table(&f.sheet, f.moving), f.root > 0.0);
    let drive = Drive {
        alive: f.alive(),
        acting: f.act.is_some(),
        meditating: f.meditating(),
        guarding: f.guard > 0.0,
        intent: f.intent * speed,
        motion,
        foe_x,
    };
    let arena = state.arena();
    let before = state.fighters[me].body.x;
    let State { rng, fighters, .. } = state;
    let events = fighters[me]
        .body
        .step(dt, &drive, &arena, &mut || rng.next());
    for e in events {
        state.emit(e.into());
    }
    let f = &mut state.fighters[me];
    f.moving = f.intent != 0.0 && (f.body.x - before).abs() > 1e-9;
    advance(state, me, dt);
}

/// Fighter `me` does what its seat chose. Meditating, it cannot act while it sits down or gets up; sitting, meditating
/// or waiting keeps it sitting and anything else gets it up, the choice dropped (`data::MEDITATE`). Meditating begins
/// only from free on the ground.
pub fn apply(state: &mut State, me: usize, choice: &Choice) {
    state.fighters[me].intent = 0.0;
    if let Some(m) = state.fighters[me].medit {
        if m.phase == Sit::Sitting && !matches!(choice, Choice::Meditate | Choice::Wait) {
            get_up(state, me);
        }
        return;
    }
    let f = &mut state.fighters[me];
    match choice {
        Choice::Move { dir } => f.intent = *dir,
        // crouch first, then spring; leaving the ground costs qi, paid as it crouches
        Choice::Jump => {
            if f.body.air == 0.0 && f.body.squat == 0.0 && f.c.qi >= JUMP_QI {
                f.c.qi -= JUMP_QI;
                f.body.squat = f.body.spec.jump_squat;
                f.body.leap = f.body.spec.jump_speed;
            }
        }
        Choice::Guard { hold } => f.guard = *hold,
        Choice::Duck { hold } => f.body.duck = *hold,
        // taking off costs a jump's qi, from the ground or in the air (falling from a jump or a landing: RULES 26);
        // without it, it does not take off
        Choice::Fly { height } => {
            let take_off = *height > 0.0 && !f.fly;
            if take_off && f.c.qi < JUMP_QI {
                return;
            }
            f.fly = *height > 0.0;
            f.hover = *height;
            if take_off {
                f.c.qi -= JUMP_QI;
            }
            if f.fly && f.body.air == 0.0 {
                f.body.vy = f.body.spec.jump_speed * 0.6;
            }
        }
        Choice::Meditate => {
            let b = &f.body;
            let free = f.act.is_none()
                && f.alive()
                && !f.fly
                && b.air == 0.0
                && !b.reeling()
                && b.squat == 0.0
                && b.duck == 0.0
                && b.shocked == 0.0;
            if free {
                sit_down(state, me);
            }
        }
        Choice::Cast { skill, target } => {
            if let Some(name) = state.fighters[me].kit.get(skill).map(|k| k.gem.id) {
                begin(state, me, name, *target);
            }
        }
        Choice::Wait => {}
    }
}

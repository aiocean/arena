//! The fight itself: the rules' duel, made for the world the window shows and the corners as seated, stepped by the
//! frame's pace (the feel's hit-stop and slow motion), its commands and events handed on as `Fx` messages and the
//! duel as it stands this frame as `Shown`. A new layout or new corners start the fight over on the same seed.
//!
//! Watching a live match (`live`), the duel is built on the server's seed and arena with two `Replay` seats on the
//! corners' growing logs, and stepped only as far as the frames received reach, `DELAY_TICKS` behind them: a page that
//! joins late or falls far behind steps there at once without drawing, one a little behind runs faster to catch up.

use crate::bots::Seating;
use crate::layout::Layout;
use crate::live::Live;
use arena_duel::data::UNIT;
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::seat::{MakeSeat, Replay, Seat};
use bevy::prelude::*;
use std::rc::Rc;

/// How far behind the frames received a live page plays: room for the network to wobble without the fight stalling.
pub const DELAY_TICKS: u32 = 18;
/// Further behind than this, a live page runs at double pace.
const CATCH_UP_TICKS: u32 = 60;
/// Further behind than this (a page that joins mid-match, or a tab that slept), it steps there at once, undrawn.
const JUMP_TICKS: u32 = 300;
use arena_duel::out::Out;
use arena_duel::view::{DuelView, view};

/// A command or event of this frame's step, in the rules' pixels (`Layout::world` brings a length to the world's).
#[derive(Message, Clone, Debug)]
pub struct Fx(pub Out);

/// How far the fight runs this frame, in seconds: the frame's time, or less in a hit-stop or a knockout's slow
/// motion (the feel sets it before the rules step).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Pace {
    /// The frame's own time.
    pub dt: f64,
    /// The fight's.
    pub run: f64,
}

/// The duel as it stands after this frame's step, for everything that draws it.
#[derive(Resource, Default, Clone)]
pub struct Shown {
    pub view: Option<DuelView>,
    pub seed: u32,
    pub names: [String; 2],
    /// Bumped each time a new fight starts.
    pub fight: u32,
}

/// The duel (it holds its seats, which hold bot instances: it stays on the main thread).
#[derive(Default)]
pub struct Fight {
    pub duel: Option<Duel>,
    /// The layout and seating it was made for.
    made_for: (u32, u32),
}

pub struct RulesPlugin;

impl Plugin for RulesPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(Fight::default())
            .init_resource::<Pace>()
            .init_resource::<Shown>()
            .add_message::<Fx>()
            .add_systems(
                Update,
                (begin_fight, step).chain().in_set(crate::Frame::Rules),
            );
    }
}

/// The fight's config in the rules' pixels for `layout`.
pub fn config(layout: &Layout, seed: u32) -> DuelConfig {
    let unit = UNIT;
    DuelConfig {
        seed,
        left: layout.left as f64 * unit,
        right: layout.right as f64 * unit,
        ground: layout.ground as f64 * unit,
        sky: layout.sky as f64 * unit,
        effects: arena_duel::effects::all(),
    }
}

fn begin_fight(
    layout: Res<Layout>,
    seating: NonSend<Seating>,
    live: NonSend<Live>,
    mut fight: NonSendMut<Fight>,
    mut shown: ResMut<Shown>,
) {
    if live.on {
        let Some(m) = live.watching.current() else {
            return;
        };
        // a live fight is the server's: its arena, not the window's, and it starts over only on a new match
        let wanted = (u32::MAX, m.id);
        if fight.made_for == wanted && fight.duel.is_some() {
            return;
        }
        let replay = |c: usize| -> MakeSeat {
            let log = Rc::clone(&m.logs[c]);
            Box::new(move |_s, _me| Box::new(Replay::live(log)) as Box<dyn Seat>)
        };
        let config = DuelConfig {
            seed: m.seed,
            left: m.arena.left,
            right: m.arena.right,
            ground: m.arena.ground,
            sky: m.arena.sky,
            effects: arena_duel::effects::all(),
        };
        // the server draws the looks from the seed (`Duel::new_drawn`): built any other way, the fight is another
        fight.duel = Some(Duel::new_drawn(config, [replay(0), replay(1)]));
        fight.made_for = wanted;
        shown.seed = m.seed;
        shown.names = m.names.clone();
        shown.fight += 1;
        return;
    }
    let wanted = (layout.version, seating.version);
    if fight.made_for == wanted && fight.duel.is_some() {
        return;
    }
    let Some(seated) = &seating.seated else {
        return;
    };
    // a fighter's seat keeps its bot instance: the corners start the new fight with a new round
    let seats = [seated[0].make(), seated[1].make()];
    fight.duel = Some(Duel::new(config(&layout, seating.seed), seats));
    fight.made_for = wanted;
    shown.seed = seating.seed;
    shown.names = seating.names.clone();
    shown.fight += 1;
}

/// Moves a rules' event's rows by `dy` rules' pixels. The game stands its fighters on its layout's ground
/// (`fighters::show`: the layout's ground less a fighter's height), so an event's row, measured down from the top of
/// the rules' arena, is moved by the layout's ground less the rules' (in the rules' pixels): an effect, a number or a
/// spark then sits where its fighter stands. On the page's own fights the two grounds are one and nothing moves; a
/// live match is fought on the server's arena, whose ground a window of another size does not share.
pub fn lift(out: &mut Out, dy: f64) {
    match out {
        Out::Play { y, .. }
        | Out::Move { y, .. }
        | Out::Hit { y, .. }
        | Out::Tick { y, .. }
        | Out::Block { y, .. }
        | Out::Clash { y, .. }
        | Out::Dodge { y, .. }
        | Out::Heal { y, .. }
        | Out::Ward { y, .. }
        | Out::Barrier { y, .. }
        | Out::Ko { y, .. }
        | Out::Bump { y, .. }
        | Out::Unyielding { y, .. } => *y += dy,
        Out::Streak { y0, y1, .. } => {
            *y0 += dy;
            *y1 += dy;
        }
        _ => {}
    }
}

fn step(
    pace: Res<Pace>,
    layout: Res<Layout>,
    live: NonSend<Live>,
    mut fight: NonSendMut<Fight>,
    mut shown: ResMut<Shown>,
    mut fx: MessageWriter<Fx>,
) {
    let Some(duel) = fight.duel.as_mut() else {
        return;
    };
    let run = if live.on {
        let Some(m) = live.watching.current() else {
            return;
        };
        let state = duel.state();
        let done = (state.time / TICK).round() as u32;
        let target = m.through.saturating_sub(DELAY_TICKS);
        let behind = target.saturating_sub(done);
        if behind > JUMP_TICKS {
            // joined late, or slept: step to the present undrawn. What starts in the jump is dropped, but an effect
            // already playing is still told to stop, or it would play on for ever
            for _ in 0..behind {
                for out in duel.step(TICK) {
                    if matches!(out, Out::Stop { .. }) {
                        fx.write(Fx(out));
                    }
                }
            }
            0.0
        } else {
            let pace_run = if behind > CATCH_UP_TICKS {
                pace.dt * 2.0
            } else {
                pace.run
            };
            // never past what has come: the last tick allowed lands exactly
            let room = (behind as f64 * TICK - state.carry).max(0.0);
            pace_run.min(room)
        }
    } else {
        pace.run
    };
    let dy = f64::from(layout.ground) * UNIT - duel.state().ground;
    for mut out in duel.step(run) {
        lift(&mut out, dy);
        fx.write(Fx(out));
    }
    shown.view = Some(view(duel.state()));
}

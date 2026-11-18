//! The fight itself: the rules' duel, made for the world the window shows and the corners as seated, stepped by the
//! frame's pace (the feel's hit-stop and slow motion), its commands and events handed on as `Fx` messages and the
//! duel as it stands this frame as `Shown`. A new layout or new corners start the fight over on the same seed.
//!
//! Watching a live match (`live`), the duel is built on the server's seed and arena with two `Replay` seats on the
//! corners' growing logs, and stepped only as far as the frames received reach, `DELAY_TICKS` behind them: a page that
//! joins late or falls far behind steps there without drawing, as many ticks a frame as `CATCH_UP_MS` allows (a whole
//! match in one frame would hold the page for a tenth of a second), one a little behind runs faster to catch up.

use crate::bots::Seating;
use crate::layout::Layout;
use crate::live::Live;
use arena_duel::data::UNIT;
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::seat::{MakeSeat, Replay, Seat};
use bevy::platform::time::Instant;
use bevy::prelude::*;
use std::rc::Rc;

/// How far behind the frames received a live page plays: room for the network to wobble without the fight stalling.
pub const DELAY_TICKS: u32 = 18;
/// Further behind than this, a live page runs at double pace.
const CATCH_UP_TICKS: u32 = 60;
/// Further behind than this (a page that joins mid-match, or a tab that slept), it steps there undrawn.
const JUMP_TICKS: u32 = 300;
/// The time a frame may spend on that undrawn stepping, in ms of the main thread: the rest waits for the next frame.
const CATCH_UP_MS: f64 = 4.0;
/// The ticks a frame steps undrawn whatever the time, so the page outruns the match (a tick a frame at 60 Hz) on any
/// machine.
const CATCH_UP_TICKS_MIN: u32 = 8;
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
    /// How far the rules were stepped this frame (`step`): the fight's time, or less while a live match's frames
    /// have not come (none while they stall). What plays to the rules' clock (the drawn effects) ages by this.
    pub ruled: f64,
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
    /// A live page stepping undrawn to the frames received, over as many frames as it takes (the fighters are shown
    /// where each frame leaves them, their effects are not): it plays at its pace again only once it is there.
    jumping: bool,
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
        fight.jumping = false;
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
    fight.jumping = false;
    shown.seed = seating.seed;
    shown.names = seating.names.clone();
    shown.fight += 1;
}

/// Moves an effect's row by `dy` rules' pixels. The rules play an effect at a screen row, measured down from the top of
/// the rules' arena (`out`), and the game stands its fighters on its layout's ground (`fighters::show`), so an effect
/// is moved by the layout's ground less the rules' (in the rules' pixels) to sit where its fighter stands. A fighter's
/// event (a hit, a tick, a heal, a block, a clash, a streak...) gives a height up from the ground, which the feel
/// draws up from the layout's ground (`feel::draw`): it is left as it is. On the page's own fights the two grounds are
/// one and nothing moves; a live match is fought on the server's arena, whose ground a window of another size does not
/// share.
pub fn lift(out: &mut Out, dy: f64) {
    match out {
        Out::Play { y, .. } | Out::Move { y, .. } => *y += dy,
        _ => {}
    }
}

fn step(
    mut pace: ResMut<Pace>,
    layout: Res<Layout>,
    live: NonSend<Live>,
    mut fight: NonSendMut<Fight>,
    mut shown: ResMut<Shown>,
    mut fx: MessageWriter<Fx>,
) {
    pace.ruled = 0.0;
    let fight = fight.as_mut();
    let Some(duel) = fight.duel.as_mut() else {
        return;
    };
    let run = if live.on {
        let Some(m) = live.watching.current() else {
            return;
        };
        let done = (duel.state().time / TICK).round() as u32;
        let target = m.through.saturating_sub(DELAY_TICKS);
        let behind = target.saturating_sub(done);
        fight.jumping |= behind > JUMP_TICKS;
        if fight.jumping {
            // joined late, or slept: step to the present undrawn, a frame's share at a time
            let start = Instant::now();
            let stepped = leap(
                duel,
                behind,
                |n| n < CATCH_UP_TICKS_MIN || start.elapsed().as_secs_f64() * 1000.0 < CATCH_UP_MS,
                |out| {
                    fx.write(Fx(out));
                },
            );
            fight.jumping = stepped < behind;
            0.0
        } else {
            let pace_run = if behind > CATCH_UP_TICKS {
                pace.dt * 2.0
            } else {
                pace.run
            };
            // never past what has come: the last tick allowed lands exactly
            let room = (behind as f64 * TICK - duel.state().carry).max(0.0);
            pace_run.min(room)
        }
    } else {
        pace.run
    };
    pace.ruled = run;
    let dy = f64::from(layout.ground) * UNIT - duel.state().ground;
    for mut out in duel.step(run) {
        lift(&mut out, dy);
        fx.write(Fx(out));
    }
    shown.view = Some(view(duel.state()));
}

/// Steps `duel` up to `behind` ticks undrawn, while `more(ticks stepped so far)` allows: how many it stepped. What
/// starts in the leap is dropped, but an effect already playing is still told to stop (`stop`), or it would play on
/// for ever.
fn leap(
    duel: &mut Duel,
    behind: u32,
    mut more: impl FnMut(u32) -> bool,
    mut stop: impl FnMut(Out),
) -> u32 {
    let mut stepped = 0;
    while stepped < behind && more(stepped) {
        for out in duel.step(TICK) {
            if matches!(out, Out::Stop { .. }) {
                stop(out);
            }
        }
        stepped += 1;
    }
    stepped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feel::fight::FightFeel;
    use arena_duel::out::{How, PlayOpts};

    /// A leap spread over frames, a few ticks each, reaches the duel a leap in one frame does, and it is told every
    /// stop the ticks make.
    #[test]
    fn a_leap_spread_over_frames_lands_where_one_leap_does() {
        let config = DuelConfig {
            seed: 7,
            left: 0.0,
            right: 880.0,
            ground: 600.0,
            sky: 384.0,
            effects: arena_duel::effects::all(),
        };
        let behind = 1500;
        let mut whole = Duel::with_brains(config.clone());
        let mut stops = Vec::new();
        assert_eq!(
            leap(
                &mut whole,
                behind,
                |_| true,
                |o| stops.push(format!("{o:?}"))
            ),
            behind
        );
        let mut spread = Duel::with_brains(config);
        let mut spread_stops = Vec::new();
        let (mut done, mut frames) = (0, 0);
        while done < behind {
            done += leap(
                &mut spread,
                behind - done,
                |n| n < 37,
                |o| spread_stops.push(format!("{o:?}")),
            );
            frames += 1;
        }
        assert_eq!(frames, behind.div_ceil(37));
        assert_eq!(view(spread.state()), view(whole.state()));
        assert_eq!(spread_stops, stops);
    }

    /// On a live match (the server's ground) in windows of every size and shape, a hit's number stands over the
    /// struck fighter, and an effect played at its chest is drawn at its chest: the window's ground is not the
    /// server's, and only the effect's screen row is moved for it.
    #[test]
    fn a_hits_number_and_effect_stand_on_the_struck_fighter_in_any_window() {
        let server_ground = 632.0;
        let (chest, body) = (152.0, 228.0);
        for (w, h, dpr) in [
            (1280.0, 800.0, 1.0),
            (1920.0, 1080.0, 1.0),
            (1366.0, 768.0, 1.0),
            (390.0, 293.0, 1.0),
            (1170.0, 879.0, 3.0),
            (2560.0, 1440.0, 2.0),
            (800.0, 1200.0, 1.0),
        ] {
            let layout = Layout::of(w, h, dpr, 0);
            let dy = f64::from(layout.ground) * UNIT - server_ground;
            let mut hit = Out::Hit {
                by: 0,
                spell: "x",
                power: 1.0,
                launch: false,
                ranged: false,
                damage: 12.0,
                crit: false,
                how: How::Counter,
                share: 10.0,
                x: 400.0,
                y: chest,
            };
            lift(&mut hit, dy);
            let mut feel = FightFeel::default();
            feel.react(&hit);
            let world = |v: f64| f64::from(Layout::world(v));
            let (feet, top) = (
                f64::from(layout.ground),
                f64::from(layout.ground) - world(body),
            );
            for n in feel.numbers() {
                let row = f64::from(layout.ground) - n.y;
                assert!(
                    row <= feet && row >= top - 40.0,
                    "{w}x{h}: {:?} at row {row}, the fighter from {top} to {feet}",
                    n.text
                );
                assert!(
                    (n.x - world(400.0)).abs() <= 12.0,
                    "{w}x{h}: {:?} at x {}",
                    n.text,
                    n.x
                );
            }
            let mut play = Out::Play {
                key: 1,
                fx: "x",
                x: 400.0,
                y: server_ground - chest,
                opts: PlayOpts::default(),
            };
            lift(&mut play, dy);
            let Out::Play { y, .. } = play else {
                unreachable!()
            };
            assert!(
                (world(y) - (f64::from(layout.ground) - world(chest))).abs() < 1e-6,
                "{w}x{h}: the effect at row {}, the chest at {}",
                world(y),
                f64::from(layout.ground) - world(chest)
            );
        }
    }
}

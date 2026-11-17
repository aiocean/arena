//! The debug view, off until asked for (the backquote key or F3, `?debug` in the page's address, or `debug()` from the
//! page): a panel of numbers and the shapes the rules test hits against, drawn over the fight.
//!
//! - **The panel**: the frame rate and the frame's time; the fight (seed, round, score, the rules' clock, the pace a
//!   hit-stop leaves); each fighter (life, qi, shield, place, speed, pose, the cast under way and its phase); watching
//!   live, the link (frames and bytes in, the decisions and keyframes, the server-to-page delay, how far the page trails
//!   what has come and whether it is catching up); and the last blows, blocks and dodges.
//! - **The shapes** (`arena_duel::debug::shapes`, the very boxes the hit tests use): each body (gold for Thunder,
//!   cyan for Frost, grey when nothing can touch it), the box a melee or blast cast strikes, red, from the tick it
//!   struck on (its only active frame) for a moment after, shots (magenta), fields (violet), units (white), the arena's
//!   walls, ground and sky, and a red cross where a blow landed.

use crate::layout::Layout;
use crate::live::Live;
use crate::rules::{DELAY_TICKS, Fight, Fx, LivePace, Pace, Shown};
use arena_duel::data::UNIT;
use arena_duel::debug::{Mark, Shape, shapes};
use arena_duel::duel::TICK;
use arena_duel::out::Out;
use bevy::prelude::*;
use bevy::text::FontSize;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Write as _;

/// Above everything the world draws (the HUD is at 30 to 32).
const Z_DEBUG: f32 = 60.0;
/// How many past events the panel lists, and how long a landed blow's cross stays.
const EVENTS: usize = 8;
const CROSS_SECONDS: f32 = 0.6;

thread_local! {
    /// The page's ask: `Some(on)`, or `None` to flip.
    static ASK: RefCell<Option<Option<bool>>> = const { RefCell::new(None) };
}

/// Turn the debug view on or off (`Some`), or flip it (`None`), from outside the schedule (the page).
pub fn ask(on: Option<bool>) {
    ASK.with_borrow_mut(|a| *a = Some(on));
}

/// What the debug view keeps between frames.
#[derive(Resource, Default)]
struct View {
    on: bool,
    fps: f32,
    frame_ms: f32,
    frame_ms_max: f32,
    /// The last events, newest last, as the panel writes them.
    events: VecDeque<String>,
    /// Where blows landed (the rules' x and height above the ground) and how long ago.
    crosses: Vec<(f64, f64, f32)>,
    hits: u64,
    damage: f64,
}

#[derive(Component)]
struct Panel;

/// A line of the shapes: one sprite each, reused frame to frame.
#[derive(Component)]
struct Line;

#[derive(Resource, Default)]
struct Lines(Vec<Entity>);

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<View>()
            .init_resource::<Lines>()
            .add_systems(Startup, spawn_panel)
            .add_systems(
                Update,
                (toggle, measure, events, panel, draw)
                    .chain()
                    .in_set(crate::Frame::React),
            );
    }
}

fn spawn_panel(mut commands: Commands) {
    commands.spawn((
        Panel,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(8.0),
            padding: UiRect::all(Val::Px(6.0)),
            max_width: Val::Px(560.0),
            ..default()
        },
        BackgroundColor(Color::srgba_u8(6, 4, 14, 210)),
        GlobalZIndex(200),
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(Color::srgb_u8(0xd8, 0xf0, 0xd0)),
        Visibility::Hidden,
    ));
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<View>) {
    if keys.just_pressed(KeyCode::Backquote) || keys.just_pressed(KeyCode::F3) {
        view.on = !view.on;
    }
    if let Some(on) = ASK.with_borrow_mut(Option::take) {
        view.on = on.unwrap_or(!view.on);
    }
}

fn measure(time: Res<Time>, mut view: ResMut<View>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let ms = dt * 1000.0;
    view.frame_ms = if view.frame_ms == 0.0 { ms } else { view.frame_ms * 0.9 + ms * 0.1 };
    view.fps = 1000.0 / view.frame_ms.max(0.001);
    // the worst frame, forgotten slowly
    view.frame_ms_max = (view.frame_ms_max * 0.995).max(ms);
    for c in &mut view.crosses {
        c.2 += dt;
    }
    view.crosses.retain(|c| c.2 < CROSS_SECONDS);
}

fn who(names: &[String; 2], by: usize) -> String {
    let side = if by == 0 { "Thunder" } else { "Frost" };
    match names.get(by) {
        Some(n) if !n.is_empty() => format!("{side} ({n})"),
        _ => side.to_string(),
    }
}

fn events(mut fx: MessageReader<Fx>, shown: Res<Shown>, layout: Res<Layout>, mut view: ResMut<View>) {
    let t = shown.view.as_ref().map_or(0.0, |v| v.time);
    // a blow's y is a row down from the top, moved onto the layout's ground (`rules::lift`): its height above the
    // ground is what the shapes use
    let ground = f64::from(layout.ground) * UNIT;
    for Fx(out) in fx.read() {
        let line = match out {
            Out::Hit {
                by,
                spell,
                damage,
                crit,
                x,
                y,
                ..
            } => {
                view.hits += 1;
                view.damage += *damage;
                view.crosses.push((*x, ground - *y, 0.0));
                format!(
                    "{t:7.2}s {} {spell} hit {:.1}{}",
                    who(&shown.names, *by),
                    damage,
                    if *crit { " crit" } else { "" }
                )
            }
            Out::Block { by, .. } => format!("{t:7.2}s {} blocked", who(&shown.names, *by)),
            Out::Dodge { .. } => format!("{t:7.2}s dodge"),
            Out::Clash { .. } => format!("{t:7.2}s clash"),
            Out::Ko { .. } => format!("{t:7.2}s KO"),
            _ => continue,
        };
        view.events.push_back(line);
        while view.events.len() > EVENTS {
            view.events.pop_front();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn panel(
    view: Res<View>,
    shown: Res<Shown>,
    pace: Res<Pace>,
    lp: Res<LivePace>,
    live: NonSend<Live>,
    fight: NonSend<Fight>,
    mut q: Query<(&mut Text, &mut Visibility), With<Panel>>,
) {
    let Ok((mut text, mut vis)) = q.single_mut() else {
        return;
    };
    *vis = if view.on {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !view.on {
        return;
    }
    let mut s = String::new();
    let _ = writeln!(
        s,
        "DEBUG (` or F3)   {:5.1} fps   frame {:5.1} ms (worst {:5.1})",
        view.fps, view.frame_ms, view.frame_ms_max
    );
    let pace_note = if pace.dt > 0.0 && pace.run < pace.dt * 0.99 {
        if pace.run <= 0.0 { "  hit-stop" } else { "  slow" }
    } else if pace.run > pace.dt * 1.01 {
        "  fast"
    } else {
        ""
    };
    if let Some(v) = &shown.view {
        let _ = writeln!(
            s,
            "fight {} | seed {} | round {} | {}-{} | rules clock {:.2}s (tick {}) | pace {:.3}/{:.3}s{pace_note}",
            shown.fight,
            shown.seed,
            v.round,
            v.wins[0],
            v.wins[1],
            v.time,
            (v.time / TICK).round() as u64,
            pace.run,
            pace.dt
        );
        if let Some(duel) = &fight.duel {
            let st = duel.state();
            let _ = writeln!(
                s,
                "arena x {:.0}..{:.0}  ground {:.0}  sky {:.0} (rules px; world = /{UNIT})",
                st.left, st.right, st.ground, st.top
            );
            for (i, f) in v.fighters.iter().enumerate() {
                let body = &st.fighters[i].body;
                let act = match &f.act {
                    Some(a) => format!("{} {:?} {:.2}s", a.spell, a.phase, a.t),
                    None => "-".into(),
                };
                let _ = writeln!(
                    s,
                    "{:<26} hp {:5.1}% qi {:5.1}% sh {:5.1}%  x {:6.1} air {:5.1}  v {:6.1},{:6.1}  {:?}{}  act {act}",
                    who(&shown.names, i),
                    f.hp,
                    f.qi,
                    f.shield,
                    f.x,
                    f.air,
                    f.vx,
                    f.vy,
                    f.pose,
                    if f.untouchable { " untouchable" } else { "" },
                );
                let _ = writeln!(
                    s,
                    "{:<26} body {:.0}w x {:.0}h (duck {:.0})  chest {:.0}",
                    "",
                    body.spec.half_width * 2.0,
                    body.spec.body,
                    body.spec.duck,
                    body.spec.chest
                );
            }
        }
    }
    let _ = writeln!(s, "blows landed {}  damage dealt {:.0}", view.hits, view.damage);

    if live.on {
        let n = crate::live::net();
        let _ = writeln!(s, "LIVE | {}", live.status);
        if let Some(m) = live.watching.current() {
            let held = [m.logs[0].borrow().decisions.len(), m.logs[1].borrow().decisions.len()];
            let _ = writeln!(
                s,
                "match #{} | through tick {} | decisions held {}/{}{}",
                m.id,
                m.through,
                held[0],
                held[1],
                match m.ended {
                    Some(e) => format!(" | ended {e:?}"),
                    None => String::new(),
                }
            );
        }
        let behind_ms = lp.behind as f64 * TICK * 1000.0;
        let _ = writeln!(
            s,
            "play tick {} / target {} (through - {DELAY_TICKS}) | behind {} ticks ({behind_ms:.0} ms){} | jumps {} | waits {}",
            lp.done,
            lp.target,
            lp.behind,
            if lp.catching_up { " CATCHING UP" } else { "" },
            lp.jumps,
            lp.starved
        );
        let age = if n.last_ms > 0.0 {
            format!("{:.0} ms ago", now_ms() - n.last_ms)
        } else {
            "never".into()
        };
        let _ = writeln!(
            s,
            "link: sessions {} | matches {} | frames {} ({} keyframes, {} decisions, {} marks, {} bad) | {:.1} KB | last {age}",
            n.sessions,
            n.matches,
            n.frames,
            n.keyframes,
            n.decisions,
            n.throughs,
            n.bad,
            n.bytes as f64 / 1024.0
        );
        let _ = writeln!(
            s,
            "delay server->page {:.0} ms (avg {:.0}, worst {:.0}; two clocks) | last keyframe {} B",
            n.delay_ms, n.delay_avg_ms, n.delay_max_ms, n.keyframe_bytes
        );
    } else {
        let _ = writeln!(s, "LOCAL | the page fights on its own");
    }
    for e in &view.events {
        let _ = writeln!(s, "{e}");
    }
    let _ = write!(
        s,
        "body: gold Thunder / cyan Frost / grey untouchable | strike red (active frame) | shot magenta | field violet | unit white | red X a blow"
    );
    text.0 = s;
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// A segment in world pixels (y down) and its colour.
struct Seg(Vec2, Vec2, Color);

fn color(mark: Mark, owner: Option<usize>) -> Color {
    match mark {
        Mark::Body => {
            if owner == Some(1) {
                Color::srgba_u8(0x6a, 0xd8, 0xff, 230)
            } else {
                Color::srgba_u8(0xff, 0xd2, 0x4a, 230)
            }
        }
        Mark::Untouchable => Color::srgba_u8(0x90, 0x90, 0x90, 200),
        Mark::Strike => Color::srgba_u8(0xff, 0x30, 0x30, 255),
        Mark::Shot => Color::srgba_u8(0xff, 0x4a, 0xe0, 240),
        Mark::Field => Color::srgba_u8(0xb0, 0x7a, 0xff, 220),
        Mark::Unit => Color::srgba_u8(0xff, 0xff, 0xff, 220),
    }
}

/// Every segment to draw this frame.
fn segments(fight: &Fight, layout: &Layout, view: &View) -> Vec<Seg> {
    let mut out = Vec::new();
    let Some(duel) = &fight.duel else {
        return out;
    };
    let st = duel.state();
    // the rules' (x, air) to world pixels, y down: as the fighters are placed (`fighters::draw`)
    let at = |x: f64, air: f64| Vec2::new(Layout::world(x), layout.ground - Layout::world(air));
    let rect = |out: &mut Vec<Seg>, x0: f64, y0: f64, x1: f64, y1: f64, c: Color| {
        let (a, b, cc, d) = (at(x0, y0), at(x1, y0), at(x1, y1), at(x0, y1));
        out.push(Seg(a, b, c));
        out.push(Seg(b, cc, c));
        out.push(Seg(cc, d, c));
        out.push(Seg(d, a, c));
    };

    // the arena: walls, ground, the highest a fighter's feet may fly
    let dim = Color::srgba_u8(0xff, 0xff, 0xff, 70);
    let sky = st.ground - st.top;
    rect(&mut out, st.left, 0.0, st.right, sky, dim);

    for d in shapes(st) {
        let c = color(d.mark, d.owner);
        match d.shape {
            Shape::Rect { x0, y0, x1, y1 } => rect(&mut out, x0, y0, x1, y1, c),
            Shape::Circle { x, y, r } => {
                let n = 16;
                let p = |i: usize| {
                    let a = i as f64 / n as f64 * std::f64::consts::TAU;
                    at(x + r * a.cos(), y + r * a.sin())
                };
                for i in 0..n {
                    out.push(Seg(p(i), p(i + 1), c));
                }
            }
        }
    }
    // each fighter's feet: a short tick at its middle
    for f in &st.fighters {
        let c = Color::srgba_u8(0xff, 0xff, 0xff, 200);
        out.push(Seg(at(f.body.x, f.body.air), at(f.body.x, f.body.air + 8.0), c));
    }
    // where blows landed
    for (x, air, age) in &view.crosses {
        let p = at(*x, *air);
        let a = 1.0 - age / CROSS_SECONDS;
        let c = Color::srgba(1.0, 0.15, 0.15, a);
        let k = 3.0;
        out.push(Seg(p + Vec2::new(-k, -k), p + Vec2::new(k, k), c));
        out.push(Seg(p + Vec2::new(-k, k), p + Vec2::new(k, -k), c));
    }
    out
}

fn draw(
    mut commands: Commands,
    view: Res<View>,
    fight: NonSend<Fight>,
    layout: Res<Layout>,
    mut lines: ResMut<Lines>,
    mut q: Query<(&mut Sprite, &mut Transform, &mut Visibility), With<Line>>,
) {
    let segs = if view.on {
        segments(&fight, &layout, &view)
    } else {
        Vec::new()
    };
    // one physical pixel and a half, in world pixels
    let thick = (1.5 / layout.scale.max(1.0)).max(0.2);
    while lines.0.len() < segs.len() {
        let e = commands
            .spawn((
                Line,
                Sprite::from_color(Color::WHITE, Vec2::ONE),
                Transform::default(),
                Visibility::Hidden,
            ))
            .id();
        lines.0.push(e);
    }
    for (i, e) in lines.0.iter().enumerate() {
        let Ok((mut sprite, mut tf, mut vis)) = q.get_mut(*e) else {
            continue;
        };
        let Some(Seg(a, b, c)) = segs.get(i) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let (pa, pb) = (layout.at(a.x, a.y), layout.at(b.x, b.y));
        let d = pb - pa;
        sprite.color = *c;
        sprite.custom_size = Some(Vec2::new(d.length().max(thick), thick));
        tf.translation = ((pa + pb) / 2.0).extend(Z_DEBUG);
        tf.rotation = Quat::from_rotation_z(d.y.atan2(d.x));
        *vis = Visibility::Visible;
    }
}

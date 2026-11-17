//! The spell effects drawn by an image model (`arena_vfx::drawn`): smooth sheets of the shapes, moved by code. A
//! playing is an entity at the rules' point with one sprite over the fighters, turned, scaled and carried as its
//! motion says, and a few fainter copies behind them for its tail, laid where it was a moment ago. When it has played
//! out, or the rules stop a loop, it leaves by its exit (`disappear/fade`: it swells and fades out). The sheets are
//! sampled smoothly: the pixels are the renderer's business, not the art's.
//!
//! An effect with a drawn sheet plays from here; the rest play from their baked recipes (`sprites`) or live.

use super::Played;
use crate::layers::Z_FIGHTERS;
use crate::layout::Layout;
use crate::rules::{Fx, Pace, Shown};
use arena_duel::out::Out;
use arena_vfx::drawn::{self, Drawn, FADE_GROW, FADE_SECS, Shot};
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use std::collections::{BTreeMap, VecDeque};
use std::sync::OnceLock;

/// Every drawn effect's layout, written beside the sheets.
const DRAWN: &str = include_str!("../../assets/drawn/drawn.json");
/// Where the sprites sit: the shape just over the fighters, its tail just behind them.
const Z_OVER: f32 = Z_FIGHTERS + 6.0;
const Z_TAIL: f32 = Z_FIGHTERS - 2.0;
/// How many copies make a tail, and how strong the nearest is.
const TAIL: usize = 8;
const TAIL_ALPHA: f32 = 0.55;
/// How long a stopped loop with no exit takes to fade out.
const FADE: f32 = 0.3;
/// The longest a loop plays unstopped, and how long a flying one goes unmoved, before it is taken for forgotten.
const LONGEST: f32 = 10.0;
const UNMOVED: f32 = 1.0;

/// The drawn effects, read once.
fn all() -> &'static [Drawn] {
    static ALL: OnceLock<Vec<Drawn>> = OnceLock::new();
    ALL.get_or_init(|| {
        drawn::read(DRAWN).unwrap_or_else(|e| {
            error!("{e}");
            Vec::new()
        })
    })
}

/// Whether effect `fx` plays from a drawn sheet. An effect that grips the fighter's own sprite (`SPRITE_AWARE`)
/// keeps playing live: a drawing cannot follow the body.
pub fn plays(fx: &str) -> bool {
    !arena_vfx::SPRITE_AWARE.contains(&fx) && all().iter().any(|d| d.fx == fx)
}

/// The sheets' pages and atlases, and each effect's frames' places in them, by school.
#[derive(Resource)]
pub struct Drawings {
    /// Each effect's pages' files, fetched once the fighters' sheets are in (`fetch`); until then `pages` is empty
    /// and an effect shows nothing, as one whose page has not arrived.
    files: Vec<Vec<String>>,
    pages: Vec<Vec<Handle<Image>>>,
    atlases: Vec<Vec<Handle<TextureAtlasLayout>>>,
    /// For each effect, each school's frames: (page, atlas index).
    cells: Vec<BTreeMap<String, Vec<(usize, usize)>>>,
}

/// A playing: which effect, in which school's frames, how long it has played, when the rules stopped it, which way
/// it faces, and where its shape was lately (camera units, newest last) for its tail.
#[derive(Component, Debug)]
pub struct Drawing {
    effect: usize,
    school: &'static str,
    t: f32,
    stopped: Option<f32>,
    /// When the rules last moved it.
    moved: f32,
    flip: bool,
    trail: VecDeque<(f32, Vec2)>,
    /// The round it was played in: a new round stops it.
    round: u32,
}

/// The fight a playing belongs to: swept away when the next begins.
#[derive(Component, Debug, Clone, Copy)]
pub struct DrawnFight(pub u32);

/// The rules' key for a playing.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawingKey(pub u32);

/// A playing's sprite: its shape (`None`) or the `i`th copy of its tail.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part(Option<usize>);

pub struct DrawnPlugin;

impl Plugin for DrawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(
            Update,
            (
                fetch,
                (sweep, commands, animate)
                    .chain()
                    .in_set(Played)
                    .in_set(crate::Frame::React),
            ),
        );
    }
}

fn load(mut commands: Commands, mut layouts: ResMut<Assets<TextureAtlasLayout>>) {
    let mut files = Vec::new();
    let mut atlases = Vec::new();
    let mut cells = Vec::new();
    for d in all() {
        let names: Vec<String> = (0..d.layout.pages.len())
            .map(|i| {
                if i == 0 {
                    format!("drawn/{}.webp", d.file)
                } else {
                    format!("drawn/{}-{i}.webp", d.file)
                }
            })
            .collect();
        let mut atlas: Vec<TextureAtlasLayout> = d
            .layout
            .pages
            .iter()
            .map(|[w, h]| TextureAtlasLayout::new_empty(UVec2::new(*w, *h)))
            .collect();
        let mut by_school = BTreeMap::new();
        for (school, clip) in &d.layout.clips {
            let placed = clip
                .iter()
                .filter_map(|c| {
                    let page = c.page as usize;
                    let a = atlas.get_mut(page)?;
                    Some((
                        page,
                        a.add_texture(URect::new(c.x, c.y, c.x + c.w, c.y + c.h)),
                    ))
                })
                .collect();
            by_school.insert(school.clone(), placed);
        }
        files.push(names);
        atlases.push(atlas.into_iter().map(|a| layouts.add(a)).collect());
        cells.push(by_school);
    }
    commands.insert_resource(Drawings {
        files,
        pages: Vec::new(),
        atlases,
        cells,
    });
}

/// Fetch the pages once the fighters' sheets are in (at once in an app with no fighters), drawn smooth (linear).
fn fetch(
    assets: Res<AssetServer>,
    sheets_in: Option<Res<crate::fighters::SheetsIn>>,
    drawings: Option<ResMut<Drawings>>,
) {
    let Some(mut drawings) = drawings else { return };
    if !drawings.pages.is_empty() || sheets_in.is_some_and(|s| !s.0) {
        return;
    }
    drawings.pages = drawings
        .files
        .iter()
        .map(|names| {
            names
                .iter()
                .map(|name| {
                    assets
                        .load_builder()
                        .with_settings(|s: &mut ImageLoaderSettings| {
                            s.sampler = ImageSampler::linear()
                        })
                        .load(name.clone())
                })
                .collect()
        })
        .collect();
}

/// A playing the rules started this frame, spawned once the frame's commands are read: a move or a stop for it in
/// the same frame (a bolt that lands at once, a page catching up) must reach it, and a spawned entity is only there
/// once the frame's commands are applied.
struct Starting {
    key: u32,
    fx: &'static str,
    drawing: Drawing,
    at: Vec3,
}

/// Where a playing of effect `effect` stands for the rules' point (`x`, `y`): there, or on the floor under it for a
/// seal that lies on the ground.
fn spot(layout: &Layout, effect: usize, x: f64, y: f64) -> Vec3 {
    let grounded = all().get(effect).is_some_and(|d| d.layout.motion.ground);
    let y = if grounded {
        layout.ground
    } else {
        Layout::world(y)
    };
    layout.at(Layout::world(x), y).extend(0.0)
}

/// The rules' commands for the drawn effects: play, move, stop.
fn commands(
    mut commands: Commands,
    mut fx: MessageReader<Fx>,
    layout: Res<Layout>,
    shown: Res<Shown>,
    mut playing: Query<(&DrawingKey, &mut Drawing, &mut Transform)>,
) {
    let mut starting: Vec<Starting> = Vec::new();
    for Fx(out) in fx.read() {
        match out {
            Out::Play {
                key,
                fx,
                x,
                y,
                opts,
            } if plays(fx) => {
                let Some(effect) = all().iter().position(|d| d.fx == *fx) else {
                    continue;
                };
                // a key played again: the old playing goes
                for (k, mut d, _) in &mut playing {
                    if k.0 == *key && d.stopped.is_none() {
                        d.stopped = Some(d.t);
                    }
                }
                starting.retain(|s| s.key != *key);
                let school = drawn::school_of(opts.palette);
                debug!("drawn {fx} at ({x:.0}, {y:.0}) in {school}");
                starting.push(Starting {
                    key: *key,
                    fx,
                    drawing: Drawing {
                        effect,
                        school,
                        t: 0.0,
                        stopped: None,
                        moved: 0.0,
                        flip: opts.flip.unwrap_or(false),
                        trail: VecDeque::new(),
                        round: round_of(&shown),
                    },
                    at: spot(&layout, effect, *x, *y),
                });
            }
            Out::Move { key, x, y } => {
                if let Some(s) = starting.iter_mut().find(|s| s.key == *key) {
                    s.at = spot(&layout, s.drawing.effect, *x, *y);
                }
                for (k, mut d, mut transform) in &mut playing {
                    if k.0 == *key {
                        transform.translation = spot(&layout, d.effect, *x, *y);
                        d.moved = d.t;
                    }
                }
            }
            Out::Stop { key } => {
                if let Some(s) = starting.iter_mut().find(|s| s.key == *key) {
                    s.drawing.stopped = Some(0.0);
                }
                for (k, mut d, _) in &mut playing {
                    if k.0 == *key && d.stopped.is_none() {
                        d.stopped = Some(d.t);
                    }
                }
            }
            _ => {}
        }
    }
    for s in starting {
        let sprite = |part: Part| {
            let z = if part.0.is_some() { Z_TAIL } else { Z_OVER };
            (
                Sprite::default(),
                Transform::from_xyz(0.0, 0.0, z),
                Visibility::Hidden,
                part,
            )
        };
        commands
            .spawn((
                Name::new(format!("drawn {}", s.fx)),
                DrawnFight(shown.fight),
                s.drawing,
                DrawingKey(s.key),
                Transform::from_translation(s.at),
                Visibility::Visible,
            ))
            .with_children(|c| {
                c.spawn(sprite(Part(None)));
                for i in 0..TAIL {
                    c.spawn(sprite(Part(Some(i))));
                }
            });
    }
}

/// The round the rules are in.
fn round_of(shown: &Shown) -> u32 {
    shown.view.as_ref().map_or(0, |v| v.round)
}

/// A new fight sweeps away what the last one played; a new round stops what the last one played, so it plays out
/// its exit instead of lingering over the fighters set back in their corners.
fn sweep(
    mut commands: Commands,
    shown: Res<Shown>,
    mut playing: Query<(Entity, &DrawnFight, &mut Drawing)>,
) {
    let round = round_of(&shown);
    for (e, fight, mut d) in &mut playing {
        if fight.0 != shown.fight {
            commands.entity(e).despawn();
        } else if d.round != round && d.stopped.is_none() {
            d.stopped = Some(d.t);
        }
    }
}

/// When a playing ends of itself, or the rules' stop: a one-shot once played out, a loop when stopped. A loop the
/// rules forgot is ended all the same: one that flies (it has a tail) once it is no longer moved for `UNMOVED`
/// seconds, any loop after `LONGEST`.
fn end_of(fx: &Drawn, d: &Drawing) -> f32 {
    let end = if let Some(life) = fx.life(d.school) {
        life
    } else if fx.layout.motion.tail.is_some() {
        LONGEST.min(d.moved + UNMOVED)
    } else {
        LONGEST
    };
    // a stop cuts it short, never longer: one stopped while it already leaves goes on leaving
    d.stopped.map_or(end, |s| s.min(end))
}

/// Where a shot's cell sits from the effect's origin, and how it is turned and scaled, facing `flip`: (translation,
/// rotation about z, scale), camera units, y up. The cell is drawn from its centre: its centre is carried round the
/// anchor as the shape turns.
fn place(shot: &Shot, k: f32, flip: bool) -> (Vec2, f32, f32) {
    let c = shot.cell;
    let centre = Vec2::new(
        c.dx as f32 + c.w as f32 / 2.0,
        -(c.dy as f32 + c.h as f32 / 2.0),
    ) * k;
    // clockwise on screen is a negative turn with y up
    let turned = Vec2::from_angle(-shot.angle).rotate(centre) + Vec2::new(shot.at.0, -shot.at.1);
    if flip {
        (Vec2::new(-turned.x, turned.y), shot.angle, k)
    } else {
        (turned, -shot.angle, k)
    }
}

/// Where the trail was `ago` seconds before `now`, between the samples either side of it.
fn back_in_time(trail: &VecDeque<(f32, Vec2)>, when: f32) -> Option<Vec2> {
    let i = trail.iter().rposition(|(t, _)| *t <= when)?;
    let (t0, p0) = trail[i];
    match trail.get(i + 1) {
        Some(&(t1, p1)) if t1 > t0 => Some(p0.lerp(p1, (when - t0) / (t1 - t0))),
        _ => Some(p0),
    }
}

/// Each playing moves on by the fight's time: its shape shows the frame, turn, size and place its motion gives, its
/// tail the shape where it was, its exit once it is done; one gone is despawned.
fn animate(
    mut commands: Commands,
    pace: Res<Pace>,
    drawings: Option<Res<Drawings>>,
    mut playing: Query<(Entity, &mut Drawing, &Transform, &Children)>,
    mut sprites: Query<(&Part, &mut Sprite, &mut Transform, &mut Visibility), Without<Drawing>>,
) {
    let dt = pace.run as f32;
    for (e, mut d, origin, children) in &mut playing {
        d.t += dt;
        let Some(fx) = all().get(d.effect) else {
            commands.entity(e).despawn();
            continue;
        };
        let t = d.t;
        let end = end_of(fx, &d);
        // its shape now, how much it is still seen and how far it has swelled as it goes
        let (shot, seen, swell) = if t < end {
            (fx.shot(d.school, t), 1.0, 1.0)
        } else if fx.layout.motion.exit.is_some() {
            let u = (t - end) / FADE_SECS;
            let ease = 1.0 - (1.0 - u.min(1.0)) * (1.0 - u.min(1.0));
            (fx.last(d.school, t), 1.0 - u, 1.0 + FADE_GROW * ease)
        } else if fx.layout.looping {
            (fx.last(d.school, t), 1.0 - (t - end) / FADE, 1.0)
        } else {
            (None, 0.0, 1.0)
        };
        let Some(shot) = shot.filter(|_| seen > 0.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let k = shot.size * fx.px() * swell;
        let (at, turn, scale) = place(&shot, k, d.flip);
        // the shape's own point, where the tail is laid from
        let anchor = Vec2::new(if d.flip { -shot.at.0 } else { shot.at.0 }, -shot.at.1);
        let here = origin.translation.truncate() + anchor;
        let tail = fx.layout.motion.tail;
        let keep = tail.map_or(0.0, |tl| tl.secs);
        d.trail.push_back((t, here));
        while d.trail.front().is_some_and(|(s, _)| *s < t - keep - 0.05) {
            d.trail.pop_front();
        }
        let Some(drawings) = drawings.as_ref() else {
            continue;
        };
        let atlas_cell = drawings
            .cells
            .get(d.effect)
            .and_then(|c| c.get(d.school).or_else(|| c.values().next()))
            .and_then(|c| c.get(shot.frame).copied());
        let Some((page, index)) = atlas_cell else {
            continue;
        };
        let (Some(image), Some(atlas)) = (
            drawings.pages.get(d.effect).and_then(|p| p.get(page)),
            drawings.atlases.get(d.effect).and_then(|a| a.get(page)),
        ) else {
            continue;
        };
        for child in children.iter() {
            let Ok((part, mut sprite, mut transform, mut visibility)) = sprites.get_mut(child)
            else {
                continue;
            };
            let (pos, alpha, size) = match (part.0, tail) {
                (None, _) => (at, seen, scale),
                (Some(i), Some(tl)) => {
                    let u = (i + 1) as f32 / (TAIL + 1) as f32;
                    let Some(then) = back_in_time(&d.trail, t - tl.secs * u) else {
                        *visibility = Visibility::Hidden;
                        continue;
                    };
                    // a copy where the shape was, narrowing toward the tail's end and clearing from `soft` on
                    let clear = if u > tl.soft && tl.soft < 1.0 {
                        (u - tl.soft) / (1.0 - tl.soft)
                    } else {
                        0.0
                    };
                    let narrow = 1.0 - (1.0 - tl.taper) * u;
                    if then.distance(here) < 0.5 {
                        *visibility = Visibility::Hidden;
                        continue;
                    }
                    (
                        at + (then - here),
                        seen * TAIL_ALPHA * (1.0 - u) * (1.0 - clear),
                        scale * narrow,
                    )
                }
                (Some(_), None) => {
                    *visibility = Visibility::Hidden;
                    continue;
                }
            };
            if sprite.image != *image {
                sprite.image = image.clone();
            }
            sprite.texture_atlas = Some(TextureAtlas {
                layout: atlas.clone(),
                index,
            });
            sprite.flip_x = d.flip;
            sprite.color = Color::srgba(1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0));
            transform.translation.x = pos.x;
            transform.translation.y = pos.y;
            transform.rotation = Quat::from_rotation_z(turn);
            transform.scale = Vec3::new(size, size, 1.0);
            *visibility = Visibility::Inherited;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every drawn effect is one the rules play, and its sheet's pages are beside it.
    #[test]
    fn every_drawn_effect_is_played_and_has_its_pages() -> Result<(), String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/drawn");
        for d in drawn::read(DRAWN)? {
            assert!(
                arena_duel::effects::EFFECTS
                    .iter()
                    .any(|(fx, _)| *fx == d.fx),
                "{} is not an effect the rules play",
                d.fx
            );
            for i in 0..d.layout.pages.len() {
                let name = if i == 0 {
                    format!("{}.webp", d.file)
                } else {
                    format!("{}-{i}.webp", d.file)
                };
                assert!(dir.join(&name).is_file(), "{}: no {name}", d.fx);
            }
        }
        Ok(())
    }

    /// A long fight, played as the game plays it and, now and then, caught up several ticks in one frame (as a page
    /// does after a pause): the drawn effects alive at once never pile up, and none outlives what started it.
    #[test]
    fn drawn_effects_do_not_pile_up_over_a_long_fight() {
        use arena_duel::duel::Duel;
        use arena_duel::seat::Brain;
        for seed in [7, 11, 23] {
            let layout = Layout::default();
            let mut duel = Duel::new(
                crate::rules::config(&layout, seed),
                [Brain::make(), Brain::make()],
            );
            let mut app = App::new();
            app.insert_resource(layout)
                .insert_resource(Shown::default())
                .insert_resource(Pace::default())
                .add_message::<Fx>()
                .add_systems(Update, (sweep, commands, animate).chain());
            let (dt, mut most, mut played) = (1.0 / 60.0, 0, 0);
            for frame in 0..60 * 240 {
                // every ten seconds, half a second caught up in one frame
                let ticks = if frame % 600 == 599 { 30 } else { 1 };
                for _ in 0..ticks {
                    for out in duel.step(dt) {
                        if matches!(&out, Out::Play { fx, .. } if plays(fx)) {
                            played += 1;
                        }
                        app.world_mut().write_message(Fx(out));
                    }
                }
                app.world_mut().resource_mut::<Pace>().run = dt * f64::from(ticks);
                app.update();
                let alive = app
                    .world_mut()
                    .query::<&Drawing>()
                    .iter(app.world())
                    .count();
                most = most.max(alive);
            }
            assert!(
                played > 20,
                "seed {seed}: only {played} drawn effects played"
            );
            assert!(
                most <= 16,
                "seed {seed}: {most} drawn effects alive at once"
            );
        }
    }

    /// A new round stops what the last one played: once the next round has begun and an exit has had time to play,
    /// no effect of an earlier round is left over the fighters set back in their corners.
    #[test]
    fn a_new_round_ends_the_last_rounds_effects() {
        use arena_duel::duel::Duel;
        use arena_duel::scripted::Scripted;
        use arena_duel::view::view;
        let layout = Layout::default();
        let mut duel = Duel::new(
            crate::rules::config(&layout, 7),
            [Scripted::aggressive(), Scripted::aggressive()],
        );
        let mut app = App::new();
        app.insert_resource(layout)
            .insert_resource(Shown::default())
            .insert_resource(Pace::default())
            .add_message::<Fx>()
            .add_systems(Update, (sweep, commands, animate).chain());
        let dt = 1.0 / 60.0;
        let (mut since, mut rounds) = (0.0, 0);
        for _ in 0..60 * 240 {
            for out in duel.step(dt) {
                app.world_mut().write_message(Fx(out));
            }
            let round = duel.state().round;
            if app
                .world()
                .resource::<Shown>()
                .view
                .as_ref()
                .map(|v| v.round)
                != Some(round)
            {
                since = 0.0;
                rounds += 1;
            }
            app.world_mut().resource_mut::<Shown>().view = Some(view(duel.state()));
            app.world_mut().resource_mut::<Pace>().run = dt;
            app.update();
            since += dt;
            if since > f64::from(FADE_SECS) + 0.1 {
                let old = app
                    .world_mut()
                    .query::<&Drawing>()
                    .iter(app.world())
                    .filter(|d| d.round != round)
                    .count();
                assert_eq!(
                    old, 0,
                    "{since:.2} s into round {round}: {old} of the last round's effects still play"
                );
            }
        }
        assert!(rounds > 2, "only {rounds} rounds");
    }

    /// A blast on the ground where a blow lands never hides the fighter it lands on, nor stands over the fight
    /// after it: the dive's slam is at most about half a fighter tall (a fighter is drawn taller than its tile) and gone in about a second and a half, the small
    /// explosion's held cloud in under two.
    #[test]
    fn ground_blasts_neither_hide_the_struck_fighter_nor_linger() {
        let fighter = Layout::world(arena_duel::data::T);
        let tallest = |d: &Drawn| {
            d.layout
                .clips
                .values()
                .flatten()
                .map(|c| c.h as f32 * d.px())
                .fold(0.0, f32::max)
        };
        let find = |fx: &str| all().iter().find(|d| d.fx == fx);
        let Some(dirt) = find("explosions/dirt-explosion") else {
            unreachable!("dirt-explosion is drawn")
        };
        assert!(
            tallest(dirt) <= fighter * 0.6,
            "the dive's slam is {} world pixels tall, a fighter's tile {fighter}",
            tallest(dirt)
        );
        for (d, most) in [(Some(dirt), 1.6), (find("explosions/small-explosion"), 2.0)] {
            let Some(d) = d else { continue };
            for school in d.layout.clips.keys() {
                let life = d.life(school).unwrap_or(f32::INFINITY);
                assert!(life <= most, "{} plays {life} s in {school}", d.fx);
            }
        }
    }

    /// A seal that lies on the ground is drawn on the floor under its caster, even one cast in the air; the rest
    /// where the rules play them.
    #[test]
    fn a_ground_seal_lies_on_the_floor_under_a_caster_in_the_air() {
        let layout = Layout::default();
        let find = |fx: &str| all().iter().position(|d| d.fx == fx);
        let (Some(seal), Some(bolt)) = (find("arcane/rune-circle"), find("projectiles/laser-bolt"))
        else {
            unreachable!("both are drawn")
        };
        let floor = layout.at(0.0, layout.ground).y;
        let in_the_air = f64::from(layout.ground) * arena_duel::data::UNIT - 140.0;
        assert_eq!(spot(&layout, seal, 400.0, in_the_air).y, floor);
        assert!(spot(&layout, bolt, 400.0, in_the_air).y > floor);
    }

    /// A one-shot stopped while it is already leaving (a new round over its fade) goes on leaving: it never comes
    /// back to full sight to fade again.
    #[test]
    fn a_stop_in_the_exit_does_not_bring_it_back() {
        let Some(effect) = all()
            .iter()
            .position(|d| d.fx == "explosions/small-explosion")
        else {
            unreachable!("small-explosion is drawn")
        };
        let fx = &all()[effect];
        let Some(life) = fx.life("thunder") else {
            unreachable!("a one-shot")
        };
        let mut d = Drawing {
            effect,
            school: "thunder",
            t: life + 0.2,
            stopped: None,
            moved: 0.0,
            flip: false,
            trail: VecDeque::new(),
            round: 0,
        };
        assert_eq!(end_of(fx, &d), life);
        d.stopped = Some(d.t);
        assert_eq!(
            end_of(fx, &d),
            life,
            "a stop in the fade moved its end later"
        );
        d.stopped = Some(life / 2.0);
        assert_eq!(end_of(fx, &d), life / 2.0);
    }

    #[test]
    fn a_shape_turning_carries_its_centre_round_the_anchor() {
        let cell = arena_vfx::drawn::Cell {
            page: 0,
            x: 0,
            y: 0,
            w: 2,
            h: 2,
            dx: 0,
            dy: -2,
        };
        let shot = Shot {
            frame: 0,
            cell,
            angle: core::f32::consts::PI,
            size: 1.0,
            at: (0.0, 0.0),
        };
        // centre (1, -1) in the sheet is (1, 1) with y up; half a turn takes it to (-1, -1)
        let (at, _, _) = place(&shot, 1.0, false);
        assert!((at - Vec2::new(-1.0, -1.0)).length() < 1e-4, "{at}");
        let (mirrored, _, _) = place(&Shot { angle: 0.0, ..shot }, 1.0, true);
        assert!(
            (mirrored - Vec2::new(-1.0, 1.0)).length() < 1e-4,
            "{mirrored}"
        );
    }
}

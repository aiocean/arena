//! The spell effects drawn by an image model (`arena_vfx::drawn`): smooth sheets of the shapes, moved by code. A
//! playing is an entity at the rules' point with one sprite over the fighters, turned, scaled and carried as its
//! motion says, and a few fainter copies behind them for its tail, laid where it was a moment ago. When it has played
//! out, or the rules stop a loop, it leaves by its exit (`disappear/fade`: it swells and fades out). The sheets are
//! sampled smoothly: the pixels are the renderer's business, not the art's.
//!
//! An effect with a drawn sheet plays from here; the rest play from their baked recipes (`sprites`) or live.

use super::Played;
use super::sprites::FightScoped;
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
    flip: bool,
    trail: VecDeque<(f32, Vec2)>,
}

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
            (commands, animate)
                .chain()
                .in_set(Played)
                .in_set(crate::Frame::React),
        );
    }
}

fn load(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let mut pages = Vec::new();
    let mut atlases = Vec::new();
    let mut cells = Vec::new();
    for d in all() {
        let handles: Vec<Handle<Image>> = (0..d.layout.pages.len())
            .map(|i| {
                let name = if i == 0 {
                    format!("{}.webp", d.file)
                } else {
                    format!("{}-{i}.webp", d.file)
                };
                assets
                    .load_builder()
                    .with_settings(|s: &mut ImageLoaderSettings| s.sampler = ImageSampler::linear())
                    .load(format!("drawn/{name}"))
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
        pages.push(handles);
        atlases.push(atlas.into_iter().map(|a| layouts.add(a)).collect());
        cells.push(by_school);
    }
    commands.insert_resource(Drawings {
        pages,
        atlases,
        cells,
    });
}

/// The rules' commands for the drawn effects: play, move, stop.
fn commands(
    mut commands: Commands,
    mut fx: MessageReader<Fx>,
    layout: Res<Layout>,
    shown: Res<Shown>,
    mut playing: Query<(&DrawingKey, &mut Drawing, &mut Transform)>,
) {
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
                let at = layout.at(Layout::world(*x), Layout::world(*y)).extend(0.0);
                debug!(
                    "drawn {fx} at ({x:.0}, {y:.0}) in {}",
                    drawn::school_of(opts.palette)
                );
                let sprite = |part: Part| {
                    let z = if part.0.is_some() { Z_TAIL } else { Z_OVER };
                    (
                        Sprite::default(),
                        Transform::from_xyz(0.0, 0.0, z),
                        Visibility::Hidden,
                        part,
                    )
                };
                let mut e = commands.spawn((
                    Name::new(format!("drawn {fx}")),
                    FightScoped(shown.fight),
                    Drawing {
                        effect,
                        school: drawn::school_of(opts.palette),
                        t: 0.0,
                        stopped: None,
                        flip: opts.flip.unwrap_or(false),
                        trail: VecDeque::new(),
                    },
                    DrawingKey(*key),
                    Transform::from_translation(at),
                    Visibility::Visible,
                ));
                e.with_children(|c| {
                    c.spawn(sprite(Part(None)));
                    for i in 0..TAIL {
                        c.spawn(sprite(Part(Some(i))));
                    }
                });
            }
            Out::Move { key, x, y } => {
                for (k, _, mut transform) in &mut playing {
                    if k.0 == *key {
                        transform.translation =
                            layout.at(Layout::world(*x), Layout::world(*y)).extend(0.0);
                    }
                }
            }
            Out::Stop { key } => {
                for (k, mut d, _) in &mut playing {
                    if k.0 == *key && d.stopped.is_none() {
                        d.stopped = Some(d.t);
                    }
                }
            }
            _ => {}
        }
    }
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
    let Some(drawings) = drawings else { return };
    let dt = pace.run as f32;
    for (e, mut d, origin, children) in &mut playing {
        d.t += dt;
        let Some(fx) = all().get(d.effect) else {
            commands.entity(e).despawn();
            continue;
        };
        let t = d.t;
        let end = d
            .stopped
            .or_else(|| fx.life(d.school))
            .unwrap_or(f32::INFINITY);
        // its shape now, how much it is still seen and how far it has swelled as it goes
        let (shot, seen, swell) = if t < end {
            (fx.shot(d.school, t), 1.0, 1.0)
        } else if fx.layout.motion.exit.is_some() {
            let u = (t - end) / FADE_SECS;
            let ease = 1.0 - (1.0 - u.min(1.0)) * (1.0 - u.min(1.0));
            (fx.last(d.school, t), 1.0 - u, 1.0 + FADE_GROW * ease)
        } else if d.stopped.is_some() {
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

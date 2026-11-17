//! The baked effects as sprites. The sheets (`assets/vfx`, written by `cargo run --release -p arena-vfx --example
//! bake`) are loaded through the asset server; each playing is an entity carrying its `Effect` clock and two child
//! sprites, behind the fighters and over them, whose atlas cell is the frame the clock is at. The clock runs on the
//! fight's time, so a hit-stop freezes the effects with the fighters. A sheet is baked facing right: one played facing
//! left is mirrored round its origin.

use super::{PlayEffect, Played};
use crate::layers::Z_FIGHTERS;
use crate::layout::Layout;
use crate::rules::{Fx, Pace, Shown};
use arena_vfx::sheet::{Cell, Index};
use arena_vfx::variant_key;
use bevy::prelude::*;
use arena_duel::out::{Out, PlayOpts};

/// The sheets' layout, baked beside them.
const INDEX: &str = include_str!("../../assets/vfx/index.txt");
/// Where the sprites sit: just behind the fighters, and just over them (the painted layers lie outside both).
const Z_BEHIND: f32 = Z_FIGHTERS - 1.0;
const Z_OVER: f32 = Z_FIGHTERS + 5.0;
/// How long a stopped loop takes to fade out, in seconds of the fight.
const FADE: f32 = 0.3;

/// A frame's atlas cell behind the fighters and over them (`None` for an empty side).
type Frame = [Option<(usize, Cell)>; 2];

/// One playing to start: which effect, where (world pixels), how, and the rules' key for it.
struct Start<'a> {
    fx: &'a str,
    at: (f32, f32),
    opts: &'a PlayOpts,
    key: Option<u32>,
}

/// The baked sheets: their layout, each page's texture and atlas, and each cell's place in its page's atlas.
#[derive(Resource)]
pub struct Sheets {
    pub index: Index,
    pages: Vec<Handle<Image>>,
    atlases: Vec<Handle<TextureAtlasLayout>>,
    /// For each variant, each frame's atlas cells.
    frames: Vec<Vec<Frame>>,
}

/// A playing of a baked effect: its sheet, how long it has played, when it was stopped, which way it faces.
#[derive(Component, Debug)]
pub struct Effect {
    pub variant: usize,
    pub t: f32,
    pub stopped: Option<f32>,
    pub flip: bool,
}

/// The rules' key for a playing, when they will move or stop it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectKey(pub u32);

/// Which of a playing's two sprites this is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Behind,
    Over,
}

/// Belongs to one fight: swept away when the next begins.
#[derive(Component, Debug, Clone, Copy)]
pub struct FightScoped(pub u32);

pub struct SpritesPlugin;

impl Plugin for SpritesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(
            Update,
            (sweep, commands, play_own, animate)
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
    let index = match Index::read(INDEX) {
        Ok(i) => i,
        Err(e) => {
            error!("the effects' sheets: {e}");
            return;
        }
    };
    let pages: Vec<Handle<Image>> = index
        .pages
        .iter()
        .map(|(file, _)| assets.load(format!("vfx/{file}")))
        .collect();
    let mut atlas: Vec<TextureAtlasLayout> = index
        .pages
        .iter()
        .map(|(_, [w, h])| TextureAtlasLayout::new_empty(UVec2::new(*w, *h)))
        .collect();
    let mut place = |c: &Option<Cell>| -> Option<(usize, Cell)> {
        let c = (*c)?;
        let layout = atlas.get_mut(c.page as usize)?;
        let i = layout.add_texture(URect::new(c.x, c.y, c.x + c.w, c.y + c.h));
        Some((i, c))
    };
    let frames = index
        .variants
        .iter()
        .map(|v| {
            v.back
                .iter()
                .zip(&v.front)
                .map(|(b, f)| [place(b), place(f)])
                .collect()
        })
        .collect();
    let atlases = atlas.into_iter().map(|a| layouts.add(a)).collect();
    commands.insert_resource(Sheets {
        index,
        pages,
        atlases,
        frames,
    });
}

/// A new fight sweeps away what the last one played.
fn sweep(mut commands: Commands, shown: Res<Shown>, playing: Query<(Entity, &FightScoped)>) {
    for (e, scope) in &playing {
        if scope.0 != shown.fight {
            commands.entity(e).despawn();
        }
    }
}

/// Where a playing's origin is, in the camera's units, snapped to whole world pixels.
fn origin(layout: &Layout, x: f32, y: f32) -> Vec3 {
    layout.at(x.round(), y.round()).extend(0.0)
}

fn spawn(commands: &mut Commands, sheets: &Sheets, layout: &Layout, fight: u32, start: Start) {
    let Start {
        fx,
        at: (x, y),
        opts,
        key,
    } = start;
    let Some(variant) = sheets
        .index
        .variants
        .iter()
        .position(|v| v.fx == fx && v.key == variant_key(opts))
        .or_else(|| sheets.index.variants.iter().position(|v| v.fx == fx))
    else {
        return;
    };
    let sprite = |side: Side| {
        (
            Sprite::default(),
            Transform::from_xyz(
                0.0,
                0.0,
                if side == Side::Behind {
                    Z_BEHIND
                } else {
                    Z_OVER
                },
            ),
            Visibility::Hidden,
            side,
        )
    };
    let mut e = commands.spawn((
        Name::new(format!("effect {fx}")),
        FightScoped(fight),
        Effect {
            variant,
            t: 0.0,
            stopped: None,
            flip: opts.flip.unwrap_or(false),
        },
        Transform::from_translation(origin(layout, x, y)),
        Visibility::Visible,
        children![sprite(Side::Behind), sprite(Side::Over)],
    ));
    if let Some(k) = key {
        e.insert(EffectKey(k));
    }
}

/// The rules' commands for the baked effects: play, move, stop.
fn commands(
    mut commands: Commands,
    mut fx: MessageReader<Fx>,
    sheets: Option<Res<Sheets>>,
    layout: Res<Layout>,
    shown: Res<Shown>,
    mut playing: Query<(&EffectKey, &mut Effect, &mut Transform)>,
) {
    let Some(sheets) = sheets else {
        return;
    };
    for Fx(out) in fx.read() {
        match out {
            Out::Play {
                key,
                fx,
                x,
                y,
                opts,
            } if arena_vfx::baked(fx) => {
                let at = (Layout::world(*x), Layout::world(*y));
                let start = Start {
                    fx,
                    at,
                    opts,
                    key: Some(*key),
                };
                spawn(&mut commands, &sheets, &layout, shown.fight, start);
            }
            Out::Move { key, x, y } => {
                for (k, _, mut transform) in &mut playing {
                    if k.0 == *key {
                        transform.translation =
                            origin(&layout, Layout::world(*x), Layout::world(*y));
                    }
                }
            }
            Out::Stop { key } => {
                for (k, mut effect, _) in &mut playing {
                    if k.0 == *key && effect.stopped.is_none() {
                        effect.stopped = Some(effect.t);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The game's own effects (the braziers).
fn play_own(
    mut commands: Commands,
    mut asked: MessageReader<PlayEffect>,
    sheets: Option<Res<Sheets>>,
    layout: Res<Layout>,
    shown: Res<Shown>,
) {
    let Some(sheets) = sheets else {
        return;
    };
    for a in asked.read() {
        let start = Start {
            fx: a.fx,
            at: (a.x, a.y),
            opts: &a.opts,
            key: None,
        };
        spawn(&mut commands, &sheets, &layout, shown.fight, start);
    }
}

/// Each playing moves on by the fight's time and shows its frame; one that ended, or faded after a stop, goes.
fn animate(
    mut commands: Commands,
    pace: Res<Pace>,
    sheets: Option<Res<Sheets>>,
    mut playing: Query<(Entity, &mut Effect, &Children)>,
    mut sprites: Query<(&Side, &mut Sprite, &mut Transform, &mut Visibility)>,
) {
    let Some(sheets) = sheets else {
        return;
    };
    let dt = pace.run as f32;
    for (e, mut effect, children) in &mut playing {
        effect.t += dt;
        let (Some(v), Some(frames)) = (
            sheets.index.variants.get(effect.variant),
            sheets.frames.get(effect.variant),
        ) else {
            commands.entity(e).despawn();
            continue;
        };
        let n = frames.len();
        let k = (effect.t * v.fps).floor().max(0.0) as usize;
        let fade = effect.stopped.map_or(1.0, |s| 1.0 - (effect.t - s) / FADE);
        let k = if v.looping {
            if k < n {
                k
            } else {
                let r = v.restart.min(n.saturating_sub(1));
                r + (k - r) % (n - r).max(1)
            }
        } else {
            k
        };
        if fade <= 0.0 || k >= n {
            commands.entity(e).despawn();
            continue;
        }
        for child in children.iter() {
            let Ok((side, mut sprite, mut transform, mut visibility)) = sprites.get_mut(child)
            else {
                continue;
            };
            let cell = frames[k][if *side == Side::Behind { 0 } else { 1 }];
            let Some((i, c)) = cell else {
                *visibility = Visibility::Hidden;
                continue;
            };
            let (Some(page), Some(atlas)) = (
                sheets.pages.get(c.page as usize),
                sheets.atlases.get(c.page as usize),
            ) else {
                continue;
            };
            if sprite.image != *page {
                sprite.image = page.clone();
            }
            sprite.texture_atlas = Some(TextureAtlas {
                layout: atlas.clone(),
                index: i,
            });
            sprite.flip_x = effect.flip;
            sprite.color = Color::srgba(1.0, 1.0, 1.0, fade.clamp(0.0, 1.0));
            // the cell's centre from the origin (y down in the sheet, up on screen), mirrored when it faces left
            let cx = c.dx as f32 + c.w as f32 / 2.0;
            let cy = c.dy as f32 + c.h as f32 / 2.0;
            transform.translation.x = if effect.flip { -cx } else { cx };
            transform.translation.y = -cy;
            *visibility = Visibility::Inherited;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every effect the rules may play has a baked sheet, unless the game paints it live; a recipe changed or added
    /// without a bake fails here (`cargo run --release -p arena-vfx --example bake -- crates/arena-game/assets/vfx`).
    #[test]
    fn every_played_effect_is_baked() -> Result<(), String> {
        let index = Index::read(INDEX)?;
        for (fx, _) in arena_duel::effects::EFFECTS {
            if arena_vfx::baked(fx) && arena_vfx::recipes::find(fx).is_some() {
                assert!(
                    index.variants.iter().any(|v| v.fx == *fx),
                    "{fx} has no baked sheet"
                );
            }
        }
        for v in &index.variants {
            assert_eq!(
                v.back.len(),
                v.front.len(),
                "{}: a frame on one side only",
                v.fx
            );
            assert!(!v.back.is_empty() && v.fps > 0.0, "{}: no frames", v.fx);
        }
        Ok(())
    }
}

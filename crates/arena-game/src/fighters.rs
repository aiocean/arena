//! The two fighters, each drawn from its sprite sheet (`assets/fighters`, made by `art/<character>/tools/sheet.rs`):
//! the frame its pose shows (`pick`), its feet on the floor or in the air, facing its way, `UNIT` times finer than
//! the world. A blow's flash brightens it to white, the grace after getting up blinks it, a hold flickers it.

use crate::feel::Jolts;
use crate::layers::Z_FIGHTERS;
use crate::layout::Layout;
use crate::rules::Shown;
use crate::sheet::{self, Cell};
use arena_duel::data::UNIT;
use arena_duel::pick::{Appearance, pick};
use arena_duel::view::FighterView;
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;

/// A fighter's sheet: its layout and its pages as textures.
pub struct Look {
    pub layout: sheet::Layout,
    pub pages: Vec<Handle<Image>>,
}

/// Each look's fighter: Thunder (Vương Lâm) first, then Frost (the Seven-Colored Daoist). Seat 0 looks like the
/// first, seat 1 like the second (`arena_duel::data::Look`).
#[derive(Resource)]
pub struct Looks(pub Vec<Look>);

/// The sprite of fighter `0` (Thunder) or `1` (Frost).
#[derive(Component)]
pub struct FighterSprite(pub usize);

/// Each look's sheet: its layout, built into the game, and its pages, loaded from the game's assets (a fighter
/// shows once they have arrived).
const SHEETS: [(&str, &[&str]); 2] = [
    (
        include_str!("../assets/fighters/cultivator.json"),
        &[
            "fighters/cultivator-thunder.png",
            "fighters/cultivator-thunder-1.png",
        ],
    ),
    (
        include_str!("../assets/fighters/seven-colored-daoist.json"),
        &[
            "fighters/seven-colored-daoist-frost.png",
            "fighters/seven-colored-daoist-frost-1.png",
        ],
    ),
];

pub struct FightersPlugin;

impl Plugin for FightersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(
            Update,
            show.in_set(crate::Frame::React).after(crate::feel::Felt),
        );
    }
}

/// A picture built into the game as a texture the GPU keeps (the game never reads its pixels).
pub fn page(png: &[u8]) -> Option<Image> {
    Image::from_buffer(
        png,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|e| error!("a fighter's sheet did not load: {e}"))
    .ok()
}

fn load(mut commands: Commands, assets: Res<AssetServer>) {
    let mut looks = Vec::new();
    for (json, pngs) in SHEETS {
        let layout = match sheet::parse(json) {
            Ok(l) => l,
            Err(e) => {
                error!("{e}");
                return;
            }
        };
        let pages = pngs.iter().map(|path| assets.load(*path)).collect();
        looks.push(Look { layout, pages });
    }
    commands.insert_resource(Looks(looks));
    for i in 0..2 {
        commands.spawn((
            Sprite::default(),
            Transform::from_xyz(0.0, 0.0, Z_FIGHTERS + i as f32),
            Visibility::Hidden,
            FighterSprite(i),
        ));
    }
}

/// What picks fighter `f`'s frame.
pub fn appearance(f: &FighterView) -> Appearance {
    arena_duel::pick::appearance(
        f.pose,
        f.pose_t,
        f.hp <= 0.0,
        f.act.map(|a| (a.spell, a.phase, a.t)),
    )
}

/// Where a frame's sprite goes: the centre of its box in world pixels, its feet at (`x`, `feet`), facing `face`
/// (the box mirrored round the feet when it faces left), snapped to the sprite's own pixels.
pub fn placed(cell: Cell, x: f32, feet: f32, face: f32) -> Vec2 {
    let unit = UNIT as f32;
    let (w, h) = (cell.w as f32, cell.h as f32);
    let left = if face < 0.0 {
        -(cell.dx as f32) - w
    } else {
        cell.dx as f32
    };
    let top = cell.dy as f32;
    let snap = |v: f32| (v * unit).round() / unit;
    Vec2::new(
        snap(x) + (left + w / 2.0) / unit,
        snap(feet) + (top + h / 2.0) / unit,
    )
}

fn show(
    looks: Option<Res<Looks>>,
    shown: Res<Shown>,
    layout: Res<Layout>,
    jolts: Res<Jolts>,
    time: Res<Time>,
    mut sprites: Query<(&FighterSprite, &mut Sprite, &mut Transform, &mut Visibility)>,
) {
    let (Some(looks), Some(view)) = (looks, &shown.view) else {
        return;
    };
    for (FighterSprite(i), mut sprite, mut transform, mut visibility) in &mut sprites {
        let f = &view.fighters[*i];
        let Some(look) = looks.0.get(f.look as usize) else {
            continue;
        };
        let shot = pick(
            &appearance(f),
            look.layout.fps,
            |c| look.layout.len(c),
            |c| arena_duel::frames::active(f.look, c),
        );
        let Some(cell) = look.layout.cell(shot.clip, shot.frame) else {
            continue;
        };
        let face = if f.face < 0.0 { -1.0 } else { 1.0 };
        let jolt = jolts.0[*i];
        let x = Layout::world(f.x) + jolt[0] as f32;
        let feet = layout.ground - Layout::world(f.air) + jolt[1] as f32;
        let at = placed(cell, x, feet, face);
        let unit = UNIT as f32;
        sprite.image = look
            .pages
            .get(cell.page as usize)
            .cloned()
            .unwrap_or_default();
        sprite.rect = Some(Rect::new(
            cell.x as f32,
            cell.y as f32,
            (cell.x + cell.w) as f32,
            (cell.y + cell.h) as f32,
        ));
        sprite.custom_size = Some(Vec2::new(cell.w as f32 / unit, cell.h as f32 / unit));
        sprite.flip_x = face < 0.0;
        sprite.color = if f.flash > 0.0 {
            // brighter than white: the screen clamps it, so the sprite flashes white
            Color::LinearRgba(LinearRgba::new(8.0, 8.0, 8.0, 1.0))
        } else if f.shocked > 0.0 && (time.elapsed_secs() * 24.0) as i32 % 2 == 0 {
            Color::LinearRgba(LinearRgba::new(1.6, 1.6, 2.4, 1.0))
        } else {
            Color::WHITE
        };
        let blink = f.untouchable && f.hp > 0.0 && (time.elapsed_secs() * 15.0) as i32 % 2 == 1;
        *visibility = if blink {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        let z = transform.translation.z;
        transform.translation = layout.at(at.x, at.y).extend(z);
    }
}

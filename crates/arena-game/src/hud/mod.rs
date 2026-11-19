//! The HUD, as the page drew it (`duel-art.js`): the bars at the top (each fighter's medallion with its face, life,
//! qi and buffs, and the plaque of rounds won) and each fighter's skill bar on the floor under its side, painted
//! in world pixels on the CPU (`Canvases.hud` under the icons, a picture of the HUD's own over them), with the faces
//! and the skill icons as sprites from their sheets, finer than the world. Hovering a skill or a buff shows what it
//! is in a panel by the pointer (`duel-info.js`). The screen's shake leaves all of it still.

pub mod board;
mod glyphs;
mod info;
mod paint;

use crate::fighters::Looks;
use crate::layers::{Canvases, Cleared, Z_HUD};
use crate::layout::{Layout, MainCamera};
use crate::raster::Canvas;
use crate::rules::Shown;
use arena_duel::data::UNIT;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::text::FontSize;
use bevy::window::PrimaryWindow;
use info::{Tip, Tone};
use paint::{Geo, Hit, Painted, Pen, Seen};

/// The skill icon atlases: 24 and 32 pixels an icon, five to a row.
const SKILLS_24: &[u8] = include_bytes!("../../assets/skills-24.png");
const SKILLS_32: &[u8] = include_bytes!("../../assets/skills-32.png");

/// The sprites over the HUD's picture, and the picture over them.
const Z_SPRITES: f32 = Z_HUD + 1.0;
const Z_OVER: f32 = Z_HUD + 2.0;
/// The most skill slots a fighter shows.
const SLOTS: usize = 6;

/// What the HUD keeps between frames.
#[derive(Resource)]
struct Hud {
    /// The fight it remembers (a new one forgets everything).
    fight: u32,
    seen: [Seen; 2],
    /// What is painted over the icons.
    over: Canvas,
    painted: Painted,
    /// The icon atlases, 24 then 32.
    atlases: [Handle<Image>; 2],
}

impl Default for Hud {
    fn default() -> Hud {
        Hud {
            fight: 0,
            seen: Default::default(),
            over: Canvas::new(1, 1),
            painted: Painted::default(),
            atlases: Default::default(),
        }
    }
}

#[derive(Component)]
struct FaceSprite(usize);

#[derive(Component)]
struct IconSprite(usize);

#[derive(Component)]
struct OverPicture;

#[derive(Component)]
struct TipPanel;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(board::BoardPlugin)
            .init_resource::<Hud>()
            .add_systems(Startup, spawn)
            .add_systems(Update, notice.in_set(crate::Frame::React))
            .add_systems(Update, draw.in_set(crate::Frame::Paint).after(Cleared))
            .add_systems(Update, (place, tip).in_set(crate::Frame::Upload));
    }
}

fn blank(images: &mut Assets<Image>) -> Handle<Image> {
    images.add(Image::new_fill(
        Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut hud: ResMut<Hud>) {
    let mut atlas = |png: &[u8]| {
        crate::fighters::page(png)
            .map(|i| images.add(i))
            .unwrap_or_default()
    };
    hud.atlases = [atlas(SKILLS_24), atlas(SKILLS_32)];
    for i in 0..2 {
        commands.spawn((
            Sprite::default(),
            Transform::from_xyz(0.0, 0.0, Z_SPRITES),
            Visibility::Hidden,
            FaceSprite(i),
        ));
    }
    for i in 0..2 * SLOTS {
        commands.spawn((
            Sprite::default(),
            Transform::from_xyz(0.0, 0.0, Z_SPRITES),
            Visibility::Hidden,
            IconSprite(i),
        ));
    }
    commands.spawn((
        Sprite::from_image(blank(&mut images)),
        Transform::from_xyz(0.0, 0.0, Z_OVER),
        OverPicture,
    ));
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(2.0)),
            row_gap: Val::Px(4.0),
            ..default()
        },
        BackgroundColor(Color::srgba_u8(18, 13, 24, 240)),
        BorderColor::all(Color::srgb_u8(0xb8, 0x89, 0x3c)),
        GlobalZIndex(100),
        TipPanel,
    ));
}

/// A new fight forgets the last.
fn notice(shown: Res<Shown>, mut hud: ResMut<Hud>) {
    if hud.fight != shown.fight {
        hud.fight = shown.fight;
        hud.seen = Default::default();
    }
}

fn draw(
    shown: Res<Shown>,
    layout: Res<Layout>,
    time: Res<Time>,
    mut canvases: ResMut<Canvases>,
    mut hud: ResMut<Hud>,
) {
    let hud = hud.as_mut();
    let (w, h) = (layout.w as i32, layout.h as i32);
    if hud.over.w != w || hud.over.h != h {
        hud.over = Canvas::new(w, h);
    }
    hud.over.clear();
    hud.painted = Painted::default();
    let Some(view) = &shown.view else {
        return;
    };
    let g = Geo::of(&layout);
    let frames = time.delta_secs_f64() * 60.0;
    let mut base = Pen(&mut canvases.hud);
    paint::bars(
        &mut base,
        view,
        &g,
        &mut hud.seen,
        frames,
        &mut hud.painted.faces,
    );
    let mut over = Pen(&mut hud.over);
    paint::skills(
        &mut base,
        &mut over,
        view,
        &g,
        &mut hud.seen,
        &mut hud.painted.icons,
    );
}

/// Where a HUD picture's pixel (`x`, `y`) is in the world: the HUD stays where the camera is.
fn at(layout: &Layout, camera: Vec3, x: f32, y: f32) -> Vec2 {
    // a picture of an odd size sits half a pixel off the centre, so its pixels meet the screen's
    let (ox, oy) = ((layout.w % 2.0) / 2.0, (layout.h % 2.0) / 2.0);
    layout.at(x, y) + Vec2::new(camera.x + ox, camera.y - oy)
}

type Others<A, B> = (Without<A>, Without<B>, Without<MainCamera>);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn place(
    layout: Res<Layout>,
    hud: Res<Hud>,
    looks: Option<Res<Looks>>,
    mut images: ResMut<Assets<Image>>,
    cameras: Query<&Transform, With<MainCamera>>,
    mut faces: Query<
        (&FaceSprite, &mut Sprite, &mut Transform, &mut Visibility),
        Others<IconSprite, OverPicture>,
    >,
    mut icons: Query<
        (&IconSprite, &mut Sprite, &mut Transform, &mut Visibility),
        Others<FaceSprite, OverPicture>,
    >,
    mut overs: Query<
        (&mut Sprite, &mut Transform),
        (With<OverPicture>, Others<FaceSprite, IconSprite>),
    >,
) {
    let camera = cameras.single().map(|t| t.translation).unwrap_or_default();
    let unit = UNIT as f32;
    for (FaceSprite(i), mut sprite, mut transform, mut visibility) in &mut faces {
        let face = hud.painted.faces.get(*i);
        let look = face.and_then(|f| looks.as_ref()?.0.get(f.look));
        let (Some(face), Some(look)) = (face, look) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        // the sheet's face scaled by whole pixels to fit its square, centred, facing into the arena
        let p = look.layout.portrait;
        let square = face.size.max(1) as u32 * UNIT as u32;
        let k = (square / p.w.max(p.h).max(1)).max(1);
        let (w, h) = ((p.w * k) as f32, (p.h * k) as f32);
        let x0 = face.x as f32 + (square.saturating_sub(p.w * k) / 2) as f32 / unit;
        let y0 = face.y as f32 + (square.saturating_sub(p.h * k) / 2) as f32 / unit;
        sprite.image = look.pages.get(p.page as usize).cloned().unwrap_or_default();
        sprite.rect = Some(Rect::new(
            p.x as f32,
            p.y as f32,
            (p.x + p.w) as f32,
            (p.y + p.h) as f32,
        ));
        sprite.custom_size = Some(Vec2::new(w / unit, h / unit));
        sprite.flip_x = face.flip;
        sprite.color = Color::srgba(1.0, 1.0, 1.0, face.alpha);
        let c = at(&layout, camera, x0 + w / unit / 2.0, y0 + h / unit / 2.0);
        transform.translation = c.extend(Z_SPRITES);
        *visibility = Visibility::Visible;
    }
    // the atlas whose pixels land on whole screen pixels
    let big = (layout.icon * layout.scale) % 32.0 == 0.0;
    let (atlas, k) = if big {
        (hud.atlases[1].clone(), 32.0)
    } else {
        (hud.atlases[0].clone(), 24.0)
    };
    for (IconSprite(i), mut sprite, mut transform, mut visibility) in &mut icons {
        let Some(icon) = hud.painted.icons.get(*i) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let (cx, cy) = (
            (icon.cell % glyphs::ICON_COLUMNS) as f32,
            (icon.cell / glyphs::ICON_COLUMNS) as f32,
        );
        sprite.image = atlas.clone();
        sprite.rect = Some(Rect::new(cx * k, cy * k, (cx + 1.0) * k, (cy + 1.0) * k));
        let n = icon.n as f32;
        sprite.custom_size = Some(Vec2::new(n, n));
        sprite.color = Color::srgba(1.0, 1.0, 1.0, icon.alpha);
        let c = at(
            &layout,
            camera,
            icon.x as f32 + n / 2.0,
            icon.y as f32 + n / 2.0,
        );
        transform.translation = c.extend(Z_SPRITES);
        *visibility = Visibility::Visible;
    }
    for (mut sprite, mut transform) in &mut overs {
        if let Some(mut image) = images.get_mut(&sprite.image) {
            let size = Extent3d {
                width: hud.over.w as u32,
                height: hud.over.h as u32,
                depth_or_array_layers: 1,
            };
            if image.texture_descriptor.size != size {
                image.resize(size);
            }
            image.data = Some(hud.over.bytes().to_vec());
        }
        sprite.custom_size = Some(Vec2::new(layout.w, layout.h));
        transform.translation = at(&layout, camera, layout.w / 2.0, layout.h / 2.0).extend(Z_OVER);
    }
}

fn tone_color(t: Tone) -> Color {
    let c = match t {
        Tone::Plain => (0xf0, 0xcf, 0x7a),
        Tone::Good => (0x6f, 0xdc, 0x8a),
        Tone::Bad => (0xff, 0x6a, 0x5a),
    };
    Color::srgb_u8(c.0, c.1, c.2)
}

/// The panel's lines for `tip`, `px` times the size it has at a world pixel of four logical ones.
fn fill(parent: &mut ChildSpawnerCommands, t: &Tip, px: f32) {
    let font = |size: f32| TextFont {
        font_size: FontSize::Px(size * px),
        ..default()
    };
    let rule = |p: &mut ChildSpawnerCommands| {
        p.spawn((
            Node {
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(0x6a, 0x4a, 0x1c)),
        ));
    };
    parent.spawn((
        Text::new(t.title.clone()),
        font(16.0),
        TextColor(tone_color(t.tone)),
    ));
    if let Some(kind) = &t.kind {
        parent.spawn((
            Text::new(kind.clone()),
            font(12.0),
            TextColor(Color::srgb_u8(0x9a, 0x92, 0xb0)),
        ));
    }
    rule(parent);
    if !t.props.is_empty() {
        for (label, value) in &t.props {
            parent
                .spawn((
                    Text::new(format!("{label}: ")),
                    font(13.0),
                    TextColor(Color::srgb_u8(0x9a, 0x92, 0xb0)),
                ))
                .with_child((
                    TextSpan::new(value.clone()),
                    font(13.0),
                    TextColor(Color::WHITE),
                ));
        }
        rule(parent);
    }
    parent.spawn((
        Text::new(t.text.clone()),
        font(13.0),
        TextColor(Color::srgb_u8(0xe8, 0xe0, 0xd0)),
    ));
    if !t.mods.is_empty() {
        rule(parent);
        for m in &t.mods {
            parent.spawn((
                Text::new(m.clone()),
                font(13.0),
                TextColor(Color::srgb_u8(0x88, 0x88, 0xff)),
            ));
        }
    }
}

/// The tooltip of what the pointer is over: a skill slot or a buff tile, kept beside the pointer and on the screen.
#[allow(clippy::type_complexity)]
fn tip(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    layout: Res<Layout>,
    shown: Res<Shown>,
    mut panels: Query<(Entity, &mut Node, &ComputedNode), With<TipPanel>>,
    mut last: Local<Option<(Tip, u32)>>,
) {
    let Ok((panel, mut node, computed)) = panels.single_mut() else {
        return;
    };
    let window = windows.single().ok();
    let cursor = window.and_then(|w| w.cursor_position().map(|c| (w, c)));
    let tip = cursor.zip(shown.view.as_ref()).and_then(|((w, c), view)| {
        // the pointer in the HUD's pixels: the HUD sits centred on the screen, unshaken
        let (pw, ph) = (w.physical_width() as f32, w.physical_height() as f32);
        let (ox, oy) = ((layout.w % 2.0) / 2.0, (layout.h % 2.0) / 2.0);
        let s = c * w.scale_factor();
        let hx = (s.x - pw / 2.0) / layout.scale - ox + layout.w / 2.0;
        let hy = (s.y - ph / 2.0) / layout.scale + layout.h / 2.0 - oy;
        let f = |i: usize| view.fighters.get(i);
        match paint::hud_at(view, &Geo::of(&layout), hx, hy)? {
            Hit::Skill(i, k) => Some(info::skill_tip(f(i)?.skill_bar.get(k)?)),
            Hit::Buff(i, k) => info::buff_tip(f(i)?.buffs.get(k)?),
        }
    });
    let (Some(tip), Some((window, c))) = (tip, cursor) else {
        if last.is_some() {
            node.display = Display::None;
            commands.entity(panel).despawn_related::<Children>();
            *last = None;
        }
        return;
    };
    // a world pixel in logical pixels, as the page's tooltip measured its own
    let px = (layout.scale / layout.dpr).max(2.0) / 4.0;
    let key = (layout.scale * 100.0) as u32;
    if last.as_ref() != Some(&(tip.clone(), key)) {
        commands.entity(panel).despawn_related::<Children>();
        commands.entity(panel).with_children(|p| fill(p, &tip, px));
        *last = Some((tip, key));
    }
    node.display = Display::Flex;
    node.width = Val::Px(260.0 * px);
    let size = computed.size() * computed.inverse_scale_factor();
    let (ww, wh) = (window.width(), window.height());
    let pad = 14.0;
    let x = (c.x + pad).min(ww - size.x - 4.0).max(4.0);
    let y = if c.y + pad + size.y > wh - 4.0 {
        c.y - size.y - pad
    } else {
        c.y + pad
    };
    node.left = Val::Px(x);
    node.top = Val::Px(y.max(4.0));
}

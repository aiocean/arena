//! The world's pictures, painted on the CPU each frame and shown as textures: `back` (the scene, the effects behind
//! the fighters and their light), `front` (the effects over the fighters, their light, the fight's numbers and
//! streaks) and `hud` (the bars and skills, which the screen's shake leaves still). The fighters are sprites between
//! `back` and `front`.

use crate::layout::{Layout, MainCamera};
use crate::profile::Uploads;
use crate::raster::Canvas;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Where each picture sits, back to front.
pub const Z_BACK: f32 = 0.0;
pub const Z_FIGHTERS: f32 = 10.0;
pub const Z_FRONT: f32 = 20.0;
pub const Z_HUD: f32 = 30.0;

/// The pictures of this frame, each the world's size.
#[derive(Resource)]
pub struct Canvases {
    /// The backdrop as the scene painted it for this layout (copied into `back` at the start of each frame).
    pub scene: Canvas,
    pub back: Canvas,
    pub front: Canvas,
    pub hud: Canvas,
}

impl Default for Canvases {
    fn default() -> Canvases {
        Canvases {
            scene: Canvas::new(1, 1),
            back: Canvas::new(1, 1),
            front: Canvas::new(1, 1),
            hud: Canvas::new(1, 1),
        }
    }
}

/// Which picture a sprite shows.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Back,
    Front,
    Hud,
}

/// Before the frame's painting: the pictures sized to the world, `back` the backdrop and the rest clear.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Cleared;

/// The pictures going to their textures: what paints over everything (the hitbox overlay) goes before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Uploaded;

pub struct LayersPlugin;

impl Plugin for LayersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Canvases>()
            .add_systems(Startup, spawn)
            .add_systems(Update, begin.in_set(Cleared).in_set(crate::Frame::Rules))
            .add_systems(Update, upload.in_set(crate::Frame::Upload).in_set(Uploaded));
    }
}

fn picture(images: &mut Assets<Image>) -> Handle<Image> {
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

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    for (layer, z) in [
        (Layer::Back, Z_BACK),
        (Layer::Front, Z_FRONT),
        (Layer::Hud, Z_HUD),
    ] {
        commands.spawn((
            Sprite::from_image(picture(&mut images)),
            Transform::from_xyz(0.0, 0.0, z),
            layer,
        ));
    }
}

fn begin(layout: Res<Layout>, mut canvases: ResMut<Canvases>) {
    let (w, h) = (layout.w as i32, layout.h as i32);
    let c = canvases.as_mut();
    if c.scene.w != w || c.scene.h != h {
        c.scene = Canvas::new(w, h);
    }
    if c.back.w != w || c.back.h != h {
        c.back = Canvas::new(w, h);
        c.front = Canvas::new(w, h);
        c.hud = Canvas::new(w, h);
    }
    c.back.px.copy_from_slice(&c.scene.px);
    c.front.clear();
    c.hud.clear();
}

/// The painted pictures to their textures, each only when it differs from what its texture already holds (an empty
/// `front` or a still HUD is not sent again), copied into the texture's own bytes; the HUD stays where the camera is
/// (the shake moves the world, not it).
fn upload(
    layout: Res<Layout>,
    canvases: Res<Canvases>,
    mut images: ResMut<Assets<Image>>,
    mut uploads: ResMut<Uploads>,
    mut sprites: Query<(&Layer, &mut Sprite, &mut Transform), Without<MainCamera>>,
    cameras: Query<&Transform, With<MainCamera>>,
) {
    let camera = cameras.single().map(|t| t.translation).unwrap_or_default();
    for (layer, mut sprite, mut transform) in &mut sprites {
        let canvas = match layer {
            Layer::Back => &canvases.back,
            Layer::Front => &canvases.front,
            Layer::Hud => &canvases.hud,
        };
        if send(&mut images, &sprite.image, canvas) {
            uploads.textures += 1;
            uploads.bytes += canvas.bytes().len();
        }
        let size = Some(Vec2::new(layout.w, layout.h));
        if sprite.custom_size != size {
            sprite.custom_size = size;
        }
        // a picture of an odd size sits half a pixel off the centre, so its pixels meet the screen's
        let (ox, oy) = ((layout.w % 2.0) / 2.0, (layout.h % 2.0) / 2.0);
        let z = transform.translation.z;
        let at = match layer {
            Layer::Hud => Vec3::new(camera.x + ox, camera.y - oy, z),
            _ => Vec3::new(ox, -oy, z),
        };
        if transform.translation != at {
            transform.translation = at;
        }
    }
}

/// `canvas` into the texture `image` when it differs from what the texture holds: whether it was sent. Reading the
/// image does not mark it changed; only a picture that differs is written, and so uploaded again.
fn send(images: &mut Assets<Image>, image: &Handle<Image>, canvas: &Canvas) -> bool {
    let size = Extent3d {
        width: canvas.w as u32,
        height: canvas.h as u32,
        depth_or_array_layers: 1,
    };
    let bytes = canvas.bytes();
    let same = images
        .get(image)
        .is_some_and(|i| i.texture_descriptor.size == size && i.data.as_deref() == Some(bytes));
    if same {
        return false;
    }
    let Some(mut image) = images.get_mut(image) else {
        return false;
    };
    if image.texture_descriptor.size != size {
        image.resize(size);
    }
    match image.data.as_mut() {
        Some(data) if data.len() == bytes.len() => data.copy_from_slice(bytes),
        _ => image.data = Some(bytes.to_vec()),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_goes_to_its_texture_only_when_it_changed() {
        let mut images = Assets::<Image>::default();
        let image = picture(&mut images);
        let mut canvas = Canvas::new(40, 30);
        assert!(
            send(&mut images, &image, &canvas),
            "the first picture is sent"
        );
        assert!(
            !send(&mut images, &image, &canvas),
            "the same picture again is not"
        );
        canvas.px[7] = [9, 9, 9, 255];
        assert!(send(&mut images, &image, &canvas), "a changed one is");
        assert_eq!(
            images.get(&image).and_then(|i| i.data.clone()).as_deref(),
            Some(canvas.bytes())
        );
        assert!(!send(&mut images, &image, &canvas));
        let bigger = Canvas::new(50, 30);
        assert!(send(&mut images, &image, &bigger), "a new size is");
        assert_eq!(
            images.get(&image).map(|i| i.texture_descriptor.size.width),
            Some(50)
        );
    }
}

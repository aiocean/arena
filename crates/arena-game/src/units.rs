//! Traps, mines, totems and minions (the rules' `unit::Unit`, read from the duel's view), drawn simply: a block of
//! colour in its owner's glow standing on the floor (a minion, a totem) or lying on it (a trap, a mine), sized by
//! kind, with a thin bar of its life over it. Their casts and blows play as the skill's own effects; their coming and
//! going plays the rules' puff.

use crate::layers::Z_FIGHTERS;
use crate::layout::Layout;
use crate::rules::Shown;
use arena_calc::device::Kind;
use arena_duel::data::{Look, T};
use arena_duel::view::UnitView;
use bevy::prelude::*;

pub struct UnitsPlugin;

impl Plugin for UnitsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show.in_set(crate::Frame::React));
    }
}

/// A unit's body sprite, by the rules' unit id; and its life bar.
#[derive(Component)]
pub struct UnitSprite(pub u32);
#[derive(Component)]
pub struct UnitLife(pub u32);

/// A unit's size in world pixels, wide and tall, by kind.
fn size(kind: Kind) -> Vec2 {
    let t = Layout::world(T);
    match kind {
        Kind::Minion => Vec2::new(t * 0.35, t * 0.55),
        Kind::Totem => Vec2::new(t * 0.2, t * 0.8),
        Kind::Trap | Kind::Mine => Vec2::new(t * 0.4, t * 0.06),
    }
}

/// Its owner's glow, darker for what lies on the floor.
fn tint(u: &UnitView, looks: [Look; 2]) -> Color {
    let (r, g, b) = match looks.get(u.owner).copied().unwrap_or(Look::Thunder) {
        Look::Thunder => (1.0, 0.85, 0.35),
        Look::Frost => (0.6, 0.9, 1.0),
    };
    let k = if u.floor { 0.7 } else { 1.0 };
    Color::srgb(r * k, g * k, b * k)
}

fn show(
    mut commands: Commands,
    shown: Res<Shown>,
    layout: Res<Layout>,
    mut bodies: Query<(Entity, &UnitSprite, &mut Sprite, &mut Transform), Without<UnitLife>>,
    mut lives: Query<(Entity, &UnitLife, &mut Sprite, &mut Transform), Without<UnitSprite>>,
) {
    let units: &[UnitView] = shown.view.as_ref().map_or(&[], |v| &v.units);
    let looks = crate::feel::looks(&shown);
    // what went
    for (e, UnitSprite(id), ..) in &bodies {
        if !units.iter().any(|u| u.id == *id) {
            commands.entity(e).despawn();
        }
    }
    for (e, UnitLife(id), ..) in &lives {
        if !units.iter().any(|u| u.id == *id) {
            commands.entity(e).despawn();
        }
    }
    for u in units {
        let s = size(u.kind);
        let x = Layout::world(u.x);
        let feet = layout.ground;
        let body_at = layout.at(x, feet - s.y / 2.0).extend(Z_FIGHTERS - 1.0);
        let life_w = s.x.max(Layout::world(T) * 0.3) * (u.hp / 100.0) as f32;
        let life_at = layout.at(x, feet - s.y - 2.0).extend(Z_FIGHTERS - 0.5);
        match bodies.iter_mut().find(|b| b.1.0 == u.id) {
            Some((_, _, mut sprite, mut transform)) => {
                sprite.custom_size = Some(s);
                transform.translation = body_at;
            }
            None => {
                commands.spawn((
                    Sprite::from_color(tint(u, looks), s),
                    Transform::from_translation(body_at),
                    UnitSprite(u.id),
                ));
            }
        }
        match lives.iter_mut().find(|b| b.1.0 == u.id) {
            Some((_, _, mut sprite, mut transform)) => {
                sprite.custom_size = Some(Vec2::new(life_w.max(0.0), 1.0));
                transform.translation = life_at;
            }
            None => {
                commands.spawn((
                    Sprite::from_color(
                        Color::srgb(0.9, 0.25, 0.2),
                        Vec2::new(life_w.max(0.0), 1.0),
                    ),
                    Transform::from_translation(life_at),
                    UnitLife(u.id),
                ));
            }
        }
    }
}

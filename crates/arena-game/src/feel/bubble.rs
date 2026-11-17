//! A fighter shouts the name of its technique: when it starts a cast, a speech bubble over its head says the skill's
//! name as the novel gives it (`arena_calc::gem::Gem::name`, "Hỏa Cầu!"), tinted by the skill's main kind of damage,
//! for about a second.
//!
//! The cast is read from the rules' state each frame (the fighter's act: its skill and phase), never from an event,
//! so nothing in the rules changes. The act names the skill as cast (the gem's id), not the motion it borrows, so a
//! skill moving as another shows its own name.
//!
//! The box and its tail are painted on `front` in world pixels, as the rest of the fight's marks; the name is a
//! `Text2d` in the world over it, laid out at the screen's own resolution (one glyph pixel a screen pixel, so the
//! Vietnamese marks stay sharp) and scaled down to the world. The font is Be Vietnam Pro Bold (SIL OFL 1.1,
//! `assets/fonts/OFL.txt`), cut to Latin and Vietnamese and built into the game, so the page needs no extra file.

use super::Overlay;
use crate::layers::{Canvases, Z_FRONT};
use crate::layout::{BARS_TALL, Layout};
use crate::raster::{Blend, Canvas, Rgba, fade, hex, mix};
use crate::rules::Shown;
use arena_calc::element::Element;
use arena_duel::fighter::Phase;
use arena_duel::view::{ActView, FighterView};
use bevy::prelude::*;
use bevy::sprite::{Anchor, Text2dShadow};
use bevy::text::{FontSize, TextLayoutInfo};

/// How long a shout stays, and the last part of it over which it fades, in seconds.
const LIFE: f32 = 1.2;
const FADE: f32 = 0.35;
/// How far above its feet a fighter's head reaches, in world pixels (the sheets' stance frames stand 243 sprite
/// pixels tall, `UNIT` to a world pixel), and the gap the tail leaves over it.
const HEAD: i32 = 61;
const GAP: i32 = 2;
/// The name's size in world pixels.
const TEXT: f32 = 7.0;
/// Room round the name inside the box, and the tail's height.
const PAD_X: i32 = 4;
const PAD_Y: i32 = 2;
const TAIL: i32 = 4;
/// The least room kept between the two bubbles, and from the screen's edges.
const APART: i32 = 2;
const EDGE: i32 = 2;

const INK: &str = "#120d1c";
const OUTLINE: &str = "#05040a";

/// The font that writes the names (Vietnamese).
const FONT: &[u8] = include_bytes!("../../assets/fonts/BeVietnamPro-Bold-vi.ttf");

/// What a fighter shouts for a skill: its name and its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Shout {
    pub name: String,
    pub tint: Rgba,
}

/// The colour of kind of damage `e` (none: the pale jade of qi).
pub fn tint(e: Option<Element>) -> Rgba {
    hex(match e {
        None => "#9ee6c8",
        Some(Element::Force) => "#f2e6cf",
        Some(Element::Metal) => "#dfe8f2",
        Some(Element::Wood) => "#7fdc6a",
        Some(Element::Water) => "#5fb8ff",
        Some(Element::Fire) => "#ff8a3c",
        Some(Element::Earth) => "#e0a95a",
        Some(Element::Thunder) => "#a58bff",
        Some(Element::Light) => "#fff0a0",
        Some(Element::Yin) => "#c45cff",
        Some(Element::Soul) => "#6ff0d8",
    })
}

/// What a fighter shouts casting skill `id` (a gem's id, as the rules name a cast).
pub fn shout(id: &str) -> Option<Shout> {
    let g = arena_calc::gem::gem(id)?;
    // the largest share; the first of equal ones
    let main = g
        .split
        .iter()
        .fold(None::<(Element, f64)>, |best, &(e, s)| match best {
            Some((_, b)) if b >= s => best,
            _ => Some((e, s)),
        })
        .map(|(e, _)| e);
    Some(Shout {
        name: format!("{}!", g.name),
        tint: tint(main),
    })
}

/// What was last seen of a fighter's cast, to tell a new one from the same one going on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Watch {
    last: Option<(&'static str, Phase, f64)>,
}

impl Watch {
    /// The fighter's act this frame; the skill when a new cast has begun since the last. A cast is new when the
    /// fighter was doing nothing or something else, or when it winds up again after recovering (the same skill cast
    /// twice) or from the start; a cast whose windup passed within one frame is caught in its recover.
    pub fn see(&mut self, act: Option<&ActView>) -> Option<&'static str> {
        let now = act.map(|a| (a.spell, a.phase, a.t));
        let new = match (self.last, now) {
            (_, None) => None,
            (None, Some((s, _, _))) => Some(s),
            (Some((was, _, _)), Some((s, _, _))) if was != s => Some(s),
            (Some((_, Phase::Recover, _)), Some((s, Phase::Windup, _))) => Some(s),
            (Some((_, Phase::Windup, t0)), Some((s, Phase::Windup, t))) if t < t0 => Some(s),
            _ => None,
        };
        self.last = now;
        new
    }
}

/// A shout on screen.
struct Live {
    shout: Shout,
    age: f32,
    /// Frames since its name was set (its text is laid out at the end of the frame it is set in).
    frames: u32,
}

#[derive(Resource, Default)]
struct Bubbles {
    /// The fight it answers.
    of: u32,
    watch: [Watch; 2],
    live: [Option<Live>; 2],
}

/// The name over fighter `0` or `1`.
#[derive(Component)]
struct ShoutText(usize);

pub struct BubblePlugin;

impl Plugin for BubblePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bubbles>()
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                watch.in_set(super::Felt).in_set(crate::Frame::React),
            )
            .add_systems(Update, paint.in_set(Overlay::Marks).before(super::marks));
    }
}

fn spawn(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let font = fonts.add(Font::from_bytes(FONT.to_vec()));
    for i in 0..2 {
        commands.spawn((
            Text2d::new(""),
            TextFont::from(font.clone()),
            TextColor(Color::NONE),
            Text2dShadow {
                offset: Vec2::new(0.0, -1.0),
                color: Color::BLACK,
            },
            Anchor::TOP_LEFT,
            Transform::from_xyz(0.0, 0.0, Z_FRONT + 1.0),
            // never hidden: a `Text2d` is laid out only while a camera sees it, and the box needs its size first
            Visibility::Visible,
            ShoutText(i),
        ));
    }
}

/// A new cast replaces its fighter's bubble; the bubbles age.
fn watch(
    shown: Res<Shown>,
    time: Res<Time>,
    mut bubbles: ResMut<Bubbles>,
    mut names: Query<(&ShoutText, &mut Text2d)>,
) {
    let b = bubbles.as_mut();
    if b.of != shown.fight {
        *b = Bubbles {
            of: shown.fight,
            ..default()
        };
    }
    let dt = time.delta_secs();
    for live in b.live.iter_mut() {
        if let Some(l) = live {
            l.age += dt;
            l.frames += 1;
            if l.age >= LIFE {
                *live = None;
            }
        }
    }
    let Some(view) = &shown.view else {
        return;
    };
    for (i, f) in view.fighters.iter().enumerate() {
        let Some(id) = b.watch[i].see(f.act.as_ref()) else {
            continue;
        };
        let Some(shout) = shout(id) else {
            continue;
        };
        for (ShoutText(n), mut text) in &mut names {
            if *n == i {
                text.0.clone_from(&shout.name);
            }
        }
        b.live[i] = Some(Live {
            shout,
            age: 0.0,
            frames: 0,
        });
    }
}

/// A bubble's box in world pixels (left, top, width, height without the tail) and where its tail points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// The tail's tip, under the box (`None`: no tail, the box stands beside the head).
    pub tail: Option<(i32, i32)>,
}

impl Place {
    fn right(&self) -> i32 {
        self.x + self.w
    }

    fn bottom(&self) -> i32 {
        self.y + self.h + if self.tail.is_some() { TAIL } else { 0 }
    }

    /// The tail under its box, its tip over the head as near as it can.
    fn settle_tail(&mut self) {
        let (x, w, bottom) = (self.x, self.w, self.y + self.h + TAIL);
        if let Some((tx, ty)) = &mut self.tail {
            *tx = (*tx).clamp(x + 3, (x + w - 4).max(x + 3));
            *ty = bottom;
        }
    }

    fn meets(&self, o: &Place) -> bool {
        self.x < o.right() + APART
            && o.x < self.right() + APART
            && self.y < o.bottom() + APART
            && o.y < self.bottom() + APART
    }
}

/// Where a box `w` x `h` goes over the head of a fighter standing at `x` with its feet at `feet`: centred over it,
/// its tail just over the head, inside the screen `screen_w` wide and under the HUD's bars (`top`); with no room
/// above, beside the head on the arena's side.
pub fn place(x: i32, feet: i32, w: i32, h: i32, screen_w: i32, top: i32) -> Place {
    let tip = feet - HEAD - GAP;
    let mut p = Place {
        x: x - w / 2,
        y: tip - TAIL - h,
        w,
        h,
        tail: Some((x, tip)),
    };
    if p.y < top {
        p.y = top;
        p.tail = None;
        p.x = if x < screen_w / 2 { x + 12 } else { x - 12 - w };
    }
    p.x = p.x.clamp(EDGE, (screen_w - EDGE - w).max(EDGE));
    p.settle_tail();
    p
}

/// Two bubbles kept from covering each other: pushed apart sideways (each away from the other, inside the screen),
/// and when the screen has no room for that, the newer one (`b`) raised over the other.
pub fn part(a: &mut Place, b: &mut Place, screen_w: i32, top: i32) {
    if !a.meets(b) {
        return;
    }
    let (l, r) = if a.x + a.w / 2 <= b.x + b.w / 2 {
        (&mut *a, &mut *b)
    } else {
        (&mut *b, &mut *a)
    };
    let need = l.right() + APART - r.x;
    let lx = (l.x - (need + 1) / 2).max(EDGE);
    let rx = (lx + l.w + APART).max(r.x).min(screen_w - EDGE - r.w);
    let lx = lx.min(rx - APART - l.w).max(EDGE);
    l.x = lx;
    r.x = rx;
    if a.meets(b) {
        b.y = (a.y - APART - b.h - if b.tail.is_some() { TAIL } else { 0 }).max(top);
        b.tail = b.tail.filter(|_| b.y + b.h < a.y);
    }
    a.settle_tail();
    b.settle_tail();
}

/// Paints bubble `p` on `canvas`: an ink box with rounded corners, a rim of `tint`, a dark outline and a tail down
/// to the head, `alpha` faded.
pub fn bubble(canvas: &mut Canvas, p: &Place, tint: Rgba, alpha: f32) {
    let tail = p.tail.map(|(tx, _)| tx - p.x);
    let (w, h) = (p.w, p.h + if tail.is_some() { TAIL } else { 0 });
    let inside = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= w || y >= h {
            return false;
        }
        if y < p.h {
            // the corners rounded by two pixels
            let cx = x.min(w - 1 - x);
            let cy = y.min(p.h - 1 - y);
            return cx + cy >= 2;
        }
        tail.is_some_and(|t| (x - t).abs() <= TAIL - 1 - (y - p.h))
    };
    let ink = mix(hex(INK), tint, 0.12);
    let line = hex(OUTLINE);
    for y in -1..h + 1 {
        for x in -1..w + 1 {
            let near = |r: i32| {
                (-r..=r).any(|dy| (-r..=r).any(|dx| (dx != 0 || dy != 0) && inside(x + dx, y + dy)))
            };
            let c = if inside(x, y) {
                let edge =
                    !(inside(x - 1, y) && inside(x + 1, y) && inside(x, y - 1) && inside(x, y + 1));
                if edge { tint } else { fade(ink, 0.92) }
            } else if near(1) {
                fade(line, 0.9)
            } else {
                continue;
            };
            canvas.put(p.x + x, p.y + y, fade(c, alpha), Blend::Over);
        }
    }
}

/// The text's size in world pixels, as laid out (`None` until it is).
fn measured(info: &TextLayoutInfo, scale: f32) -> Option<Vec2> {
    (info.size.x > 0.0).then(|| info.size / info.scale_factor.max(0.01) * scale)
}

/// Where fighter `f`'s head is: its x and its feet's row, in world pixels.
fn head(f: &FighterView, layout: &Layout) -> (i32, i32) {
    let x = Layout::world(f.x).round() as i32;
    let feet = (layout.ground - Layout::world(f.air)).round() as i32;
    (x, feet)
}

#[allow(clippy::type_complexity)]
fn paint(
    shown: Res<Shown>,
    layout: Res<Layout>,
    mut bubbles: ResMut<Bubbles>,
    mut canvases: ResMut<Canvases>,
    mut names: Query<(
        &ShoutText,
        &TextLayoutInfo,
        &mut TextFont,
        &mut TextColor,
        &mut Text2dShadow,
        &mut Transform,
    )>,
) {
    let Some(view) = &shown.view else {
        for (_, _, _, mut color, mut shadow, _) in &mut names {
            color.0 = Color::NONE;
            shadow.color = Color::NONE;
        }
        return;
    };
    // the name laid out at the screen's pixels, scaled down to the world's
    let ratio = layout.scale / layout.dpr.max(0.01);
    let scale = ratio.recip();
    let top = (layout.bar_top + BARS_TALL + 13.0) as i32;
    let screen_w = layout.w as i32;
    let mut places: [Option<Place>; 2] = [None, None];
    for (ShoutText(i), info, mut font, ..) in &mut names {
        let Some(live) = bubbles.live[*i].as_mut() else {
            continue;
        };
        let size = FontSize::Px((TEXT * ratio).round());
        if font.font_size != size {
            font.font_size = size;
            live.frames = 0;
        }
        if live.frames == 0 {
            continue;
        }
        let Some(text) = measured(info, scale) else {
            continue;
        };
        let (x, feet) = head(&view.fighters[*i], &layout);
        let (w, h) = (
            text.x.ceil() as i32 + 2 * PAD_X,
            text.y.ceil() as i32 + 2 * PAD_Y,
        );
        places[*i] = Some(place(x, feet, w, h, screen_w, top));
    }
    if let [Some(a), Some(b)] = &mut places {
        // the newer bubble gives way upward
        if bubbles.live[0].as_ref().map(|l| l.age) < bubbles.live[1].as_ref().map(|l| l.age) {
            part(b, a, screen_w, top);
        } else {
            part(a, b, screen_w, top);
        }
    }
    let (ox, oy) = ((layout.w % 2.0) / 2.0, (layout.h % 2.0) / 2.0);
    for (ShoutText(i), info, _, mut color, mut shadow, mut transform) in &mut names {
        let (Some(p), Some(live)) = (places[*i], bubbles.live[*i].as_ref()) else {
            // unseen, but where the camera looks, so it is laid out
            color.0 = Color::NONE;
            shadow.color = Color::NONE;
            transform.translation = Vec3::new(0.0, 0.0, Z_FRONT + 1.0);
            continue;
        };
        let alpha = ((LIFE - live.age) / FADE).clamp(0.0, 1.0);
        let s = &live.shout;
        bubble(&mut canvases.front, &p, s.tint, alpha);
        let ink = mix([255, 255, 255, 255], s.tint, 0.3);
        color.0 = Color::srgba_u8(ink[0], ink[1], ink[2], (alpha * 255.0) as u8);
        shadow.color = Color::srgba(0.0, 0.0, 0.0, alpha * 0.9);
        // half a world pixel down
        shadow.offset = Vec2::new(0.0, -0.5 * ratio);
        let text = measured(info, scale).unwrap_or_default();
        // the name centred in its box, on whole world pixels
        let tx = p.x as f32 + ((p.w as f32 - text.x) / 2.0).floor();
        let ty = p.y as f32 + ((p.h as f32 - text.y) / 2.0).floor();
        let at = layout.at(tx, ty);
        *transform =
            Transform::from_xyz(at.x + ox, at.y - oy, Z_FRONT + 1.0).with_scale(Vec3::splat(scale));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(spell: &'static str, phase: Phase, t: f64) -> ActView {
        ActView {
            spell,
            phase,
            t,
            span: 1.0,
        }
    }

    #[test]
    fn a_cast_is_seen_once() {
        let mut w = Watch::default();
        assert_eq!(w.see(None), None);
        assert_eq!(
            w.see(Some(&act("fireball", Phase::Windup, 0.0))),
            Some("fireball")
        );
        // the same windup frames later, then its recover: the same cast
        assert_eq!(w.see(Some(&act("fireball", Phase::Windup, 0.1))), None);
        assert_eq!(w.see(Some(&act("fireball", Phase::Windup, 0.2))), None);
        assert_eq!(w.see(Some(&act("fireball", Phase::Recover, 0.0))), None);
        assert_eq!(w.see(Some(&act("fireball", Phase::Recover, 0.1))), None);
        // cast again straight from its recover: a new cast
        assert_eq!(
            w.see(Some(&act("fireball", Phase::Windup, 0.0))),
            Some("fireball")
        );
        // another skill; then nothing; then a cast whose windup passed between two frames
        assert_eq!(
            w.see(Some(&act("slash", Phase::Windup, 0.05))),
            Some("slash")
        );
        assert_eq!(w.see(None), None);
        assert_eq!(w.see(None), None);
        assert_eq!(
            w.see(Some(&act("blink", Phase::Recover, 0.02))),
            Some("blink")
        );
        assert_eq!(w.see(Some(&act("blink", Phase::Recover, 0.04))), None);
    }

    #[test]
    fn a_skill_shouts_its_own_name() {
        let name = |id: &str| shout(id).map(|s| s.name);
        assert_eq!(
            shout("fireball"),
            Some(Shout {
                name: "Hỏa Cầu!".into(),
                tint: tint(Some(Element::Fire)),
            })
        );
        // Trăm Quyền Sinh Cơ moves as the basic slash (Chém), and still shouts its own name
        assert_eq!(
            arena_calc::gem::gem("hundredfist").map(|g| g.spell),
            Some("slash")
        );
        assert_eq!(name("hundredfist").as_deref(), Some("Trăm Quyền Sinh Cơ!"));
        // a top-tier skill borrowing the tribulation's motion
        assert_eq!(name("remnantdusk").as_deref(), Some("Tàn Dạ!"));
        // the basics shout too
        assert_eq!(name("slash").as_deref(), Some("Chém!"));
        assert_eq!(name("dive").as_deref(), Some("Lao Kiếm!"));
        assert_eq!(name("blink").as_deref(), Some("Độn!"));
        assert_eq!(shout("blink").map(|s| s.tint), Some(tint(None)));
        assert_eq!(shout("nothing"), None);
        // every skill of the pool has words
        for g in arena_calc::gem::GEMS {
            assert!(shout(g.id).is_some(), "{}", g.id);
        }
    }

    #[test]
    fn the_font_writes_vietnamese() {
        // every letter of every name has a glyph in the font built in
        let face = ttf_cmap(FONT);
        for g in arena_calc::gem::GEMS {
            for c in g.name.chars().chain(['!']) {
                assert!(face.contains(&(c as u32)), "{} in {}", c, g.name);
            }
        }
    }

    /// The characters a TrueType font maps (its cmap's format 4 table).
    fn ttf_cmap(font: &[u8]) -> Vec<u32> {
        let u16_at = |o: usize| u16::from_be_bytes([font[o], font[o + 1]]) as usize;
        let u32_at = |o: usize| {
            u32::from_be_bytes([font[o], font[o + 1], font[o + 2], font[o + 3]]) as usize
        };
        let tables = u16_at(4);
        let mut out = Vec::new();
        let Some(cmap) = (0..tables)
            .map(|i| 12 + 16 * i)
            .find(|&r| &font[r..r + 4] == b"cmap")
            .map(|r| u32_at(r + 8))
        else {
            return out;
        };
        for i in 0..u16_at(cmap + 2) {
            let sub = cmap + u32_at(cmap + 4 + 8 * i + 4);
            if u16_at(sub) != 4 {
                continue;
            }
            let segs = u16_at(sub + 6) / 2;
            let ends = sub + 14;
            let starts = ends + 2 * segs + 2;
            for s in 0..segs {
                let (a, b) = (u16_at(starts + 2 * s), u16_at(ends + 2 * s));
                if a != 0xffff {
                    out.extend(a as u32..=b as u32);
                }
            }
        }
        out
    }

    #[test]
    fn bubbles_stay_on_screen_and_apart() {
        // over the head, its tail under it
        let p = place(160, 158, 40, 12, 320, 45);
        assert_eq!(p.x, 140);
        assert_eq!(p.y + p.h + TAIL, 158 - HEAD - GAP);
        assert_eq!(p.tail, Some((160, 158 - HEAD - GAP)));
        // at a wall, inside the screen with the tail still over the head
        let mut p = place(4, 158, 40, 12, 320, 45);
        assert_eq!(p.x, EDGE);
        let mut q = place(316, 158, 40, 12, 320, 45);
        assert_eq!(q.right(), 320 - EDGE);
        part(&mut p, &mut q, 320, 45);
        assert_eq!(p.tail.map(|t| t.0), Some(4.max(p.x + 3)));
        // close together: pushed apart, neither covering the other
        let mut a = place(150, 158, 60, 12, 320, 45);
        let mut b = place(170, 158, 60, 12, 320, 45);
        assert!(a.meets(&b));
        part(&mut a, &mut b, 320, 45);
        assert!(!a.meets(&b));
        assert!(a.x >= EDGE && b.right() <= 320 - EDGE);
        // a screen too narrow for both side by side: the newer one goes up
        let mut a = place(60, 158, 100, 12, 128, 20);
        let mut b = place(70, 158, 100, 12, 128, 20);
        part(&mut a, &mut b, 128, 20);
        assert!(!a.meets(&b));
        // flying high: under the bars, beside the head, no tail
        let p = place(100, 80, 40, 12, 320, 45);
        assert_eq!(p.y, 45);
        assert!(p.tail.is_none());
        assert!(p.x >= 100);
    }
}

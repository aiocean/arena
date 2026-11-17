//! The light the duel throws on its world (the page's `duel-light.js` over the engine's `pixfx-light`): each spell
//! lights the dais, the peaks and the fighters around it in its school's colour while it plays, a blow flashes where
//! it lands, and the braziers glow and flicker. A light swells in fast and fades out slow, aged by the fight's time,
//! and shows as hard-edged rings, one pixel per world pixel, added over `back`; over `front` (the fighters and the
//! effects over them) it goes on faintly, as a tint of its colour (a picture laid over others cannot add).

use crate::feel::Overlay;
use crate::feel::math::{js_min, js_round, sin, sqrt};
use crate::layers::Canvases;
use crate::layout::Layout;
use crate::raster::{Blend, Rgba, hex};
use crate::rules::{Fx, Pace, Shown};
use bevy::prelude::*;
use pixfx_duel::out::Out;

/// How far an effect's light reaches, in world pixels; those not listed reach `REACH_DEFAULT`.
const REACH: [(&str, f64); 10] = [
    ("explosions/plasma-explosion", 70.0),
    ("explosions/implosion", 60.0),
    ("explosions/dirt-explosion", 46.0),
    ("explosions/small-explosion", 40.0),
    ("holy/feather-burst", 54.0),
    ("fire/spirit-flame", 40.0),
    ("arcane/rune-circle", 34.0),
    ("impacts/parry", 30.0),
    ("spawn/poof", 28.0),
    // a trail rides on a fighter: a faint glow
    ("projectiles/trail", 12.0),
];
const REACH_DEFAULT: f64 = 36.0;
/// A one-shot's light lasts this long; a looping effect's until it stops, then goes out over `OUT`.
const FLASH: f64 = 0.5;
const OUT: f64 = 0.2;
/// A blow's own flash: reach, life, colour.
const HIT: (f64, f64, &str) = (30.0, 0.16, "#fff4dc");
/// The braziers' warm light: reach, colour, strength.
const BRAZIER: (f64, &str, f64) = (64.0, "#ff9a3c", 0.32);
/// How bright a light is at its heart, 0 to 1.
const STRENGTH: f64 = 0.42;
/// How much of the light goes on over the fighters.
const FRONT: f64 = 0.2;

/// The rings a glow is drawn in, from the heart out: (out to this share of its reach, this share of its strength).
const RINGS: [(f64, f64); 4] = [(0.3, 1.0), (0.55, 0.6), (0.8, 0.3), (1.0, 0.12)];
/// A light with a life swells to full in `1 / RISE` seconds, then fades to nothing at its end; one held until put
/// out swells in `1 / RISE_HELD` and holds. Its reach swells from `REACH_BASE` of itself to all of it.
const RISE: f64 = 12.0;
const RISE_HELD: f64 = 8.0;
const REACH_BASE: f64 = 0.8;
const FAINT: f64 = 0.01;

/// A light in the world: where, how far it reaches, how bright, how long it lives and has lived.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub key: Option<u32>,
    pub x: f64,
    pub y: f64,
    pub reach: f64,
    pub color: Rgba,
    pub strength: f64,
    /// `None` until it is put out.
    pub life: Option<f64>,
    pub age: f64,
}

/// A light as it shows at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    pub x: f64,
    pub y: f64,
    pub reach: f64,
    pub color: Rgba,
    pub strength: f64,
}

impl Light {
    fn put_out(self, within: f64) -> Light {
        let end = self.age + within;
        Light {
            life: Some(self.life.map_or(end, |l| js_min(l, end))),
            ..self
        }
    }

    pub fn glow(&self) -> Glow {
        let k = match self.life {
            None => js_min(1.0, self.age * RISE_HELD),
            Some(life) => js_min(1.0, self.age * RISE) * (1.0 - self.age / life),
        };
        Glow {
            x: self.x,
            y: self.y,
            reach: self.reach * (REACH_BASE + (1.0 - REACH_BASE) * k),
            color: self.color,
            strength: self.strength * k,
        }
    }
}

/// The lights of the fight now playing.
#[derive(Resource, Default)]
pub struct Lights {
    pub list: Vec<Light>,
    fight: u32,
}

pub struct LightPlugin;

impl Plugin for LightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lights>()
            .add_systems(Update, kindle.in_set(crate::Frame::React))
            .add_systems(Update, paint.in_set(Overlay::Light));
    }
}

/// The step's commands change the lights (a spell with a palette lights its spot, a move carries its light, a stop
/// puts it out, a blow or a turned blow flashes), then every light ages by the fight's time.
fn kindle(
    mut fx: MessageReader<Fx>,
    mut lights: ResMut<Lights>,
    shown: Res<Shown>,
    layout: Res<Layout>,
    pace: Res<Pace>,
) {
    if lights.fight != shown.fight {
        *lights = Lights {
            list: Vec::new(),
            fight: shown.fight,
        };
    }
    let w = |v: f64| f64::from(Layout::world(v));
    let ground = f64::from(layout.ground);
    let list = &mut lights.list;
    for Fx(out) in fx.read() {
        match out {
            Out::Play {
                key,
                fx,
                x,
                y,
                opts,
            } => {
                let Some(ramp) = opts.palette.filter(|r| !r.is_empty()) else {
                    continue;
                };
                let reach = REACH
                    .iter()
                    .find(|(id, _)| id == fx)
                    .map_or(REACH_DEFAULT, |(_, r)| *r);
                let color = hex(ramp[((ramp.len() as f64 * 0.7) as usize).min(ramp.len() - 1)]);
                list.push(Light {
                    key: Some(*key),
                    x: w(*x),
                    y: w(*y),
                    reach,
                    color,
                    strength: STRENGTH,
                    life: (!pixfx_duel::effects::loops(fx)).then_some(FLASH),
                    age: 0.0,
                });
            }
            Out::Move { key, x, y } => {
                for l in list.iter_mut().filter(|l| l.key == Some(*key)) {
                    l.x = w(*x);
                    l.y = w(*y);
                }
            }
            Out::Stop { key } => {
                for l in list.iter_mut().filter(|l| l.key == Some(*key)) {
                    *l = l.put_out(OUT);
                }
            }
            Out::Hit { x, y, .. } | Out::Block { x, y, .. } => list.push(Light {
                key: None,
                x: w(*x),
                y: ground - w(*y),
                reach: HIT.0,
                color: hex(HIT.2),
                strength: STRENGTH,
                life: Some(HIT.1),
                age: 0.0,
            }),
            _ => {}
        }
    }
    let dt = pace.run;
    list.retain_mut(|l| {
        l.age += dt;
        l.life.is_none_or(|life| l.age < life)
    });
}

/// The braziers' glow at `time`: two slow waves and a fast one, out of step from lamp to lamp.
pub fn lamp_glows(lamps: [f32; 2], ground: f32, time: f64) -> [Glow; 2] {
    let glow = |i: usize| {
        let i_f = i as f64;
        let flicker = 0.85 + 0.1 * sin(time * 7.0 + i_f * 2.0) + 0.05 * sin(time * 23.0 + i_f);
        Glow {
            x: f64::from(lamps[i]),
            y: f64::from(ground) - 18.0,
            reach: BRAZIER.0 * flicker,
            color: hex(BRAZIER.1),
            strength: BRAZIER.2 * flicker,
        }
    };
    [glow(0), glow(1)]
}

/// What a canvas fill of channel `v` at alpha `alpha` adds in "lighter" mode, to 8 bits.
fn lighter(v: u8, alpha: f64) -> u16 {
    let a = alpha.clamp(0.0, 1.0) as f32;
    ((f32::from(v) / 255.0) * a * 255.0 + 0.5) as u16
}

/// Adds glow `g` to the light layer `layer` (`w` wide) in its rings: a disc of whole pixels each, the widest first.
fn ring_glow(layer: &mut [[u16; 3]], w: i32, h: i32, g: &Glow) {
    let finite = [g.x, g.y, g.reach, g.strength]
        .iter()
        .all(|v| v.is_finite());
    if !finite || g.strength <= FAINT || g.reach < 1.0 {
        return;
    }
    let (cx, cy) = (js_round(g.x) as i32, js_round(g.y) as i32);
    for (i, &(out, bright)) in RINGS.iter().enumerate().rev() {
        let next = RINGS.get(i + 1).map_or(0.0, |r| r.1);
        let alpha = g.strength * (bright - next);
        let add = [0, 1, 2].map(|c| lighter(g.color[c], alpha));
        if add == [0; 3] {
            continue;
        }
        let r = js_round(g.reach * out) as i32;
        for y in -r..=r {
            let yy = cy + y;
            if yy < 0 || yy >= h {
                continue;
            }
            let half = js_round(sqrt(f64::from(r * r - y * y))) as i32;
            for x in (cx - half).max(0)..=(cx + half).min(w - 1) {
                let p = &mut layer[(yy * w + x) as usize];
                for c in 0..3 {
                    p[c] = (p[c] + add[c]).min(255);
                }
            }
        }
    }
}

/// The braziers' glow and every light, added over `back` and faintly over `front`.
fn paint(
    lights: Res<Lights>,
    shown: Res<Shown>,
    layout: Res<Layout>,
    mut canvases: ResMut<Canvases>,
) {
    let c = canvases.as_mut();
    let (w, h) = (c.back.w, c.back.h);
    let time = shown.view.as_ref().map_or(0.0, |v| v.time);
    let mut layer = vec![[0u16; 3]; (w * h) as usize];
    let glows = lamp_glows(layout.lamps, layout.ground, time)
        .into_iter()
        .chain(lights.list.iter().map(Light::glow));
    for g in glows {
        ring_glow(&mut layer, w, h, &g);
    }
    for (i, l) in layer.iter().enumerate() {
        if *l == [0; 3] {
            continue;
        }
        let (x, y) = (i as i32 % w, i as i32 / w);
        let m = l[0].max(l[1]).max(l[2]);
        c.back
            .put(x, y, [l[0] as u8, l[1] as u8, l[2] as u8, 255], Blend::Add);
        let tint = [0, 1, 2].map(|k| (u32::from(l[k]) * 255 / u32::from(m)) as u8);
        let a = (f64::from(m) * FRONT).min(255.0) as u8;
        c.front
            .put(x, y, [tint[0], tint[1], tint[2], a], Blend::Over);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flash_swells_and_fades_while_a_held_light_holds_until_put_out() {
        let spark = |life| Light {
            key: Some(1),
            x: 0.0,
            y: 0.0,
            reach: 30.0,
            color: [255, 128, 0, 255],
            strength: 0.42,
            life,
            age: 0.25,
        };
        assert_eq!(spark(Some(0.5)).glow().strength, 0.42 * 0.5);
        assert_eq!(spark(None).glow().strength, 0.42);
        let out = spark(None).put_out(0.2);
        assert_eq!(out.life, Some(0.45));
    }

    #[test]
    fn rings_add_more_light_towards_the_heart_and_nothing_past_the_reach() {
        let mut layer = vec![[0u16; 3]; 40];
        let g = Glow {
            x: 20.0,
            y: 0.0,
            reach: 10.0,
            color: [255, 255, 255, 255],
            strength: 1.0,
        };
        ring_glow(&mut layer, 40, 1, &g);
        let red = |x: usize| layer[x + 20][0];
        assert_eq!([red(0), red(9), red(10), red(11)], [255, 31, 31, 0]);
    }
}

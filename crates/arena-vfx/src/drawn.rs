//! The spell effects drawn by an image model, smooth, and moved by code (`art/vfx/`, the owner's hybrid rule): each
//! one's sheet holds only the shapes it was drawn as, a few frames a school (`thunder`, `frost`), and its `Motion` says
//! what code lays on them: a turn the whole time, a scale-in, a flight from where it forms to where the rules play it,
//! a held or looping middle, a tail and an exit. The sheets and their layouts are written by
//! `art/vfx/tools/vfx.rs --all` into `crates/arena-game/assets/drawn/` (`drawn.json`, every effect's layout in one
//! file); the game plays an effect from here when it has one, and from its painted recipe otherwise.
//!
//! Lengths are world pixels and the anchor's frame (y down), facing east; angles clockwise; times seconds.

use serde::Deserialize;
use std::collections::BTreeMap;

/// One frame's box on a page, and where its top-left sits from the anchor (the point the rules play it at).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct Cell {
    #[serde(default)]
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub dx: i32,
    pub dy: i32,
}

/// A scale-in: the effect starts `from` times its size and reaches it over `secs`, slowing as it arrives.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grow {
    pub from: f32,
    pub secs: f32,
}

/// A flight: the frames before `hold` play where the effect forms, `from`; then frame `hold` flies to the rules'
/// point over `secs`, slowing as it arrives; then the rest play there.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fly {
    pub from: [f32; 2],
    pub secs: f32,
    pub hold: usize,
}

/// An idle: frames `from..=to` play round and round for `secs` (a shield holding, a seal glowing) before the rest.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Idle {
    pub from: usize,
    pub to: usize,
    pub secs: f32,
}

/// The tail it drags while it moves: as long as `secs` of its way, narrowing to `taper` of its size at the end (0 a
/// point), turning clear from `soft` along it; `breakup` is the dither the pixel engine ate it with (unused here).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tail {
    #[serde(default = "tail_secs")]
    pub secs: f32,
    #[serde(default = "one")]
    pub taper: f32,
    #[serde(default = "one")]
    pub soft: f32,
    #[serde(default)]
    pub breakup: f32,
}

fn tail_secs() -> f32 {
    0.12
}

fn one() -> f32 {
    1.0
}

/// How an effect moves, beside its frames. `px` is the world pixels one sheet pixel covers (the sheets are drawn
/// finer than the world, smooth); `turns` a second, clockwise, from its first frame to its last; `exit`, the way it
/// leaves once played out or stopped (`disappear/fade`: it swells and fades out); `ground`, a seal that lies on the
/// floor, drawn there under the point it is played at even when that point is in the air.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    #[serde(default)]
    pub ground: bool,
    #[serde(default)]
    pub px: Option<f32>,
    #[serde(default)]
    pub grow: Option<Grow>,
    #[serde(default)]
    pub turns: f32,
    #[serde(default)]
    pub fly: Option<Fly>,
    #[serde(default)]
    pub idle: Option<Idle>,
    #[serde(default)]
    pub tail: Option<Tail>,
    #[serde(default)]
    pub exit: Option<String>,
}

/// A sheet's layout: its pages' sizes, frames a second, whether it loops, each school's frames, how it moves.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub pages: Vec<[u32; 2]>,
    pub fps: f32,
    #[serde(default)]
    pub looping: bool,
    pub clips: BTreeMap<String, Vec<Cell>>,
    #[serde(default)]
    pub motion: Motion,
}

/// One drawn effect: the id the rules play, its sheet's file name (`<file>.webp`, then `<file>-1.webp` and on) and
/// its layout.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Drawn {
    pub fx: String,
    pub file: String,
    pub layout: Layout,
}

/// Every drawn effect, from `drawn.json`.
pub fn read(json: &str) -> Result<Vec<Drawn>, String> {
    serde_json::from_str(json).map_err(|e| format!("drawn.json: {e}"))
}

/// The seconds `disappear/fade` takes, and how much it swells by its end.
pub const FADE_SECS: f32 = 0.45;
pub const FADE_GROW: f32 = 0.25;

/// What an effect shows at a moment: frame `frame` of its school's clip, turned `angle` radians clockwise about its
/// anchor, `size` times its size (its scale-in), the anchor `at` world pixels from the rules' point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub frame: usize,
    pub cell: Cell,
    pub angle: f32,
    pub size: f32,
    pub at: (f32, f32),
}

/// Which school's frames an effect played in `palette` shows: frost for a blue palette, thunder otherwise (the rules
/// give each fighter's effects its look's colours).
pub fn school_of(palette: Option<&[&str]>) -> &'static str {
    let Some(p) = palette.filter(|p| !p.is_empty()) else {
        return "thunder";
    };
    let blue = p
        .iter()
        .filter_map(|c| u32::from_str_radix(c.trim_start_matches('#'), 16).ok())
        .map(|v| ((v & 255) as i32) - ((v >> 16) as i32))
        .sum::<i32>();
    if blue > 0 { "frost" } else { "thunder" }
}

impl Drawn {
    /// The clip `school` plays: its own, else any school's, else the first.
    pub fn clip(&self, school: &str) -> Option<&Vec<Cell>> {
        let clips = &self.layout.clips;
        clips
            .get(school)
            .or_else(|| clips.get("any"))
            .or_else(|| clips.values().next())
    }

    /// World pixels a sheet pixel at full size.
    pub fn px(&self) -> f32 {
        self.layout.motion.px.unwrap_or(1.0)
    }

    /// The beats of a one-shot played through, over `n` frames: (first frame, last frame, seconds, flying).
    fn beats(&self, n: usize) -> Vec<(usize, usize, f32, bool)> {
        let m = &self.layout.motion;
        let step = 1.0 / self.layout.fps.max(1e-3);
        let hold = m.fly.map(|f| f.hold.min(n.saturating_sub(1)));
        let mut out = Vec::new();
        let mut i = 0;
        while i < n {
            match (hold, m.idle) {
                (Some(h), _) if h == i => out.push((i, i, m.fly.map_or(0.0, |f| f.secs), true)),
                (_, Some(idle)) if idle.from == i && idle.to >= i && idle.to < n => {
                    out.push((idle.from, idle.to, idle.secs, false));
                    i = idle.to;
                }
                _ => out.push((i, i, step, false)),
            }
            i += 1;
        }
        out
    }

    /// Seconds a one-shot plays for `school` before it exits; none for a loop.
    pub fn life(&self, school: &str) -> Option<f32> {
        let n = self.clip(school)?.len();
        if self.layout.looping || n == 0 {
            return None;
        }
        Some(self.beats(n).iter().map(|b| b.2).sum())
    }

    /// How far it has grown in at `t`.
    fn grown(&self, t: f32) -> f32 {
        let Some(g) = self.layout.motion.grow else {
            return 1.0;
        };
        let u = (t / g.secs.max(1e-6)).clamp(0.0, 1.0);
        g.from + (1.0 - g.from) * (1.0 - (1.0 - u) * (1.0 - u))
    }

    fn angle(&self, t: f32) -> f32 {
        self.layout.motion.turns * core::f32::consts::TAU * t
    }

    /// What it shows `t` seconds after it began, for `school`: none once a one-shot has played out (see
    /// [`Drawn::last`] for its exit).
    pub fn shot(&self, school: &str, t: f32) -> Option<Shot> {
        let cells = self.clip(school)?;
        let n = cells.len();
        let t = t.max(0.0);
        let fps = self.layout.fps;
        let (angle, size) = (self.angle(t), self.grown(t));
        let frame = |i: usize, at| {
            cells.get(i).map(|&cell| Shot {
                frame: i,
                cell,
                angle,
                size,
                at,
            })
        };
        if self.layout.looping {
            return frame((t * fps).floor() as usize % n.max(1), (0.0, 0.0));
        }
        let m = &self.layout.motion;
        let from = m.fly.map_or((0.0, 0.0), |f| (f.from[0], f.from[1]));
        let mut start = 0.0;
        let mut flown = m.fly.is_none();
        for (first, last, secs, flying) in self.beats(n) {
            if t < start + secs {
                if flying {
                    // easing out: fast from the hand, slowing as it arrives
                    let u = (t - start) / secs.max(1e-6);
                    let k = 1.0 - (1.0 - u) * (1.0 - u);
                    return frame(first, (from.0 * (1.0 - k), from.1 * (1.0 - k)));
                }
                let at = if flown { (0.0, 0.0) } else { from };
                let i = first + ((t - start) * fps).floor() as usize % (last - first + 1);
                return frame(i, at);
            }
            flown |= flying;
            start += secs;
        }
        None
    }

    /// The frame it leaves on, `t` seconds after it began, still turning: a one-shot's last frame where it ended, a
    /// stopped loop's frame at `t`.
    pub fn last(&self, school: &str, t: f32) -> Option<Shot> {
        if self.layout.looping {
            return self.shot(school, t);
        }
        let cells = self.clip(school)?;
        let i = cells.len().checked_sub(1)?;
        Some(Shot {
            frame: i,
            cell: cells[i],
            angle: self.angle(t),
            size: 1.0,
            at: (0.0, 0.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thunder's two 1x1 frames and frost's one, at 10 fps, with `motion`.
    fn drawn(looping: bool, motion: &str) -> Result<Drawn, String> {
        let cell = |x: u32| format!(r#"{{"x":{x},"y":0,"w":1,"h":1,"dx":0,"dy":-1}}"#);
        let json = format!(
            r#"[{{"fx":"t/t","file":"t-t","layout":{{"pages":[[3,1]],"fps":10,"looping":{looping},"clips":{{"thunder":[{},{}],"frost":[{}]}},"motion":{{{motion}}}}}}}]"#,
            cell(0),
            cell(1),
            cell(2)
        );
        read(&json)?
            .into_iter()
            .next()
            .ok_or_else(|| "empty".to_owned())
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn a_school_plays_its_own_frames() -> Result<(), String> {
        let d = drawn(false, "")?;
        assert_eq!(d.shot("thunder", 0.15).map(|s| s.cell.x), Some(1));
        assert_eq!(d.shot("frost", 0.0).map(|s| s.cell.x), Some(2));
        assert_eq!(d.shot("thunder", 0.25), None, "played out");
        assert_eq!(
            d.shot("other", 0.0).map(|s| s.cell.x),
            Some(2),
            "the first clip by name"
        );
        Ok(())
    }

    #[test]
    fn it_forms_flies_holding_a_frame_and_turns_the_whole_time() -> Result<(), String> {
        let d = drawn(
            false,
            r#""turns":1,"fly":{"from":[-10,0],"secs":0.2,"hold":1}"#,
        )?;
        assert!(d.life("thunder").is_some_and(|l| near(l, 0.3)));
        let at = |t| d.shot("thunder", t).map(|s| (s.cell.x, s.at.0));
        assert_eq!(at(0.05), Some((0, -10.0)), "forming at the start");
        assert!(
            at(0.2).is_some_and(|(x, ax)| x == 1 && ax > -10.0 && ax < 0.0),
            "on its way"
        );
        assert!(
            d.shot("thunder", 0.25)
                .is_some_and(|s| near(s.angle, core::f32::consts::FRAC_PI_2))
        );
        assert_eq!(at(0.31), None);
        Ok(())
    }

    #[test]
    fn an_idle_plays_round_and_a_scale_in_grows() -> Result<(), String> {
        let d = drawn(
            false,
            r#""grow":{"from":0.5,"secs":0.1},"idle":{"from":0,"to":0,"secs":0.5}"#,
        )?;
        assert!(d.life("thunder").is_some_and(|l| near(l, 0.6)));
        let x = |t| d.shot("thunder", t).map(|s| s.cell.x);
        assert_eq!((x(0.45), x(0.55)), (Some(0), Some(1)));
        assert!(d.shot("thunder", 0.0).is_some_and(|s| near(s.size, 0.5)));
        assert!(d.shot("thunder", 0.2).is_some_and(|s| near(s.size, 1.0)));
        Ok(())
    }

    #[test]
    fn a_loop_goes_round_and_leaves_on_its_frame_then() -> Result<(), String> {
        let d = drawn(true, r#""turns":1"#)?;
        assert_eq!(d.life("thunder"), None);
        assert_eq!(d.shot("thunder", 0.25).map(|s| s.cell.x), Some(0));
        assert_eq!(d.last("thunder", 0.15).map(|s| s.cell.x), Some(1));
        let one = drawn(false, r#""turns":1"#)?;
        assert!(
            one.last("thunder", 0.5)
                .is_some_and(|s| s.cell.x == 1 && near(s.angle, core::f32::consts::PI))
        );
        Ok(())
    }

    #[test]
    fn a_blue_palette_is_frost() {
        assert_eq!(school_of(Some(&["#2c58aa", "#62c8f4"])), "frost");
        assert_eq!(school_of(Some(&["#b07a1a", "#ffd84a"])), "thunder");
        assert_eq!(school_of(None), "thunder");
    }

    #[test]
    fn the_game_s_sheets_read() -> Result<(), String> {
        let all = read(include_str!("../../arena-game/assets/drawn/drawn.json"))?;
        assert!(all.len() >= 30);
        for d in &all {
            assert!(
                d.clip("thunder").is_some_and(|c| !c.is_empty()),
                "{}: no frames",
                d.fx
            );
            assert!(d.layout.fps > 0.0, "{}", d.fx);
        }
        Ok(())
    }
}

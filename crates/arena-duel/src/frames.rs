//! A fighter's boxes by the frame it shows: frame data tied to the clips, as a fighting game keeps it.
//!
//! Each look's every clip and frame carries the boxes measured on its sprite (`frames_data`, generated offline by
//! `cargo run -p arena-game --example frame_boxes`): one to three **hurtboxes** (the head and torso, the legs, an arm
//! held out), on a blade clip's active frame the **hitbox** where the hand and blade are, and on a cast's active frame
//! the **hand** a bolt leaves from. The rules pick the frame a fighter shows as the game does (`pick`, from its pose,
//! how long it has held it and the cast it gathers), so a fighter is struck where it is drawn: crouched, its boxes
//! are low; jumping, they are where its body is in the air; striking, its arm is out and can be hit.
//!
//! Boxes are [x0, y0, x1, y1] in the rules' pixels from the feet, facing right; `place` puts one in the arena.

use crate::data::Look;
use crate::fighter::State;
use crate::frames_data::LOOKS;
use crate::pick::{appearance, pick};

/// One frame's boxes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub hurt: &'static [[f64; 4]],
    pub hit: Option<[f64; 4]>,
    pub hand: Option<[f64; 2]>,
    /// On a clip that strikes or casts, the hand held out ahead: its wrist (the arm's hurtbox ends there) and its far
    /// side (a blade's hitbox starts there, the hilt; a palm's fingertips), tracked on the hand's skin.
    pub wrist: Option<[f64; 2]>,
    pub hilt: Option<[f64; 2]>,
}

/// One look's clips: the frames a second they play at, and each clip's active window (its first frame is where a
/// blade or a casting hand reaches farthest ahead, the frame the rules strike or cast on; a blade's window runs on
/// while the blade stays out ahead) and frames in order.
#[derive(Debug)]
pub struct LookFrames {
    pub fps: f64,
    pub clips: &'static [Clip],
}

/// A clip's first and last active frames.
pub type Window = Option<(usize, usize)>;
/// A clip: its name, its active window, its frames.
pub type Clip = (&'static str, Window, &'static [Frame]);

fn look_frames(look: Look) -> &'static LookFrames {
    match look {
        Look::Thunder => &LOOKS[0],
        Look::Frost => &LOOKS[1],
    }
}

/// Clip `clip` of look `look`: its active window and its frames.
fn clip_of(look: Look, clip: &str) -> Option<(Window, &'static [Frame])> {
    look_frames(look)
        .clips
        .iter()
        .find(|(n, _, _)| *n == clip)
        .map(|(_, a, fr)| (*a, *fr))
}

/// How many frames clip `clip` of look `look` has (0 when it has none).
pub fn len(look: Look, clip: &str) -> usize {
    clip_of(look, clip).map_or(0, |(_, fr)| fr.len())
}

/// The active frame of clip `clip` of look `look`: where its wind-up ends and the blow or the cast goes out.
pub fn active(look: Look, clip: &str) -> Option<usize> {
    clip_of(look, clip)
        .and_then(|(a, _)| a)
        .map(|(start, _)| start)
}

/// The frames a second look `look`'s clips play at.
pub fn fps(look: Look) -> f64 {
    look_frames(look).fps
}

/// The clip and frame fighter `f` shows now, as `pick` chooses them (and the game draws them).
fn shown(state: &State, f: usize) -> Option<(&'static str, usize)> {
    let fi = state.fighters.get(f)?;
    let b = &fi.body;
    let a = appearance(
        b.pose,
        b.pose_t,
        !fi.alive(),
        fi.act
            .map(|a| (a.spell, a.phase, a.t, crate::act::phase_span(&a))),
    );
    let shot = pick(
        &a,
        fps(fi.look),
        |c| len(fi.look, c),
        |c| active(fi.look, c),
    );
    Some((shot.clip, shot.frame))
}

/// The frame fighter `f` shows now.
pub fn frame_of(state: &State, f: usize) -> Option<&'static Frame> {
    let (clip, i) = shown(state, f)?;
    clip_of(state.fighters.get(f)?.look, clip)?.1.get(i)
}

/// Whether fighter `f` shows a frame of its blade blow's active window now: a blow that has not landed may still.
pub fn in_window(state: &State, f: usize) -> bool {
    let (Some((clip, i)), Some(fi)) = (shown(state, f), state.fighters.get(f)) else {
        return false;
    };
    act_clip(state, f) == Some(clip)
        && clip_of(fi.look, clip)
            .and_then(|(w, _)| w)
            .is_some_and(|(start, end)| i >= start && i <= end)
}

/// Box `r` (from the feet, facing right) of a fighter standing at `x`, its feet at `air`, facing `face`.
pub fn place(r: [f64; 4], x: f64, air: f64, face: f64) -> [f64; 4] {
    if face < 0.0 {
        [x - r[2], air + r[1], x - r[0], air + r[3]]
    } else {
        [x + r[0], air + r[1], x + r[2], air + r[3]]
    }
}

/// Fighter `f`'s hurtboxes in the arena: its frame's, or its body's box when the frame has none.
pub fn hurtboxes(state: &State, f: usize) -> Vec<[f64; 4]> {
    let Some(fi) = state.fighters.get(f) else {
        return Vec::new();
    };
    let b = &fi.body;
    match frame_of(state, f) {
        Some(fr) if !fr.hurt.is_empty() => fr
            .hurt
            .iter()
            .map(|r| place(*r, b.x, b.air, b.face))
            .collect(),
        _ => {
            let top = if b.duck > 0.0 {
                b.spec.duck
            } else {
                b.spec.body
            };
            vec![[
                b.x - b.spec.half_width,
                b.air,
                b.x + b.spec.half_width,
                b.air + top,
            ]]
        }
    }
}

/// Do boxes `a` and `b` overlap?
pub fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// The clip fighter `f`'s cast plays (a blade blow's `slash` or `overhead`, a spell's `cast` or `dash`), if it casts.
fn act_clip(state: &State, f: usize) -> Option<&'static str> {
    let a = state.fighters.get(f)?.act?;
    crate::data::spell(a.spell).map(clip_for)
}

/// The clip a cast of `s` plays: a blow from the palm off the air is the `overhead` cut, a lunge the `dash`, any
/// other blade the `slash`, any other spell the `cast`.
pub fn clip_for(s: &crate::data::Spell) -> &'static str {
    match (crate::pose::gather_of(s.kind), s.dive) {
        (crate::pose::Gather::Palm, true) => "overhead",
        (crate::pose::Gather::Lunge, _) => "dash",
        _ if s.kind == crate::data::Kind::Melee => "slash",
        _ => "cast",
    }
}

/// When clip `clip` of look `look`'s active window opens and how long it stays open, in seconds, for a cast of
/// `windup` and `recover`: it opens as the wind-up ends on the active frame, and stays open while the recovery's
/// progress shows the window's frames (`pick` plays the frames from the active one to the last over the recovery,
/// each the nearest to its progress), the last of them to half a frame past it. `None` for a clip with no window.
pub fn window_of(look: Look, clip: &str, windup: f64, recover: f64) -> Option<(f64, f64)> {
    let (window, frames) = clip_of(look, clip)?;
    let (first, end) = window?;
    let last = frames.len().saturating_sub(1);
    let first = first.min(last);
    let span = last - first;
    let share = if span == 0 {
        1.0
    } else {
        ((end.clamp(first, last) - first) as f64 + 0.5) / span as f64
    };
    Some((windup, recover * share.min(1.0)))
}

/// The active frame of fighter `f`'s cast: where its blade or casting hand reaches farthest (the frame the rules
/// strike or cast on, as `pick` draws it when the wind-up ends).
fn active_frame(state: &State, f: usize) -> Option<&'static Frame> {
    let fi = state.fighters.get(f)?;
    let clip = act_clip(state, f)?;
    let (window, frames) = clip_of(fi.look, clip)?;
    frames.get(window?.0)
}

/// The hitbox of fighter `f`'s blade blow in the arena: where the hand and blade are drawn on the frame it shows, if
/// that frame is in the clip's active window, else on the window's first frame. `None` for a clip drawn with no blade
/// out ahead (the rules then strike their own box).
pub fn strike_hitbox(state: &State, f: usize) -> Option<[f64; 4]> {
    let b = &state.fighters.get(f)?.body;
    let now = if in_window(state, f) {
        frame_of(state, f).and_then(|fr| fr.hit)
    } else {
        None
    };
    now.or_else(|| active_frame(state, f)?.hit)
        .map(|r| place(r, b.x, b.air, b.face))
}

/// Where fighter `f`'s casting hand is in the arena (x, height above the ground) on its clip's active frame: where a
/// bolt leaves it.
pub fn cast_hand(state: &State, f: usize) -> Option<(f64, f64)> {
    let b = &state.fighters.get(f)?.body;
    let h = active_frame(state, f)?.hand?;
    let x = if b.face < 0.0 { b.x - h[0] } else { b.x + h[0] };
    Some((x, b.air + h[1]))
}

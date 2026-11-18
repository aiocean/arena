//! Where a frame's time goes, measured in the game itself (a headless browser draws on the CPU, so its frame rate
//! says nothing of a player's): the time each of the frame's phases (`Frame`) took on the main thread, and the bytes
//! the world's pictures sent to the GPU (`Uploads`, counted by `layers`). Every `WINDOW` frames the means and the
//! worst frame are logged natively when `ARENA_PROFILE=1` (target `arena::profile`) and, on the page, set as
//! `window.__arena.profile` (`frames`, `ms` the mean of each phase and the total, `worst_ms`, `gap_ms` the mean time
//! from one frame's start to the next and `worst_gap_ms` the longest, `upload_kb` a frame, `uploads` textures a
//! frame), which the smoke test reads.
//!
//! The phases leave out the render world's own time (the textures' upload, the draw) and the assets' loading: on the
//! page those run in the same requestAnimationFrame, after the frame's systems, so the gap from one frame to the
//! next holds them (and, natively, the wait for the display).

use crate::Frame;
use bevy::platform::time::Instant;
use bevy::prelude::*;

/// Frames a report covers.
pub const WINDOW: u32 = 120;
/// The phases measured, in their order.
pub const PHASES: [&str; 5] = ["pace", "rules", "react", "paint", "upload"];

/// The world's pictures sent to the GPU this frame: how many textures and their bytes.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uploads {
    pub textures: u32,
    pub bytes: usize,
}

/// One report: the frames it covers, each phase's mean, the mean of the whole and the worst frame (ms), and the
/// uploads a frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub frames: u32,
    pub ms: [f64; PHASES.len()],
    pub total_ms: f64,
    pub worst_ms: f64,
    pub gap_ms: f64,
    pub worst_gap_ms: f64,
    pub upload_kb: f64,
    pub uploads: f64,
}

/// The marks of this frame and the sums since the last report.
#[derive(Resource)]
pub struct Profile {
    marks: [Option<Instant>; PHASES.len() + 1],
    sums: [f64; PHASES.len()],
    worst: f64,
    /// The start of the last frame, and the gaps from one start to the next since the last report.
    started: Option<Instant>,
    gaps: f64,
    worst_gap: f64,
    frames: u32,
    bytes: usize,
    textures: u32,
    /// The last report, for a test or a reader.
    pub last: Option<Report>,
    log: bool,
}

impl Default for Profile {
    fn default() -> Profile {
        Profile {
            marks: [None; PHASES.len() + 1],
            sums: [0.0; PHASES.len()],
            worst: 0.0,
            started: None,
            gaps: 0.0,
            worst_gap: 0.0,
            frames: 0,
            bytes: 0,
            textures: 0,
            last: None,
            log: !cfg!(target_arch = "wasm32")
                && std::env::var("ARENA_PROFILE").is_ok_and(|v| v == "1"),
        }
    }
}

impl Profile {
    /// The frame's marks done and its uploads counted: add them in, and report every `WINDOW` frames.
    fn close(&mut self, uploads: Uploads) -> Option<Report> {
        let marks: Vec<Instant> = self.marks.iter().flatten().copied().collect();
        self.marks = [None; PHASES.len() + 1];
        if marks.len() != PHASES.len() + 1 {
            return None;
        }
        if let Some(last) = self.started.replace(marks[0]) {
            let gap = (marks[0] - last).as_secs_f64() * 1000.0;
            self.gaps += gap;
            self.worst_gap = self.worst_gap.max(gap);
        }
        let mut total = 0.0;
        for (i, w) in marks.windows(2).enumerate() {
            let ms = (w[1] - w[0]).as_secs_f64() * 1000.0;
            self.sums[i] += ms;
            total += ms;
        }
        self.worst = self.worst.max(total);
        self.frames += 1;
        self.bytes += uploads.bytes;
        self.textures += uploads.textures;
        if self.frames < WINDOW {
            return None;
        }
        let n = f64::from(self.frames);
        let report = Report {
            frames: self.frames,
            ms: self.sums.map(|s| s / n),
            total_ms: self.sums.iter().sum::<f64>() / n,
            worst_ms: self.worst,
            gap_ms: self.gaps / n,
            worst_gap_ms: self.worst_gap,
            upload_kb: self.bytes as f64 / 1024.0 / n,
            uploads: f64::from(self.textures) / n,
        };
        self.sums = [0.0; PHASES.len()];
        self.worst = 0.0;
        self.gaps = 0.0;
        self.worst_gap = 0.0;
        self.frames = 0;
        self.bytes = 0;
        self.textures = 0;
        self.last = Some(report.clone());
        Some(report)
    }
}

pub struct ProfilePlugin;

impl Plugin for ProfilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Profile>()
            .init_resource::<Uploads>()
            .add_systems(Update, mark::<0>.before(Frame::Pace))
            .add_systems(Update, mark::<1>.after(Frame::Pace).before(Frame::Rules))
            .add_systems(Update, mark::<2>.after(Frame::Rules).before(Frame::React))
            .add_systems(Update, mark::<3>.after(Frame::React).before(Frame::Paint))
            .add_systems(Update, mark::<4>.after(Frame::Paint).before(Frame::Upload))
            .add_systems(Update, (mark::<5>, report).chain().after(Frame::Upload));
    }
}

/// The time at boundary `I` of the frame's phases (0 before the first, 5 after the last).
fn mark<const I: usize>(mut profile: ResMut<Profile>) {
    profile.marks[I] = Some(Instant::now());
}

fn report(mut profile: ResMut<Profile>, mut uploads: ResMut<Uploads>) {
    let Some(r) = profile.close(core::mem::take(&mut *uploads)) else {
        return;
    };
    if profile.log {
        let phases: Vec<String> = PHASES
            .iter()
            .zip(r.ms)
            .map(|(p, ms)| format!("{p} {ms:.2}"))
            .collect();
        info!(
            target: "arena::profile",
            "{} frames: {:.2} ms a frame ({}), worst {:.2} ms; frame to frame {:.2} ms, worst {:.2} ms; uploads {:.1} a frame, {:.0} KB",
            r.frames,
            r.total_ms,
            phases.join(", "),
            r.worst_ms,
            r.gap_ms,
            r.worst_gap_ms,
            r.uploads,
            r.upload_kb
        );
    }
    #[cfg(target_arch = "wasm32")]
    publish(&r);
}

/// The report as `window.__arena.profile`; whatever else the page keeps on `window.__arena` is left be.
#[cfg(target_arch = "wasm32")]
fn publish(r: &Report) {
    use js_sys::{Object, Reflect};
    use wasm_bindgen::JsValue;
    let Some(window) = web_sys::window() else {
        return;
    };
    let ms = Object::new();
    for (p, v) in PHASES.iter().zip(r.ms) {
        let _ = Reflect::set(&ms, &JsValue::from_str(p), &JsValue::from_f64(v));
    }
    let _ = Reflect::set(
        &ms,
        &JsValue::from_str("total"),
        &JsValue::from_f64(r.total_ms),
    );
    let profile = Object::new();
    for (k, v) in [
        ("frames", JsValue::from_f64(f64::from(r.frames))),
        ("ms", ms.into()),
        ("worst_ms", JsValue::from_f64(r.worst_ms)),
        ("gap_ms", JsValue::from_f64(r.gap_ms)),
        ("worst_gap_ms", JsValue::from_f64(r.worst_gap_ms)),
        ("upload_kb", JsValue::from_f64(r.upload_kb)),
        ("uploads", JsValue::from_f64(r.uploads)),
    ] {
        let _ = Reflect::set(&profile, &JsValue::from_str(k), &v);
    }
    let key = JsValue::from_str("__arena");
    let arena = Reflect::get(&window, &key)
        .ok()
        .filter(|v| v.is_object())
        .unwrap_or_else(|| Object::new().into());
    let _ = Reflect::set(&arena, &JsValue::from_str("profile"), &profile);
    let _ = Reflect::set(&window, &key, &arena);
}

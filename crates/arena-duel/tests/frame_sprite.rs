//! Frame data and the sprite are one timeline (`art/README.md`, "Frame data and the sprite"), made executable.
//!
//! A skill's wind-up plays the first part of its clip, up to the clip's active frame, stretched over its `windup`
//! (`pick`); its recovery plays the rest stretched over its `recover`, so the body is drawn open for exactly as long
//! as the rules keep it punishable. So the clip a skill plays (`cast`: every skill is a spell), the clip's frame
//! count and active
//! frame (`frames_data`, measured from the sheets) and the skill's `windup` and `recover` (`data`) must agree:
//!
//! - every wind-up has a frame of its own before the active one (held as a hard rule);
//! - a wind-up shows at least `SMOOTH` frames a second, or it reads choppy (`choppy`);
//! - a recovery shows at least `SMOOTH` frames a second too, or its follow-through reads choppy (`choppy_recover`).
//!
//! Every skill's timing in each look, with those verdicts, is pinned in `goldens/frame_sprite.txt`: a skill's
//! `windup` or `recover` changed without its clip, or a clip redrawn without the skills that play it, fails here,
//! naming the lines. When the change is meant, look at the new verdicts, then delete the file and run again.

use arena_duel::data::{Look, spells};
use arena_duel::frames::{active, len};
use std::fmt::Write as _;

const FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/goldens/frame_sprite.txt"
);

/// The fewest frames a second a wind-up may show and still read as motion.
const SMOOTH: f64 = 10.0;

/// One skill in one look: (the table's line, whether its wind-up has a frame of its own).
fn rows() -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for look in [Look::VuongLam, Look::ThatThai] {
        let name = look.name();
        for s in spells() {
            let clip = arena_duel::frames::CAST;
            let n = len(look, clip);
            let last = n.saturating_sub(1);
            let act = active(look, clip).unwrap_or(last / 2).min(last);
            let own = s.windup <= 0.0 || act >= 1;
            let shown = if s.windup > 0.0 {
                act as f64 / s.windup
            } else {
                f64::INFINITY
            };
            let follow = if s.recover > 0.0 {
                (last - act) as f64 / s.recover
            } else {
                f64::INFINITY
            };
            let mut verdict = Vec::new();
            if shown < SMOOTH {
                verdict.push("choppy");
            }
            if follow < SMOOTH {
                verdict.push("choppy_recover");
            }
            let line = format!(
                "{name:<7} {:<18} {clip:<8} frames {n:>2} active {act:>2} | windup {:.2} s ({:.0} fps) recover {:.2} s ({:.0} fps) | {}",
                s.name,
                s.windup,
                shown.min(999.0),
                s.recover,
                follow.min(999.0),
                if verdict.is_empty() {
                    "ok".to_owned()
                } else {
                    verdict.join(" ")
                },
            );
            out.push((line, own));
        }
    }
    out
}

#[test]
fn every_wind_up_has_a_frame_of_its_own() {
    let bad: Vec<String> = rows()
        .into_iter()
        .filter(|(_, own)| !own)
        .map(|(l, _)| l)
        .collect();
    assert!(
        bad.is_empty(),
        "wind-ups with no frame before the active one:\n{}",
        bad.join("\n")
    );
}

#[test]
fn each_skills_timing_agrees_with_its_clip_as_pinned() {
    let mut got = String::new();
    for (line, _) in rows() {
        let _ = writeln!(got, "{line}");
    }
    match std::fs::read_to_string(FILE) {
        Ok(want) => {
            let was: std::collections::BTreeSet<&str> = want.lines().collect();
            let now: std::collections::BTreeSet<&str> = got.lines().collect();
            let mut diff = String::new();
            for l in was.difference(&now) {
                let _ = writeln!(diff, "- {l}");
            }
            for l in now.difference(&was) {
                let _ = writeln!(diff, "+ {l}");
            }
            assert!(
                got == want,
                "a skill's timing or its clip changed without the other (art/README.md, \"Frame data and the \
                 sprite\"):\n{diff}if that was meant, look at the verdicts, delete {FILE} and run again"
            );
        }
        Err(_) => {
            assert!(std::fs::write(FILE, &got).is_ok(), "could not write {FILE}");
            eprintln!("wrote {FILE}: the first run pins every skill's timing against its clip");
        }
    }
}

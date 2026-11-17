//! Frame data and the sprite are one timeline (`art/README.md`, "Frame data and the sprite"), made executable.
//!
//! A skill's wind-up plays the first part of its clip, up to the clip's active frame, stretched over its `windup`
//! (`pick`); its recovery plays the rest at the sheet's frames a second. So the clip a skill plays (`slash`, or
//! `overhead` for a blow from the air, `dash` for a lunge, `cast` for the rest), the clip's frame count and active
//! frame (`frames_data`, measured from the sheets) and the skill's `windup` and `recover` (`data`) must agree:
//!
//! - every wind-up has a frame of its own before the active one (held as a hard rule);
//! - a wind-up shows at least `SMOOTH` frames a second, or it reads choppy (`choppy`);
//! - the drawn follow-through lasts as long as the rules keep the fighter open to a punish, give or take a frame, or
//!   it stands back in its stance while still punishable (`open_short`).
//!
//! Every skill's timing in each look, with those verdicts, is pinned in `goldens/frame_sprite.txt`: a skill's
//! `windup` or `recover` changed without its clip, or a clip redrawn without the skills that play it, fails here,
//! naming the lines. When the change is meant, look at the new verdicts, then delete the file and run again.

use arena_duel::data::{Kind, Look, spells};
use arena_duel::frames::{active, fps, len};
use std::fmt::Write as _;

const FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/goldens/frame_sprite.txt"
);

/// The fewest frames a second a wind-up may show and still read as motion.
const SMOOTH: f64 = 10.0;

/// The clip a skill of `kind` plays (`pick::part_of`).
fn clip_of(kind: Kind, dive: bool) -> &'static str {
    match kind {
        Kind::Melee if dive => "overhead",
        Kind::Melee => "slash",
        Kind::Dash => "dash",
        _ => "cast",
    }
}

/// One skill in one look: (the table's line, whether its wind-up has a frame of its own).
fn rows() -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for (look, name) in [(Look::Thunder, "thunder"), (Look::Frost, "frost")] {
        let rate = fps(look);
        for s in spells() {
            let clip = clip_of(s.kind, s.dive);
            let n = len(look, clip);
            let last = n.saturating_sub(1);
            let act = active(look, clip).unwrap_or(last / 2).min(last);
            let own = s.windup <= 0.0 || act >= 1;
            let shown = if s.windup > 0.0 {
                act as f64 / s.windup
            } else {
                f64::INFINITY
            };
            let follow = (last - act) as f64 / rate;
            let mut verdict = Vec::new();
            if shown < SMOOTH {
                verdict.push("choppy");
            }
            if follow + 1.0 / rate < s.recover {
                verdict.push("open_short");
            }
            let line = format!(
                "{name:<7} {:<18} {clip:<8} frames {n:>2} active {act:>2} at {rate} fps | windup {:.2} s ({:.0} fps) recover {:.2} s, drawn {:.2} s | {}",
                s.name,
                s.windup,
                shown.min(999.0),
                s.recover,
                follow,
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

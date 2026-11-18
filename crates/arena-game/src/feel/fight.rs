//! The fight's feel as state over the rules' events and real time: the engine's `FightFeel` (`pixfx-fighter`'s
//! `feel.rs`, with the screen's part from `pixfx-core`'s `feel.rs`: trauma, shake, freeze and slow motion) as the page
//! plays it, fed the duel's `Out` events in the world's pixels. A blow freezes the fight a few frames while the one
//! struck shudders and the screen shakes; its damage floats up; a body hitting the floor kicks up dust; a knockout
//! lingers in slow motion. Heights (`y`) are up from the ground.

use super::math::{floor, js_max, js_min, js_round, perlin, sqrt};
use crate::layout::Layout;
use arena_duel::out::{How, Out};

/// Hit-stop frames at 60 a second: light, medium, launch, and what a crit, a ranged blow, a counter, a punish add.
const STOP_LIGHT: f64 = 6.0;
const STOP_MEDIUM: f64 = 8.0;
const STOP_LAUNCH: f64 = 11.0;
const STOP_CRIT: f64 = 3.0;
const STOP_RANGED: f64 = -2.0;
const STOP_COUNTER: f64 = 2.0;
const STOP_PUNISH: f64 = 3.0;
const MEDIUM_POWER: f64 = 9.0;
/// The one struck shudders this many pixels at first, changing side this many times a second.
const JOLT: f64 = 3.0;
const JOLT_RATE: f64 = 30.0;
const HIT_TRAUMA: f64 = 0.3;
const SHARE_TRAUMA: f64 = 40.0;
const CRIT_TRAUMA: f64 = 0.2;
/// The screen: pixels at full trauma, the noise's rate, trauma drained a second, the longest freeze.
const SHAKE: f64 = 6.0;
const SHAKE_RATE: f64 = 12.0;
const TRAUMA_DRAIN: f64 = 1.0;
const FREEZE_CAP: f64 = 13.0 / 60.0;
/// (stop, trauma) of a fighter refusing to fall, a barrier ringing, two spells meeting.
const UNYIELDING: (f64, f64) = (0.18, 0.4);
const BLOCK: (f64, f64) = (0.05, 0.15);
const CLASH: (f64, f64) = (0.06, 0.25);
const LAND_TRAUMA: f64 = 0.15;
const BUMP_TRAUMA: f64 = 0.15;
const DUST_LIFE: f64 = 0.4;
const DUST_LEAST: f64 = 0.3;
const KO_STOP: f64 = 0.25;
const KO_SPEED: f64 = 0.35;
const KO_SLOW: f64 = 1.2;
const KO_TRAUMA: f64 = 0.8;
const NUMBER_RISE: f64 = 14.0;
const NUMBER_LIFE: f64 = 0.8;
const STREAK: f64 = 0.28;
/// A counter's or a punish's word floats this far above its damage.
const WORD_ABOVE: f64 = 10.0;

/// What a floating number says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberKind {
    Hit,
    Crit,
    Tick,
    Heal,
    Ward,
    Dodge,
    Counter,
    Punish,
}

/// A floating number as drawn this frame: where (height above the ground), how faded, its text, whose.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    pub x: f64,
    pub y: f64,
    pub alpha: f64,
    pub text: String,
    pub by: Option<usize>,
    pub kind: NumberKind,
}

/// A puff of dust: where, how strong, how far through its life (0 new, 1 gone).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dust {
    pub x: f64,
    pub strength: f64,
    pub k: f64,
}

/// A qi streak: its ends (heights up from the ground), whose, how faded (1 new, 0 gone).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Streak {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub by: usize,
    pub fade: f64,
}

#[derive(Clone, Debug)]
struct Floating {
    x: f64,
    y: f64,
    text: String,
    by: Option<usize>,
    kind: NumberKind,
    t: f64,
}

#[derive(Clone, Copy, Debug)]
struct Jolt {
    who: usize,
    left: f64,
    full: f64,
    air: bool,
}

/// What is left of a timer after `dt`, in whole frames.
fn run_down(left: f64, dt: f64) -> f64 {
    if left - dt > 1e-6 { left - dt } else { 0.0 }
}

/// An amount as a whole number: the damage system deals fractions, a player reads whole points.
fn text(v: f64) -> String {
    let v = js_round(v);
    if v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// The screen's answer to a fight.
#[derive(Clone, Debug, Default)]
pub struct FightFeel {
    trauma: f64,
    stop: f64,
    slow: f64,
    slow_speed: f64,
    numbers: Vec<Floating>,
    streaks: Vec<(Streak, f64)>,
    dust: Vec<(f64, f64, f64)>,
    jolt: Option<Jolt>,
    /// Real seconds gone by, for the shake's noise and the jolt's side.
    clock: f64,
}

impl FightFeel {
    fn add_trauma(&mut self, k: f64) {
        if k.is_finite() {
            self.trauma = (self.trauma + k).clamp(0.0, 1.0);
        }
    }

    fn freeze(&mut self, secs: f64) {
        if secs.is_finite() {
            self.stop = js_max(self.stop, secs);
        }
    }

    fn answer(&mut self, (stop, trauma): (f64, f64)) {
        self.freeze(stop);
        self.add_trauma(trauma);
    }

    /// Take in one of the rules' events (in the rules' pixels); effect commands and the rest pass by.
    pub fn react(&mut self, out: &Out) {
        let w = |v: f64| f64::from(Layout::world(v));
        match *out {
            Out::Hit {
                by,
                power,
                launch,
                ranged,
                damage,
                crit,
                how,
                share,
                x,
                y,
                ..
            } => {
                let (x, y) = (w(x), w(y));
                let mut frames = if launch {
                    STOP_LAUNCH
                } else if power >= MEDIUM_POWER {
                    STOP_MEDIUM
                } else {
                    STOP_LIGHT
                };
                if crit {
                    frames += STOP_CRIT;
                }
                if ranged {
                    frames += STOP_RANGED;
                }
                frames += match how {
                    How::Plain => 0.0,
                    How::Counter => STOP_COUNTER,
                    How::Punish => STOP_PUNISH,
                    // caught meditating: the longest stop of all
                    How::Meditate => STOP_PUNISH,
                };
                let stop = js_min(FREEZE_CAP * 60.0, frames) / 60.0;
                self.freeze(stop);
                // grounded, it shudders side to side; in the air, up and down
                self.jolt = Some(Jolt {
                    who: 1usize.wrapping_sub(by),
                    left: stop,
                    full: stop,
                    air: y > 24.0,
                });
                let caught = how != How::Plain;
                self.add_trauma(
                    HIT_TRAUMA
                        + share / SHARE_TRAUMA
                        + if crit { CRIT_TRAUMA } else { 0.0 }
                        + if caught { CRIT_TRAUMA } else { 0.0 },
                );
                let kind = if crit {
                    NumberKind::Crit
                } else {
                    NumberKind::Hit
                };
                self.number(x, y, Some(by), kind, text(damage));
                let word = match how {
                    How::Plain => None,
                    How::Counter => Some(("COUNTER!", NumberKind::Counter)),
                    How::Punish => Some(("PUNISH!", NumberKind::Punish)),
                    How::Meditate => Some(("BROKEN!", NumberKind::Punish)),
                };
                if let (Some((word, kind)), Some(last)) = (word, self.numbers.last()) {
                    let over = Floating {
                        x: last.x,
                        y: last.y + WORD_ABOVE,
                        text: word.to_string(),
                        by: last.by,
                        kind,
                        t: 0.0,
                    };
                    self.numbers.push(over);
                }
            }
            Out::Tick {
                by, damage, x, y, ..
            } => self.number(w(x), w(y), Some(by), NumberKind::Tick, text(damage)),
            Out::Heal { by, amount, x, y } => self.number(
                w(x),
                w(y),
                Some(by),
                NumberKind::Heal,
                format!("+{}", text(amount)),
            ),
            Out::Ward { by, amount, x, y } => self.number(
                w(x),
                w(y),
                Some(by),
                NumberKind::Ward,
                format!("+{}", text(amount)),
            ),
            Out::Dodge { x, y } => self.number(w(x), w(y), None, NumberKind::Dodge, "-".into()),
            Out::Unyielding { .. } => self.answer(UNYIELDING),
            Out::Block { .. } => self.answer(BLOCK),
            Out::Clash { .. } => self.answer(CLASH),
            Out::Land { x, strength } => {
                self.add_trauma(LAND_TRAUMA * strength);
                if strength > DUST_LEAST {
                    self.dust.push((w(x), strength, 0.0));
                }
            }
            Out::Bump { strength, .. } => self.add_trauma(BUMP_TRAUMA * strength),
            Out::Ko { .. } => {
                self.freeze(KO_STOP);
                self.slow = js_max(0.0, KO_SLOW);
                self.slow_speed = KO_SPEED;
                self.add_trauma(KO_TRAUMA);
            }
            Out::Streak { by, x0, y0, x1, y1 } => self.streaks.push((
                Streak {
                    x0: w(x0),
                    y0: w(y0),
                    x1: w(x1),
                    y1: w(y1),
                    by,
                    fade: 1.0,
                },
                0.0,
            )),
            _ => {}
        }
    }

    fn number(&mut self, x: f64, y: f64, by: Option<usize>, kind: NumberKind, text: String) {
        // numbers from one spot step aside so a barrage stays readable
        let near = self
            .numbers
            .iter()
            .filter(|n| (n.x - x).abs() < 12.0 && n.t < 0.3)
            .count() as f64;
        let side = if near % 2.0 != 0.0 { 7.0 } else { -7.0 };
        self.numbers.push(Floating {
            x: x + side * js_min(near, 1.0),
            y: y + near * 6.0,
            text,
            by,
            kind,
            t: 0.0,
        });
    }

    /// Let `dt` real seconds pass; how many the fight runs for them: none in a hit-stop, a share in slow motion.
    pub fn advance(&mut self, dt: f64) -> f64 {
        if !dt.is_finite() || dt <= 0.0 {
            return 0.0;
        }
        self.trauma = js_max(0.0, self.trauma - TRAUMA_DRAIN * dt);
        let run = if self.stop > 0.0 {
            self.stop = run_down(self.stop, dt);
            0.0
        } else if self.slow > 0.0 {
            self.slow = run_down(self.slow, dt);
            dt * self.slow_speed
        } else {
            dt
        };
        for n in &mut self.numbers {
            n.t += dt;
        }
        self.numbers.retain(|n| n.t < NUMBER_LIFE);
        for (_, t) in &mut self.streaks {
            *t += dt;
        }
        self.streaks.retain(|(_, t)| *t < STREAK);
        for d in &mut self.dust {
            d.2 += dt;
        }
        self.dust.retain(|d| d.2 < DUST_LIFE);
        self.clock += dt;
        if let Some(j) = &mut self.jolt {
            j.left -= dt;
            if j.left <= 0.0 {
                self.jolt = None;
            }
        }
        run
    }

    /// Where the screen sits this frame, in whole world pixels: trauma squared times smooth noise over real time.
    pub fn shake(&self) -> (i32, i32) {
        let a = self.trauma * self.trauma * SHAKE;
        let wander = |axis: f64| perlin(self.clock * SHAKE_RATE + axis / 3.0, axis + 1.0);
        (
            js_round(wander(0.0) * a) as i32,
            js_round(wander(1.0) * a) as i32,
        )
    }

    /// How far each fighter is jolted off its place, in whole world pixels (y down).
    pub fn jolts(&self) -> [[i32; 2]; 2] {
        let mut out = [[0; 2]; 2];
        let Some(j) = self.jolt else {
            return out;
        };
        let side = floor(self.clock * JOLT_RATE) as i64;
        let a = js_round(JOLT * (j.left / j.full) + 0.4) * if side % 2 == 0 { -1.0 } else { 1.0 };
        if let Some(slot) = out.get_mut(j.who) {
            *slot = if j.air { [0, a as i32] } else { [a as i32, 0] };
        }
        out
    }

    /// Each floating number.
    pub fn numbers(&self) -> Vec<Number> {
        self.numbers
            .iter()
            .map(|n| Number {
                x: n.x,
                y: n.y + 16.0 + NUMBER_RISE * sqrt(n.t / NUMBER_LIFE),
                alpha: js_min(1.0, 2.5 * (1.0 - n.t / NUMBER_LIFE)),
                text: n.text.clone(),
                by: n.by,
                kind: n.kind,
            })
            .collect()
    }

    /// Each qi streak.
    pub fn streaks(&self) -> Vec<Streak> {
        self.streaks
            .iter()
            .map(|(s, t)| Streak {
                fade: 1.0 - t / STREAK,
                ..*s
            })
            .collect()
    }

    /// Each puff of dust.
    pub fn dust(&self) -> Vec<Dust> {
        self.dust
            .iter()
            .map(|&(x, strength, t)| Dust {
                x,
                strength,
                k: t / DUST_LIFE,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_duel::data::UNIT;

    fn hit(power: f64, crit: bool) -> Out {
        Out::Hit {
            by: 0,
            spell: "x",
            power,
            launch: false,
            ranged: false,
            damage: 12.0,
            crit,
            how: How::Counter,
            share: 10.0,
            x: 100.0 * UNIT,
            y: 0.0,
        }
    }

    #[test]
    fn a_blow_stops_the_fight_jolts_the_one_struck_and_floats_its_damage_and_word() {
        let mut f = FightFeel::default();
        f.react(&hit(10.0, false));
        // medium 8 + counter 2 frames
        assert_eq!(f.advance(1.0 / 60.0), 0.0);
        assert_ne!(f.jolts()[1][0], 0);
        assert_eq!(f.jolts()[0], [0, 0]);
        let n = f.numbers();
        assert_eq!(n.len(), 2);
        assert_eq!((n[0].text.as_str(), n[1].text.as_str()), ("12", "COUNTER!"));
        for _ in 0..12 {
            f.advance(1.0 / 60.0);
        }
        assert_eq!(f.advance(1.0 / 60.0), 1.0 / 60.0);
    }

    #[test]
    fn a_knockout_slows_the_fight_after_its_stop() {
        let mut f = FightFeel::default();
        f.react(&Out::Ko { x: 0.0, y: 0.0 });
        for _ in 0..15 {
            f.advance(1.0 / 60.0);
        }
        let run = f.advance(0.1);
        assert!((run - 0.035).abs() < 1e-9);
    }
}

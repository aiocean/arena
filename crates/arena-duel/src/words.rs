//! A fighter's moment in plain words, for a judge that reads text (a decision model asked whether a move was
//! sensible, a person reading a replay): what it and its foe are doing, their life and qi, the space between them,
//! what flies at it, and what each of its slots holds. It only reads the state; nothing here decides anything.

use arena_mind::policy::{ACTIONS, Act, Aim};
use arena_mind::{ActPhase, Doing, Sense, Skill, SkillKind, ThreatShape};

use crate::data::UNIT;
use crate::fighter::State;
use crate::sense::{sense_of, skill_of};

/// Within this share of the arena's width of a wall, a fighter has its back to it.
const WALL: f64 = 0.08;

/// The numbers a match's log keeps for each decision, besides its words.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brief {
    pub time: f64,
    pub hp: f64,
    pub foe_hp: f64,
    pub qi: f64,
    pub foe_qi: f64,
    /// The arena's pixels between the two, centre to centre (the range bands' pixels: the arena is 880 wide).
    pub gap: f64,
    pub flying: bool,
    /// Height of its feet over the ground, the arena's pixels.
    pub air: f64,
}

/// A length the minds see (`sense::seen`) in the arena's pixels, the ones the rules and `docs/rules.md` speak in.
fn px(v: f64) -> f64 {
    v * UNIT
}

/// The numbers of fighter `me`'s moment.
pub fn brief(state: &State, me: usize) -> Brief {
    let s = sense_of(state, me);
    Brief {
        time: s.time,
        hp: s.me.hp,
        foe_hp: s.foe.hp,
        qi: s.me.qi_level(),
        foe_qi: s.foe.qi,
        gap: px((s.foe.x - s.me.x).abs()),
        flying: s.me.flying,
        air: px(s.me.air),
    }
}

fn kind(k: SkillKind) -> &'static str {
    match k {
        SkillKind::Blast => "a short-range palm blast",
        SkillKind::Bolt => "a bolt that flies level",
        SkillKind::Seeker => "a slow homing orb",
        SkillKind::Area => "a strike that lands on a spot after a delay",
        SkillKind::Field => "a lasting field that hurts whoever stands in it",
        SkillKind::Heal => "a heal",
        SkillKind::Ward => "a ward that soaks blows",
        SkillKind::Barrier => "a barrier that turns every blow aside for a moment",
        SkillKind::Blink => "a blink (instant teleport)",
    }
}

fn skill_line(s: &Skill) -> String {
    let mut out = format!(
        "{} ({}; costs {:.0} qi; wind-up {:.2}s, recovery {:.2}s",
        s.name,
        kind(s.kind),
        s.cost,
        s.windup,
        s.recover
    );
    if s.damage > 0.0 {
        let total = s.damage * s.strikes.max(1) as f64;
        out += &match s.kind {
            SkillKind::Heal => format!("; heals {total:.0}"),
            _ => format!("; {total:.0} damage"),
        };
    }
    if s.reach > 0.0 {
        out += &format!("; reach {:.0}px", px(s.reach));
    }
    out + ")"
}

fn doing(d: &Doing) -> String {
    match d {
        Doing::Idle => "standing still".into(),
        Doing::Move { .. } => "walking".into(),
        Doing::Guard => "guarding".into(),
        Doing::Meditate => "meditating (sitting, regaining qi, defenceless)".into(),
        Doing::Fly => "flying".into(),
        Doing::Act {
            kind: k,
            phase,
            t,
            windup,
            recover,
            damage,
            ..
        } => match phase {
            ActPhase::Windup => format!(
                "winding up {} ({:.0} damage), releases in {:.2}s",
                kind(*k),
                damage,
                (windup - t).max(0.0)
            ),
            ActPhase::Recover => format!(
                "recovering from {}, free again in {:.2}s",
                kind(*k),
                (recover - t).max(0.0)
            ),
        },
    }
}

fn pct(x: f64) -> String {
    format!("{:.0}%", x.clamp(0.0, 100.0))
}

/// Where `x` stands against the walls.
fn walls(s: &Sense, x: f64) -> &'static str {
    let near = WALL * (s.arena.right - s.arena.left);
    if x - s.arena.left < near || s.arena.right - x < near {
        ", back against a wall"
    } else {
        ""
    }
}

/// Fighter `me`'s moment, in a few short lines of English.
pub fn moment(state: &State, me: usize) -> String {
    let s = sense_of(state, me);
    let f = &state.fighters[me];
    let skills: Vec<Skill> = f.spells.iter().map(|n| skill_of(n)).collect();
    let gap = px((s.foe.x - s.me.x).abs());
    let width = px(s.arena.right - s.arena.left);
    let mut lines = vec![format!(
        "Spell duel between two cultivators, {:.1}s into the round. The arena is {width:.0}px wide; the two stand \
         {gap:.0}px apart.",
        s.time
    )];

    let m = &s.me;
    let mut me_line = format!(
        "Me: life {}, qi {}{}{}.",
        pct(m.hp),
        pct(m.qi_level()),
        if m.flying {
            ", flying"
        } else if m.air > 1.0 {
            ", in the air"
        } else {
            ""
        },
        walls(&s, m.x)
    );
    if let Some(a) = &m.act {
        me_line += &match a.phase {
            ActPhase::Windup => format!(
                " I am winding up a cast, {:.2}s to release.",
                (a.windup - a.t).max(0.0)
            ),
            ActPhase::Recover => format!(
                " I am recovering from a cast, {:.2}s left.",
                (a.recover - a.t).max(0.0)
            ),
        };
    } else if !m.can_act {
        me_line += " I cannot act right now.";
    }
    for (on, what) in [
        (m.silenced, "silenced (attacks only)"),
        (m.sealed, "blink sealed"),
        (m.barrier, "barrier up"),
        (m.shield, "ward up"),
    ] {
        if on {
            me_line += &format!(" {what}.");
        }
    }
    lines.push(me_line);

    let o = &s.foe;
    let mut foe_line = format!(
        "Foe: life {}, qi {}, {}{}{}.",
        pct(o.hp),
        pct(o.qi),
        doing(&o.doing),
        if o.flying {
            ", flying"
        } else if o.air > 1.0 {
            ", in the air"
        } else {
            ""
        },
        walls(&s, o.x)
    );
    if o.out_for > 0.0 {
        foe_line += &format!(" It cannot act for {:.2}s.", o.out_for);
    }
    if o.open_in > 0.0 {
        foe_line += &format!(" No blow can touch it for {:.2}s.", o.open_in);
    }
    if o.frozen > 0.0 {
        foe_line += " It is frozen.";
    }
    if o.shocked > 0.0 {
        foe_line += " It is shocked.";
    }
    if o.barrier {
        foe_line += " Its barrier is up.";
    }
    let ready = o.ready.iter().filter(|r| **r).count();
    foe_line += &format!(" {ready} of its {} skills are ready.", o.ready.len());
    lines.push(foe_line);

    if s.threats.is_empty() {
        lines.push("Nothing of the foe's is flying at me.".into());
    } else {
        for t in &s.threats {
            let what = match &t.shape {
                ThreatShape::Projectile { x, vx, .. } => {
                    let closing = (s.me.x - x) * vx > 0.0;
                    let eta = if closing && vx.abs() > 1e-6 {
                        format!("reaches me in about {:.2}s", (s.me.x - x).abs() / vx.abs())
                    } else {
                        "moving away".into()
                    };
                    format!("a bolt {:.0}px away, {eta}", px((s.me.x - x).abs()))
                }
                ThreatShape::Area {
                    x,
                    radius,
                    lands_in,
                    ..
                } => format!(
                    "a strike landing {:.0}px from me (radius {:.0}px) in {lands_in:.2}s",
                    px((s.me.x - x).abs()),
                    px(*radius)
                ),
                ThreatShape::Seeker { x, .. } => {
                    format!("a homing orb {:.0}px away", px((s.me.x - x).abs()))
                }
                ThreatShape::Field { x, reach } => format!(
                    "a field {:.0}px from me, hurting within {:.0}px",
                    px((s.me.x - x).abs()),
                    px(*reach)
                ),
                ThreatShape::Barrage { lands_in } => {
                    format!("a barrage whose next blow lands in {lands_in:.2}s")
                }
            };
            lines.push(format!("Incoming: {what}, {:.0} damage.", t.damage));
        }
    }

    let mut slots = Vec::new();
    for (i, k) in s.slots.iter().enumerate() {
        if let Some(sk) = k.and_then(|k| skills.get(k)) {
            let r = s.ready.get(k.unwrap_or(0)).copied().unwrap_or(false);
            slots.push(format!(
                "slot {}: {}{}",
                i + 1,
                skill_line(sk),
                if r { "" } else { " [not ready]" }
            ));
        }
    }
    lines.push(format!("My skills: {}.", slots.join("; ")));
    lines.join("\n")
}

/// What each of the policy's acts would be for fighter `me` now, in words (an empty slot's casts name the slot).
pub fn acts(state: &State, me: usize) -> Vec<String> {
    let s = sense_of(state, me);
    let f = &state.fighters[me];
    (0..ACTIONS)
        .map(|i| match Act::from_index(i) {
            Some(Act::Wait) => "wait".into(),
            Some(Act::Toward) => "walk toward the foe".into(),
            Some(Act::Away) => "walk away from the foe".into(),
            Some(Act::Jump) => "jump".into(),
            Some(Act::Duck) => "duck".into(),
            Some(Act::Guard) => "guard".into(),
            Some(Act::DuckLong) => "duck long".into(),
            Some(Act::GuardLong) => "guard long".into(),
            Some(Act::FlyHigh) => "fly up".into(),
            Some(Act::Land) => "land".into(),
            Some(Act::Meditate) => "sit down and meditate to regain qi".into(),
            Some(Act::Cast { slot, aim }) => {
                let name = s
                    .slots
                    .get(slot)
                    .copied()
                    .flatten()
                    .and_then(|k| f.spells.get(k))
                    .map_or_else(|| format!("slot {}", slot + 1), |n| n.to_string());
                match aim {
                    Aim::Foe => format!("cast {name} at the foe"),
                    Aim::Ahead => format!("cast {name} aimed past the foe"),
                    Aim::Behind => format!("cast {name} aimed short of the foe"),
                }
            }
            None => "wait".into(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;
    use crate::train::{Env, EnvConfig, Opponent};

    #[test]
    fn a_moment_reads_both_fighters_and_every_act() {
        let env = Env::new(EnvConfig {
            seed: 7,
            learner: 0,
            style: Style::Balanced,
            foe_style: Style::Balanced,
            opponent: Opponent::Brain,
            limit: 30.0,
            effects: Vec::new(),
        });
        let text = env.moment();
        assert!(text.contains("Me: life"), "{text}");
        assert!(text.contains("Foe: life"), "{text}");
        assert!(text.contains("My skills: slot 1"), "{text}");
        let acts = env.acts();
        assert_eq!(acts.len(), ACTIONS);
        assert_eq!(acts[0], "wait");
        assert!(acts[9].starts_with("cast "), "{}", acts[9]);
    }
}

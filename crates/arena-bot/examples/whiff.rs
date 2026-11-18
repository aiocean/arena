//! Whiffs: for every cast begun in a set of matches, whether the foe was within the skill's reach when it began and
//! when its blow went out, and what came of it (a hit, a blow stopped, a cast broken, or nothing: a whiff). Then the
//! runs of whiffed blade blows thrown from out of reach, and, for the house AI's, what it saw and chose.
//!
//! `cargo run --release -p arena-bot --features host,onnx --example whiff -- [--seeds 150] [--rounds 3] [--limit 120]
//! [--moments 5] [--out whiff.json]`
//!
//! Reach is the skill's motion's `range` (a blade, a blast) or `distance` (a dash); the other kinds (bolts, orbs,
//! areas, fields) land wherever the foe is and have none. The gap is from the caster's x to the nearest edge of the
//! foe's hurtboxes (`frames::hurtboxes`), arena pixels. A cast's outcome counts the `Out::Hit`s and the
//! `Note::Stopped`s of its skill by its caster from its start until 2.5 s after its act ends (a bolt in flight) or
//! the same skill's next cast.

use arena_bot::onnx::Onnx;
use arena_duel::data::{Kind, spell};
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::fighter::{Phase, State};
use arena_duel::first_mind::FirstMind;
use arena_duel::frames;
use arena_duel::js::js_round;
use arena_duel::kit::Way;
use arena_duel::ladder::looks_of;
use arena_duel::out::{How, Note, Out, Stop};
use arena_duel::scripted::Scripted;
use arena_duel::seat::{BotSeat, Brain, Decider, Fault, MakeSeat, Seat};
use arena_duel::sense::sense_of;
use arena_duel::style::Style;
use arena_mind::policy::{ACTIONS, Act, Aim, OBS, SLOTS, sample};
use arena_mind::{Choice, Outcome, Posture};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// One decision of the house AI.
#[derive(Clone, Debug, Default)]
struct Dec {
    fight: usize,
    seat: usize,
    round: u32,
    t: f64,
    act: usize,
    legal: Vec<bool>,
    probs: Vec<f32>,
    gap: f64,
    /// Each slot's skill.
    slots: [Option<&'static str>; SLOTS],
    /// What it was shown, kept only for a blade cast begun more than 20 px beyond the slash's reach.
    obs: Option<Box<[f32]>>,
}

type Log = Rc<RefCell<Vec<Dec>>>;

/// The house AI, recording what it was shown and what it chose.
struct Recording {
    onnx: Onnx,
    log: Log,
}

impl Decider for Recording {
    fn start(&mut self, _seed: u32) -> Result<(), Fault> {
        Ok(())
    }
    fn round(&mut self) -> Result<(), Fault> {
        Ok(())
    }
    fn decide(&mut self, obs: &[f32; OBS], legal: &[bool; ACTIONS], u: f32) -> Result<i32, Fault> {
        let l = self.onnx.logits(obs)?;
        let act = sample(&l, legal, u);
        self.log.borrow_mut().push(Dec {
            act,
            legal: legal.to_vec(),
            probs: masked_softmax(&l, legal),
            obs: Some(obs.to_vec().into_boxed_slice()),
            ..Dec::default()
        });
        Ok(act as i32)
    }
}

// a report of the policy's odds, never a rule: the platform's exp is fine
#[allow(clippy::disallowed_methods)]
fn masked_softmax(l: &[f32; ACTIONS], legal: &[bool; ACTIONS]) -> Vec<f32> {
    let m = l
        .iter()
        .zip(legal)
        .filter(|(_, ok)| **ok)
        .map(|(v, _)| *v)
        .fold(f32::NEG_INFINITY, f32::max);
    let e: Vec<f32> = l
        .iter()
        .zip(legal)
        .map(|(v, ok)| if *ok { (v - m).exp() } else { 0.0 })
        .collect();
    let s: f32 = e.iter().sum::<f32>().max(1e-30);
    e.iter().map(|v| v / s).collect()
}

/// A seat that stamps the house's new decisions with the fight, the seat, the time and the gap.
struct Stamped {
    inner: Box<dyn Seat>,
    log: Log,
    fight: Rc<RefCell<usize>>,
}

impl Seat for Stamped {
    fn think(&mut self, state: &State, me: usize, dt: f64) -> Option<Choice> {
        let before = self.log.borrow().len();
        let c = self.inner.think(state, me, dt);
        let mut log = self.log.borrow_mut();
        if log.len() > before {
            let sense = sense_of(state, me);
            let f = &state.fighters[me];
            let mut slots = [None; SLOTS];
            for (i, s) in sense.slots.iter().enumerate().take(SLOTS) {
                slots[i] = s.and_then(|k| f.spells.get(k).copied());
            }
            let g = gap(state, me);
            let fight = *self.fight.borrow();
            for d in &mut log[before..] {
                d.fight = fight;
                d.seat = me;
                d.round = state.round;
                d.t = state.time;
                d.gap = g;
                d.slots = slots;
                if !(g > 116.0 && matches!(cast_skill(d, d.act), Some("slash" | "dive"))) {
                    d.obs = None;
                }
            }
        }
        c
    }
    fn tell(&mut self, outcome: &Outcome) {
        self.inner.tell(outcome);
    }
    fn reset(&mut self) {
        self.inner.reset();
    }
    fn hold(&mut self, seconds: f64) {
        self.inner.hold(seconds);
    }
    fn forfeit(&self) -> bool {
        self.inner.forfeit()
    }
    fn posture(&self) -> Option<Posture> {
        self.inner.posture()
    }
}

/// From fighter `me`'s x to the nearest edge of its foe's hurtboxes, arena pixels (0 inside one).
fn gap(state: &State, me: usize) -> f64 {
    let x = state.fighters[me].body.x;
    frames::hurtboxes(state, 1 - me)
        .iter()
        .map(|h| (h[0] - x).max(x - h[2]).max(0.0))
        .fold(f64::INFINITY, f64::min)
}

#[derive(Clone, Copy, PartialEq)]
enum Bot {
    House(Option<Style>),
    Brain,
    First,
    Aggressive,
    Turtle,
}

impl Bot {
    fn name(self) -> &'static str {
        match self {
            Bot::House(_) => "house",
            Bot::Brain => "brain",
            Bot::First => "first",
            Bot::Aggressive => "aggressive",
            Bot::Turtle => "turtle",
        }
    }
}

/// One cast.
#[derive(Clone, Debug, Default)]
struct Cast {
    row: &'static str,
    bot: &'static str,
    fight: usize,
    seed: u32,
    seat: usize,
    round: u32,
    spell: &'static str,
    kind: &'static str,
    way: String,
    /// Strikes something (a blow, a damaging domain); devices and utilities do not count for whiffs.
    striking: bool,
    t0: f64,
    t_release: Option<f64>,
    t_end: Option<f64>,
    gap0: f64,
    gap_act: Option<f64>,
    reach: Option<f64>,
    x0: f64,
    /// The caster's x when its act ended, and the foe's side at the start (+1 right).
    x_end: Option<f64>,
    side: f64,
    walk: f64,
    qi: f64,
    cooldown: f64,
    hits: u32,
    first_hit: Option<f64>,
    damage: f64,
    stopped: u32,
    clashed: u32,
    broken: bool,
    /// The foe struck the caster within 1 s of the cast's start; a PUNISH among them.
    hit_back_1s: bool,
    punished: bool,
    taken_1s: f64,
    decision: Option<usize>,
}

impl Cast {
    fn outcome(&self) -> &'static str {
        if !self.striking {
            "utility"
        } else if self.hits > 0 {
            "hit"
        } else if self.stopped > 0 {
            "stopped"
        } else if self.broken {
            "broken"
        } else {
            "whiff"
        }
    }
    fn far(&self, by: f64) -> bool {
        self.reach.is_some_and(|r| self.gap0 > r + by)
    }
    fn melee(&self) -> bool {
        self.kind == "melee" && self.way == "Motion"
    }
}

fn kind_name(k: Kind) -> &'static str {
    k.name()
}

fn reach_of(way: Way, k: Kind, sp: &arena_duel::data::Spell) -> Option<f64> {
    if way != Way::Motion {
        return None;
    }
    match k {
        Kind::Melee | Kind::Blast => sp.range,
        Kind::Dash => sp.distance,
        _ => None,
    }
}

struct Tracker {
    casts: Vec<Cast>,
    /// Each seat's cast in play (its act), by index.
    acting: [Option<usize>; 2],
    /// The latest cast of each (seat, skill), by index.
    latest: BTreeMap<(usize, &'static str), usize>,
    seconds: BTreeMap<&'static str, f64>,
    hits_on: BTreeMap<&'static str, u32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let seeds: u32 = arg("--seeds").map_or(Ok(150), |s| s.parse())?;
    let rounds: u32 = arg("--rounds").map_or(Ok(3), |s| s.parse())?;
    let limit: f64 = arg("--limit").map_or(Ok(120.0), |s| s.parse())?;
    let moments: usize = arg("--moments").map_or(Ok(5), |s| s.parse())?;
    let out = arg("--out");
    let moments_out = arg("--moments-out");
    let onnx = Onnx::house().map_err(|e| e.0)?;
    let effects = arena_duel::effects::all();
    let log: Log = Rc::new(RefCell::new(Vec::new()));
    let fight_id = Rc::new(RefCell::new(0_usize));
    let bal = Some(Style::Balanced);
    let rows: Vec<(&'static str, Bot, Bot)> = vec![
        ("house vs brain", Bot::House(bal), Bot::Brain),
        ("house vs first", Bot::House(bal), Bot::First),
        ("house vs aggressive", Bot::House(bal), Bot::Aggressive),
        ("house vs turtle", Bot::House(bal), Bot::Turtle),
        ("house mirror", Bot::House(None), Bot::House(None)),
        ("brain vs first", Bot::Brain, Bot::First),
    ];
    let make = |b: Bot| -> MakeSeat {
        let inner: MakeSeat = match b {
            Bot::House(style) => BotSeat::make(
                Rc::new(RefCell::new(Recording {
                    onnx: onnx.clone(),
                    log: Rc::clone(&log),
                })),
                style,
            ),
            Bot::Brain => Brain::make(),
            Bot::First => FirstMind::make(),
            Bot::Aggressive => Scripted::aggressive(),
            Bot::Turtle => Scripted::turtle(),
        };
        let (log, fight) = (Rc::clone(&log), Rc::clone(&fight_id));
        Box::new(move |state, me| {
            Box::new(Stamped {
                inner: inner(state, me),
                log,
                fight,
            })
        })
    };
    let mut tr = Tracker {
        casts: Vec::new(),
        acting: [None, None],
        latest: BTreeMap::new(),
        seconds: BTreeMap::new(),
        hits_on: BTreeMap::new(),
    };
    // each fight: its row, seed, and which bot sits in each seat
    let mut fights: Vec<(&'static str, u32, [&'static str; 2])> = Vec::new();
    for (row, a, b) in &rows {
        eprintln!("{row} ...");
        for seed in 1..=seeds {
            for corner in 0..2_usize {
                let bots = if corner == 0 { [*a, *b] } else { [*b, *a] };
                let names = bots.map(Bot::name);
                let fight = fights.len();
                fights.push((row, seed, names));
                *fight_id.borrow_mut() = fight;
                let config = DuelConfig {
                    seed,
                    left: 0.0,
                    right: 880.0,
                    ground: 600.0,
                    sky: 384.0,
                    effects: effects.clone(),
                };
                let mut duel =
                    Duel::new_with_looks(config, bots.map(make), looks_of(seed + corner as u32));
                tr.acting = [None, None];
                tr.latest.clear();
                let mut started = duel.state().time;
                let mut round = duel.state().round;
                while duel.state().wins[0] + duel.state().wins[1] < rounds {
                    let prev = duel.state().fighters.each_ref().map(|f| f.act);
                    let fighting = duel.state().winner.is_none();
                    let outs = duel.step(TICK);
                    let s = duel.state();
                    if s.round != round {
                        started = s.time;
                        round = s.round;
                        tr.acting = [None, None];
                    }
                    if fighting {
                        for n in names {
                            *tr.seconds.entry(n).or_default() += TICK;
                        }
                        track(&mut tr, s, &prev, &outs, row, fight, seed, names);
                    }
                    if s.winner.is_none() && s.time - started > limit {
                        break;
                    }
                }
            }
        }
    }
    let decs = log.borrow();
    // link each house cast to the decision that chose it: the seat's last cast decision of that skill before it
    {
        let mut by_seat: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (i, d) in decs.iter().enumerate() {
            by_seat.entry((d.fight, d.seat)).or_default().push(i);
        }
        for c in tr.casts.iter_mut().filter(|c| c.bot == "house") {
            let Some(list) = by_seat.get(&(c.fight, c.seat)) else {
                continue;
            };
            c.decision = list.iter().rev().copied().find(|&i| {
                let d = &decs[i];
                d.t <= c.t0 && c.t0 - d.t < 0.5 && cast_skill(d, d.act) == Some(c.spell)
            });
        }
    }
    let picked = report(&tr, &decs, &fights, moments);
    if let Some(path) = moments_out {
        // the decisions behind the example moments' casts: what the house saw, for the critic's value (train/)
        let list: Vec<Value> = picked
            .iter()
            .flatten()
            .filter_map(|&i| tr.casts[i].decision)
            .filter_map(|di| {
                let d = &decs[di];
                let (row, seed, _) = fights[d.fight];
                d.obs.as_ref().map(|o| {
                    json!({"row": row, "seed": seed, "seat": d.seat, "round": d.round, "t": round3(d.t),
                    "gap": round1(d.gap), "act": d.act, "legal": d.legal, "obs": o.to_vec()})
                })
            })
            .collect();
        std::fs::write(&path, serde_json::to_string(&json!({"moments": list}))?)?;
        eprintln!("wrote {} decisions to {path}", list.len());
    }
    if let Some(path) = out {
        let casts: Vec<Value> = tr
            .casts
            .iter()
            .map(|c| {
                json!({"row": c.row, "bot": c.bot, "seed": c.seed, "seat": c.seat, "round": c.round,
                "spell": c.spell, "kind": c.kind, "way": c.way, "t": round3(c.t0),
                "gap0": round1(c.gap0), "gap_act": c.gap_act.map(round1), "reach": c.reach,
                "in_reach_at_cast": c.reach.map(|r| c.gap0 <= r),
                "in_reach_at_active": c.reach.zip(c.gap_act).map(|(r, g)| g <= r),
                "outcome": c.outcome(), "hits": c.hits, "damage": round3(c.damage), "stopped": c.stopped,
                "broken": c.broken, "qi": round1(c.qi), "cooldown": round3(c.cooldown),
                "moved_toward": c.x_end.map(|x| round1((x - c.x0) * c.side)),
                "act_s": c.t_end.map(|e| round3(e - c.t0)),
                "hit_back_1s": c.hit_back_1s, "punished": c.punished})
            })
            .collect();
        std::fs::write(&path, serde_json::to_string(&json!({"casts": casts}))?)?;
        eprintln!("wrote {} casts to {path}", tr.casts.len());
    }
    Ok(())
}

fn round1(x: f64) -> f64 {
    js_round(x * 10.0) / 10.0
}
fn round3(x: f64) -> f64 {
    js_round(x * 1000.0) / 1000.0
}

/// The skill decision `d`'s act `a` casts, if a cast.
fn cast_skill(d: &Dec, a: usize) -> Option<&'static str> {
    match Act::from_index(a)? {
        Act::Cast { slot, .. } => d.slots.get(slot).copied().flatten(),
        _ => None,
    }
}

fn act_name(d: &Dec, a: usize) -> String {
    match Act::from_index(a) {
        Some(Act::Cast { slot, aim }) => {
            let n = cast_skill(d, a).unwrap_or("?");
            match aim {
                Aim::Foe => format!("cast {n}"),
                Aim::Ahead => format!("cast {n}>"),
                Aim::Behind => format!("cast {n}<"),
            }
            .replace("cast ?", &format!("cast slot{slot}"))
        }
        Some(a) => format!("{a:?}").to_lowercase(),
        None => "?".into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn track(
    tr: &mut Tracker,
    s: &State,
    prev: &[Option<arena_duel::fighter::Act>; 2],
    outs: &[Out],
    row: &'static str,
    fight: usize,
    seed: u32,
    names: [&'static str; 2],
) {
    for me in 0..2 {
        let f = &s.fighters[me];
        let cur = f.act;
        let p = prev[me];
        let new = match (p, cur) {
            (_, Some(c)) if c.phase == Phase::Windup => match p {
                None => true,
                Some(p) => p.phase != Phase::Windup || p.spell != c.spell || c.t < p.t,
            },
            _ => false,
        };
        let released = matches!(p, Some(p) if p.phase == Phase::Windup)
            && !matches!(cur, Some(c) if c.phase == Phase::Windup && !new);
        // the act in play before this tick ends or releases
        if let Some(i) = tr.acting[me] {
            if released && tr.casts[i].t_release.is_none() {
                tr.casts[i].t_release = Some(s.time);
                tr.casts[i].gap_act = Some(gap(s, me));
            }
            if cur.is_none() || new {
                tr.casts[i].t_end = Some(s.time);
                tr.casts[i].x_end = Some(f.body.x);
                tr.acting[me] = None;
            }
        }
        if new && let Some(c) = cur {
            let Some(sp) = spell(c.spell) else { continue };
            let ks = f.kit.get(c.spell);
            let way = ks.map_or(Way::Motion, |k| k.way);
            let kind = ks.map_or(sp.kind, |k| k.motion.kind);
            let striking = match way {
                Way::Motion => matches!(
                    kind,
                    Kind::Melee
                        | Kind::Dash
                        | Kind::Blast
                        | Kind::Bolt
                        | Kind::Orb
                        | Kind::Zone
                        | Kind::Void
                ),
                Way::Domain { damaging } => damaging,
                _ => false,
            };
            let foe_x = s.fighters[1 - me].body.x;
            let cast = Cast {
                row,
                bot: names[me],
                fight,
                seed,
                seat: me,
                round: s.round,
                spell: c.spell,
                kind: kind_name(kind),
                way: match way {
                    Way::Device(_) => "Device".into(),
                    Way::Domain { .. } => "Domain".into(),
                    w => format!("{w:?}"),
                },
                striking,
                t0: s.time,
                gap0: gap(s, me),
                reach: reach_of(way, kind, sp),
                x0: f.body.x,
                side: if foe_x >= f.body.x { 1.0 } else { -1.0 },
                walk: f.body.spec.walk,
                qi: ks.map_or(0.0, |k| k.qi()),
                cooldown: ks.map_or(0.0, |k| k.uses.cooldown),
                ..Cast::default()
            };
            tr.casts.push(cast);
            let i = tr.casts.len() - 1;
            tr.acting[me] = Some(i);
            tr.latest.insert((me, c.spell), i);
        }
    }
    let live = |tr: &Tracker, by: usize, spell: &'static str| {
        tr.latest.get(&(by, spell)).copied().filter(|&i| {
            let c = &tr.casts[i];
            c.t_end.is_none_or(|e| s.time - e <= 2.5)
        })
    };
    for o in outs {
        if let Out::Hit {
            by,
            spell,
            share,
            how,
            ..
        } = o
        {
            let by = *by;
            if let Some(i) = live(tr, by, spell) {
                let c = &mut tr.casts[i];
                c.hits += 1;
                c.damage += share;
                c.first_hit.get_or_insert(s.time);
            }
            // the struck: was it within 1 s of one of its casts?
            let struck = 1 - by;
            *tr.hits_on.entry(names[struck]).or_default() += 1;
            for c in tr.casts.iter_mut().rev().take(12) {
                if c.fight == fight && c.seat == struck && s.time - c.t0 <= 1.0 && s.time >= c.t0 {
                    c.hit_back_1s = true;
                    c.taken_1s += share;
                    if *how == How::Punish {
                        c.punished = true;
                    }
                }
            }
        }
    }
    for (_, n) in &s.notes {
        match *n {
            Note::Stopped {
                by,
                spell,
                how,
                chip,
            } => {
                if let Some(i) = live(tr, by, spell) {
                    let c = &mut tr.casts[i];
                    c.stopped += 1;
                    c.damage += chip;
                    if how == Stop::Clash {
                        c.clashed += 1;
                    }
                }
            }
            Note::Broken { who, spell } => {
                if let Some(i) = tr.latest.get(&(who, spell)).copied() {
                    tr.casts[i].broken = true;
                }
            }
            _ => {}
        }
    }
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

/// Prints the tables and the example moments; the moments' runs, by cast index.
fn report(
    tr: &Tracker,
    decs: &[Dec],
    fights: &[(&'static str, u32, [&'static str; 2])],
    moments: usize,
) -> Vec<Vec<usize>> {
    let bots = ["house", "brain", "first", "aggressive", "turtle"];
    println!(
        "per bot x skill (striking casts; utility casts listed with outcome n/a). gap/reach in arena px."
    );
    println!(
        "{:<11} {:<13} {:<7} {:>6} {:>6} {:>6} {:>7} {:>7} {:>6} {:>6} {:>6} {:>7} {:>7}",
        "bot",
        "skill",
        "kind",
        "casts",
        "/min",
        "reach",
        "gap0",
        "out@0%",
        "out@A%",
        "whiff%",
        "stop%",
        "brk%",
        "dmg/c"
    );
    for bot in bots {
        let minutes = tr.seconds.get(bot).copied().unwrap_or(0.0) / 60.0;
        let mut by: BTreeMap<&str, Vec<&Cast>> = BTreeMap::new();
        for c in tr.casts.iter().filter(|c| c.bot == bot) {
            by.entry(c.spell).or_default().push(c);
        }
        let mut rows: Vec<_> = by.into_iter().collect();
        rows.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        for (skill, v) in rows {
            let n = v.len();
            let reach = v[0].reach;
            let striking = v[0].striking;
            let mean_gap = v.iter().map(|c| c.gap0).sum::<f64>() / n as f64;
            let out0 = v
                .iter()
                .filter(|c| c.reach.is_some_and(|r| c.gap0 > r))
                .count();
            let outa = v
                .iter()
                .filter(|c| c.reach.zip(c.gap_act).is_some_and(|(r, g)| g > r))
                .count();
            let cnt = |o: &str| v.iter().filter(|c| c.outcome() == o).count();
            let dmg = v.iter().map(|c| c.damage).sum::<f64>() / n as f64;
            let r = reach.map_or("-".into(), |r| format!("{r:.0}"));
            let opt = |x: f64| {
                if reach.is_some() {
                    format!("{x:.1}")
                } else {
                    "-".into()
                }
            };
            let o = |x: f64| {
                if striking {
                    format!("{x:.1}")
                } else {
                    "n/a".into()
                }
            };
            println!(
                "{:<11} {:<13} {:<7} {:>6} {:>6.1} {:>6} {:>7.0} {:>7} {:>6} {:>6} {:>6} {:>7} {:>7.2}",
                bot,
                skill,
                if v[0].way == "Motion" {
                    v[0].kind
                } else {
                    v[0].way.as_str()
                },
                n,
                n as f64 / minutes.max(1e-9),
                r,
                mean_gap,
                opt(pct(out0, n)),
                opt(pct(outa, n)),
                o(pct(cnt("whiff"), n)),
                o(pct(cnt("stopped"), n)),
                o(pct(cnt("broken"), n)),
                dmg
            );
        }
        println!();
    }
    println!("per bot: totals and the top whiffers (striking casts)");
    for bot in bots {
        let v: Vec<&Cast> = tr
            .casts
            .iter()
            .filter(|c| c.bot == bot && c.striking)
            .collect();
        let minutes = tr.seconds.get(bot).copied().unwrap_or(0.0) / 60.0;
        let wh = v.iter().filter(|c| c.outcome() == "whiff").count();
        let ranged: Vec<&&Cast> = v.iter().filter(|c| c.reach.is_some()).collect();
        let far = ranged.iter().filter(|c| c.far(0.0)).count();
        let far20 = ranged.iter().filter(|c| c.far(20.0)).count();
        let mut whiffers: BTreeMap<&str, usize> = BTreeMap::new();
        for c in v.iter().filter(|c| c.outcome() == "whiff") {
            *whiffers.entry(c.spell).or_default() += 1;
        }
        let mut w: Vec<_> = whiffers.into_iter().collect();
        w.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let top: Vec<String> = w
            .iter()
            .take(4)
            .map(|(s, n)| format!("{s} {n} ({:.1}/min)", *n as f64 / minutes.max(1e-9)))
            .collect();
        println!(
            "  {bot:<10} {:>6} striking casts ({:.1}/min), whiff {:.1}% ({:.1} whiffs/min); finite-reach casts {}: begun out of reach {:.1}%, > reach+20 {:.1}%; top: {}",
            v.len(),
            v.len() as f64 / minutes.max(1e-9),
            pct(wh, v.len()),
            wh as f64 / minutes.max(1e-9),
            ranged.len(),
            pct(far, ranged.len()),
            pct(far20, ranged.len()),
            top.join(", ")
        );
    }
    println!();

    // whiff rate by gap bin, melee only
    println!(
        "melee (blade) casts by gap at cast start - reach: share whiffed, and share in reach at the active frame"
    );
    println!(
        "  {:<10} {:>16} {:>16} {:>16} {:>16} {:>16}",
        "bot", "<=0", "0..20", "20..60", "60..150", ">150"
    );
    for bot in bots {
        let v: Vec<&Cast> = tr
            .casts
            .iter()
            .filter(|c| c.bot == bot && c.melee())
            .collect();
        let bins = [
            (-1e9, 0.0),
            (0.0, 20.0),
            (20.0, 60.0),
            (60.0, 150.0),
            (150.0, 1e9),
        ];
        let cells: Vec<String> = bins
            .iter()
            .map(|(lo, hi)| {
                let b: Vec<&&Cast> = v
                    .iter()
                    .filter(|c| {
                        let d = c.gap0 - c.reach.unwrap_or(0.0);
                        d > *lo && d <= *hi
                    })
                    .collect();
                let w = b.iter().filter(|c| c.outcome() == "whiff").count();
                let ina = b
                    .iter()
                    .filter(|c| c.reach.zip(c.gap_act).is_some_and(|(r, g)| g <= r))
                    .count();
                format!(
                    "{}: {:.0}%w {:.0}%a",
                    b.len(),
                    pct(w, b.len()),
                    pct(ina, b.len())
                )
            })
            .collect();
        println!(
            "  {bot:<10} {}",
            cells.iter().map(|c| format!("{c:>16}")).collect::<String>()
        );
    }
    println!();

    hypotheses(tr, decs);
    spam(tr, decs, fights, moments)
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

// a report: the platform's exp is fine
#[allow(clippy::disallowed_methods)]
fn hypotheses(tr: &Tracker, decs: &[Dec]) {
    println!("=== hypotheses ===");
    // (a) legality vs range, and the policy's own probability of a melee cast by distance
    println!(
        "(a) house decisions: melee basic slots (slash, dive) legal and the policy's probability, by gap - reach"
    );
    for skill in ["slash", "dive"] {
        let reach = spell(skill).and_then(|s| s.range).unwrap_or(0.0);
        let bins = [
            (-1e9, 0.0),
            (0.0, 20.0),
            (20.0, 60.0),
            (60.0, 150.0),
            (150.0, 1e9),
        ];
        let mut cells = Vec::new();
        for (lo, hi) in bins {
            let mut n = 0;
            let mut legal = 0;
            let mut p = Vec::new();
            let mut chose = 0;
            for d in decs {
                let dd = d.gap - reach;
                if !(dd > lo && dd <= hi) {
                    continue;
                }
                let Some(slot) = d.slots.iter().position(|s| *s == Some(skill)) else {
                    continue;
                };
                let a = Act::Cast {
                    slot,
                    aim: Aim::Foe,
                }
                .index();
                n += 1;
                if d.legal[a] {
                    legal += 1;
                    p.push(f64::from(d.probs[a]));
                    if d.act == a {
                        chose += 1;
                    }
                }
            }
            cells.push(format!(
                "n {n} legal {:.0}% p {:.3} chose {:.1}%",
                pct(legal, n),
                mean(&p),
                pct(chose, legal)
            ));
        }
        println!("  {skill} (reach {reach}):");
        for (c, b) in cells
            .iter()
            .zip(["<=0", "0..20", "20..60", "60..150", ">150"])
        {
            println!("    gap-reach {b:<8} {c}");
        }
    }
    let all_legal_far = decs
        .iter()
        .filter(|d| d.gap > 116.0)
        .filter(|d| {
            d.slots.iter().enumerate().any(|(slot, s)| {
                *s == Some("slash")
                    && d.legal[Act::Cast {
                        slot,
                        aim: Aim::Foe,
                    }
                    .index()]
            })
        })
        .count();
    let far = decs.iter().filter(|d| d.gap > 116.0).count();
    println!(
        "  share of decisions at gap > reach+20 (slash) where slash is legal: {:.1}% of {far}",
        pct(all_legal_far, far)
    );

    // (b) cost of a whiff
    println!(
        "(b) the cost of a far melee whiff (gap > reach + 20) vs a melee cast in reach, per bot"
    );
    for bot in ["house", "brain", "first", "aggressive", "turtle"] {
        let mel: Vec<&Cast> = tr
            .casts
            .iter()
            .filter(|c| c.bot == bot && c.melee())
            .collect();
        let far: Vec<&&Cast> = mel
            .iter()
            .filter(|c| c.far(20.0) && c.outcome() == "whiff")
            .collect();
        let near: Vec<&&Cast> = mel
            .iter()
            .filter(|c| c.reach.is_some_and(|r| c.gap0 <= r))
            .collect();
        if mel.is_empty() {
            continue;
        }
        let rate = |v: &[&&Cast], f: &dyn Fn(&Cast) -> bool| {
            pct(v.iter().filter(|c| f(c)).count(), v.len())
        };
        let secs = tr.seconds.get(bot).copied().unwrap_or(1.0);
        let base = tr.hits_on.get(bot).copied().unwrap_or(0) as f64 / secs;
        let by_skill = |v: &[&&Cast]| {
            let mut m: BTreeMap<&str, (usize, f64, f64)> = BTreeMap::new();
            for c in v {
                let e = m.entry(c.spell).or_default();
                e.0 += 1;
                e.1 += c.qi;
                e.2 += c.cooldown;
            }
            m.iter()
                .map(|(s, (n, q, cd))| {
                    format!("{s} {n} qi {:.1} cd {:.2}s", q / *n as f64, cd / *n as f64)
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        println!(
            "  {bot:<10} far whiffs {}: hit back within 1 s {:.1}% (punish {:.1}%), mean taken {:.2}% life | in-reach casts {}: hit back {:.1}% (punish {:.1}%) | baseline: struck {:.2}/s -> P(>=1 in 1 s) ~{:.0}%",
            far.len(),
            rate(&far, &|c| c.hit_back_1s),
            rate(&far, &|c| c.punished),
            mean(&far.iter().map(|c| c.taken_1s).collect::<Vec<_>>()),
            near.len(),
            rate(&near, &|c| c.hit_back_1s),
            rate(&near, &|c| c.punished),
            base,
            100.0 * (1.0 - (-base).exp())
        );
        println!("             far whiffs by skill: {}", by_skill(&far));
    }

    // (c) the retry
    println!(
        "(c) melee hits: landed on the release tick vs later in the window (retry), and from out of reach at the active frame"
    );
    for bot in ["house", "brain", "first", "aggressive", "turtle"] {
        let hits: Vec<&Cast> = tr
            .casts
            .iter()
            .filter(|c| c.bot == bot && c.melee() && c.hits > 0)
            .collect();
        if hits.is_empty() {
            continue;
        }
        let later = hits
            .iter()
            .filter(|c| {
                c.first_hit
                    .zip(c.t_release)
                    .is_some_and(|(h, r)| h > r + 1e-6)
            })
            .count();
        let out_a = hits
            .iter()
            .filter(|c| c.reach.zip(c.gap_act).is_some_and(|(r, g)| g > r))
            .count();
        let far0 = hits.iter().filter(|c| c.far(0.0)).count();
        println!(
            "  {bot:<10} {} melee hits: {:.1}% after the release tick (retry), {:.1}% with the foe out of reach at release, {:.1}% begun out of reach",
            hits.len(),
            pct(later, hits.len()),
            pct(out_a, hits.len()),
            pct(far0, hits.len())
        );
    }
    for skill in ["slash", "dive"] {
        if let Some(sp) = spell(skill) {
            let clip = frames::clip_for(sp);
            for look in [
                arena_duel::data::Look::Thunder,
                arena_duel::data::Look::Frost,
            ] {
                if let Some((open, len)) = frames::window_of(look, clip, sp.windup, sp.recover) {
                    println!(
                        "  {skill} ({clip}, {look:?}): window opens at {open:.2} s, stays open {len:.3} s of a {:.2} s recover",
                        sp.recover
                    );
                }
            }
        }
    }

    // (d) a melee cast's motion
    println!(
        "(d) a melee cast's displacement toward the foe over its act vs walking for the same time"
    );
    for bot in ["house", "brain", "first", "aggressive", "turtle"] {
        for skill in ["slash", "dive"] {
            let v: Vec<&Cast> = tr
                .casts
                .iter()
                .filter(|c| {
                    c.bot == bot && c.spell == skill && c.x_end.is_some() && c.t_end.is_some()
                })
                .collect();
            if v.is_empty() {
                continue;
            }
            let moved: Vec<f64> = v
                .iter()
                .map(|c| (c.x_end.unwrap_or(c.x0) - c.x0) * c.side)
                .collect();
            let dur: Vec<f64> = v.iter().map(|c| c.t_end.unwrap_or(c.t0) - c.t0).collect();
            let walk: Vec<f64> = v.iter().zip(&dur).map(|(c, d)| c.walk * d).collect();
            let far: Vec<f64> = v
                .iter()
                .zip(&moved)
                .filter(|(c, _)| c.far(20.0) && c.outcome() == "whiff")
                .map(|(_, m)| *m)
                .collect();
            println!(
                "  {bot:<10} {skill:<6} n {:>6}: moved {:.1} px toward the foe over {:.2} s (walking: {:.1} px, {:.0} px/s); far whiffs moved {:.1} px",
                v.len(),
                mean(&moved),
                mean(&dur),
                mean(&walk),
                v[0].walk,
                mean(&far)
            );
        }
    }

    // (f) the repetition penalties on the house's actual decisions
    println!(
        "(f) train.rs's REPEAT (0.002, a run of 6) and HABIT (0.004, >= 40% of the last 40) on the house's decisions"
    );
    let mut groups: BTreeMap<(usize, usize, u32), Vec<&Dec>> = BTreeMap::new();
    for d in decs {
        groups
            .entry((d.fight, d.seat, d.round))
            .or_default()
            .push(d);
    }
    let (mut n, mut rep, mut hab, mut slash_hab) = (0usize, 0usize, 0usize, 0usize);
    let mut per_round = Vec::new();
    let mut casts_per_round = Vec::new();
    let mut slash_share = Vec::new();
    for v in groups.values() {
        let (mut last, mut run) = (None, 0u32);
        let mut recent: std::collections::VecDeque<usize> = Default::default();
        let mut pen = 0.0;
        let mut slash = 0;
        for d in v {
            n += 1;
            let a = d.act;
            if a != 0 && last == Some(a) {
                run += 1;
            } else {
                run = 1;
            }
            last = Some(a);
            if a != 0 && run >= 6 {
                rep += 1;
                pen += 0.002;
            }
            let same = recent.iter().filter(|&&x| x == a).count();
            if recent.len() == 40 && same >= 16 {
                hab += 1;
                pen += 0.004;
                if cast_skill(d, a) == Some("slash") {
                    slash_hab += 1;
                }
            }
            recent.push_back(a);
            if recent.len() > 40 {
                recent.pop_front();
            }
            if cast_skill(d, a) == Some("slash") {
                slash += 1;
            }
        }
        per_round.push(pen);
        casts_per_round.push(slash as f64);
        slash_share.push(slash as f64 / v.len().max(1) as f64);
    }
    println!(
        "  {n} decisions in {} seat-rounds ({:.0} a round): REPEAT fired on {:.2}%, HABIT on {:.2}% (on a slash: {slash_hab}); penalty a round {:.4} (vs +-1 for the round); slash chosen {:.1} times a round, {:.1}% of decisions",
        groups.len(),
        n as f64 / groups.len().max(1) as f64,
        pct(rep, n),
        pct(hab, n),
        mean(&per_round),
        mean(&casts_per_round),
        100.0 * mean(&slash_share)
    );
    let mut acts: BTreeMap<String, usize> = BTreeMap::new();
    for d in decs {
        let name = act_name(d, d.act);
        let name = if name.starts_with("cast ") && !name.contains("slash") && !name.contains("dive")
        {
            "cast (other)".to_string()
        } else {
            name
        };
        *acts.entry(name).or_default() += 1;
    }
    let mut a: Vec<_> = acts.into_iter().collect();
    a.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!(
        "  house's act shares: {}",
        a.iter()
            .take(10)
            .map(|(k, v)| format!("{k} {:.1}%", pct(*v, n)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();
}

fn spam(
    tr: &Tracker,
    decs: &[Dec],
    fights: &[(&'static str, u32, [&'static str; 2])],
    moments: usize,
) -> Vec<Vec<usize>> {
    // runs of >= 3 consecutive melee casts by one seat in one round, each a whiff begun > reach + 20 px away
    let mut runs: Vec<Vec<usize>> = Vec::new();
    let mut by: BTreeMap<(usize, usize, u32), Vec<usize>> = BTreeMap::new();
    for (i, c) in tr.casts.iter().enumerate() {
        by.entry((c.fight, c.seat, c.round)).or_default().push(i);
    }
    for list in by.values() {
        let mut cur: Vec<usize> = Vec::new();
        for &i in list {
            let c = &tr.casts[i];
            if c.melee() && c.outcome() == "whiff" && c.far(20.0) {
                cur.push(i);
            } else if c.melee() || c.striking {
                if cur.len() >= 3 {
                    runs.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
        if cur.len() >= 3 {
            runs.push(cur);
        }
    }
    println!(
        "=== whiff spam: runs of >= 3 whiffed melee casts in a row begun > reach + 20 px away ==="
    );
    let mut count: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in &runs {
        let e = count.entry(tr.casts[r[0]].bot).or_default();
        e.0 += 1;
        e.1 += r.len();
    }
    for (bot, (n, casts)) in &count {
        let minutes = tr.seconds.get(bot).copied().unwrap_or(0.0) / 60.0;
        println!(
            "  {bot:<10} {n} runs ({casts} casts, {:.2} runs/min, longest {})",
            *n as f64 / minutes.max(1e-9),
            runs.iter()
                .filter(|r| tr.casts[r[0]].bot == *bot)
                .map(Vec::len)
                .max()
                .unwrap_or(0)
        );
    }
    let mut house: Vec<&Vec<usize>> = runs
        .iter()
        .filter(|r| tr.casts[r[0]].bot == "house")
        .collect();
    house.sort_by_key(|r| std::cmp::Reverse(r.len()));
    // spread the examples over rows
    let mut picked: Vec<&Vec<usize>> = Vec::new();
    for r in &house {
        if picked.len() >= moments {
            break;
        }
        let row = tr.casts[r[0]].row;
        if picked.iter().filter(|p| tr.casts[p[0]].row == row).count() < 2 {
            picked.push(r);
        }
    }
    let picked: Vec<Vec<usize>> = picked.into_iter().cloned().collect();
    for r in &picked {
        let c0 = &tr.casts[r[0]];
        let (row, seed, names) = fights[c0.fight];
        println!(
            "\n-- {row}, seed {seed}, seat {} (house; foe {}), round {}, {} whiffs from t {:.2} s to {:.2} s",
            c0.seat,
            names[1 - c0.seat],
            c0.round,
            r.len(),
            c0.t0,
            tr.casts[*r.last().unwrap_or(&r[0])].t0
        );
        for &i in r.iter() {
            let c = &tr.casts[i];
            println!(
                "   t {:>7.2} {:<6} gap {:>5.0} reach {:>4.0} gap@active {:>5.0} | moved {:>+5.0} px over {:.2} s | dmg {:.1} qi {:.0} cd {:.2}s | foe hit back <1s: {} ({:.1}% life{})",
                c.t0,
                c.spell,
                c.gap0,
                c.reach.unwrap_or(0.0),
                c.gap_act.unwrap_or(f64::NAN),
                (c.x_end.unwrap_or(c.x0) - c.x0) * c.side,
                c.t_end.unwrap_or(c.t0) - c.t0,
                c.damage,
                c.qi,
                c.cooldown,
                if c.hit_back_1s { "yes" } else { "no" },
                c.taken_1s,
                if c.punished { ", PUNISH" } else { "" }
            );
            if let Some(di) = c.decision {
                let d = &decs[di];
                let legal: Vec<String> = (0..ACTIONS)
                    .filter(|&a| d.legal[a])
                    .map(|a| act_name(d, a))
                    .collect();
                let mut top: Vec<(usize, f32)> = d.probs.iter().copied().enumerate().collect();
                top.sort_by(|a, b| b.1.total_cmp(&a.1));
                let top: Vec<String> = top
                    .iter()
                    .take(5)
                    .map(|(a, p)| format!("{} {:.3}", act_name(d, *a), p))
                    .collect();
                println!(
                    "      decided t {:.2} at gap {:.0}: chose {} | legal: {} | top-5: {}",
                    d.t,
                    d.gap,
                    act_name(d, d.act),
                    legal.join(", "),
                    top.join(", ")
                );
            }
        }
    }
    println!();
    picked
}

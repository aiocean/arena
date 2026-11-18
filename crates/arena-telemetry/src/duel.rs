//! The duel observer: what one `Duel` step returned (its `Out` events and the state's `Note`s) and the duel's state
//! after it, as structured events. It only reads: it rolls no dice and changes nothing, so a fight observed is the
//! fight unobserved, tick for tick (`tests/observer.rs`).
//!
//! Events, each with `match_id` and `round` (and inside a `match` span and a `round` span):
//!
//! - target `arena::match`: `match.start` (one per seat: seed, its bot, its loadout; from the
//!   live server also what runs the bot, `runtime`, and the server's commit, `build`),
//!   `round.start`, `round.end` (winner, seconds), `forfeit` (a seat that gave up: a bot that faulted out),
//!   `match.end` (result, rounds, seconds, cut);
//! - target `arena::duel`: `cast`, `hit`, `stopped` (guard, barrier, ward, dodge, clash),
//!   `broken`, `dot`, `heal`, `ward`, `barrier`, `unyielding`, `ko`, `status`, `ailment`, `curse`,
//!   `unit.spawn`, `unit.act`, `unit.hurt`, `unit.die`, and a `snapshot` of each fighter every `every` seconds of
//!   fight time;
//! - target `arena::metrics`: `arena_matches_total`, `arena_rounds_total`, `arena_casts_total` (skill),
//!   `arena_damage_total` (skill, kind), `arena_step_seconds` (a step's wall time).
//!
//! A cast is read from the fighter's act (a new act, or the same skill begun again): a skill with no windup and no
//! recover that never stands as an act within one step is not seen as a cast, though its hits are.

use crate::target;
use arena_calc::ailment::{AILMENTS, Ailment};
use arena_calc::gem::gem;
use arena_calc::keystone::ANCIENT_QI_MAX;
use arena_calc::loadout::Loadout;
use arena_duel::duel::Duel;
use arena_duel::fighter::{Fighter, Phase, State};
use arena_duel::out::{How, Note, Out, Stop};
use std::time::{Duration, Instant};
use tracing::{Level, Span, event};

/// What the observer remembers of a fighter between steps, to see what changed.
#[derive(Clone, Debug, Default, PartialEq)]
struct Seen {
    act: Option<(&'static str, bool, f64)>,
    ailments: [u32; AILMENTS],
    curses: Vec<String>,
    statuses: [bool; STATUSES.len()],
}

/// What a seat's damage over time is made of: the rules note each step's burn and bleed (`Note::Ticked`) and show their
/// sum every `DOT_SHOWN` seconds (`Out::Tick`); a shown tick that matches none is a curse's strike.
const DOT_CAUSES: [&str; 2] = ["burn", "bleed"];

/// How long a status must hold before it is reported, seconds of fight time.
const HOLD: f64 = 0.25;

/// The statuses reported as they turn on and off.
const STATUSES: [&str; 9] = [
    "guard", "meditate", "fly", "sealed", "rooted", "no_guard", "silenced", "barrier", "down",
];

fn statuses(f: &Fighter) -> [bool; STATUSES.len()] {
    [
        f.guard > 0.0,
        f.meditating(),
        f.fly,
        f.sealed > 0.0,
        f.root > 0.0,
        f.no_guard > 0.0,
        f.silenced(),
        f.barrier > 0.0,
        f.body.down.is_some(),
    ]
}

fn ailments(f: &Fighter) -> [u32; AILMENTS] {
    Ailment::ALL.map(|a| f.c.ailments.stacks(a).max(0.0) as u32)
}

fn curses(f: &Fighter) -> Vec<String> {
    f.curses
        .on
        .iter()
        .map(|c| format!("{:?}", c.kind).to_lowercase())
        .collect()
}

fn seen(f: &Fighter) -> Seen {
    Seen {
        act: f.act.map(|a| (a.spell, a.phase == Phase::Windup, a.t)),
        ailments: ailments(f),
        curses: curses(f),
        statuses: statuses(f),
    }
}

/// `name:stacks` of every ailment on, comma separated.
fn ailment_list(stacks: &[u32; AILMENTS]) -> String {
    Ailment::ALL
        .iter()
        .zip(stacks)
        .filter(|(_, n)| **n > 0)
        .map(|(a, n)| format!("{}:{n}", a.name()))
        .collect::<Vec<_>>()
        .join(",")
}

/// Whether a qi bar of `qi_max` is the uncapped one (Cổ Thần Quyết's `ANCIENT_QI_MAX`), not a size.
pub fn qi_uncapped(qi_max: f64) -> bool {
    qi_max >= ANCIENT_QI_MAX
}

/// A loadout's parts as flat strings, as every event carries them: its actives (`skill[support+support]`, comma
/// separated), its supports, its keystones and its root element.
pub fn loadout_fields(l: &Loadout) -> (String, String, String, String) {
    let actives = l
        .actives
        .iter()
        .map(|s| {
            if s.supports.is_empty() {
                s.skill.clone()
            } else {
                format!("{}[{}]", s.skill, s.supports.join("+"))
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let supports = l
        .actives
        .iter()
        .flat_map(|s| s.supports.iter().cloned())
        .collect::<Vec<_>>()
        .join(",");
    let keystones = l
        .keystones
        .iter()
        .map(|k| format!("{k:?}").to_lowercase())
        .collect::<Vec<_>>()
        .join(",");
    let root = l.root.map_or(String::new(), |e| e.name().to_string());
    (actives, supports, keystones, root)
}

/// A skill's display name, its tags and its kinds of damage (`fire,thunder`), from the pool.
fn skill_info(id: &str) -> (&'static str, String, String) {
    match gem(id) {
        Some(g) => (
            g.name,
            g.tags
                .iter()
                .map(|t| t.name())
                .collect::<Vec<_>>()
                .join(","),
            g.split
                .iter()
                .map(|(e, _)| e.name())
                .collect::<Vec<_>>()
                .join(","),
        ),
        None => ("", String::new(), String::new()),
    }
}

fn how(h: How) -> &'static str {
    match h {
        How::Plain => "plain",
        How::Counter => "counter",
        How::Punish => "punish",
        How::Meditate => "meditate",
    }
}

fn stop(s: Stop) -> &'static str {
    match s {
        Stop::Guard => "guard",
        Stop::Barrier => "barrier",
        Stop::Ward => "ward",
        Stop::Dodge => "dodge",
        Stop::Clash => "clash",
    }
}

/// Watches one match. Make it when the duel is made (`start`), hand it every step (`step`, or `observe` after a
/// step of your own), and `end` it when the match is over.
pub struct Observer {
    match_id: String,
    seats: [String; 2],
    span: Span,
    round_span: Span,
    round: u32,
    round_began: f64,
    round_over: bool,
    every: f64,
    next_snapshot: f64,
    seen: [Seen; 2],
    /// Each fighter's statuses as last reported, and since when a different one has held.
    reported: [[bool; STATUSES.len()]; 2],
    since: [[Option<f64>; STATUSES.len()]; 2],
    began: Instant,
    /// Each seat's forfeit, once reported.
    forfeited: [bool; 2],
    /// The burn and bleed shares each seat's damage over time took since its last shown tick (`DOT_CAUSES`).
    dots: [[f64; DOT_CAUSES.len()]; 2],
    on: bool,
    ended: bool,
}

impl Observer {
    /// Begin watching `duel` as match `match_id`, its seats named `seats` (a bot, a brain, a corner): emits
    /// `match.start` and opens the match's span. Snapshots every second of fight time (`every`).
    pub fn start(duel: &Duel, match_id: impl Into<String>, seats: [&str; 2]) -> Observer {
        Observer::begin(duel, match_id.into(), seats, [None, None], None)
    }

    /// As [`Observer::start`], each `match.start` row also saying what runs its seat's bot (`runtime`: `onnx`,
    /// `wasm`, `builtin`) and the build of the program fighting the match (`build`: its commit), as the live server
    /// reports its matches.
    pub fn start_seated(
        duel: &Duel,
        match_id: impl Into<String>,
        seats: [&str; 2],
        runtimes: [&str; 2],
        build: &str,
    ) -> Observer {
        Observer::begin(
            duel,
            match_id.into(),
            seats,
            runtimes.map(Some),
            Some(build),
        )
    }

    fn begin(
        duel: &Duel,
        match_id: String,
        seats: [&str; 2],
        runtimes: [Option<&str>; 2],
        build: Option<&str>,
    ) -> Observer {
        let s = duel.state();
        let on = tracing::enabled!(target: target::MATCH, Level::INFO)
            || tracing::enabled!(target: target::DUEL, Level::INFO)
            || tracing::enabled!(target: target::METRICS, Level::INFO);
        let span = tracing::info_span!(target: target::MATCH, "match", match_id = %match_id, seed = s.seed);
        let mut o = Observer {
            match_id,
            seats: seats.map(str::to_string),
            span,
            round_span: Span::none(),
            round: 0,
            round_began: s.time,
            round_over: false,
            every: 1.0,
            next_snapshot: s.time,
            seen: [seen(&s.fighters[0]), seen(&s.fighters[1])],
            reported: [statuses(&s.fighters[0]), statuses(&s.fighters[1])],
            since: [[None; STATUSES.len()]; 2],
            began: Instant::now(),
            forfeited: [false; 2],
            dots: [[0.0; DOT_CAUSES.len()]; 2],
            on,
            ended: false,
        };
        if on {
            o.span.in_scope(|| {
                for (i, f) in s.fighters.iter().enumerate() {
                    let (actives, supports, keystones, root) = loadout_fields(&f.loadout);
                    // Cổ Thần Quyết has no qi cap: the rules give it a bar of `ANCIENT_QI_MAX`, which is no size to
                    // chart, so the start leaves `qi` out (null in the store) and says `qi_uncapped`
                    let uncapped = qi_uncapped(f.c.qi_max);
                    let qi = (!uncapped).then_some(f.c.qi_max);
                    event!(target: target::MATCH, Level::INFO, {
                        match_id = %o.match_id, seed = s.seed, seat = i, bot = %o.seats[i],
                        actives = %actives, supports = %supports,
                        keystones = %keystones, root = %root, life = f.c.life_max, qi = qi,
                        qi_uncapped = uncapped, foe = %o.seats[1 - i], runtime = runtimes[i], build = build,
                    }, "match.start");
                }
                event!(target: target::METRICS, Level::INFO, { monotonic_counter.arena_matches_total = 1_u64 });
            });
            o.new_round(s);
        }
        o
    }

    /// Snapshot every `seconds` of fight time instead (0 or less: never).
    pub fn snapshot_every(mut self, seconds: f64) -> Observer {
        self.every = seconds;
        self
    }

    /// The match's id.
    pub fn match_id(&self) -> &str {
        &self.match_id
    }

    /// The match's span, to run a caller's own events inside it.
    pub fn span(&self) -> &Span {
        &self.span
    }

    /// Step `duel` by `dt` (timed) and observe the step; returns what the step returned.
    pub fn step(&mut self, duel: &mut Duel, dt: f64) -> Vec<Out> {
        let t = Instant::now();
        let out = duel.step(dt);
        let took = t.elapsed();
        self.observe(duel, &out, Some(took));
        out
    }

    /// Observe a step already taken: `out` is what it returned, `took` how long it ran.
    pub fn observe(&mut self, duel: &Duel, out: &[Out], took: Option<Duration>) {
        if !self.on || self.ended {
            return;
        }
        let s = duel.state();
        if let Some(took) = took {
            event!(target: target::METRICS, Level::INFO, { histogram.arena_step_seconds = took.as_secs_f64() });
        }
        if s.round != self.round {
            self.new_round(s);
        }
        let (match_span, round_span) = (self.span.clone(), self.round_span.clone());
        let _match = match_span.enter();
        let _round = round_span.enter();
        self.events(s, out);
        self.changes(s);
        for seat in 0..2 {
            if !self.forfeited[seat] && duel.seat(seat).forfeit() {
                self.forfeited[seat] = true;
                event!(target: target::MATCH, Level::WARN, {
                    match_id = %self.match_id, round = self.round, seat = seat, bot = %self.seats[seat],
                    seconds = s.time,
                }, "forfeit");
            }
        }
        if let Some(w) = s.winner
            && !self.round_over
        {
            self.round_over = true;
            event!(target: target::MATCH, Level::INFO, {
                match_id = %self.match_id, round = self.round, winner = w, winner_bot = %self.seats[w],
                seconds = s.time - self.round_began, wins_0 = s.wins[0], wins_1 = s.wins[1],
                life_0 = s.fighters[0].hp(), life_1 = s.fighters[1].hp(),
            }, "round.end");
        }
        if self.every > 0.0 && s.time >= self.next_snapshot {
            self.next_snapshot = s.time + self.every;
            self.snapshot(s);
        }
    }

    /// The match is over: emits `match.end` with its result. `cut` when it was stopped before anyone won it (a time
    /// or tick limit).
    pub fn end(mut self, duel: &Duel, cut: bool) {
        self.finish(duel.state(), cut, None);
    }

    /// The match was stopped from outside before it ended (the server shutting down: `"server_stopped"`): emits
    /// `match.end` with `result` and `cut`, and the fight as it stood.
    pub fn stop(mut self, duel: &Duel, result: &str) {
        self.finish(duel.state(), true, Some(result));
    }

    fn finish(&mut self, s: &State, cut: bool, stopped: Option<&str>) {
        if self.ended || !self.on {
            self.ended = true;
            return;
        }
        self.ended = true;
        let result = match (stopped, s.wins[0].cmp(&s.wins[1])) {
            (Some(why), _) => why,
            _ if cut => "cut",
            (None, std::cmp::Ordering::Greater) => "seat0",
            (None, std::cmp::Ordering::Less) => "seat1",
            (None, std::cmp::Ordering::Equal) => "draw",
        };
        let leader = match s.wins[0].cmp(&s.wins[1]) {
            std::cmp::Ordering::Greater => self.seats[0].as_str(),
            std::cmp::Ordering::Less => self.seats[1].as_str(),
            std::cmp::Ordering::Equal => "",
        };
        self.round_span = Span::none();
        self.span.in_scope(|| {
            event!(target: target::MATCH, Level::INFO, {
                match_id = %self.match_id, result = result, winner_bot = %leader, cut = cut,
                rounds = s.round, wins_0 = s.wins[0], wins_1 = s.wins[1], seconds = s.time,
                ticks = s.ticks, wall_seconds = self.began.elapsed().as_secs_f64(),
                bot_0 = %self.seats[0], bot_1 = %self.seats[1],
            }, "match.end");
        });
    }

    fn new_round(&mut self, s: &State) {
        self.round = s.round;
        self.round_began = s.time;
        self.round_over = false;
        self.round_span = tracing::info_span!(target: target::MATCH, parent: &self.span, "round",
            match_id = %self.match_id, round = s.round);
        self.round_span.in_scope(|| {
            event!(target: target::MATCH, Level::INFO, {
                match_id = %self.match_id, round = s.round, wins_0 = s.wins[0], wins_1 = s.wins[1],
            }, "round.start");
            event!(target: target::METRICS, Level::INFO, { monotonic_counter.arena_rounds_total = 1_u64 });
        });
        self.seen = [seen(&s.fighters[0]), seen(&s.fighters[1])];
        self.reported = [statuses(&s.fighters[0]), statuses(&s.fighters[1])];
        self.since = [[None; STATUSES.len()]; 2];
        self.dots = [[0.0; DOT_CAUSES.len()]; 2];
    }

    /// The step's events and notes, in the order the rules made them.
    fn events(&mut self, s: &State, out: &[Out]) {
        let (match_id, round) = (self.match_id.clone(), self.round);
        let id = match_id.as_str();
        // ailments that rose on each fighter this step: laid by the blows that hit it
        let laid: [String; 2] = [0, 1].map(|f| {
            let now = ailments(&s.fighters[f]);
            Ailment::ALL
                .iter()
                .zip(now.iter().zip(&self.seen[f].ailments))
                .filter(|(_, (n, b))| n > b)
                .map(|(a, _)| a.name())
                .collect::<Vec<_>>()
                .join(",")
        });
        for (_, n) in &s.notes {
            if let Note::Ticked { by, cause, share } = *n
                && let Some(i) = DOT_CAUSES.iter().position(|c| *c == cause)
            {
                self.dots[by.min(1)][i] += share;
            }
        }
        for o in out {
            match *o {
                Out::Hit {
                    by,
                    spell,
                    damage,
                    crit,
                    how: h,
                    share,
                    launch,
                    ranged,
                    power,
                    ..
                } => {
                    let (name, tags, kinds) = skill_info(spell);
                    let foe = 1 - by.min(1);
                    event!(target: target::DUEL, Level::INFO, {
                        match_id = id, round = round, by = by, bot = %self.seats[by.min(1)], skill = spell,
                        skill_name = name, tags = %tags, kinds = %kinds, damage = damage, share = share,
                        how = how(h), crit = crit, launch = launch, ranged = ranged, power = power,
                        ailments = %laid[foe], foe_life = s.fighters[foe].hp(),
                    }, "hit");
                    self.damage_metric(spell, damage);
                }
                Out::Tick {
                    by, damage, share, ..
                } => {
                    let cause = self.dot_cause(by.min(1), share);
                    event!(target: target::DUEL, Level::INFO, {
                        match_id = id, round = round, by = by, damage = damage, share = share, cause = %cause,
                    }, "dot");
                    self.damage_metric(&cause, damage);
                }
                Out::Heal { by, amount, .. } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, by = by, amount = amount,
                }, "heal"),
                Out::Ward { by, amount, .. } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, by = by, amount = amount,
                }, "ward"),
                Out::Barrier { by, .. } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, by = by,
                }, "barrier"),
                Out::Unyielding { by, .. } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, by = by,
                }, "unyielding"),
                Out::Ko { x, .. } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, x = x, life_0 = s.fighters[0].hp(), life_1 = s.fighters[1].hp(),
                    winner = s.winner.map_or(-1, |w| w as i64), seconds = s.time - self.round_began,
                }, "ko"),
                Out::Spawn {
                    id: unit,
                    by,
                    kind,
                    skill,
                    x,
                } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, unit = unit, by = by, kind = %format!("{kind:?}").to_lowercase(),
                    skill = skill, x = x,
                }, "unit.spawn"),
                Out::UnitAct { id: unit, x } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, unit = unit, x = x,
                }, "unit.act"),
                Out::UnitHurt {
                    id: unit,
                    by,
                    damage,
                    share,
                    ..
                } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, unit = unit, by = by, damage = damage, share = share,
                }, "unit.hurt"),
                Out::UnitDie { id: unit, x } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, unit = unit, x = x,
                }, "unit.die"),
                _ => {}
            }
        }
        for (_, n) in &s.notes {
            match *n {
                Note::Stopped {
                    by,
                    spell,
                    how: h,
                    chip,
                } => {
                    event!(target: target::DUEL, Level::INFO, {
                        match_id = id, round = round, by = by, bot = %self.seats[by.min(1)], skill = spell,
                        how = stop(h), chip = chip,
                    }, "stopped");
                }
                Note::Broken { who, spell } => event!(target: target::DUEL, Level::INFO, {
                    match_id = id, round = round, who = who, skill = spell,
                }, "broken"),
                Note::Ticked { .. } | Note::Healed { .. } => {}
            }
        }
    }

    /// What a shown tick of seat `by`'s damage over time (`share` of its foe's life) was: the burns and bleeds noted
    /// since its last one when they add up to it (`burn`, `bleed`, or `burn+bleed`), else a curse's strike (`curse`).
    fn dot_cause(&mut self, by: usize, share: f64) -> String {
        let noted: f64 = self.dots[by].iter().sum();
        if noted <= 0.0 || (noted - share).abs() > 1e-6 * share.abs().max(1e-3) {
            return "curse".to_string();
        }
        let cause = DOT_CAUSES
            .iter()
            .zip(self.dots[by])
            .filter(|(_, v)| *v > 0.0)
            .map(|(c, _)| *c)
            .collect::<Vec<_>>()
            .join("+");
        self.dots[by] = [0.0; DOT_CAUSES.len()];
        cause
    }

    fn damage_metric(&self, skill: &str, damage: f64) {
        match gem(skill) {
            Some(g) if !g.split.is_empty() => {
                let total: f64 = g.split.iter().map(|(_, w)| w).sum();
                for (e, w) in g.split {
                    let part = if total > 0.0 { damage * w / total } else { 0.0 };
                    event!(target: target::METRICS, Level::INFO, {
                        monotonic_counter.arena_damage_total = part, skill = skill, kind = e.name(),
                    });
                }
            }
            _ => event!(target: target::METRICS, Level::INFO, {
                monotonic_counter.arena_damage_total = damage, skill = skill, kind = "other",
            }),
        }
    }

    /// What changed on each fighter since the last step: casts, statuses, ailments, curses.
    fn changes(&mut self, s: &State) {
        let (id, round) = (self.match_id.clone(), self.round);
        for (who, f) in s.fighters.iter().enumerate() {
            let now = seen(f);
            let was = &self.seen[who];
            if let Some((spell, windup, t)) = now.act {
                let fresh = match was.act {
                    None => true,
                    Some((before, was_windup, was_t)) => {
                        before != spell || t < was_t || (windup && !was_windup)
                    }
                };
                if fresh {
                    let (name, tags, kinds) = skill_info(spell);
                    let k = f.kit.get(spell);
                    event!(target: target::DUEL, Level::INFO, {
                        match_id = %id, round = round, who = who, bot = %self.seats[who], skill = spell,
                        skill_name = name, tags = %tags, kinds = %kinds,
                        basic = k.is_some_and(|k| matches!(k.place, arena_duel::kit::Place::Basic)),
                        qi = f.qi(),
                    }, "cast");
                    event!(target: target::METRICS, Level::INFO, {
                        monotonic_counter.arena_casts_total = 1_u64, skill = spell,
                    });
                }
            }
            // a status is reported once it has held `HOLD` seconds: a flight's every hop is not news
            for (i, name) in STATUSES.iter().enumerate() {
                let reported = self.reported[who][i];
                if now.statuses[i] == reported {
                    self.since[who][i] = None;
                    continue;
                }
                let since = *self.since[who][i].get_or_insert(s.time);
                if s.time - since >= HOLD {
                    self.reported[who][i] = now.statuses[i];
                    self.since[who][i] = None;
                    event!(target: target::DUEL, Level::INFO, {
                        match_id = %id, round = round, who = who, status = *name, on = now.statuses[i],
                    }, "status");
                }
            }
            for (i, a) in Ailment::ALL.iter().enumerate() {
                let (n, b) = (now.ailments[i], was.ailments[i]);
                if n != b {
                    event!(target: target::DUEL, Level::INFO, {
                        match_id = %id, round = round, who = who, ailment = a.name(), stacks = n, before = b,
                    }, "ailment");
                }
            }
            for c in now.curses.iter().filter(|c| !was.curses.contains(c)) {
                event!(target: target::DUEL, Level::INFO, {
                    match_id = %id, round = round, who = who, curse = %c, on = true,
                }, "curse");
            }
            for c in was.curses.iter().filter(|c| !now.curses.contains(c)) {
                event!(target: target::DUEL, Level::INFO, {
                    match_id = %id, round = round, who = who, curse = %c, on = false,
                }, "curse");
            }
            self.seen[who] = now;
        }
    }

    fn snapshot(&self, s: &State) {
        for (who, f) in s.fighters.iter().enumerate() {
            let units = s.units.iter().filter(|u| u.owner == who).count();
            event!(target: target::DUEL, Level::INFO, {
                match_id = %self.match_id, round = self.round, who = who, bot = %self.seats[who], time = s.time,
                life = f.c.life, life_max = f.c.life_max, shield = f.c.shield, sense = f.c.sense, qi = f.c.qi,
                x = f.body.x, y = f.body.air, ailments = %ailment_list(&ailments(f)),
                curses = %curses(f).join(","), units = units, act = f.act.map_or("", |a| a.spell),
            }, "snapshot");
        }
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        // an observer dropped without `end` still closes its match, as cut
        if !self.ended && self.on {
            self.ended = true;
            self.span.in_scope(|| {
                event!(target: target::MATCH, Level::INFO, {
                    match_id = %self.match_id, result = "abandoned", cut = true,
                    wall_seconds = self.began.elapsed().as_secs_f64(),
                }, "match.end");
            });
        }
    }
}

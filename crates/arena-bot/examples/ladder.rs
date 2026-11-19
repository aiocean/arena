//! The ladder: a bot (the house AI by default) against the brain, the first mind, the scripted fighters and itself, printed as
//! the "Done when" table of `docs/specs/2026-11-15-duel-rules-in-rust-design.md`.
//!
//! `cargo run --release -p arena-bot --features host,onnx --example ladder -- [--seeds 150] [--rounds 3] [--limit 120]
//! [--onnx FILE | --bot FILE.wasm] [--telemetry] [--telemetry-dir DIR]`
//!
//! The bot is the house AI (`weights/policy.onnx`) unless `--onnx` names another ONNX policy or `--bot` a bot module.
//!
//! `--telemetry` adds, for each mirror, what each seat's skills did, seat 0 beside seat 1.
//!
//! `--telemetry-dir DIR` (or `ARENA_TELEMETRY_DIR` set) writes each row and each seat's skill table to the telemetry
//! files as `arena::ladder` lines under one `run_id`, as service `arena-ladder` (`ops/telemetry/README.md`).
//! The bot's own measures (think-time and fuel percentiles, faults) go as `bot.end` events, one an instance (a row),
//! under service `arena-bot`, with `runtime` "onnx" or "wasm" and `bot` "house", `onnx-<hash>` or `<file>-<hash>`.

use arena_bot::host::Runner;
use arena_bot::onnx::Onnx;
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::first_mind::FirstMind;
use arena_duel::ladder::{Match, Record, SeatLog, SkillLog, match_up};
use arena_duel::scripted::Scripted;
use arena_duel::seat::{BotSeat, Brain, MakeSeat, SharedDecider, policy_mind};
use arena_duel::sense::sense_of;
use arena_duel::style::Style;
use std::rc::Rc;
use std::time::Instant;

/// Makes a fresh seat for each fight.
type Maker<'a> = Box<dyn Fn() -> MakeSeat + 'a>;

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
    let telemetry = args.iter().any(|a| a == "--telemetry");
    let _export = arena_telemetry::requested(&args)?.map(|dir| {
        arena_telemetry::init(
            "arena-ladder",
            arena_telemetry::Options {
                dir: Some(dir),
                ..arena_telemetry::Options::tool(env!("CARGO_PKG_VERSION"))
            },
        )
    });
    let run = arena_telemetry::run_id("ladder");
    let new: Rc<dyn Fn() -> SharedDecider> = match (arg("--bot"), arg("--onnx")) {
        (Some(path), _) => {
            let admitted =
                arena_bot::admit(&std::fs::read(&path)?).map_err(|e| format!("{path}: {e}"))?;
            let runner = Runner::new(&admitted).map_err(|e| format!("{path}: {}", e.0))?;
            // its telemetry's `bot`: the file's name, then the start of its hash
            let stem = std::path::Path::new(&path)
                .file_stem()
                .map_or_else(|| "module".into(), |s| s.to_string_lossy());
            let runner = runner.named(&format!("{stem}-{}", &admitted.hash[..8]));
            Rc::new(move || runner.decider())
        }
        (None, path) => {
            let onnx = match path {
                Some(path) => {
                    Onnx::load(&std::fs::read(&path)?).map_err(|e| format!("{path}: {}", e.0))?
                }
                None => Onnx::house().map_err(|e| e.0)?,
            };
            Rc::new(move || onnx.decider())
        }
    };
    let effects: Vec<String> = arena_duel::effects::all();
    let m = Match {
        seeds: 1..seeds + 1,
        rounds,
        limit,
        effects: effects.clone(),
    };
    // one instance a row (a match), as the tournament plays a module
    let styled = |s: Style| {
        let bot = new();
        move || BotSeat::make(Rc::clone(&bot), Some(s))
    };
    let drawn = || {
        let bot = new();
        move || BotSeat::make(Rc::clone(&bot), None)
    };

    println!("ladder: {seeds} seeds x both ways x {rounds} rounds, rounds cut at {limit} s\n");
    println!(
        "{:<34} {:>7} {:>9} {:>7} {:>7} {:>5} {:>6} {:>7} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "match",
        "win",
        "seat 0",
        "seat 1",
        "len s",
        "cut",
        "top",
        "attack",
        "dist",
        "median",
        "<80",
        "<40",
        "c/min",
        "w/min",
        "air",
        "guard",
        "topdm"
    );
    println!(
        "  (c/min, w/min: the first contestant's casts of a hurting skill and its whiffs a minute; air, guard: its \
         share of the time off the ground and guarding; topdm: its heaviest skill's share of its damage)"
    );
    // each row: its name, its seats, and whether it is a mirror whose seats the telemetry compares
    let rows: Vec<(&str, Maker<'_>, Maker<'_>, bool)> = vec![
        (
            "balanced vs brain (>=75%, 65% each)",
            Box::new(styled(Style::Balanced)),
            Box::new(Brain::make),
            false,
        ),
        (
            "balanced vs first mind (>=80%)",
            Box::new(styled(Style::Balanced)),
            Box::new(FirstMind::make),
            false,
        ),
        (
            "balanced vs aggressive script",
            Box::new(styled(Style::Balanced)),
            Box::new(Scripted::aggressive),
            false,
        ),
        (
            "balanced vs turtle script",
            Box::new(styled(Style::Balanced)),
            Box::new(Scripted::turtle),
            false,
        ),
        (
            "mirror, styles drawn (45-55%)",
            Box::new(drawn()),
            Box::new(drawn()),
            true,
        ),
        (
            "brain vs first mind (baseline)",
            Box::new(Brain::make),
            Box::new(FirstMind::make),
            false,
        ),
        (
            "brain mirror (seat baseline)",
            Box::new(Brain::make),
            Box::new(Brain::make),
            true,
        ),
        (
            "first mind mirror (baseline)",
            Box::new(FirstMind::make),
            Box::new(FirstMind::make),
            true,
        ),
    ];
    let mut mirrors = Vec::new();
    let mut all = Vec::new();
    for (name, a, b, mirror) in &rows {
        let r = match_up(a.as_ref(), b.as_ref(), &m);
        print_row(name, &r);
        export_row(&run, name, &m, &r);
        if *mirror {
            mirrors.push((*name, r));
        } else {
            all.push((*name, r));
        }
    }
    println!("\nmirrors: the win column is seat 0's share of the rounds in the seat 0 column");
    println!(
        "looks are drawn apart from seats (each seed fought in both looks, `ladder::looks_of`):"
    );
    for (name, r) in &all {
        if !mirrors.iter().any(|(n, _)| n == name) {
            println!(
                "  {name:<36} as Vương Lâm {:.1}%, as Thất Thải {:.1}%",
                100.0 * r.as_look(0),
                100.0 * r.as_look(1)
            );
        }
    }
    for (name, r) in &mirrors {
        let all = f64::from((r.by_look[0] + r.by_look[1]).max(1));
        println!(
            "  {name:<36} Vương Lâm won {:.1}% of the rounds, Thất Thải {:.1}%",
            100.0 * f64::from(r.by_look[0]) / all,
            100.0 * f64::from(r.by_look[1]) / all
        );
    }
    println!();
    if telemetry {
        for (name, r) in &mirrors {
            print_telemetry(name, r);
        }
    }
    // a benchmark, not a match: its instance is kept out of the telemetry (bot.end, think time)
    tracing::subscriber::with_default(tracing::subscriber::NoSubscriber::default(), || {
        decision_cost(&*new, &effects);
    });
    Ok(())
}

/// A row and each seat's skill table as `arena::ladder` events (nothing when telemetry is off).
fn export_row(run: &str, name: &str, m: &Match, r: &Record) {
    let top = r
        .choices
        .iter()
        .max_by_key(|(_, n)| **n)
        .map_or("-", |(k, _)| k.as_str());
    let me = &r.first;
    let top_skill = heaviest(me);
    tracing::info!(target: "arena::ladder", run_id = run, row = name, seeds = m.seeds.len(), rounds = m.rounds,
        win = r.win_rate(), seat0 = r.as_seat(0), seat1 = r.as_seat(1), won_0 = r.won[0], won_1 = r.won[1],
        lost_0 = r.lost[0], lost_1 = r.lost[1], round_seconds = r.mean_length(), timeouts = r.timeouts,
        top = top, top_share = r.most_used(), attack_share = r.attack_share(), distance = r.mean_distance,
        median_gap = r.median_gap(), within_80 = r.within(80), within_40 = r.within(40),
        casts_per_min = me.per_minute(me.attacks()), whiffs_per_min = me.per_minute(me.whiffs),
        airborne = pct(me.airborne, me.seconds) / 100.0, guarding = pct(me.guarding, me.seconds) / 100.0,
        top_skill = top_skill.0, top_skill_share = top_skill.1 / 100.0, "ladder.row");
    for (seat, log) in r.seats.iter().enumerate() {
        let dealt = log.dealt();
        for (skill, k) in &log.skills {
            tracing::info!(target: "arena::ladder", run_id = run, row = name, seat = seat, skill = *skill,
                casts = k.casts, landed = k.landed, damage = k.damage, damage_share = pct(k.damage, dealt),
                guarded = k.guarded, chip = k.chip, barrier = k.barrier, ward = k.ward, dodged = k.dodged,
                clashed = k.clashed, broken = k.broken, punished = k.punished,
                finals = log.finals.get(skill).copied().unwrap_or(0), "ladder.skill");
        }
        tracing::info!(target: "arena::ladder", run_id = run, row = name, seat = seat, dealt = dealt,
            lost = log.life_lost, healed = log.healed, qi_spent = log.qi_spent,
            seconds = log.seconds, meditating = log.meditating, guarding = log.guarding, flying = log.flying,
            airborne = log.airborne, attacks = log.attacks(), whiffs = log.whiffs, "ladder.seat");
    }
}

fn print_row(name: &str, r: &Record) {
    let pct = |x: f64| format!("{:.1}%", x * 100.0);
    let me = &r.first;
    let share = |a: f64| format!("{:.0}%", self::pct(a, me.seconds));
    let top = r
        .choices
        .iter()
        .max_by_key(|(_, n)| **n)
        .map_or("-", |(k, _)| k.as_str());
    println!(
        "{:<34} {:>7} {:>9} {:>7} {:>7.1} {:>5} {:>6} {:>7} {:>6.0} {:>6.0} {:>6} {:>6} {:>6.1} {:>6.1} {:>6} {:>6} {:>6}   top: {top}",
        name,
        pct(r.win_rate()),
        pct(r.as_seat(0)),
        pct(r.as_seat(1)),
        r.mean_length(),
        r.timeouts,
        pct(r.most_used()),
        pct(r.attack_share()),
        r.mean_distance,
        r.median_gap(),
        pct(r.within(80)),
        pct(r.within(40)),
        me.per_minute(me.attacks()),
        me.per_minute(me.whiffs),
        share(me.airborne),
        share(me.guarding),
        format!("{:.0}%", heaviest(me).1),
    );
}

/// A seat's skill that took the most of the foe's life, and its share of what the seat dealt, %.
fn heaviest(s: &SeatLog) -> (&'static str, f64) {
    s.skills
        .iter()
        .max_by(|a, b| a.1.damage.total_cmp(&b.1.damage))
        .map_or(("-", 0.0), |(n, k)| (*n, pct(k.damage, s.dealt())))
}

/// What each seat did in a mirror, seat 0 beside seat 1: per skill, then per seat.
fn print_telemetry(name: &str, r: &Record) {
    let [t, f] = &r.seats;
    let wins = [r.won[0] + r.lost[1], r.won[1] + r.lost[0]];
    let rounds = wins[0] + wins[1];
    println!(
        "telemetry: {name}, {rounds} rounds (seat 0 won {}, seat 1 {})",
        wins[0], wins[1]
    );
    println!(
        "  dmg: lives of the foe taken (1.00 = one life), chip and field ticks included; dmg% of the seat's total. \
         grd/bar/wrd/ddg/clh: blows the foe's guard held, its barrier turned, a ward erased, it dodged, blades met. \
         brk: casts broken while gathering. pun: struck while recovering. fin: last blow of a round it won."
    );
    let cols = [
        "cast", "land", "dmg", "dmg%", "grd", "bar", "wrd", "ddg", "clh", "brk", "pun", "fin",
    ];
    let head: String = cols
        .iter()
        .map(|c| {
            let w = if *c == "dmg" { 8 } else { 6 };
            format!("{c:>w$}")
        })
        .collect();
    println!("  {:<12} | {:^74} | {:^74}", "", "seat 0", "seat 1");
    println!("  {:<12} |{head} |{head}", "skill");
    let mut names: Vec<&str> = t.skills.keys().chain(f.skills.keys()).copied().collect();
    names.sort_unstable();
    names.dedup();
    // the heaviest hitters first, by the two seats' damage together
    let dmg = |s: &SeatLog, n: &str| s.skills.get(n).map_or(0.0, |k| k.damage);
    names.sort_by(|a, b| (dmg(t, b) + dmg(f, b)).total_cmp(&(dmg(t, a) + dmg(f, a))));
    for n in names {
        println!("  {n:<12} |{} |{}", skill_cells(t, n), skill_cells(f, n));
    }
    for status in ["burn", "bleed", "shock"] {
        let cell = |s: &SeatLog| {
            let d = s.statuses.get(status).copied().unwrap_or(0.0);
            let fin = s.finals.get(status).copied().unwrap_or(0);
            format!(
                "{:>12}{:>8.2}{:>6.1}{:>48}{fin:>6}",
                "",
                d / 100.0,
                pct(d, s.dealt()),
                ""
            )
        };
        println!("  {status:<12} |{} |{}", cell(t), cell(f));
    }
    let line = |label: &str, v: &dyn Fn(&SeatLog) -> String| {
        println!("  {label:<34} {:>14} {:>14}", v(t), v(f));
    };
    println!("  {:<34} {:>14} {:>14}", "per seat", "seat 0", "seat 1");
    let per_round = |x: f64| x / f64::from(rounds.max(1));
    line("lives dealt", &|s| format!("{:.2}", s.dealt() / 100.0));
    line("lives lost", &|s| format!("{:.2}", s.life_lost / 100.0));
    line("lives healed", &|s| format!("{:.2}", s.healed / 100.0));
    line("dealt per round, % of a life", &|s| {
        format!("{:.1}", per_round(s.dealt()))
    });
    line("status share of dealt", &|s| {
        format!("{:.1}%", pct(s.statuses.values().sum(), s.dealt()))
    });
    line("chip share of dealt", &|s| {
        format!(
            "{:.1}%",
            pct(s.skills.values().map(|k| k.chip).sum(), s.dealt())
        )
    });
    line("casts per round", &|s| {
        format!("{:.1}", per_round(f64::from(sum(s, |k| k.casts))))
    });
    line("hits landed per round", &|s| {
        format!("{:.1}", per_round(f64::from(sum(s, |k| k.landed))))
    });
    line("casts broken / punished", &|s| {
        format!("{} / {}", sum(s, |k| k.broken), sum(s, |k| k.punished))
    });
    line("its blows guarded/barrier/ward/dodged", &|s| {
        format!(
            "{}/{}/{}/{}",
            sum(s, |k| k.guarded),
            sum(s, |k| k.barrier),
            sum(s, |k| k.ward),
            sum(s, |k| k.dodged)
        )
    });
    line("qi on casts per round", &|s| {
        format!("{:.0}", per_round(s.qi_spent))
    });
    // the top tier (đỉnh cấp): the former ultimates
    const PEAK: [&str; 4] = ["tribulation", "remnantdusk", "sukiep", "daofusion"];
    line("top-tier casts", &|s| {
        let u = s
            .skills
            .iter()
            .filter(|(n, _)| PEAK.contains(n))
            .map(|(_, k)| k.casts)
            .sum::<u32>();
        format!("{u}")
    });
    line("top-tier damage, lives", &|s| {
        let d = s
            .skills
            .iter()
            .filter(|(n, _)| PEAK.contains(n))
            .map(|(_, k)| k.damage)
            .sum::<f64>();
        format!("{:.2}", d / 100.0)
    });
    line("time meditating", &|s| {
        format!("{:.1}%", pct(s.meditating, s.seconds))
    });
    line("time guarding", &|s| {
        format!("{:.1}%", pct(s.guarding, s.seconds))
    });
    line("time flying", &|s| {
        format!("{:.1}%", pct(s.flying, s.seconds))
    });
    line("time off the ground", &|s| {
        format!("{:.1}%", pct(s.airborne, s.seconds))
    });
    line("hurting casts / whiffs a minute", &|s| {
        format!(
            "{:.1} / {:.1}",
            s.per_minute(s.attacks()),
            s.per_minute(s.whiffs)
        )
    });
    println!();
}

/// One seat's cells for skill `n`.
fn skill_cells(s: &SeatLog, n: &str) -> String {
    let k = s.skills.get(n).cloned().unwrap_or_default();
    let fin = s.finals.get(n).copied().unwrap_or(0);
    format!(
        "{:>6}{:>6}{:>8.2}{:>6.1}{:>6}{:>6}{:>6}{:>6}{:>6}{:>6}{:>6}{fin:>6}",
        k.casts,
        k.landed,
        k.damage / 100.0,
        pct(k.damage, s.dealt()),
        k.guarded,
        k.barrier,
        k.ward,
        k.dodged,
        k.clashed,
        k.broken,
        k.punished,
    )
}

fn sum(s: &SeatLog, of: impl Fn(&SkillLog) -> u32) -> u32 {
    s.skills.values().map(of).sum()
}

fn pct(a: f64, b: f64) -> f64 {
    if b == 0.0 { 0.0 } else { a / b * 100.0 }
}

/// The time one decision of the bot takes, natively, over 5,000 ticks of a real fight.
fn decision_cost(new: &dyn Fn() -> SharedDecider, effects: &[String]) {
    let config = DuelConfig {
        seed: 1,
        left: 0.0,
        right: 880.0,
        ground: 600.0,
        sky: 384.0,
        effects: effects.to_vec(),
    };
    let mut duel = Duel::new(config, [Brain::make(), Brain::make()]);
    let mut senses = Vec::new();
    while senses.len() < 5000 {
        duel.step(TICK);
        senses.push(sense_of(duel.state(), 0));
    }
    let mut mind = policy_mind(duel.state(), 0, Style::Balanced);
    let bot = new();
    // a module decides only inside a fight it was started on (as `BotSeat::new` starts it): unstarted, every call traps
    let started = {
        let mut b = bot.borrow_mut();
        b.start(1).and_then(|()| b.round())
    };
    if let Err(e) = started {
        println!("decision cost: the bot would not start: {e:?}");
        return;
    }
    let mut decided = 0_u32;
    let start = Instant::now();
    for _ in 0..20 {
        for s in &senses {
            let _ = mind.begin(s, TICK);
            if let Some(d) = mind.due(s) {
                let act = bot.borrow_mut().decide(&d.obs, &d.legal, d.u).unwrap_or(0);
                mind.queue(usize::try_from(act).unwrap_or(0));
                decided += 1;
            }
        }
        mind.reset();
    }
    let per = start.elapsed().as_secs_f64() / f64::from(decided.max(1));
    println!(
        "decision cost (native, release): {:.3} ms over {decided} decisions",
        per * 1000.0
    );
}

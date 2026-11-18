//! The one-cast cap's table (`arena_calc::cap`; rules, "One cast, 40% of a life"): each damaging skill of the pool,
//! bare (what the rule holds), with its worst single support and its worst pair of supports (measured, not held
//! yet), as a share of a life and its shield.
//!
//! ```text
//! cargo run --release -p arena-sim --example cap -- [--over] [--telemetry-dir DIR]
//! ```
//!
//! `--over` prints only the skills with a form over the cap. `--suggest` prints, for each skill over the cap bare,
//! the base that would bring it to 38% and, for a skill that lands more than once, the hits that would at its base. `--telemetry-dir DIR` (or `ARENA_TELEMETRY_DIR` set) writes
//! one `balance.cap` event a skill and form (`form` "bare", "single" or "pair"; `supports`; `share` of 1440) under
//! one `run_id`, service `arena-ladder`, target `arena::ladder` (`ops/telemetry/README.md`).

use arena_calc::cap::{CAST_CAP, Dealt, cap_life, cast_of, damaging, fitting, skill_of};
use arena_calc::gem::{Effect, GEMS, Gem};
use arena_calc::support::{Support, excluded};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _export = arena_telemetry::requested(&args)?.map(|dir| {
        arena_telemetry::init(
            "arena-ladder",
            arena_telemetry::Options {
                dir: Some(dir),
                ..arena_telemetry::Options::tool(env!("CARGO_PKG_VERSION"))
            },
        )
    });
    let only_over = args.iter().any(|a| a == "--over");
    if args.iter().any(|a| a == "--suggest") {
        suggest();
        return Ok(());
    }
    let run = arena_telemetry::run_id("cap");
    let of = cap_life();
    let pct = |d: &Dealt| 100.0 * d.share(of);
    println!(
        "one cast against {of} (a life and its shield), cap {:.0}%: bare is the rule, supports are measured\n",
        100.0 * CAST_CAP
    );
    println!(
        "{:<20} {:>2} {:>7} {:>7} {:>7}  {:<16} {:>7}  pair",
        "skill", "t", "bare", "hit", "dot", "worst single", ""
    );
    let mut rows = Vec::new();
    for g in GEMS.iter().filter(|g| damaging(g)) {
        let bare = cast_of(g, &[]);
        let fit = fitting(&skill_of(g));
        let single = fit
            .iter()
            .map(|&s| (cast_of(g, &[s]), vec![s]))
            .max_by(|a, b| a.0.total().total_cmp(&b.0.total()));
        let mut pair: Option<(Dealt, Vec<&Support>)> = None;
        for (i, &a) in fit.iter().enumerate() {
            for &b in fit[i + 1..].iter().filter(|b| !excluded(a, b)) {
                let d = cast_of(g, &[a, b]);
                if pair.as_ref().is_none_or(|p| d.total() > p.0.total()) {
                    pair = Some((d, vec![a, b]));
                }
            }
        }
        let names = |v: &[&Support]| v.iter().map(|s| s.id).collect::<Vec<_>>().join("+");
        for (form, d, on) in [("bare", Some(bare), String::new())]
            .into_iter()
            .chain(single.as_ref().map(|s| ("single", Some(s.0), names(&s.1))))
            .chain(pair.as_ref().map(|p| ("pair", Some(p.0), names(&p.1))))
        {
            let d = d.unwrap_or_default();
            tracing::info!(target: "arena::ladder", run_id = %run, id = g.id, tier = %format!("{:?}", g.tier),
                form, supports = %on, hit = d.hit, over_time = d.over_time, share = d.share(of),
                over = d.over(of), "balance.cap");
        }
        let worst = pair
            .as_ref()
            .map(|p| p.0)
            .or(single.as_ref().map(|s| s.0))
            .unwrap_or(bare);
        rows.push((worst.total().max(bare.total()), g, bare, single, pair));
    }
    rows.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (_, g, bare, single, pair) in rows {
        let any_over = bare.over(of)
            || single.as_ref().is_some_and(|s| s.0.over(of))
            || pair.as_ref().is_some_and(|p| p.0.over(of));
        if only_over && !any_over {
            continue;
        }
        let (s_pct, s_name) = single
            .as_ref()
            .map_or((0.0, String::new()), |s| (pct(&s.0), s.1[0].id.to_string()));
        let (p_pct, p_name) = pair.as_ref().map_or((0.0, String::new()), |p| {
            (pct(&p.0), format!("{}+{}", p.1[0].id, p.1[1].id))
        });
        println!(
            "{:<20} {:>2} {:>6.1}% {:>6.1}% {:>6.1}%  {:<16} {:>6.1}%  {:>6.1}% {}",
            g.id,
            format!("{:?}", g.tier),
            pct(&bare),
            100.0 * bare.hit / of,
            100.0 * bare.over_time / of,
            s_name,
            s_pct,
            p_pct,
            p_name
        );
    }
    Ok(())
}

/// The base, and the hits, that bring each skill over the cap bare to 38% of a life and its shield.
fn suggest() {
    let of = cap_life();
    let aim = 0.38;
    for g in GEMS.iter().filter(|g| damaging(g)) {
        let bare = cast_of(g, &[]);
        if !bare.over(of) {
            continue;
        }
        let at = |base: f64| cast_of(&Gem { base, ..*g }, &[]).share(of);
        let (mut lo, mut hi) = (0.0, g.base);
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if at(mid) > aim { hi = mid } else { lo = mid }
        }
        let hits = if g.hits() > 1.0 {
            let mut n = g.hits();
            while n > 1.0 {
                let effects: &'static [Effect] = Box::leak(
                    g.effects
                        .iter()
                        .map(|e| match e {
                            Effect::Hits(_) => Effect::Hits(n),
                            e => *e,
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                );
                if cast_of(&Gem { effects, ..*g }, &[]).share(of) <= aim {
                    break;
                }
                n -= 0.5;
            }
            format!("hits {} -> {n}", g.hits())
        } else {
            String::new()
        };
        println!(
            "{:<20} {:>2} {:6.1}%  base {:6.2} -> {:6.2} (x{:.2})  {hits}",
            g.id,
            format!("{:?}", g.tier),
            100.0 * bare.share(of),
            g.base,
            lo,
            lo / g.base
        );
    }
}

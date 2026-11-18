//! The telemetry files queried: every line of a time range that passes the filters, in time order across services,
//! printed, counted, grouped or summed up. It streams the files (one line of each held at a time); only `--stats`
//! keeps its numbers and `--last` its lines. The ready commands are in `ops/telemetry/README.md`.
//!
//! ```text
//! cargo run -p arena-telemetry --example query -- [--dir DIR] [--service S[,S]] [--since 2h | --from T] [--to T]
//!     [--kind log|span|metric] [--name N[,N]] [--match ID] [--where KEY=V]... [--fields K,K]
//!     [--json | --count | --group-by K[,K] | --stats FIELD] [--limit N | --last N]
//! cargo run -p arena-telemetry --example query -- --files [--dir DIR]   # each file and its size
//! ```
//!
//! The directory is `--dir`, else `ARENA_TELEMETRY_DIR`, else `~/services/arena/telemetry`. Times are UTC
//! (`2026-11-18`, `2026-11-18T10:05`); `--since` takes `90s`, `15m`, `2h`, `1d`, `2w`. A name or a `=` value may hold
//! `*` wildcards; `KEY=a|b` is either; `KEY!=V`, `KEY>N`, `KEY<N`, `KEY>=N`, `KEY<=N` too. A key is an attribute,
//! else the line's own field (`level`, `service`, `type`, `duration_ms`, …); `attrs.K` and `.K` say which.
//! `--stats value` over histogram metric lines merges their buckets: the percentiles of every sample.

use arena_telemetry::read::{Cond, Filter, Scope, Stats, Summary, field, group_of, scan, text};
use arena_telemetry::{files, time};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
use std::io::Write as _;
use std::path::PathBuf;

/// What a run prints.
#[derive(Clone, Debug, PartialEq)]
enum Output {
    Lines { json: bool },
    Count,
    Groups,
    Stats(String),
}

#[derive(Clone, Debug)]
struct Query {
    dir: PathBuf,
    scope: Scope,
    filter: Filter,
    output: Output,
    group_by: Vec<String>,
    fields: Vec<String>,
    limit: Option<usize>,
    last: Option<usize>,
    files: bool,
}

fn list(v: &str) -> Vec<String> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse(args: &[String], now_us: i64) -> Result<Query, String> {
    let mut q = Query {
        dir: files::dir(),
        scope: Scope::default(),
        filter: Filter::default(),
        output: Output::Lines { json: false },
        group_by: Vec::new(),
        fields: Vec::new(),
        limit: None,
        last: None,
        files: false,
    };
    let (mut json, mut count, mut stats) = (false, false, None);
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
        let time_of = |v: &str| time::parse_ts(v).ok_or(format!("{flag} {v}: not a UTC time"));
        let count_of = |v: &str| {
            v.parse::<usize>()
                .map_err(|_| format!("{flag} {v}: not a count"))
        };
        match flag.as_str() {
            "--dir" => q.dir = PathBuf::from(value()?),
            "--service" => q.scope.services.extend(list(&value()?)),
            "--since" => {
                let v = value()?;
                let ago =
                    time::parse_ago(&v).ok_or(format!("--since {v}: e.g. 90s, 15m, 2h, 1d"))?;
                q.scope.from_us = Some(now_us - ago);
            }
            "--from" => q.scope.from_us = Some(time_of(&value()?)?),
            "--to" => q.scope.to_us = Some(time_of(&value()?)?),
            "--kind" => q.filter.kinds.extend(list(&value()?)),
            "--name" => q.filter.names.extend(list(&value()?)),
            "--match" => q.filter.match_id = Some(value()?),
            "--where" => q.filter.wheres.push(Cond::parse(&value()?)?),
            "--fields" => q.fields.extend(list(&value()?)),
            "--json" => json = true,
            "--count" => count = true,
            "--group-by" => q.group_by.extend(list(&value()?)),
            "--stats" => stats = Some(value()?),
            "--limit" => q.limit = Some(count_of(&value()?)?),
            "--last" => q.last = Some(count_of(&value()?)?),
            "--files" => q.files = true,
            "-h" | "--help" => return Err(USAGE.into()),
            other => return Err(format!("unknown argument {other}\n{USAGE}")),
        }
    }
    for k in &q.filter.kinds {
        if !["log", "span", "metric"].contains(&k.as_str()) {
            return Err(format!("--kind {k}: log, span or metric"));
        }
    }
    q.output = match (stats, count, q.group_by.is_empty()) {
        (Some(f), _, _) => Output::Stats(f),
        (None, true, true) => Output::Count,
        (None, _, false) => Output::Groups,
        (None, false, true) => Output::Lines { json },
    };
    Ok(q)
}

const USAGE: &str = "usage: query [--dir DIR] [--service S] [--since 2h | --from T] [--to T] [--kind log|span|metric] \
[--name N] [--match ID] [--where K=V]... [--fields K,K] [--json | --count | --group-by K | --stats FIELD] [--limit N | --last N] | --files";

/// A line as one row of text: its time, service, kind and name, then its level and fields (or only `fields`).
fn pretty(v: &Value, fields: &[String]) -> String {
    let s = |k: &str| v.get(k).map(text).unwrap_or_default();
    let ts = s("ts");
    let mut out = format!(
        "{} {} {} {}",
        ts.get(..23).unwrap_or(&ts),
        s("service"),
        s("kind"),
        s("name")
    );
    if !fields.is_empty() {
        for f in fields {
            let val = field(v, f).map_or_else(|| "-".into(), text);
            out.push_str(&format!(" {f}={val}"));
        }
        return out;
    }
    match s("kind").as_str() {
        "span" => out.push_str(&format!(" {}ms", s("duration_ms"))),
        "metric" if s("type") == "histogram" => {
            let mut st = Stats::default();
            st.add_line(v, "value");
            if let Some(m) = st.summary() {
                out.push_str(&format!(
                    " count={} mean={} p50~{} p99~{} max={}",
                    m.count,
                    num(m.mean),
                    num(m.p50),
                    num(m.p99),
                    num(m.max)
                ));
            }
        }
        "metric" => out.push_str(&format!(" value={}", s("value"))),
        _ => {
            let level = s("level");
            if level != "INFO" {
                out.push_str(&format!(" [{level}]"));
            }
        }
    }
    if let Some(Value::Object(attrs)) = v.get("attrs") {
        for (k, val) in attrs {
            out.push_str(&format!(" {k}={}", text(val)));
        }
    }
    out
}

/// A number short: four significant digits.
fn num(x: f64) -> String {
    if x == 0.0 || !x.is_finite() {
        return format!("{x}");
    }
    let digits = (3 - libm::floor(libm::log10(x.abs())) as i32).max(0) as usize;
    let s = format!("{x:.digits$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

fn stats_row(group: &str, m: &Summary) -> String {
    format!(
        "{group:<40} lines {:>7}  n {:>9}  sum {:>11}  mean {:>9}  p50 {:>9}  p95 {:>9}  p99 {:>9}  max {:>9}",
        m.lines,
        m.count,
        num(m.sum),
        num(m.mean),
        num(m.p50),
        num(m.p95),
        num(m.p99),
        num(m.max)
    )
}

fn run(q: &Query) -> Result<(), String> {
    let out = std::io::stdout();
    let mut out = out.lock();
    if q.files {
        let mut total = 0u64;
        for f in files::list(&q.dir).map_err(|e| format!("{}: {e}", q.dir.display()))? {
            if q.scope.services.is_empty() || q.scope.services.contains(&f.service) {
                total += f.bytes;
                let _ = writeln!(out, "{:>12}  {}", f.bytes, f.path.display());
            }
        }
        let _ = writeln!(out, "{total:>12}  total ({:.1} MB)", total as f64 / 1e6);
        return Ok(());
    }
    let mut matched = 0usize;
    let mut groups: BTreeMap<String, (u64, Stats)> = BTreeMap::new();
    let mut last: VecDeque<String> = VecDeque::new();
    let mut failed = None;
    let res = scan(&q.dir, &q.scope, |_, raw| {
        if !q.filter.may_match(raw) {
            return true;
        }
        let Ok(v) = serde_json::from_str::<Value>(raw) else {
            return true;
        };
        if !q.filter.matches(&v) {
            return true;
        }
        matched += 1;
        match &q.output {
            Output::Lines { json } => {
                let row = if *json {
                    raw.to_string()
                } else {
                    pretty(&v, &q.fields)
                };
                match q.last {
                    Some(n) => {
                        last.push_back(row);
                        if last.len() > n {
                            last.pop_front();
                        }
                    }
                    None => {
                        if let Err(e) = writeln!(out, "{row}") {
                            // a closed pipe (`| head`) ends the run quietly
                            failed = Some(e);
                            return false;
                        }
                    }
                }
            }
            Output::Count => {}
            Output::Groups | Output::Stats(_) => {
                let g = groups.entry(group_of(&v, &q.group_by)).or_default();
                g.0 += 1;
                if let Output::Stats(f) = &q.output {
                    g.1.add_line(&v, f);
                }
            }
        }
        q.limit.is_none_or(|n| matched < n)
    });
    res.map_err(|e| format!("{}: {e}", q.dir.display()))?;
    if failed.is_some() {
        return Ok(());
    }
    for row in last {
        let _ = writeln!(out, "{row}");
    }
    match &q.output {
        Output::Lines { .. } => {}
        Output::Count => {
            let _ = writeln!(out, "{matched}");
        }
        Output::Groups => {
            let mut rows: Vec<_> = groups.iter().collect();
            rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
            let _ = writeln!(out, "{:>9}  {}", "count", q.group_by.join(" "));
            for (g, (n, _)) in rows {
                let _ = writeln!(out, "{n:>9}  {g}");
            }
        }
        Output::Stats(f) => {
            let _ = writeln!(
                out,
                "{f} over {matched} lines{}",
                if q.group_by.is_empty() {
                    String::new()
                } else {
                    format!(", by {}", q.group_by.join(" "))
                }
            );
            for (g, (_, s)) in &groups {
                let label = if g.is_empty() { "all" } else { g };
                match s.summary() {
                    Some(m) => {
                        let _ = writeln!(out, "{}", stats_row(label, &m));
                    }
                    None => {
                        let _ = writeln!(out, "{label:<40} no {f}");
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let q = match parse(&args, time::now_us()) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(&q) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &str) -> Vec<String> {
        a.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn flags_make_the_query() -> Result<(), String> {
        let now = time::parse_ts("2026-11-18T12:00").ok_or("time")?;
        let q = parse(
            &args(
                "--dir /tmp/t --service arena-server,arena-bot --since 2h --name match.start,match.end --where seat=1 --where faults>2 --group-by bot_0,bot_1",
            ),
            now,
        )?;
        assert_eq!(q.dir, PathBuf::from("/tmp/t"));
        assert_eq!(q.scope.services, ["arena-server", "arena-bot"]);
        assert_eq!(q.scope.from_us, Some(now - 7200 * time::SECOND));
        assert_eq!(q.filter.names, ["match.start", "match.end"]);
        assert_eq!(q.filter.wheres.len(), 2);
        assert_eq!(q.output, Output::Groups);
        assert_eq!(q.group_by, ["bot_0", "bot_1"]);
        let q = parse(&args("--stats value --group-by runtime --count"), now)?;
        assert_eq!(q.output, Output::Stats("value".into()));
        assert_eq!(parse(&args("--count"), now)?.output, Output::Count);
        assert_eq!(
            parse(&args("--json --last 5"), now)?.output,
            Output::Lines { json: true }
        );
        assert!(parse(&args("--kind trace"), now).is_err());
        assert!(parse(&args("--since soon"), now).is_err());
        assert!(parse(&args("--where nothing"), now).is_err());
        assert!(parse(&args("--bogus"), now).is_err());
        Ok(())
    }

    #[test]
    fn numbers_print_short() {
        assert_eq!(num(0.000_123_456), "0.0001235");
        assert_eq!(num(1234.5678), "1235");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(0.0), "0");
    }
}

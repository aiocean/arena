//! The files read back: every line of a time range, in time order across services, streamed (one line of each file
//! held at a time) and started by a binary search on the first day's file, so the last fifteen minutes of a big day
//! cost what fifteen minutes do. Then what the query tool, the watchdog and the smoke test ask of a line: a `Filter`
//! (kind, name, match, `key=value`, `key>n`) and `Stats` (count, mean, percentiles, max), a histogram line's buckets
//! merged in.
//!
//! A key names an attribute (`attrs.<key>`) or, when the line's attributes have none of that name, a field of the
//! line itself (`ts`, `kind`, `service`, `name`, `level`, `target`, `trace_id`, `type`, `value`, `count`, `max`,
//! `duration_ms`, …). `attrs.kind` and `.kind` say which outright.

use crate::files;
use crate::hist::Hist;
use crate::time;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// How far out of order the lines of one file may be (two processes appending to one file, each a second at a
/// time): a range is read this much wider and cut exactly.
const SLACK_US: i64 = 120 * time::SECOND;

/// What to read: which services (none: all) and when (inclusive bounds; none: all).
#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub services: Vec<String>,
    pub from_us: Option<i64>,
    pub to_us: Option<i64>,
}

/// The time of a line: its `ts`, found at its head without parsing (the writer puts it first), else parsed.
pub fn ts_of(line: &str) -> Option<i64> {
    if let Some(ts) = line
        .strip_prefix("{\"ts\":\"")
        .and_then(|r| r.get(..time::TS_LEN))
        .and_then(time::parse_ts)
    {
        return Some(ts);
    }
    let v: Value = serde_json::from_str(line).ok()?;
    time::parse_ts(v.get("ts")?.as_str()?)
}

/// One file being read.
struct Cursor {
    lines: BufReader<std::fs::File>,
    current: Option<(i64, String)>,
    to_us: Option<i64>,
}

impl Cursor {
    fn open(path: &Path, from_us: Option<i64>, to_us: Option<i64>) -> std::io::Result<Cursor> {
        let mut file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        let start = match from_us {
            Some(t) => seek_to(&mut file, len, t - SLACK_US)?,
            None => 0,
        };
        file.seek(SeekFrom::Start(start))?;
        let mut lines = BufReader::with_capacity(1 << 16, file);
        if start > 0 {
            // the middle of a line
            let mut skip = Vec::new();
            lines.read_until(b'\n', &mut skip)?;
        }
        let mut c = Cursor {
            lines,
            current: None,
            to_us,
        };
        c.advance()?;
        Ok(c)
    }

    /// The next line with a time; none once past the range's end (and its slack) or the file's.
    fn advance(&mut self) -> std::io::Result<()> {
        self.current = None;
        let mut buf = String::new();
        loop {
            buf.clear();
            if self.lines.read_line(&mut buf)? == 0 {
                return Ok(());
            }
            let line = buf.trim_end_matches(['\n', '\r']);
            let Some(ts) = ts_of(line) else { continue };
            if self.to_us.is_some_and(|to| ts > to + SLACK_US) {
                return Ok(());
            }
            self.current = Some((ts, line.to_string()));
            return Ok(());
        }
    }
}

/// The time of the first whole line at or after `pos`.
fn ts_after(file: &mut std::fs::File, pos: u64) -> std::io::Result<Option<i64>> {
    file.seek(SeekFrom::Start(pos))?;
    let mut r = BufReader::with_capacity(1 << 14, file.take(1 << 20));
    let mut buf = Vec::new();
    if pos > 0 {
        r.read_until(b'\n', &mut buf)?;
    }
    for _ in 0..16 {
        buf.clear();
        if r.read_until(b'\n', &mut buf)? == 0 {
            return Ok(None);
        }
        if let Some(ts) = std::str::from_utf8(&buf)
            .ok()
            .and_then(|l| ts_of(l.trim_end()))
        {
            return Ok(Some(ts));
        }
    }
    Ok(None)
}

/// An offset at or before the first line at `target` or later, in a file whose lines are in time order (near
/// enough: `SLACK_US`).
fn seek_to(file: &mut std::fs::File, len: u64, target: i64) -> std::io::Result<u64> {
    let (mut lo, mut hi) = (0u64, len);
    while hi - lo > 1 << 16 {
        let mid = lo + (hi - lo) / 2;
        match ts_after(file, mid)? {
            Some(ts) if ts < target => lo = mid,
            _ => hi = mid,
        }
    }
    Ok(lo)
}

/// Call `each` with every line of `scope` under `dir`, in time order (each day's files merged), with its time,
/// until it returns false.
pub fn scan(
    dir: &Path,
    scope: &Scope,
    mut each: impl FnMut(i64, &str) -> bool,
) -> std::io::Result<()> {
    let first = scope.from_us.map(time::day);
    let last = scope.to_us.map(time::day);
    let mut files: Vec<files::File> = files::list(dir)?
        .into_iter()
        .filter(|f| scope.services.is_empty() || scope.services.contains(&f.service))
        .filter(|f| first.as_ref().is_none_or(|d| &f.day >= d))
        .filter(|f| last.as_ref().is_none_or(|d| &f.day <= d))
        .collect();
    files.sort_by(|a, b| a.day.cmp(&b.day));
    let mut i = 0;
    while i < files.len() {
        let day = files[i].day.clone();
        let mut cursors = Vec::new();
        while i < files.len() && files[i].day == day {
            // the range's start is searched for in its own day; a later day is read from its top
            let from = scope.from_us.filter(|t| time::day(*t) == day);
            match Cursor::open(&files[i].path, from, scope.to_us) {
                Ok(c) => cursors.push(c),
                // the retention deleted it since the listing
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            i += 1;
        }
        loop {
            let next = cursors
                .iter()
                .enumerate()
                .filter_map(|(k, c)| c.current.as_ref().map(|(ts, _)| (*ts, k)))
                .min();
            let Some((ts, k)) = next else { break };
            let in_range =
                scope.from_us.is_none_or(|f| ts >= f) && scope.to_us.is_none_or(|t| ts <= t);
            if in_range
                && let Some((_, line)) = &cursors[k].current
                && !each(ts, line)
            {
                return Ok(());
            }
            cursors[k].advance()?;
        }
    }
    Ok(())
}

// ---- a line's fields ----

/// The value of `key` on a line: `attrs.<key>` first, then the line's own field (`.key` and `attrs.key` choose).
pub fn field<'a>(line: &'a Value, key: &str) -> Option<&'a Value> {
    if let Some(k) = key.strip_prefix("attrs.") {
        return line.get("attrs")?.get(k);
    }
    if let Some(k) = key.strip_prefix('.') {
        return line.get(k);
    }
    line.get("attrs")
        .and_then(|a| a.get(key))
        .or_else(|| line.get(key))
}

/// A value as text: a string as itself, anything else as JSON.
pub fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A value as a number: a number, or a string that is one.
pub fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.trim().parse().ok())
}

/// Whether `s` matches `pattern`, where `*` stands for any run of characters.
pub fn glob(pattern: &str, s: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == s;
    }
    let mut rest = s;
    for (i, p) in parts.iter().enumerate() {
        if i == 0 {
            let Some(r) = rest.strip_prefix(p) else {
                return false;
            };
            rest = r;
        } else if i == parts.len() - 1 {
            return rest.ends_with(p);
        } else {
            match rest.find(p) {
                Some(at) => rest = &rest[at + p.len()..],
                None => return false,
            }
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

/// A condition on a field: `key=value` (`a|b` for either, `*` a wildcard), `key!=value`, `key>n`, `key<n`,
/// `key>=n`, `key<=n`.
#[derive(Clone, Debug, PartialEq)]
pub struct Cond {
    pub key: String,
    pub op: Op,
    pub value: String,
}

impl Cond {
    pub fn parse(s: &str) -> Result<Cond, String> {
        for (sym, op) in [
            ("!=", Op::Ne),
            (">=", Op::Ge),
            ("<=", Op::Le),
            ("=", Op::Eq),
            (">", Op::Gt),
            ("<", Op::Lt),
        ] {
            if let Some((k, v)) = s.split_once(sym) {
                let key = k.trim();
                if key.is_empty() {
                    return Err(format!("{s}: no key"));
                }
                if matches!(op, Op::Gt | Op::Lt | Op::Ge | Op::Le)
                    && v.trim().parse::<f64>().is_err()
                {
                    return Err(format!("{s}: {sym} takes a number"));
                }
                return Ok(Cond {
                    key: key.to_string(),
                    op,
                    value: v.trim().to_string(),
                });
            }
        }
        Err(format!("{s}: not key=value, key!=value, key>n or key<n"))
    }

    pub fn holds(&self, line: &Value) -> bool {
        let got = field(line, &self.key);
        match self.op {
            Op::Eq => got.is_some_and(|v| equal(v, &self.value)),
            Op::Ne => !got.is_some_and(|v| equal(v, &self.value)),
            op => {
                let (Some(a), Ok(b)) = (got.and_then(number), self.value.parse::<f64>()) else {
                    return false;
                };
                match op {
                    Op::Gt => a > b,
                    Op::Lt => a < b,
                    Op::Ge => a >= b,
                    _ => a <= b,
                }
            }
        }
    }
}

/// Whether `v` is any of `want`'s `|` alternatives: as text (with `*`), or as the same number.
fn equal(v: &Value, want: &str) -> bool {
    let t = text(v);
    want.split('|').any(|w| {
        glob(w, &t)
            || (v.is_number()
                && w.parse::<f64>()
                    .is_ok_and(|n| v.as_f64().is_some_and(|x| x == n)))
    })
}

/// Which lines a query keeps.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    /// `log`, `span`, `metric`: any of them (none: all).
    pub kinds: Vec<String>,
    /// The line's `name` (an event's message, a span's or a metric's name), any of them, `*` a wildcard.
    pub names: Vec<String>,
    /// A match's or a run's id: `match_id` or `run_id`.
    pub match_id: Option<String>,
    pub wheres: Vec<Cond>,
}

impl Filter {
    /// A cheap look at the raw line, before it is parsed: false when it cannot match.
    pub fn may_match(&self, raw: &str) -> bool {
        if let Some(id) = &self.match_id
            && !raw.contains(id.as_str())
        {
            return false;
        }
        if !self.names.is_empty() && self.names.iter().all(|n| !n.contains('*')) {
            return self.names.iter().any(|n| raw.contains(n.as_str()));
        }
        true
    }

    pub fn matches(&self, line: &Value) -> bool {
        let top = |k: &str| line.get(k).and_then(Value::as_str).unwrap_or("");
        if !self.kinds.is_empty() && !self.kinds.iter().any(|k| k == top("kind")) {
            return false;
        }
        if !self.names.is_empty() && !self.names.iter().any(|n| glob(n, top("name"))) {
            return false;
        }
        if let Some(id) = &self.match_id {
            let attr = |k| line.get("attrs").and_then(|a| a.get(k)).map(text);
            if attr("match_id").as_deref() != Some(id) && attr("run_id").as_deref() != Some(id) {
                return false;
            }
        }
        self.wheres.iter().all(|c| c.holds(line))
    }
}

// ---- stats ----

/// A histogram line's distribution (`count`, `sum`, `min`, `max`, `zero`, `buckets`).
pub fn hist_of(line: &Value) -> Option<Hist> {
    if line.get("type")?.as_str()? != "histogram" {
        return None;
    }
    let mut h = Hist {
        count: line.get("count")?.as_u64()?,
        sum: line.get("sum")?.as_f64()?,
        min: line.get("min").and_then(Value::as_f64).unwrap_or(0.0),
        max: line.get("max").and_then(Value::as_f64).unwrap_or(0.0),
        zero: line.get("zero").and_then(Value::as_u64).unwrap_or(0),
        ..Hist::default()
    };
    for b in line.get("buckets")?.as_array()? {
        let (Some(i), Some(n)) = (
            b.get(0).and_then(Value::as_i64),
            b.get(1).and_then(Value::as_u64),
        ) else {
            continue;
        };
        *h.buckets.entry(i as i32).or_insert(0) += n;
    }
    Some(h)
}

/// The distribution of a field over lines. A histogram line's `value` is its whole distribution (its buckets
/// merged), so `--stats value` over `arena_bot_think_seconds` lines is every think, not every ten seconds.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// The lines that gave a value.
    pub lines: u64,
    exact: Vec<f64>,
    hist: Hist,
    bucketed: bool,
}

/// What `Stats` sums up to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub lines: u64,
    pub count: u64,
    pub sum: f64,
    pub mean: f64,
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

impl Stats {
    pub fn add(&mut self, v: f64) {
        if v.is_finite() {
            self.exact.push(v);
            self.hist.add(v);
        }
    }

    pub fn add_hist(&mut self, h: &Hist) {
        self.bucketed = true;
        self.hist.merge(h);
    }

    /// Take `key` of `line`, if it has one.
    pub fn add_line(&mut self, line: &Value, key: &str) {
        if key == "value"
            && let Some(h) = hist_of(line)
        {
            self.lines += 1;
            self.add_hist(&h);
            return;
        }
        if let Some(v) = field(line, key).and_then(number) {
            self.lines += 1;
            self.add(v);
        }
    }

    pub fn summary(&self) -> Option<Summary> {
        let h = &self.hist;
        if h.count == 0 {
            return None;
        }
        let q = |p: f64| -> f64 {
            if self.bucketed {
                h.quantile(p).unwrap_or(f64::NAN)
            } else {
                let mut v = self.exact.clone();
                v.sort_by(f64::total_cmp);
                let rank = (libm::ceil(p * v.len() as f64) as usize).clamp(1, v.len());
                v[rank - 1]
            }
        };
        Some(Summary {
            lines: self.lines,
            count: h.count,
            sum: h.sum,
            mean: h.sum / h.count as f64,
            min: h.min,
            p50: q(0.5),
            p95: q(0.95),
            p99: q(0.99),
            max: h.max,
        })
    }
}

/// The group of a line under `keys`: their values, `-` for a missing one, joined by a space.
pub fn group_of(line: &Value, keys: &[String]) -> String {
    keys.iter()
        .map(|k| field(line, k).map_or_else(|| "-".to_string(), text))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(kind: &str, name: &str, attrs: Value) -> Value {
        json!({"ts": "2026-11-18T10:00:00.000000Z", "kind": kind, "service": "arena-server", "name": name,
               "level": "INFO", "attrs": attrs})
    }

    #[test]
    fn conditions_parse_and_hold() -> Result<(), String> {
        let l = line(
            "log",
            "bot.fault",
            json!({"kind": "trap", "faults": 3, "bot": "module-ab12", "share": 0.25}),
        );
        let holds = |s: &str| Cond::parse(s).map(|c| c.holds(&l));
        assert!(holds("kind=trap")?, "attrs before the line's own kind");
        assert!(holds(".kind=log")?);
        assert!(!holds("level=ERROR|WARN")?);
        assert!(holds("level=INFO|WARN")?);
        assert!(holds("bot=module-*")?);
        assert!(holds("faults=3.0")?);
        assert!(holds("faults>2")?);
        assert!(holds("faults>=3")?);
        assert!(!holds("faults<3")?);
        assert!(holds("share<=0.25")?);
        assert!(!holds("missing=x")?);
        assert!(holds("missing!=x")?);
        assert!(!holds("kind!=trap")?);
        assert!(Cond::parse("faults>many").is_err());
        assert!(Cond::parse("=x").is_err());
        assert!(Cond::parse("nothing").is_err());
        Ok(())
    }

    #[test]
    fn globs_match_whole_strings() {
        assert!(glob("match.*", "match.start"));
        assert!(!glob("match.*", "a.match.start"));
        assert!(glob("*fault*", "bot.fault"));
        assert!(glob("a*c*e", "abcde"));
        assert!(!glob("a*c*e", "abcd"));
        assert!(glob("exact", "exact"));
        assert!(!glob("exact", "exactly"));
    }

    #[test]
    fn a_filter_keeps_by_kind_name_match_and_wheres() -> Result<(), String> {
        let f = Filter {
            kinds: vec!["log".into()],
            names: vec!["match.*".into()],
            match_id: Some("live-7".into()),
            wheres: vec![Cond::parse("seat=1")?],
        };
        let start = line(
            "log",
            "match.start",
            json!({"match_id": "live-7", "seat": 1}),
        );
        assert!(f.matches(&start));
        assert!(!f.matches(&line(
            "log",
            "match.start",
            json!({"match_id": "live-8", "seat": 1})
        )));
        assert!(!f.matches(&line(
            "log",
            "hit",
            json!({"match_id": "live-7", "seat": 1})
        )));
        assert!(!f.matches(&line(
            "span",
            "match.x",
            json!({"match_id": "live-7", "seat": 1})
        )));
        assert!(!f.matches(&line(
            "log",
            "match.end",
            json!({"match_id": "live-7", "seat": 0})
        )));
        let run = Filter {
            match_id: Some("ladder-1".into()),
            ..Filter::default()
        };
        assert!(run.matches(&line("log", "ladder.row", json!({"run_id": "ladder-1"}))));
        // the cheap look: literal names and the id must be in the raw text
        assert!(f.may_match(r#"{"name":"match.start","attrs":{"match_id":"live-7"}}"#));
        assert!(!f.may_match(r#"{"name":"match.start","attrs":{"match_id":"live-9"}}"#));
        let names = Filter {
            names: vec!["hit".into(), "cast".into()],
            ..Filter::default()
        };
        assert!(names.may_match(r#"{"name":"cast"}"#));
        assert!(!names.may_match(r#"{"name":"snapshot"}"#));
        Ok(())
    }

    #[test]
    fn exact_stats_use_nearest_rank() -> Result<(), String> {
        let mut s = Stats::default();
        for i in 1..=100 {
            s.add_line(&line("log", "bot.end", json!({"think_ms": i})), "think_ms");
        }
        s.add_line(&line("log", "bot.end", json!({})), "think_ms");
        let m = s.summary().ok_or("values")?;
        assert_eq!((m.lines, m.count), (100, 100));
        assert_eq!(
            (m.min, m.p50, m.p95, m.p99, m.max),
            (1.0, 50.0, 95.0, 99.0, 100.0)
        );
        assert_eq!(m.mean, 50.5);
        assert_eq!(m.sum, 5050.0);
        assert!(Stats::default().summary().is_none());
        Ok(())
    }

    #[test]
    fn histogram_lines_merge_their_buckets() -> Result<(), String> {
        let mut a = Hist::default();
        let mut b = Hist::default();
        for i in 1..=500 {
            a.add(f64::from(i) * 1e-5);
            b.add(f64::from(i + 500) * 1e-5);
        }
        let as_line = |h: &Hist| {
            let buckets: Vec<Value> = h.buckets.iter().map(|(i, n)| json!([i, n])).collect();
            json!({"kind": "metric", "name": "arena_bot_think_seconds", "type": "histogram", "count": h.count,
                   "sum": h.sum, "min": h.min, "max": h.max, "zero": h.zero, "buckets": buckets,
                   "attrs": {"runtime": "onnx"}})
        };
        let mut s = Stats::default();
        s.add_line(&as_line(&a), "value");
        s.add_line(&as_line(&b), "value");
        let m = s.summary().ok_or("values")?;
        assert_eq!((m.lines, m.count), (2, 1000));
        assert_eq!(m.max, 0.01);
        assert_eq!(m.min, 1e-5);
        for (got, want) in [(m.p50, 0.005), (m.p95, 0.0095), (m.p99, 0.0099)] {
            assert!((got / want - 1.0).abs() < 0.05, "{got} vs {want}");
        }
        assert!((m.mean - 0.005005).abs() < 1e-9);
        // a counter's value is a plain number: --stats value sums it
        let mut c = Stats::default();
        for v in [3, 4] {
            c.add_line(&json!({"type": "counter", "value": v}), "value");
        }
        assert_eq!(c.summary().map(|m| m.sum), Some(7.0));
        Ok(())
    }

    #[test]
    fn groups_join_their_keys() {
        let l = line(
            "log",
            "match.end",
            json!({"bot_0": "house", "bot_1": "brain"}),
        );
        let keys = [
            "bot_0".to_string(),
            "bot_1".to_string(),
            "winner_bot".to_string(),
        ];
        assert_eq!(group_of(&l, &keys), "house brain -");
        assert_eq!(group_of(&l, &["service".to_string()]), "arena-server");
    }

    #[test]
    fn a_scan_merges_services_in_time_order_and_seeks() -> std::io::Result<()> {
        let dir = std::env::temp_dir().join(format!("arena-telemetry-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t0 = time::parse_ts("2026-11-18T00:00").unwrap_or(0);
        let lines = |service: &str, offset: i64| -> std::io::Result<()> {
            let p = files::path(&dir, service, "2026-11-18");
            std::fs::create_dir_all(p.parent().unwrap_or(&dir))?;
            let mut text = String::new();
            // a line every 2 s of the day, about 3 MB a file
            for i in 0..43_200 {
                let ts = t0 + offset + i * 2 * time::SECOND;
                text.push_str(&format!(
                    "{{\"ts\":\"{}\",\"kind\":\"log\",\"service\":\"{service}\",\"name\":\"n\",\"attrs\":{{\"i\":{i}}}}}\n",
                    time::format_ts(ts)
                ));
            }
            std::fs::write(p, text)
        };
        lines("arena-server", 0)?;
        lines("arena-bot", time::SECOND)?;
        let from = t0 + 36_000 * time::SECOND;
        let scope = Scope {
            services: vec![],
            from_us: Some(from),
            to_us: Some(from + 9 * time::SECOND),
        };
        let mut seen = Vec::new();
        scan(&dir, &scope, |ts, raw| {
            seen.push((ts, raw.contains("arena-bot")));
            true
        })?;
        let _ = std::fs::remove_dir_all(&dir);
        // ten seconds: five lines of each, alternating, in order
        assert_eq!(seen.len(), 10, "{seen:?}");
        assert!(seen.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(seen[0], (from, false));
        assert_eq!(seen[1], (from + time::SECOND, true));
        Ok(())
    }
}

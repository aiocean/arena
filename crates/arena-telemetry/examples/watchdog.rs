//! The arena's watchdog: a few checks on the live server and its telemetry, run every five minutes by launchd
//! (`io.aiocean.arena-watchdog`, `ops/observe/README.md`, "Alerts"). No email or webhook: each run writes one row a
//! check to the stream `arena_alerts`, appends a line to `~/Library/Logs/game-factory/arena-alerts.log`, and raises a
//! macOS notification when a check turns from ok to failing.
//!
//! ```text
//! ARENA_OTLP_ENDPOINT=http://127.0.0.1:5080/api/default ARENA_OTLP_AUTH_FILE=~/projects/game-factory/ops/arena/observe.env \
//!   cargo run -p arena-telemetry --example watchdog            # one run; --dry prints the checks and writes nothing
//! cargo build --release -p arena-telemetry --example watchdog   # target/release/examples/watchdog, installed as
//!                                                              # ~/services/arena/observe/watchdog
//! ```
//!
//! It exits 0 whatever it finds, the store down included; only its own bugs end it otherwise.

use arena_telemetry::Endpoint;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// No `match.start` from the live server for this long: it is down or stuck between matches.
const MATCH_START_MAX_AGE_S: f64 = 600.0;
/// The newest live snapshot (one a second of fight) older than this: no match is being fought.
const SNAPSHOT_MAX_AGE_S: f64 = 120.0;
/// The window the error and tick checks count over.
const WINDOW_S: i64 = 900;
/// ERROR and WARN rows in `arena_server` over the window; a deploy's restart makes about ten.
const ERRORS_MAX: f64 = 20.0;
/// Ticks begun a whole tick late over the window (60 is one second of fight caught up).
const MISSED_TICKS_MAX: f64 = 60.0;
/// Tick schedule restarts (more than 250 ms behind) over the window.
const RESYNCS_MAX: f64 = 0.0;
/// The worst tick lateness over the window, seconds.
const LATE_MAX_S: f64 = 0.1;
/// The OpenObserve data directory's size cap, bytes.
const DATA_MAX_BYTES: f64 = 5e9;
/// The least free space on the data directory's volume, bytes.
const FREE_MIN_BYTES: f64 = 10e9;

/// Where a check stands this run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Ok,
    Fail,
    /// Not measured (the store is down): it keeps its last state.
    Unknown,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Fail => "fail",
            Status::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Check {
    name: &'static str,
    status: Status,
    value: f64,
    threshold: f64,
    message: String,
}

/// Fails when `value` is above `max`.
fn above(name: &'static str, value: f64, max: f64, what: &str) -> Check {
    let status = if value > max {
        Status::Fail
    } else {
        Status::Ok
    };
    Check {
        name,
        status,
        value,
        threshold: max,
        message: format!("{what}: {value} (fails above {max})"),
    }
}

/// Fails when `value` is below `min`.
fn below(name: &'static str, value: f64, min: f64, what: &str) -> Check {
    let status = if value < min {
        Status::Fail
    } else {
        Status::Ok
    };
    Check {
        name,
        status,
        value,
        threshold: min,
        message: format!("{what}: {value} (fails below {min})"),
    }
}

fn unknown(name: &'static str, threshold: f64, why: &str) -> Check {
    Check {
        name,
        status: Status::Unknown,
        value: f64::NAN,
        threshold,
        message: why.to_string(),
    }
}

/// How old the newest row is, seconds; none in the window counts as the whole window old and fails.
fn freshness(
    name: &'static str,
    newest_us: Option<i64>,
    now_us: i64,
    max_s: f64,
    what: &str,
) -> Check {
    match newest_us {
        Some(t) => above(name, ((now_us - t) / 1_000_000) as f64, max_s, what),
        None => Check {
            name,
            status: Status::Fail,
            value: f64::INFINITY,
            threshold: max_s,
            message: format!("{what}: none in the window (fails above {max_s} s)"),
        },
    }
}

/// A counter or gauge that may have no stream yet: none is ok.
fn optional(name: &'static str, value: Option<f64>, max: f64, what: &str) -> Check {
    match value {
        Some(v) => above(name, v, max, what),
        None => Check {
            name,
            status: Status::Ok,
            value: 0.0,
            threshold: max,
            message: format!("{what}: no data (the stream may not exist yet)"),
        },
    }
}

/// The checks a run makes from what it measured; `None` for a store reading means the store could not be read.
struct Readings {
    now_us: i64,
    store: Result<StoreReadings, String>,
    data_bytes: Result<f64, String>,
    free_bytes: Result<f64, String>,
}

struct StoreReadings {
    last_match_start: Option<i64>,
    last_snapshot: Option<i64>,
    errors: f64,
    panics: f64,
    missed_ticks: Option<f64>,
    resyncs: Option<f64>,
    late_max: Option<f64>,
}

fn judge(r: &Readings) -> Vec<Check> {
    let mut out = Vec::new();
    match &r.store {
        Ok(s) => {
            out.push(Check {
                name: "telemetry",
                status: Status::Ok,
                value: 1.0,
                threshold: 1.0,
                message: "OpenObserve answered".into(),
            });
            out.push(freshness(
                "live_match_start",
                s.last_match_start,
                r.now_us,
                MATCH_START_MAX_AGE_S,
                "seconds since the last match.start",
            ));
            out.push(freshness(
                "live_snapshot",
                s.last_snapshot,
                r.now_us,
                SNAPSHOT_MAX_AGE_S,
                "seconds since the newest snapshot",
            ));
            out.push(above(
                "server_errors",
                s.errors,
                ERRORS_MAX,
                "ERROR/WARN rows in 15 min",
            ));
            out.push(above("server_panics", s.panics, 0.0, "panics in 15 min"));
            out.push(optional(
                "missed_ticks",
                s.missed_ticks,
                MISSED_TICKS_MAX,
                "missed ticks in 15 min",
            ));
            out.push(optional(
                "tick_resyncs",
                s.resyncs,
                RESYNCS_MAX,
                "tick resyncs in 15 min",
            ));
            out.push(optional(
                "tick_late",
                s.late_max,
                LATE_MAX_S,
                "worst tick lateness in 15 min, s",
            ));
        }
        Err(e) => {
            out.push(Check {
                name: "telemetry",
                status: Status::Fail,
                value: 0.0,
                threshold: 1.0,
                message: format!("OpenObserve unreachable: {e}"),
            });
            for (name, t) in [
                ("live_match_start", MATCH_START_MAX_AGE_S),
                ("live_snapshot", SNAPSHOT_MAX_AGE_S),
                ("server_errors", ERRORS_MAX),
                ("server_panics", 0.0),
                ("missed_ticks", MISSED_TICKS_MAX),
                ("tick_resyncs", RESYNCS_MAX),
                ("tick_late", LATE_MAX_S),
            ] {
                out.push(unknown(name, t, "not measured: the store is down"));
            }
        }
    }
    out.push(match &r.data_bytes {
        Ok(b) => above(
            "disk_data",
            *b,
            DATA_MAX_BYTES,
            "OpenObserve data dir, bytes",
        ),
        Err(e) => unknown("disk_data", DATA_MAX_BYTES, e),
    });
    out.push(match &r.free_bytes {
        Ok(b) => below(
            "disk_free",
            *b,
            FREE_MIN_BYTES,
            "free on the data volume, bytes",
        ),
        Err(e) => unknown("disk_free", FREE_MIN_BYTES, e),
    });
    out
}

/// Each check's last known state: `true` when it failed.
type State = BTreeMap<String, bool>;

/// The checks that have just turned from ok (or never seen) to failing, and the state after this run. An unknown
/// check keeps its last state.
fn flips<'a>(prev: &State, checks: &'a [Check]) -> (Vec<&'a Check>, State) {
    let mut next = prev.clone();
    let mut flipped = Vec::new();
    for c in checks {
        let was_failing = prev.get(c.name).copied().unwrap_or(false);
        match c.status {
            Status::Fail => {
                if !was_failing {
                    flipped.push(c);
                }
                next.insert(c.name.to_string(), true);
            }
            Status::Ok => {
                next.insert(c.name.to_string(), false);
            }
            Status::Unknown => {}
        }
    }
    (flipped, next)
}

/// The silence file: one check name a line, or `all`; `#` starts a comment.
fn silenced(text: &str, name: &str) -> bool {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .any(|l| l == "all" || l == name)
}

/// The rows written to `arena_alerts`.
fn rows(checks: &[Check], now_us: i64, silence: &str, flipped: &[&Check]) -> Vec<Value> {
    checks
        .iter()
        .map(|c| {
            json!({
                "_timestamp": now_us,
                "check": c.name,
                "status": c.status.name(),
                "ok": c.status != Status::Fail,
                // JSON has no infinity or NaN: the unmeasured are null
                "value": c.value.is_finite().then_some(c.value),
                "threshold": c.threshold,
                "message": c.message,
                "flipped": flipped.iter().any(|f| f.name == c.name),
                "silenced": silenced(silence, c.name),
                "service_name": "arena-watchdog",
            })
        })
        .collect()
}

/// One log line for the run.
fn log_line(checks: &[Check], when: &str) -> String {
    let failing: Vec<String> = checks
        .iter()
        .filter(|c| c.status == Status::Fail)
        .map(|c| format!("{} ({})", c.name, c.message))
        .collect();
    let unknown = checks
        .iter()
        .filter(|c| c.status == Status::Unknown)
        .count();
    format!(
        "{when} {} checks, {} failing{}{}",
        checks.len(),
        failing.len(),
        if unknown > 0 {
            format!(", {unknown} unknown")
        } else {
            String::new()
        },
        if failing.is_empty() {
            String::new()
        } else {
            format!(": {}", failing.join("; "))
        }
    )
}

/// `secs` since the epoch as `YYYY-MM-DDTHH:MM:SSZ` (Howard Hinnant's civil-from-days).
fn utc(secs: i64) -> String {
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

// ---- the world: the store, the disk, the files, the notification ----

struct Store {
    base: String,
    auth: String,
    agent: ureq::Agent,
}

impl Store {
    /// The first row of `sql` over the last `seconds`.
    fn one(&self, sql: &str, kind: &str, seconds: i64, now_us: i64) -> Result<Value, String> {
        let body = json!({"query": {"sql": sql, "start_time": now_us - seconds * 1_000_000,
            "end_time": now_us, "from": 0, "size": 1}});
        let url = format!("{}/_search?type={kind}", self.base);
        let v: Value = self
            .agent
            .post(&url)
            .header("Authorization", &self.auth)
            .send_json(&body)
            .map_err(|e| format!("{url}: {e}"))?
            .body_mut()
            .read_json()
            .map_err(|e| format!("{url}: {e}"))?;
        Ok(v["hits"].get(0).cloned().unwrap_or(Value::Null))
    }

    fn read(&self, now_us: i64) -> Result<StoreReadings, String> {
        let live = "service_name = 'arena-server'";
        let delta = "aggregation_temporality = 'AGGREGATION_TEMPORALITY_DELTA'";
        let newest = |sql: &str, seconds| -> Result<Option<i64>, String> {
            Ok(self.one(sql, "logs", seconds, now_us)?["t"].as_i64())
        };
        let number = |sql: &str, kind| -> Result<Option<f64>, String> {
            Ok(self.one(sql, kind, WINDOW_S, now_us)?["v"].as_f64())
        };
        Ok(StoreReadings {
            last_match_start: newest(
                &format!(
                    "SELECT max(_timestamp) AS t FROM arena_match WHERE body = 'match.start' AND {live}"
                ),
                MATCH_START_MAX_AGE_S as i64,
            )?,
            last_snapshot: newest(
                &format!(
                    "SELECT max(_timestamp) AS t FROM arena_duel_events WHERE body = 'snapshot' AND {live}"
                ),
                MATCH_START_MAX_AGE_S as i64,
            )?,
            errors: number(
                "SELECT count(*) AS v FROM arena_server WHERE severity IN ('ERROR', 'WARN')",
                "logs",
            )?
            .unwrap_or(0.0),
            panics: number(
                "SELECT count(*) AS v FROM arena_server WHERE body = 'panic'",
                "logs",
            )?
            .unwrap_or(0.0),
            missed_ticks: number(
                &format!(
                    "SELECT sum(value) AS v FROM arena_server_missed_ticks_total WHERE {delta}"
                ),
                "metrics",
            )?,
            resyncs: number(
                &format!("SELECT sum(value) AS v FROM arena_server_resyncs_total WHERE {delta}"),
                "metrics",
            )?,
            late_max: number(
                &format!("SELECT max(value) AS v FROM arena_server_late_seconds_max WHERE {delta}"),
                "metrics",
            )?,
        })
    }

    fn write(&self, rows: &[Value]) -> Result<(), String> {
        let url = format!("{}/arena_alerts/_json", self.base);
        self.agent
            .post(&url)
            .header("Authorization", &self.auth)
            .send_json(rows)
            .map_err(|e| format!("{url}: {e}"))?;
        Ok(())
    }
}

/// The bytes `dir` takes on disk, as `du` counts them: allocated blocks, so sparse files count what they use (links
/// not followed).
fn dir_bytes(dir: &Path) -> Result<f64, String> {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let Ok(e) = e else { continue };
            let Ok(m) = e.path().symlink_metadata() else {
                continue;
            };
            if m.is_dir() {
                stack.push(e.path());
            } else {
                total += std::os::unix::fs::MetadataExt::blocks(&m) * 512;
            }
        }
    }
    Ok(total as f64)
}

/// The free bytes on the volume of `path` (`df -k`, its Available column).
fn free_bytes(path: &Path) -> Result<f64, String> {
    let out = Command::new("/bin/df")
        .arg("-k")
        .arg(path)
        .output()
        .map_err(|e| format!("df: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .last()
        .and_then(|l| l.split_whitespace().nth(3))
        .and_then(|k| k.parse::<f64>().ok())
        .map(|k| k * 1024.0)
        .ok_or_else(|| format!("df {}: {}", path.display(), text.trim()))
}

/// A macOS notification, best-effort.
fn notify(title: &str, text: &str) {
    let _ = Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 2 of argv) with title (item 1 of argv)",
            "-e",
            "end run",
            title,
            text,
        ])
        .output();
}

fn append(path: &Path, line: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path);
    match file {
        Ok(mut f) => {
            let _ = writeln!(f, "{line}");
        }
        Err(e) => eprintln!("{}: {e}", path.display()),
    }
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    std::env::var_os(key).map_or(default, PathBuf::from)
}

fn main() {
    let dry = std::env::args().any(|a| a == "--dry");
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let observe = home.join("services/arena/observe");
    let data_dir = env_path("ARENA_WATCHDOG_DATA", observe.join("data"));
    let state_path = env_path("ARENA_WATCHDOG_STATE", observe.join("watchdog-state.json"));
    let silence_path = env_path("ARENA_WATCHDOG_SILENCE", observe.join("watchdog-silence"));
    let log_path = env_path(
        "ARENA_WATCHDOG_LOG",
        home.join("Library/Logs/game-factory/arena-alerts.log"),
    );

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let now_us = now.as_micros() as i64;
    let store = match Endpoint::from_env() {
        Ok(Some(e)) => Ok(Store {
            base: e.url.trim_end_matches('/').to_string(),
            auth: e.auth.unwrap_or_default(),
            agent: ureq::Agent::config_builder()
                .timeout_global(Some(Duration::from_secs(20)))
                .build()
                .into(),
        }),
        Ok(None) => Err("ARENA_OTLP_ENDPOINT is not set".to_string()),
        Err(e) => Err(e),
    };
    let readings = Readings {
        now_us,
        store: store
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|s| s.read(now_us)),
        data_bytes: dir_bytes(&data_dir),
        free_bytes: free_bytes(&data_dir),
    };
    let checks = judge(&readings);

    let prev: State = std::fs::read_to_string(&state_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let silence = std::fs::read_to_string(&silence_path).unwrap_or_default();
    let (flipped, next) = flips(&prev, &checks);
    let when = utc(now.as_secs() as i64);
    let mut line = log_line(&checks, &when);

    if dry {
        for c in &checks {
            println!("{:<18} {:<7} {}", c.name, c.status.name(), c.message);
        }
        println!("{line}");
        return;
    }

    if let Ok(s) = &store
        && readings.store.is_ok()
        && let Err(e) = s.write(&rows(&checks, now_us, &silence, &flipped))
    {
        line.push_str(&format!(" | arena_alerts not written: {e}"));
    }
    append(&log_path, &line);
    for c in flipped {
        if !silenced(&silence, c.name) {
            notify(&format!("Arena alert: {}", c.name), &c.message);
        }
    }
    if let Ok(text) = serde_json::to_string_pretty(&next) {
        if let Some(dir) = state_path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&state_path, text) {
            append(&log_path, &format!("{when} state not saved: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000_000_000;

    type Change = Box<dyn Fn(&mut StoreReadings)>;

    fn healthy() -> StoreReadings {
        StoreReadings {
            last_match_start: Some(NOW - 60_000_000),
            last_snapshot: Some(NOW - 2_000_000),
            errors: 3.0,
            panics: 0.0,
            missed_ticks: None,
            resyncs: None,
            late_max: Some(0.01),
        }
    }

    fn readings(store: Result<StoreReadings, String>) -> Readings {
        Readings {
            now_us: NOW,
            store,
            data_bytes: Ok(71e6),
            free_bytes: Ok(33e9),
        }
    }

    fn status(checks: &[Check], name: &str) -> Option<Status> {
        checks.iter().find(|c| c.name == name).map(|c| c.status)
    }

    #[test]
    fn a_healthy_server_passes_and_missing_streams_are_ok() {
        let checks = judge(&readings(Ok(healthy())));
        assert_eq!(checks.len(), 10);
        assert!(checks.iter().all(|c| c.status == Status::Ok), "{checks:?}");
    }

    #[test]
    fn each_threshold_fails_just_past_it() {
        let cases: Vec<(&str, Change)> = vec![
            (
                "live_match_start",
                Box::new(|s| s.last_match_start = Some(NOW - 601_000_000)),
            ),
            ("live_match_start", Box::new(|s| s.last_match_start = None)),
            (
                "live_snapshot",
                Box::new(|s| s.last_snapshot = Some(NOW - 121_000_000)),
            ),
            ("live_snapshot", Box::new(|s| s.last_snapshot = None)),
            ("server_errors", Box::new(|s| s.errors = 21.0)),
            ("server_panics", Box::new(|s| s.panics = 1.0)),
            ("missed_ticks", Box::new(|s| s.missed_ticks = Some(61.0))),
            ("tick_resyncs", Box::new(|s| s.resyncs = Some(1.0))),
            ("tick_late", Box::new(|s| s.late_max = Some(0.15))),
        ];
        for (name, change) in cases {
            let mut s = healthy();
            change(&mut s);
            let checks = judge(&readings(Ok(s)));
            assert_eq!(status(&checks, name), Some(Status::Fail), "{name}");
            let failing = checks.iter().filter(|c| c.status == Status::Fail).count();
            assert_eq!(failing, 1, "{name} alone fails: {checks:?}");
        }
        // at the threshold itself it still passes
        let mut s = healthy();
        s.errors = 20.0;
        s.missed_ticks = Some(60.0);
        s.resyncs = Some(0.0);
        s.late_max = Some(0.1);
        s.last_snapshot = Some(NOW - 120_000_000);
        assert!(
            judge(&readings(Ok(s)))
                .iter()
                .all(|c| c.status == Status::Ok)
        );
    }

    #[test]
    fn disk_over_cap_or_short_of_space_fails() {
        let mut r = readings(Ok(healthy()));
        r.data_bytes = Ok(6e9);
        r.free_bytes = Ok(9e9);
        let checks = judge(&r);
        assert_eq!(status(&checks, "disk_data"), Some(Status::Fail));
        assert_eq!(status(&checks, "disk_free"), Some(Status::Fail));
        r.data_bytes = Err("gone".into());
        assert_eq!(status(&judge(&r), "disk_data"), Some(Status::Unknown));
    }

    #[test]
    fn a_store_down_fails_telemetry_and_leaves_the_rest_unknown() {
        let checks = judge(&readings(Err("connection refused".into())));
        assert_eq!(status(&checks, "telemetry"), Some(Status::Fail));
        assert_eq!(status(&checks, "live_match_start"), Some(Status::Unknown));
        assert_eq!(status(&checks, "disk_free"), Some(Status::Ok));
        // the rows carry no NaN
        let r = rows(&checks, NOW, "", &[]);
        assert!(r.iter().all(|row| !row.to_string().contains("NaN")));
    }

    #[test]
    fn only_ok_to_fail_notifies_and_unknown_keeps_the_last_state() {
        let fail = |name| Check {
            name,
            status: Status::Fail,
            value: 1.0,
            threshold: 0.0,
            message: String::new(),
        };
        let ok = |name| Check {
            status: Status::Ok,
            ..fail(name)
        };
        let unk = |name| Check {
            status: Status::Unknown,
            ..fail(name)
        };
        // first run: a failing check flips (never seen counts as ok)
        let run1 = [fail("a"), ok("b")];
        let (f, s1) = flips(&State::new(), &run1);
        assert_eq!(f.iter().map(|c| c.name).collect::<Vec<_>>(), ["a"]);
        // still failing: no second notification
        let run2 = [fail("a"), ok("b")];
        let (f, s2) = flips(&s1, &run2);
        assert!(f.is_empty());
        // unknown keeps a failing; b flips
        let run3 = [unk("a"), fail("b")];
        let (f, s3) = flips(&s2, &run3);
        assert_eq!(f.iter().map(|c| c.name).collect::<Vec<_>>(), ["b"]);
        assert_eq!(s3.get("a"), Some(&true));
        // a recovers, then fails again: it notifies again
        let (_, s4) = flips(&s3, &[ok("a")]);
        let run5 = [fail("a")];
        let (f, _) = flips(&s4, &run5);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn the_silence_file_names_checks_or_all() {
        let text =
            "# quiet while I deploy\nserver_errors\n  disk_free  # until the disk is cleaned\n";
        assert!(silenced(text, "server_errors"));
        assert!(silenced(text, "disk_free"));
        assert!(!silenced(text, "telemetry"));
        assert!(silenced("all\n", "telemetry"));
        assert!(!silenced("", "telemetry"));
    }

    #[test]
    fn utc_formats_the_epoch() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc(1_794_942_538), "2026-11-17T19:08:58Z");
    }

    #[test]
    fn the_log_line_names_the_failing() {
        let mut s = healthy();
        s.panics = 2.0;
        let line = log_line(&judge(&readings(Ok(s))), "t=1");
        assert!(
            line.starts_with("t=1 10 checks, 1 failing: server_panics"),
            "{line}"
        );
    }
}

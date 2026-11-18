//! The arena's telemetry, at the source: `tracing` everywhere, and one `init` per process that writes it to stderr
//! and, when `ARENA_TELEMETRY_DIR` is set (or a tool is given `--telemetry-dir DIR`), to JSON-lines files in that
//! directory: logs, finished spans and metrics, one file a service and UTC day (`<dir>/<service>/<YYYY-MM-DD>.jsonl`),
//! kept 14 days and under 2 GB. No collector, no network, no store. Writing is off the hot path (a bounded channel to
//! a writer thread; a full one drops and counts), flushed when the `Guard` drops. Unconfigured, nothing is written and
//! the arena's own event targets are off at their call sites. See `ops/telemetry/README.md`.
//!
//! `read` streams the files back (the query tool, the watchdog and the smoke test use it). With the `duel` feature,
//! `duel::Observer` turns what a duel step returns into events, without touching the fight.

#[cfg(feature = "duel")]
pub mod duel;
#[cfg(feature = "export")]
mod export;
pub mod files;
pub mod hist;
pub mod read;
pub mod time;

#[cfg(feature = "export")]
pub use export::{Guard, Options, init, layer};
pub use files::{ENV, Retention, default_dir, env_dir};

/// The targets the arena's events go to (a line's `target`).
pub mod target {
    /// What happens in a fight (casts, hits, statuses, units, snapshots): `arena::duel`.
    pub const DUEL: &str = "arena::duel";
    /// A match's and a round's start and end.
    pub const MATCH: &str = "arena::match";
    /// The bot host (think time, fuel, faults): filed under service `arena-bot`, whichever process runs the bot.
    pub const BOT: &str = "arena::bot";
    /// Balance and ladder runs (match summaries, per-gem rows).
    pub const LADDER: &str = "arena::ladder";
    /// Metric events (`monotonic_counter.*`, `counter.*`, `histogram.*`): summed into metric lines, never a log.
    pub const METRICS: &str = "arena::metrics";
}

/// A routed target: the targets written whole (info and above), and the service filed under when not the process's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub target: &'static str,
    pub service: Option<&'static str>,
}

/// Every routed target. The rest of a process's events go through its `export_filter`.
pub const ROUTES: [Route; 4] = [
    Route {
        target: target::DUEL,
        service: None,
    },
    Route {
        target: target::MATCH,
        service: None,
    },
    Route {
        target: target::BOT,
        service: Some("arena-bot"),
    },
    Route {
        target: target::LADDER,
        service: None,
    },
];

/// The route of `target`, if it has one (a target or one under it: `arena::duel` and `arena::duel::x`).
pub fn route_of(target: &str) -> Option<&'static Route> {
    ROUTES.iter().find(|r| under(target, r.target))
}

/// Whether `target` is `prefix` or a target under it.
pub fn under(target: &str, prefix: &str) -> bool {
    target
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
}

/// The flag a tool (the ladder, the balance run, a sim) takes to write its runs: `--telemetry-dir DIR`.
pub const FLAG: &str = "--telemetry-dir";

/// Where a tool should write: the directory after `--telemetry-dir` among `args`, else `ARENA_TELEMETRY_DIR`;
/// `None`: nowhere. The flag without a directory is an error, so a run meant to be measured is never silently lost.
pub fn requested(args: &[String]) -> Result<Option<std::path::PathBuf>, String> {
    match args.iter().position(|a| a == FLAG) {
        Some(i) => match args.get(i + 1).filter(|d| !d.starts_with("--")) {
            Some(d) => Ok(Some(std::path::PathBuf::from(d))),
            None => Err(format!("{FLAG} needs a directory")),
        },
        None => Ok(env_dir()),
    }
}

/// `args` without `--telemetry-dir` and its directory, for a tool that reads the rest itself.
pub fn strip(args: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == FLAG {
            it.next();
        } else {
            out.push(a);
        }
    }
    out
}

/// An id for one run of a tool, `prefix-<unix ms>`, on every event it sends.
pub fn run_id(prefix: &str) -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    format!("{prefix}-{ms}")
}

/// The commit the telemetry was built at (`build.rs`), or "unknown".
pub const GIT_HASH: &str = env!("ARENA_GIT_HASH");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_match_whole_path_segments() {
        assert_eq!(
            route_of("arena::duel").map(|r| r.target),
            Some(target::DUEL)
        );
        assert_eq!(
            route_of("arena::duel::snap").map(|r| r.target),
            Some(target::DUEL)
        );
        assert_eq!(route_of("arena::duelist"), None);
        assert_eq!(route_of("arena_server"), None);
        assert_eq!(
            route_of(target::BOT).and_then(|r| r.service),
            Some("arena-bot")
        );
    }

    #[test]
    fn the_flag_names_a_directory_and_is_stripped() {
        let args = |a: &[&str]| a.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        assert_eq!(
            requested(&args(&["--seeds", "3", FLAG, "/tmp/t"])),
            Ok(Some(std::path::PathBuf::from("/tmp/t")))
        );
        assert!(requested(&args(&[FLAG])).is_err());
        assert!(requested(&args(&[FLAG, "--seeds"])).is_err());
        assert_eq!(
            strip(args(&["--seeds", "3", FLAG, "/tmp/t", "--x"])),
            args(&["--seeds", "3", "--x"])
        );
    }
}

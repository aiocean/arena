//! The arena's telemetry, at the source: `tracing` everywhere, and one `init` per process that writes it to stderr
//! and, when `ARENA_OTLP_ENDPOINT` is set (`config`), exports it over OTLP/HTTP (protobuf) into the central
//! OpenObserve: logs, traces and metrics, batched, flushed when the `Guard` drops. Unconfigured, nothing leaves the
//! process and the arena's own event targets are off at their call sites. See `ops/observe/README.md`.
//!
//! What goes where is decided by the event's target (`target`): each routed target is a log stream of its own,
//! everything else a process's service stream (`arena-server` → `arena_server`). With the `duel` feature,
//! `duel::Observer` turns what a duel step returns into events on those targets, without touching the fight.

pub mod config;
#[cfg(feature = "duel")]
pub mod duel;
#[cfg(feature = "otlp")]
mod pipeline;

pub use config::Endpoint;
#[cfg(feature = "otlp")]
pub use pipeline::{Guard, Options, init};

/// The targets the arena's events go to, each a stream of its own when exported.
pub mod target {
    /// What happens in a fight (casts, hits, statuses, units, snapshots): stream `arena_duel_events`.
    pub const DUEL: &str = "arena::duel";
    /// A match's and a round's start and end: stream `arena_match`.
    pub const MATCH: &str = "arena::match";
    /// The bot host (think time, fuel, faults): stream `arena_bot`, service `arena-bot`.
    pub const BOT: &str = "arena::bot";
    /// Balance and ladder runs (match summaries, per-gem rows): stream `arena_ladder`.
    pub const LADDER: &str = "arena::ladder";
    /// Metric events (`monotonic_counter.*`, `histogram.*`): to the metrics pipeline only, never a log.
    pub const METRICS: &str = "arena::metrics";
}

/// A routed target: its stream and, when not the process's own, the service it is filed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub target: &'static str,
    pub stream: &'static str,
    pub service: Option<&'static str>,
}

/// Every routed target. The rest of a process's events go to its service's stream.
pub const ROUTES: [Route; 4] = [
    Route {
        target: target::DUEL,
        stream: "arena_duel_events",
        service: None,
    },
    Route {
        target: target::MATCH,
        stream: "arena_match",
        service: None,
    },
    Route {
        target: target::BOT,
        stream: "arena_bot",
        service: Some("arena-bot"),
    },
    Route {
        target: target::LADDER,
        stream: "arena_ladder",
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

/// The log stream of service `service`'s own events: `arena-server` → `arena_server`.
pub fn service_stream(service: &str) -> String {
    service
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// The flag a tool (the ladder, the balance run, a sim) takes to export its runs.
pub const FLAG: &str = "--telemetry-otlp";

/// Whether a tool should export: `--telemetry-otlp` among `args`, or `ARENA_OTLP_ENDPOINT` set. The flag without
/// an endpoint is an error, so a run meant to be measured is never silently lost.
pub fn requested(args: &[String]) -> Result<bool, String> {
    let flag = args.iter().any(|a| a == FLAG);
    let endpoint = std::env::var("ARENA_OTLP_ENDPOINT").is_ok_and(|v| !v.trim().is_empty());
    if flag && !endpoint {
        return Err(format!(
            "{FLAG} needs ARENA_OTLP_ENDPOINT (and ARENA_OTLP_AUTH_FILE): see ops/observe/README.md"
        ));
    }
    Ok(flag || endpoint)
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
            route_of("arena::duel").map(|r| r.stream),
            Some("arena_duel_events")
        );
        assert_eq!(
            route_of("arena::duel::snap").map(|r| r.stream),
            Some("arena_duel_events")
        );
        assert_eq!(route_of("arena::duelist"), None);
        assert_eq!(route_of("arena_server"), None);
        assert_eq!(
            route_of(target::BOT).and_then(|r| r.service),
            Some("arena-bot")
        );
        assert_eq!(service_stream("arena-server"), "arena_server");
        assert_eq!(service_stream("arena-ladder"), "arena_ladder");
    }
}

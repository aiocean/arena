//! The telemetry store's dashboards (OpenObserve, `ops/observe/README.md`), made here and pushed through its API.
//!
//! ```text
//! ARENA_OTLP_ENDPOINT=http://127.0.0.1:5080/api/default ARENA_OTLP_AUTH_FILE=~/projects/game-factory/ops/arena/observe.env \
//!   cargo run -p arena-telemetry --example dashboards -- --push [--out ops/observe/dashboards]
//! cargo run -p arena-telemetry --example dashboards -- --from ops/observe/dashboards    # restore the committed ones
//! cargo run -p arena-telemetry --example dashboards -- --check     # run every panel's query against the store
//! ```
//!
//! `--push` creates each dashboard below, or updates the one of the same title, then writes what the store exports
//! of it to `--out` (the files committed beside the README). `--from DIR` pushes those files as they are, to a fresh
//! store. Backend only: game events come from the live server (`service_name = 'arena-server'`), balance from the
//! ladder and sim runs, bots from the bot host. The few cumulative metric points from before the metrics went delta
//! are left out (`DELTA`).

use arena_telemetry::Endpoint;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The width of the dashboard grid, in columns.
const COLS: u32 = 48;
/// The live server's own rows.
const LIVE: &str = "service_name = 'arena-server'";
/// Metric points sent as deltas (each its interval's own counts).
const DELTA: &str = "aggregation_temporality = 'AGGREGATION_TEMPORALITY_DELTA'";

/// A panel: what it draws, from what, and how wide and tall it is.
struct Panel {
    kind: &'static str,
    title: &'static str,
    description: &'static str,
    stream: &'static str,
    stream_type: &'static str,
    sql: String,
    /// The x columns (label, alias), the y columns, the breakdown columns.
    x: Vec<(&'static str, &'static str)>,
    y: Vec<(&'static str, &'static str)>,
    by: Vec<(&'static str, &'static str)>,
    unit: Option<&'static str>,
    w: u32,
    h: u32,
}

fn panel(
    kind: &'static str,
    title: &'static str,
    stream: &'static str,
    sql: impl Into<String>,
) -> Panel {
    Panel {
        kind,
        title,
        description: "",
        stream,
        stream_type: "logs",
        sql: sql.into(),
        x: vec![("", "x_axis_1")],
        y: vec![("", "y_axis_1")],
        by: Vec::new(),
        unit: None,
        w: COLS / 2,
        h: 9,
    }
}

impl Panel {
    fn about(mut self, d: &'static str) -> Panel {
        self.description = d;
        self
    }
    fn metrics(mut self) -> Panel {
        self.stream_type = "metrics";
        self
    }
    fn axes(mut self, x: &'static str, y: &'static str) -> Panel {
        self.x = vec![(x, "x_axis_1")];
        self.y = vec![(y, "y_axis_1")];
        self
    }
    fn ys(mut self, y: &[(&'static str, &'static str)]) -> Panel {
        self.y = y.to_vec();
        self
    }
    fn by(mut self, label: &'static str, alias: &'static str) -> Panel {
        self.by = vec![(label, alias)];
        self
    }
    /// A table's columns: the first is its x, the rest its y.
    fn columns(mut self, cols: &[(&'static str, &'static str)]) -> Panel {
        self.x = cols[..1].to_vec();
        self.y = cols[1..].to_vec();
        self
    }
    fn unit(mut self, u: &'static str) -> Panel {
        self.unit = Some(u);
        self
    }
    fn size(mut self, w: u32, h: u32) -> Panel {
        self.w = w;
        self.h = h;
        self
    }
}

fn field(label: &str, alias: &str) -> Value {
    json!({ "label": label, "alias": alias, "column": alias, "color": null })
}

/// A dashboard in OpenObserve's own form (version 5): one tab, its panels laid left to right, row after row.
fn dashboard(title: &str, description: &str, period: &str, panels: Vec<Panel>) -> Value {
    let (mut x, mut y, mut row_h) = (0, 0, 0);
    let mut out = Vec::new();
    for (i, p) in panels.into_iter().enumerate() {
        if x + p.w > COLS {
            x = 0;
            y += row_h;
            row_h = 0;
        }
        out.push(json!({
            "id": format!("Panel_{}", i + 1),
            "type": p.kind,
            "title": p.title,
            "description": p.description,
            "config": {
                "show_legends": !p.by.is_empty() || p.y.len() > 1 || matches!(p.kind, "pie" | "donut"),
                "legends_position": null, "unit": p.unit, "unit_custom": null, "decimals": 2,
                "axis_border_show": false, "connect_nulls": true, "no_value_replacement": "",
                "wrap_table_cells": false, "table_transpose": false, "table_dynamic_columns": false,
                "mark_line": [], "override_config": [],
            },
            "queryType": "sql",
            "queries": [{
                "query": p.sql,
                "vrlFunctionQuery": "",
                "customQuery": true,
                "fields": {
                    "stream": p.stream,
                    "stream_type": p.stream_type,
                    // a single number has no x
                    "x": p.x.iter().filter(|_| p.kind != "metric").map(|(l, a)| field(l, a)).collect::<Vec<_>>(),
                    "y": p.y.iter().map(|(l, a)| field(l, a)).collect::<Vec<_>>(),
                    "z": [],
                    "breakdown": p.by.iter().map(|(l, a)| field(l, a)).collect::<Vec<_>>(),
                    "filter": { "filterType": "group", "logicalOperator": "AND", "conditions": [] },
                },
                "config": {
                    "promql_legend": "", "layer_type": "scatter", "weight_fixed": 1, "limit": 0,
                    "min": 0, "max": 100, "time_shift": [],
                },
            }],
            "layout": { "x": x, "y": y, "w": p.w, "h": p.h, "i": i + 1 },
            "htmlContent": "",
            "markdownContent": "",
            "customChartContent": "",
        }));
        x += p.w;
        row_h = row_h.max(p.h);
    }
    json!({
        "version": 5,
        "dashboardId": "",
        "title": title,
        "description": description,
        "role": "",
        "owner": "",
        "created": "2026-11-18T00:00:00.000Z",
        "tabs": [{ "tabId": "default", "name": "Default", "panels": out }],
        "variables": { "list": [], "showDynamicFilters": true },
        "defaultDatetimeDuration": { "type": "relative", "relativeTimePeriod": period, "startTime": 0, "endTime": 0 },
    })
}

/// A histogram's buckets as counts per bucket (the store keeps them cumulative over `le`), summed over the range.
fn buckets(stream: &str, filter: &str) -> String {
    format!(
        "SELECT concat('≤ ', le) AS x_axis_1, c - coalesce(lag(c) OVER (ORDER BY CAST(le AS DOUBLE)), 0) AS y_axis_1 \
         FROM (SELECT le, sum(value) AS c FROM {stream} WHERE {DELTA}{filter} GROUP BY le) b ORDER BY CAST(le AS DOUBLE)"
    )
}

/// A histogram's quantile `q` over the range: the upper bound of the bucket it falls in.
fn quantile(name: &str, q: f64) -> String {
    format!(
        "SELECT min(CAST(le AS DOUBLE)) AS y_axis_1 FROM (SELECT le, sum(value) AS c FROM {name}_bucket WHERE {DELTA} \
         GROUP BY le) b WHERE c >= {q} * (SELECT sum(value) FROM {name}_count WHERE {DELTA})"
    )
}

/// A histogram's mean and largest value per minute, in milliseconds.
fn mean_max_ms(name: &str) -> String {
    format!(
        "SELECT s.x_axis_1, s.v / c.v * 1000 AS mean, m.v * 1000 AS most FROM \
         (SELECT histogram(_timestamp, '1 minute') AS x_axis_1, sum(value) AS v FROM {name}_sum WHERE {DELTA} GROUP BY x_axis_1) s \
         JOIN (SELECT histogram(_timestamp, '1 minute') AS x_axis_1, sum(value) AS v FROM {name}_count WHERE {DELTA} GROUP BY x_axis_1) c \
         ON s.x_axis_1 = c.x_axis_1 \
         JOIN (SELECT histogram(_timestamp, '1 minute') AS x_axis_1, max(value) AS v FROM {name}_max WHERE {DELTA} GROUP BY x_axis_1) m \
         ON s.x_axis_1 = m.x_axis_1 ORDER BY s.x_axis_1"
    )
}

/// The rows of the latest run of `body` in `arena_ladder` (by run id, `prefix-<unix ms>`), with `cols`.
fn latest(body: &str, cols: &str, filter: &str, order: &str) -> String {
    format!(
        "SELECT {cols} FROM (SELECT *, max(run_id) OVER () AS latest FROM arena_ladder WHERE body = '{body}'{filter}) t \
         WHERE run_id = latest ORDER BY {order}"
    )
}

fn live_matches() -> Value {
    let m = "arena_match";
    dashboard(
        "Arena: live matches",
        "The live server's house matches (arena-server): how many, how they end, how long they last.",
        "24h",
        vec![
            panel("metric", "Matches in progress", m, format!(
                "SELECT count(DISTINCT match_id) AS y_axis_1 FROM arena_match WHERE body = 'match.start' AND {LIVE} \
                 AND _timestamp >= (to_unixtime(now()) - 180) * 1000000 \
                 AND match_id NOT IN (SELECT match_id FROM arena_match WHERE body = 'match.end')"))
                .about("Started in the last three minutes (a match is cut at two) and not ended.")
                .axes("", "In progress").size(COLS / 4, 6),
            panel("metric", "Matches played", m, format!(
                "SELECT count(*) AS y_axis_1 FROM arena_match WHERE body = 'match.end' AND result != 'abandoned' AND {LIVE}"))
                .axes("", "Matches").size(COLS / 4, 6),
            panel("metric", "Mean match length", m, format!(
                "SELECT avg(seconds) AS y_axis_1 FROM arena_match WHERE body = 'match.end' AND result != 'abandoned' AND {LIVE}"))
                .about("Fight time, seconds.").axes("", "Seconds").unit("seconds").size(COLS / 4, 6),
            panel("metric", "Mean rounds a match", m, format!(
                "SELECT avg(rounds) AS y_axis_1 FROM arena_match WHERE body = 'match.end' AND result != 'abandoned' AND {LIVE}"))
                .axes("", "Rounds").size(COLS / 4, 6),
            panel("bar", "Matches per hour", m, format!(
                "SELECT histogram(_timestamp, '1 hour') AS x_axis_1, count(DISTINCT match_id) AS y_axis_1 FROM arena_match \
                 WHERE body = 'match.start' AND {LIVE} GROUP BY x_axis_1 ORDER BY x_axis_1"))
                .axes("Hour", "Matches started"),
            panel("pie", "Results", m, format!(
                "SELECT result AS x_axis_1, count(*) AS y_axis_1 FROM arena_match WHERE body = 'match.end' AND {LIVE} \
                 GROUP BY x_axis_1 ORDER BY y_axis_1 DESC"))
                .about("seat0 / seat1: two round wins; cut: the two-minute limit (a draw); abandoned: the match stopped \
                        before it ended (a restart).")
                .axes("Result", "Matches"),
            panel("bar", "Match length", m, format!(
                "SELECT floor(seconds / 15) * 15 AS x_axis_1, count(*) AS y_axis_1 FROM arena_match \
                 WHERE body = 'match.end' AND result != 'abandoned' AND {LIVE} GROUP BY x_axis_1 ORDER BY x_axis_1"))
                .about("Fight seconds, in bins of 15 s (120 is the cut).")
                .axes("Seconds (from)", "Matches"),
            panel("bar", "Rounds per match", m, format!(
                "SELECT rounds AS x_axis_1, count(*) AS y_axis_1 FROM arena_match WHERE body = 'match.end' \
                 AND result != 'abandoned' AND {LIVE} GROUP BY x_axis_1 ORDER BY x_axis_1"))
                .axes("Rounds", "Matches"),
            panel("table", "Recent matches", m, format!(
                "SELECT to_timestamp_micros(_timestamp) AS ended, match_id, result, winner_bot, rounds, wins_0, wins_1, \
                 seconds, wall_seconds, bot_0, bot_1 FROM arena_match WHERE body = 'match.end' AND {LIVE} \
                 ORDER BY _timestamp DESC LIMIT 50"))
                .columns(&[
                    ("Ended", "ended"), ("Match", "match_id"), ("Result", "result"), ("Winner", "winner_bot"),
                    ("Rounds", "rounds"), ("Wins 0", "wins_0"), ("Wins 1", "wins_1"), ("Fight s", "seconds"),
                    ("Wall s", "wall_seconds"), ("Seat 0", "bot_0"), ("Seat 1", "bot_1"),
                ])
                .size(COLS, 12),
        ],
    )
}

fn game_events() -> Value {
    let d = "arena_duel_events";
    dashboard(
        "Arena: game events",
        "What happens in the live matches (arena-server, the duel observer): casts, damage, hits, ailments, units, KOs.",
        "24h",
        vec![
            panel("h-bar", "Casts per skill (top 20)", d, format!(
                "SELECT skill AS x_axis_1, count(*) AS y_axis_1 FROM arena_duel_events WHERE body = 'cast' AND {LIVE} \
                 GROUP BY x_axis_1 ORDER BY y_axis_1 DESC LIMIT 20"))
                .axes("Skill", "Casts").size(COLS / 2, 14),
            panel("h-bar", "Damage per skill (top 20)", d, format!(
                "SELECT CASE WHEN body = 'hit' THEN skill ELSE concat('dot: ', cause) END AS x_axis_1, \
                 sum(damage) AS y_axis_1 FROM arena_duel_events WHERE body IN ('hit', 'dot') AND {LIVE} \
                 GROUP BY x_axis_1 ORDER BY y_axis_1 DESC LIMIT 20"))
                .about("Hits by skill, and damage over time by cause (burn, bleed, a curse's strike).")
                .axes("Skill", "Damage").size(COLS / 2, 14),
            panel("pie", "Damage per kind", "arena_damage_total", format!(
                "SELECT kind AS x_axis_1, sum(value) AS y_axis_1 FROM arena_damage_total WHERE {DELTA} AND {LIVE} \
                 GROUP BY x_axis_1 ORDER BY y_axis_1 DESC"))
                .about("A hit's damage split over its skill's kinds; other: damage over time and curses.")
                .metrics().axes("Kind", "Damage").size(COLS / 3, 10),
            panel("pie", "Hits by how", d, format!(
                "SELECT how AS x_axis_1, count(*) AS y_axis_1 FROM arena_duel_events WHERE body = 'hit' AND {LIVE} \
                 GROUP BY x_axis_1"))
                .about("plain; counter: struck in the foe's windup; punish: struck in its recovery.")
                .axes("How", "Hits").size(COLS / 3, 10),
            panel("pie", "Blows stopped by how", d, format!(
                "SELECT how AS x_axis_1, count(*) AS y_axis_1 FROM arena_duel_events WHERE body = 'stopped' AND {LIVE} \
                 GROUP BY x_axis_1"))
                .axes("How", "Blows").size(COLS / 3, 10),
            panel("bar", "Ailments laid", d, format!(
                "SELECT ailment AS x_axis_1, count(*) AS y_axis_1 FROM arena_duel_events WHERE body = 'ailment' \
                 AND stacks > before AND {LIVE} GROUP BY x_axis_1 ORDER BY y_axis_1 DESC"))
                .about("Each time a fighter's stacks of an ailment rose.")
                .axes("Ailment", "Times laid").size(COLS / 3, 10),
            panel("bar", "Units spawned, by kind", d, format!(
                "SELECT kind AS x_axis_1, count(*) AS y_axis_1 FROM arena_duel_events WHERE body = 'unit.spawn' \
                 AND {LIVE} GROUP BY x_axis_1 ORDER BY y_axis_1 DESC"))
                .axes("Kind", "Spawned").size(COLS / 3, 10),
            panel("bar", "Units: spawned, acted, hurt, killed", d, format!(
                "SELECT CASE body WHEN 'unit.spawn' THEN '1 spawned' WHEN 'unit.act' THEN '2 acted' \
                 WHEN 'unit.hurt' THEN '3 hurt' ELSE '4 killed' END AS x_axis_1, count(*) AS y_axis_1 \
                 FROM arena_duel_events WHERE body IN ('unit.spawn', 'unit.act', 'unit.hurt', 'unit.die') AND {LIVE} \
                 GROUP BY x_axis_1 ORDER BY x_axis_1"))
                .axes("", "Units").size(COLS / 3, 10),
            panel("bar", "KOs per round", d, format!(
                "SELECT round AS x_axis_1, count(*) AS y_axis_1 FROM arena_duel_events WHERE body = 'ko' AND {LIVE} \
                 GROUP BY x_axis_1 ORDER BY x_axis_1"))
                .about("KOs by the round of the match they ended (a round with no KO was cut).")
                .axes("Round", "KOs"),
            panel("line", "Game events per minute", d, format!(
                "SELECT histogram(_timestamp, '1 minute') AS x_axis_1, body AS b, count(*) AS y_axis_1 \
                 FROM arena_duel_events WHERE {LIVE} AND body IN ('cast', 'hit', 'dot', 'stopped', 'ko') \
                 GROUP BY x_axis_1, b ORDER BY x_axis_1"))
                .about("A flat line while a match runs means the observer has stopped reporting.")
                .axes("Minute", "Events").by("Event", "b"),
        ],
    )
}

fn server_health() -> Value {
    let s = "arena_server";
    dashboard(
        "Arena: server health",
        "The live server (arena-server): its step and publish times, how late it runs, the relay, its errors.",
        "24h",
        vec![
            panel("bar", "Step time", "arena_step_seconds_bucket", buckets("arena_step_seconds_bucket", &format!(" AND {LIVE}")))
                .about("How many duel steps took up to each bound, seconds (one step is a 60th of a second of fight).")
                .metrics().axes("Seconds", "Steps"),
            panel("line", "Step time, mean and most (ms)", "arena_step_seconds_sum", mean_max_ms("arena_step_seconds"))
                .metrics().ys(&[("Mean", "mean"), ("Most", "most")]).unit("milliseconds"),
            panel("line", "Publish time, mean and most (ms)", "arena_server_publish_seconds_sum",
                mean_max_ms("arena_server_publish_seconds"))
                .about("A tick stepped and written to the MoQ track.")
                .metrics().ys(&[("Mean", "mean"), ("Most", "most")]).unit("milliseconds"),
            panel("line", "Behind the wall clock, mean and most (ms)", "arena_server_late_seconds_sum",
                mean_max_ms("arena_server_late_seconds"))
                .about("How far a tick lands after its time; the server sleeps to its next one.")
                .metrics().ys(&[("Mean", "mean"), ("Most", "most")]).unit("milliseconds"),
            panel("bar", "Errors and warnings", s,
                "SELECT histogram(_timestamp, '1 hour') AS x_axis_1, severity AS sev, count(*) AS y_axis_1 \
                 FROM arena_server WHERE severity IN ('ERROR', 'WARN') GROUP BY x_axis_1, sev ORDER BY x_axis_1")
                .axes("Hour", "Events").by("Severity", "sev"),
            panel("bar", "Relay subscription changes", s,
                "SELECT histogram(_timestamp, '1 hour') AS x_axis_1, count(*) AS y_axis_1 FROM arena_server \
                 WHERE body = 'relay subscription' GROUP BY x_axis_1 ORDER BY x_axis_1")
                .about("The relay taking or dropping a match's track (it sees the relay, not each viewer). Its \
                        `watched` field is in Logs, stream arena_server.")
                .axes("Hour", "Changes"),
            panel("metric", "Panics", s,
                "SELECT count(*) AS y_axis_1 FROM arena_server WHERE body = 'panic'")
                .about("Each with its reason and location in Logs, stream arena_server, body = 'panic'.")
                .axes("", "Panics").size(COLS / 4, 9),
            panel("metric", "Server starts", s,
                "SELECT count(*) AS y_axis_1 FROM arena_server WHERE body = 'publishing to the relay'")
                .about("Each start (a deploy, a crash and relaunch).")
                .axes("", "Starts").size(COLS / 4, 9),
            panel("table", "Latest errors and warnings", s,
                "SELECT to_timestamp_micros(_timestamp) AS t, severity, body, match_id, error FROM arena_server \
                 WHERE severity IN ('ERROR', 'WARN') ORDER BY _timestamp DESC LIMIT 50")
                .columns(&[("Time", "t"), ("Severity", "severity"), ("Event", "body"), ("Match", "match_id"), ("Error", "error")])
                .size(COLS, 10),
            panel("table", "Alerts: each check's latest result", "arena_alerts",
                "SELECT \"check\" AS c, status, value, threshold, message, to_timestamp_micros(_timestamp) AS t \
                 FROM (SELECT *, row_number() OVER (PARTITION BY \"check\" ORDER BY _timestamp DESC) AS rn \
                 FROM arena_alerts) a WHERE rn = 1 ORDER BY ok, c")
                .about("The watchdog (io.aiocean.arena-watchdog, every five minutes): failing first. What each check \
                        means and how to silence it: ops/observe/README.md, Alerts.")
                .columns(&[("Check", "c"), ("Status", "status"), ("Value", "value"), ("Threshold", "threshold"),
                    ("Message", "message"), ("Checked", "t")])
                .size(COLS / 2, 10),
            panel("bar", "Alerts: failures over time", "arena_alerts",
                "SELECT histogram(_timestamp, '1 hour') AS x_axis_1, \"check\" AS c, count(*) AS y_axis_1 \
                 FROM arena_alerts WHERE status = 'fail' GROUP BY x_axis_1, c ORDER BY x_axis_1")
                .about("Failing runs per hour and check (twelve runs an hour: 12 is failing all hour).")
                .axes("Hour", "Failing runs").by("Check", "c").size(COLS / 2, 10),
        ],
    )
}

fn balance() -> Value {
    let l = "arena_ladder";
    let row_cols: &[(&str, &str)] = &[
        ("Id", "id"),
        ("Tier", "tier"),
        ("Win %", "win"),
        ("Games", "games"),
        ("Picks", "picks"),
        ("Used", "used_rate"),
        ("Damage a game", "damage_per_game"),
        ("Casts", "casts"),
        ("Hits", "hits"),
    ];
    let cols = "id, tier, win, games, picks, used_rate, damage_per_game, casts, hits";
    dashboard(
        "Arena: balance",
        "The latest balance run (gems, supports, keystones: arena-sim's balance example) and ladder run (arena-bot's \
         ladder example), from arena_ladder.",
        "7d",
        vec![
            panel("h-bar", "Keystones: win rate (latest balance run)", l,
                latest("balance.row", "id AS x_axis_1, win AS y_axis_1", " AND kind = 'keystone'", "y_axis_1 DESC"))
                .axes("Keystone", "Win %").size(COLS / 2, 12),
            panel("h-bar", "Damage per game per skill (top 25, latest balance run)", l,
                latest("balance.row", "id AS x_axis_1, damage_per_game AS y_axis_1", " AND kind = 'gem'",
                    "y_axis_1 DESC LIMIT 25"))
                .axes("Skill", "Damage a game").size(COLS / 2, 12),
            panel("table", "Gems (latest balance run)", l,
                latest("balance.row", cols, " AND kind = 'gem'", "win DESC"))
                .columns(row_cols).size(COLS / 2, 14),
            panel("table", "Supports (latest balance run)", l,
                latest("balance.row", cols, " AND kind = 'support'", "win DESC"))
                .columns(row_cols).size(COLS / 2, 14),
            panel("table", "Ladder rows: seat win rates (latest ladder run)", l,
                latest("ladder.row", "row, win, seat0, seat1, round_seconds, timeouts, top, attack_share", "", "row"))
                .about("win: the row's first bot's share of the rounds; seat0 / seat1: its share from each seat.")
                .columns(&[
                    ("Row", "row"), ("Win", "win"), ("As seat 0", "seat0"), ("As seat 1", "seat1"),
                    ("Round s", "round_seconds"), ("Timeouts", "timeouts"), ("Top act", "top"),
                    ("Attack share", "attack_share"),
                ])
                .size(COLS / 2, 10),
            panel("bar", "Sim matches: wins by seat", l,
                "SELECT seat AS x_axis_1, avg(CASE WHEN won THEN 1.0 ELSE 0.0 END) AS y_axis_1 FROM arena_ladder \
                 WHERE body = 'sim.match' GROUP BY x_axis_1 ORDER BY x_axis_1")
                .about("arena-sim's duel example, over the range.")
                .axes("Seat", "Won").unit("percent-1").size(COLS / 2, 10),
        ],
    )
}

fn bots() -> Value {
    let b = "arena_bot";
    let think = "arena_bot_think_seconds";
    dashboard(
        "Arena: bots",
        "Bot modules in the host (wasmtime): think time, fuel, faults and forfeits.",
        "7d",
        vec![
            panel("metric", "Think time p50", "arena_bot_think_seconds_bucket", quantile(think, 0.5))
                .about("The bucket bound it falls under.")
                .metrics().axes("", "p50").unit("seconds").size(COLS / 4, 6),
            panel("metric", "Think time p95", "arena_bot_think_seconds_bucket", quantile(think, 0.95))
                .about("The bucket bound it falls under.")
                .metrics().axes("", "p95").unit("seconds").size(COLS / 4, 6),
            panel("metric", "Mean think (ms)", b,
                "SELECT avg(think_mean_ms) AS y_axis_1 FROM arena_bot WHERE body = 'bot.end'")
                .about("Over the bot instances that ended (bot.end).")
                .axes("", "ms").size(COLS / 4, 6),
            panel("metric", "Decisions", b,
                "SELECT sum(decisions) AS y_axis_1 FROM arena_bot WHERE body = 'bot.end'")
                .axes("", "Decisions").size(COLS / 4, 6),
            panel("bar", "Think time", "arena_bot_think_seconds_bucket", buckets("arena_bot_think_seconds_bucket", ""))
                .about("Decisions that took up to each bound, seconds.")
                .metrics().axes("Seconds", "Decisions"),
            panel("bar", "Fuel a decision", "arena_bot_fuel_bucket", buckets("arena_bot_fuel_bucket", ""))
                .about("Decisions that burned up to each bound of fuel (the cap is 2e7).")
                .metrics().axes("Fuel", "Decisions"),
            panel("line", "Fuel per instance: mean and most", b,
                "SELECT histogram(_timestamp) AS x_axis_1, avg(fuel_mean) AS mean, max(fuel_most) AS most, \
                 max(fuel_cap) AS cap FROM arena_bot WHERE body = 'bot.end' GROUP BY x_axis_1 ORDER BY x_axis_1")
                .ys(&[("Mean", "mean"), ("Most", "most"), ("Cap", "cap")]),
            panel("pie", "Faults by kind", "arena_bot_faults_total", format!(
                "SELECT kind AS x_axis_1, sum(value) AS y_axis_1 FROM arena_bot_faults_total WHERE {DELTA} \
                 GROUP BY x_axis_1"))
                .about("trap: the module failed; out_of_fuel: it thought too long.")
                .metrics().axes("Kind", "Faults").size(COLS / 4, 10),
            panel("bar", "Faults by call", "arena_bot_faults_total", format!(
                "SELECT call AS x_axis_1, kind AS k, sum(value) AS y_axis_1 FROM arena_bot_faults_total WHERE {DELTA} \
                 GROUP BY x_axis_1, k"))
                .metrics().axes("Call", "Faults").by("Kind", "k").size(COLS / 4, 10),
            panel("table", "Forfeits", "arena_match",
                "SELECT to_timestamp_micros(_timestamp) AS t, bot, seat, match_id, round, service_name FROM arena_match \
                 WHERE body = 'forfeit' ORDER BY _timestamp DESC LIMIT 50")
                .about("A seat that gave up: its bot faulted out.")
                .columns(&[("Time", "t"), ("Bot", "bot"), ("Seat", "seat"), ("Match", "match_id"), ("Round", "round"),
                    ("Service", "service_name")])
                .size(COLS / 2, 10),
        ],
    )
}

/// Every dashboard, with the file it is kept in.
fn all() -> Vec<(&'static str, Value)> {
    vec![
        ("live-matches", live_matches()),
        ("game-events", game_events()),
        ("server-health", server_health()),
        ("balance", balance()),
        ("bots", bots()),
    ]
}

struct Store {
    base: String,
    auth: String,
}

impl Store {
    fn call(&self, method: &str, path: &str, body: Option<&Value>) -> Result<Value, String> {
        let url = format!("{}{path}", self.base);
        let res = match (method, body) {
            ("GET", _) => ureq::get(&url).header("Authorization", &self.auth).call(),
            ("POST", Some(b)) => ureq::post(&url)
                .header("Authorization", &self.auth)
                .send_json(b),
            ("PUT", Some(b)) => ureq::put(&url)
                .header("Authorization", &self.auth)
                .send_json(b),
            _ => return Err(format!("{method} {path}: no body")),
        };
        res.map_err(|e| format!("{method} {url}: {e}"))?
            .body_mut()
            .read_json()
            .map_err(|e| format!("{method} {url}: {e}"))
    }

    /// Run every panel's query of `d` over the last week; returns the panels that failed, with why.
    fn check(&self, d: &Value) -> Vec<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |t| t.as_micros() as i64);
        let mut failed = Vec::new();
        for p in d["tabs"][0]["panels"].as_array().into_iter().flatten() {
            let q = &p["queries"][0];
            let body = json!({ "query": {
                "sql": q["query"], "start_time": now - 7 * 86_400_000_000, "end_time": now, "size": 5,
            }});
            let kind = q["fields"]["stream_type"].as_str().unwrap_or("logs");
            let title = p["title"].as_str().unwrap_or("?");
            match self.call("POST", &format!("/_search?type={kind}"), Some(&body)) {
                Ok(r) => println!(
                    "  ok   {title}: {} rows",
                    r["hits"].as_array().map_or(0, Vec::len)
                ),
                Err(e) => failed.push(format!("{title}: {e}")),
            }
        }
        failed
    }

    /// Create `d`, or update the dashboard of the same title; returns its id.
    fn push(&self, d: &Value) -> Result<String, String> {
        let title = d["title"].as_str().unwrap_or_default();
        let list = self.call("GET", "/dashboards", None)?;
        let found = list["dashboards"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|x| x["title"].as_str() == Some(title));
        let mut d = d.clone();
        let saved = match found {
            Some(x) => {
                let id = x["dashboard_id"].as_str().unwrap_or_default().to_string();
                let hash = x["hash"].as_str().unwrap_or_default().to_string();
                d["dashboardId"] = json!(id);
                d["owner"] = x["owner"].clone();
                d["created"] = x["created"].clone();
                let folder = x["folder_id"].as_str().unwrap_or("default").to_string();
                self.call(
                    "PUT",
                    &format!("/dashboards/{id}?folder={folder}&hash={hash}"),
                    Some(&d),
                )?
            }
            None => self.call("POST", "/dashboards?folder=default", Some(&d))?,
        };
        saved["v5"]["dashboardId"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("{title}: the store answered {saved}"))
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let endpoint =
        Endpoint::from_env()?.ok_or("set ARENA_OTLP_ENDPOINT (and ARENA_OTLP_AUTH_FILE)")?;
    let store = Store {
        base: endpoint.url.trim_end_matches('/').to_string(),
        auth: endpoint.auth.clone().unwrap_or_default(),
    };
    // the UI's address for a dashboard (through the arena tunnel)
    let view = |id: &str| {
        format!(
            "https://arena-obs.aiocean.io/web/dashboards/view?org_identifier=default&dashboard={id}&folder=default"
        )
    };
    if let Some(dir) = flag("--from") {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map_err(|e| format!("{dir}: {e}"))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        for f in files {
            let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let d: Value =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?;
            let id = store.push(&d)?;
            println!("{:<28} {}", d["title"].as_str().unwrap_or("?"), view(&id));
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--check") {
        let mut failed = Vec::new();
        for (_, d) in all() {
            println!("{}", d["title"].as_str().unwrap_or("?"));
            failed.extend(store.check(&d));
        }
        if failed.is_empty() {
            return Ok(());
        }
        return Err(format!("failed:\n{}", failed.join("\n")));
    }
    if !args.iter().any(|a| a == "--push") {
        return Err(
            "--push (make them and export them) or --from DIR (push the exported ones)".into(),
        );
    }
    let out = PathBuf::from(flag("--out").unwrap_or_else(|| "ops/observe/dashboards".to_string()));
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    for (file, d) in all() {
        let id = store.push(&d)?;
        let exported = store.call("GET", &format!("/dashboards/{id}/export"), None)?;
        write(&out.join(format!("{file}.json")), &exported["v5"])?;
        println!("{:<28} {}", d["title"].as_str().unwrap_or("?"), view(&id));
    }
    Ok(())
}

fn write(path: &Path, v: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())? + "\n";
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

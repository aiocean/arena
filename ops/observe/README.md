# The arena's telemetry store

Telemetry is made at the source with `tracing` and exported by `crates/arena-telemetry` over OpenTelemetry (OTLP/HTTP,
protobuf) into one OpenObserve on the kiosk. The UI is https://arena-obs.aiocean.io (through the arena tunnel).

## Why OpenObserve

One container takes logs, metrics and traces over OTLP natively, stores them compressed on local disk, and serves an
SQL query UI with dashboards, alerts and its own login. Here it runs in about 400 MB of RAM (capped at 1 GB). Graylog
needs OpenSearch and MongoDB beside it (several GB); Grafana with Loki, Tempo and Prometheus is four services to run
and keep in step. One small service fits a kiosk whose disk and memory are tight.

## Run it

```
docker compose --env-file ~/projects/game-factory/ops/arena/observe.env -f ops/observe/compose.yaml up -d
docker compose -f ops/observe/compose.yaml ps
```

- Listens on 127.0.0.1 only: 5080 (UI, HTTP API, OTLP/HTTP at `/api/default/v1/{logs,metrics,traces}`), 5081 (OTLP
  gRPC). The outside reaches the UI only through the tunnel, behind OpenObserve's login.
- Credentials: `~/projects/game-factory/ops/arena/observe.env` (`ZO_ROOT_USER_EMAIL`, `ZO_ROOT_USER_PASSWORD`; mode
  600, never in the repo, never on a command line or in a log).
- Data: `~/services/arena/observe/data`. Kept 14 days (`ZO_COMPACT_DATA_RETENTION_DAYS` in `compose.yaml`; change it
  there and `up -d` again; a stream can also keep its own, in the UI under Streams, then the stream's settings).
- Disk: `du -sh ~/services/arena/observe/data`, or per stream:
  `cargo run -p arena-telemetry --example query -- --streams` (with the two variables below).

The owner logs in at https://arena-obs.aiocean.io with the root user's email and password from `observe.env`.

## Sending to it

A process calls `arena_telemetry::init("<service>", Options { .. })` once in `main` and keeps the guard. It always
prints to stderr (`RUST_LOG`); it exports only when the environment names the store:

| Variable | Value |
|---|---|
| `ARENA_OTLP_ENDPOINT` | `http://127.0.0.1:5080/api/default` |
| `ARENA_OTLP_AUTH_FILE` | `/Users/firegroup/projects/game-factory/ops/arena/observe.env` (or `ARENA_OTLP_AUTH`: a header value or `user:password`) |

Unset, nothing leaves the process, and the per-event targets (`arena::duel`, `arena::metrics`) are off at their call
sites, so the duel observer costs nothing. The guard flushes on drop; a process killed by a signal loses its last
batch (about a second of logs, ten seconds of metrics).

The tools take `--telemetry-otlp` (or the same variables): `cargo run --release -p arena-sim --example duel -- 7
--telemetry-otlp`, `--example balance -- --seeds 20 --pairs 200 --telemetry-otlp`, and `cargo run --release -p
arena-bot --features host,onnx --example ladder -- --seeds 150 --telemetry-otlp`, and the smoke test after a deploy,
`cargo run --release -p arena-smoke -- --review ~/services/arena/site/review/smoke --telemetry-otlp`. `cargo run --release -p
arena-telemetry --example observe` fights brain matches through the observer, as a live match is reported.

### The live server (io.aiocean.arena-live)

Add to the plist's `EnvironmentVariables`, then reload it; the password stays in the file. The plist also sets
`ProcessType` to `Interactive`: without it launchd throttles the job's timers and the ticks fall up to 200 ms behind
(`arena_server_late_seconds`):

```xml
<key>ARENA_OTLP_ENDPOINT</key><string>http://127.0.0.1:5080/api/default</string>
<key>ARENA_OTLP_AUTH_FILE</key><string>/Users/firegroup/projects/game-factory/ops/arena/observe.env</string>
<!-- beside EnvironmentVariables, not inside it -->
<key>ProcessType</key><string>Interactive</string>
```

A change to `ProcessType` needs the job reloaded (`launchctl bootout gui/$(id -u)/io.aiocean.arena-live`, then
`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.aiocean.arena-live.plist`); `kickstart -k` only restarts it.

## Backend and frontend are separate

Game events come only from the backend, where a match runs: arena-server, and the sim and ladder tools. The browser
never reports game events. **Browser telemetry is deferred**: it will come with the two-way relay path (the browser
publishing on MoQ) once players act live, as service `arena-web` in its own stream. The only links between the two
sides will be `match_id` and the viewer's session id.

## Services and streams

| service.name | What |
|---|---|
| `arena-server` | the live match server: its matches and its health |
| `arena-bot` | a bot as the arena runs it, a module in the host (wasmtime, `runtime` wasm) or an ONNX policy natively (tract, `runtime` onnx: the house AI in the ladder): think time, fuel, faults (filed under this name whichever process runs the bot) |
| `arena-ladder` | the sim, balance and ladder runs |
| `arena-watchdog` | the watchdog's check results, stream `arena_alerts` (Alerts, below) |
| `arena-smoke` | the smoke test of the live site after each deploy (`crates/arena-smoke`), stream `arena_smoke` |

Log streams (OpenObserve type `logs`). Every row has `_timestamp`, `body` (the event's name), `severity`,
`service_name`, `service_version`, `vcs_revision` (the commit), `host_name`, `trace_id` and `span_id` (the match's
trace), and the event's own fields:

| Stream | Events (`body`) and fields |
|---|---|
| `arena_match` | `match.start`, one per seat: `match_id`, `seed`, `seat`, `bot`, `foe`, `actives` (`skill[support+support],…`), `supports`, `keystones`, `root`, `life`, `qi` (the qi bar; null when `qi_uncapped`: Cổ Thần Quyết has no cap), `qi_uncapped`; from the live server also `runtime` (onnx for the house AI, builtin for brain, first, aggressive,
turtle) and `build` (the server's commit). `round.start`: `round`, `wins_0`, `wins_1`. `round.end`: `winner`, `winner_bot`, `seconds`, `life_0`, `life_1`. `forfeit`: `seat`, `bot`. `match.end`: `result` (seat0, seat1, draw, cut, abandoned, server_stopped, connection_closed), `winner_bot`, `cut`, `rounds`, `wins_0`, `wins_1`, `seconds` (fight time), `ticks`, `wall_seconds`, `bot_0`, `bot_1` |
| `arena_duel_events` | each with `match_id`, `round`. `cast`: `who`, `bot`, `skill`, `skill_name`, `tags`, `kinds`, `basic`, `qi`. `hit`: `by`, `bot`, `skill`, `skill_name`, `tags`, `kinds`, `damage`, `share` (of full life), `how` (plain, counter, punish), `crit`, `launch`, `ranged`, `power`, `ailments` (laid this step), `foe_life`. `stopped`: `by`, `skill`, `how` (guard, barrier, ward, dodge, clash), `chip`. `broken`: `who`, `skill`. `dot` (damage over time, shown every half second (`DOT_SHOWN`), or a curse's strike): `by`, `damage`, `share`, `cause` (burn, bleed, burn+bleed, curse). `heal`, `ward`: `by`, `amount`. `barrier`, `unyielding`: `by`. `ko`: `winner`, `life_0`, `life_1`, `seconds`. `status`: `who`, `status` (guard, meditate, fly, sealed, rooted, no_guard, silenced, barrier, down), `on` (after holding a quarter second). `ailment`: `who`, `ailment`, `stacks`, `before`. `curse`: `who`, `curse`, `on`. `unit.spawn`: `unit`, `by`, `kind`, `skill`, `x`; `unit.act`, `unit.hurt`, `unit.die`. `snapshot` (each fighter, each second of fight time): `who`, `bot`, `time`, `life`, `life_max`, `shield`, `sense`, `qi`, `x`, `y`, `ailments` (`burn:2,soak:1`), `curses`, `units`, `act` |
| `arena_server` | the server's own: `new match`, `match done` (`ticks`, `groups`, `frames`, `wall_seconds`, `missed`, `resyncs`), `match stopped` (`why`: server_stopped, connection_closed), `stopping` (`signal`), `tick schedule resynced` (`late_ms`), `relay subscription` (`watched`: the relay took or dropped the track; the server sees the relay, not each viewer), `server stopped` (`error`), `panic` (`reason`, `location`), moq warnings |
| `arena_bot` | each with `runtime` (wasm, onnx) and `bot` (`house`, `onnx-<hash>`, `module-<hash>`, or the name the tool gives it: the ladder's `--bot` is `<file>-<hash>`). `bot.fault`: `call` (start, round, decide), `kind` (trap, out_of_fuel; error for ONNX), `fault`, `faults` (the first ten of an instance, then every thousandth). `bot.broken`: a module that will not instantiate. `bot.end` (an instance's totals, when it is dropped: one match in the tournament, one row in the ladder): `decisions`, `matches` (fights started on it), `faults`, `think_mean_ms`, `think_p50_ms`, `think_p95_ms`, `think_p99_ms`, `think_max_ms`, `fuel_mean`, `fuel_p50`, `fuel_p95`, `fuel_max`, `fuel_most` (= `fuel_max`, kept for older queries), `fuel_cap`; the fuel fields are null for ONNX. The percentiles come from a fixed log-linear histogram in the host (32 buckets to each power of two, `crates/arena-bot/src/meter.rs`): within 1/64 of the true sample, the max exact. Rows before 2026-11-18 have no `runtime` and only the means |
| `arena_smoke` | `smoke.run`, one a run of the smoke test: `run_id`, `at` (the run's UTC stamp, its review page's name), `url`, `result` (pass, fail; error: the browser could not run), `check_page`, `check_errors`, `check_live_json`, `check_live`, `check_hud`, `check_advances` (each PASS, FAIL or none), `failed` (the failed checks' names), `game_start_seconds` (navigation to the game's status line; absent when it did not start), `first_frame_seconds` (to `window.__arena.live.frames` > 0; absent when none came), `load_1`, `load_5`, `load_15` (the machine's load averages when the attempt began), `attempt` (1, or 2 when retried), `retried`, `retry_reason` (empty unless retried: only a game that did not start in the fixed 30 s is), `deployed_commit` (the page's build stamp, `window.__arena.build.commit`; `none` before it had one), `deployed_dirty`, `server_build` (the `build` on the server's newest `match.start`; `unknown` without the store), `page_rules`, `server_rules`, `check_build`, `build_warning` (empty unless the page and the server are built from different commits on one rules build, or a build is not known), `errors`, `seconds` (the whole run), `mbps` (the connection `--mbps` emulated; 0: the machine's own, whose timings follow its load), `status_line`. The fields are the final attempt's |
| `arena_ladder` | each with `run_id` or `match_id`. `ladder.row`: `row`, `win`, `seat0`, `seat1`, `round_seconds`, `timeouts`, `top`, `attack_share`, `distance`. `ladder.skill`: `row`, `seat`, `skill`, `casts`, `landed`, `damage`, `damage_share`, `guarded`, `chip`, `barrier`, `ward`, `dodged`, `clashed`, `broken`, `punished`, `finals`. `ladder.seat`. `balance.run`; `balance.row`: `kind` (gem, support, keystone), `id`, `tier`, `picks`, `games`, `win`, `used_rate`, `damage_per_game`, `casts`, `hits`. `sim.match` (by seat: loadout, `won`, `damage`), `sim.round`, `sim.skill` |

Traces (type `traces`, streams `arena_server`, `arena_ladder`): a `match` span (`match_id`, `seed`) with a `round`
span for each round; the events above carry its `trace_id`.

Metrics (type `metrics`, one stream each; a histogram also has `_bucket`, `_count`, `_sum`, `_min`, `_max`):
`arena_matches_total`, `arena_rounds_total`, `arena_casts_total` (`skill`), `arena_damage_total` (`skill`, `kind`),
`arena_step_seconds` (a duel step's wall time), `arena_server_publish_seconds` (a tick stepped and written to the
track), `arena_server_late_seconds` (how far a tick runs behind the wall clock), `arena_server_missed_ticks_total` (ticks begun a whole tick or more late, caught up back to back), `arena_server_resyncs_total` (times the server fell more than 250 ms behind and restarted its tick schedule from now), `arena_bot_think_seconds`,
`arena_bot_fuel` (both with `runtime` and `bot`; fuel for modules only), `arena_bot_faults_total` (`call`, `kind`, `runtime`, `bot`). The metric points are filed under the process's service (`arena-ladder` for the ladder), the `arena_bot` rows under `arena-bot`.
From the smoke test (service `arena-smoke`): `arena_smoke_game_start_seconds` and `arena_smoke_first_frame_seconds`
(buckets 0.5 s to 60 s) and `arena_smoke_runs_total` (`result`).

Metrics are sent as deltas (since 2026-11-18; `aggregation_temporality` = `AGGREGATION_TEMPORALITY_DELTA`): each point
is its ten seconds' own counts, so `sum(value)` over any range is what happened in it, and a histogram's `_max` is its
interval's. The few cumulative points from before are left out by the dashboards.

## Dashboards

Open https://arena-obs.aiocean.io, log in with the root user's email and password from
`~/projects/game-factory/ops/arena/observe.env` (never paste them anywhere), then Dashboards, folder `default`:

| Dashboard | What it shows |
|---|---|
| Arena: live matches | matches in progress, played, mean length and rounds; matches per hour, results (seat0, seat1, cut, abandoned), match length, rounds per match, results by pairing (the house AI against each mind: its wins, the mind's, draws and cuts, its win rate), the latest 50 matches |
| Arena: game events | casts per skill, damage per skill (dots by cause) and per kind, hits by how, blows stopped by how, ailments laid, units spawned / acted / hurt / killed, KOs per round, events per minute |
| Arena: server health | step time (buckets; mean and most per minute), publish time, how far behind the wall clock, errors and warnings per hour, relay subscription changes, panics, server starts, the latest errors; the watchdog's alerts (each check's latest result, failures over time) |
| Arena: balance | the latest balance run (keystones, gems, supports: win, damage a game) and ladder run (seat win rates), sim wins by seat |
| Arena: bots | think time p50, p95, p99 and most by runtime and bot (a table, and p95 and p99 over time), decisions per match, fuel p95 and most against the cap, fuel by module, think time p50 and p95 over all decisions from the metric's buckets, think and fuel buckets, faults by kind and call, forfeits |
| Arena: smoke | the smoke test's runs: how many, failed, retried, mean time to game start; time to game start and to the first live frame beside the machine's load averages, run by run; pass and fail per run; time-to-start buckets; the latest 50 runs with each check |

The live and game-event panels read only the live server's rows (`service_name = 'arena-server'`); the metric panels
only delta points. They are made in code, `crates/arena-telemetry/examples/dashboards.rs`, and what the store exports of
them is kept in `dashboards/`:

```
cargo run -p arena-telemetry --example dashboards -- --check     # every panel's SQL against the store
cargo run -p arena-telemetry --example dashboards -- --push      # create or update them; re-export into dashboards/
cargo run -p arena-telemetry --example dashboards -- --from ops/observe/dashboards   # restore them to a fresh store
```

(with `ARENA_OTLP_ENDPOINT` and `ARENA_OTLP_AUTH_FILE` as above). A dashboard edited in the UI is lost at the next
`--push`: change it in the example. A panel that names a field the stream has never had (`watched`, a panic's `reason`)
fails until one arrives, so those fields are left to Logs.

## Alerts

OpenObserve sends its own alerts only to a destination (email or a webhook), and none is set up. Until one is, a
watchdog on the kiosk does the checking: `crates/arena-telemetry/examples/watchdog.rs`, run every five minutes by the
launchd job `io.aiocean.arena-watchdog` (`ops/observe/io.aiocean.arena-watchdog.plist`). Each run:

- writes one row a check to the stream `arena_alerts` (`check`, `status` ok / fail / unknown, `ok`, `value`,
  `threshold`, `message`, `flipped`, `silenced`; `service_name` = `arena-watchdog`);
- appends one line to `~/Library/Logs/game-factory/arena-alerts.log` (the failing checks and why; the job's own
  stderr is `arena-watchdog.log` beside it);
- raises a macOS notification for each check that has just turned from ok to failing (best-effort; the last state of
  each is kept in `~/services/arena/observe/watchdog-state.json`, so a check failing for an hour notifies once).

It always exits 0. When the store is down only `telemetry` fails and the store's checks are `unknown` (they keep
their last state); nothing reaches `arena_alerts` then, so look in the log file. The windows count back from the run.

| Check | Fails when | Means | Where to look |
|---|---|---|---|
| `telemetry` | OpenObserve does not answer a query | the store is down, so every other check is blind | `docker compose -f ops/observe/compose.yaml ps`, the log file |
| `live_match_start` | no `match.start` from `arena-server` in 600 s | the live server is down or stuck between matches | `launchctl print gui/$(id -u)/io.aiocean.arena-live`, `~/Library/Logs/game-factory/arena-live.log` |
| `live_snapshot` | the newest live `snapshot` is older than 120 s | no match is being fought (one is written a second of fight) | as above; Arena: live matches |
| `server_errors` | over 20 ERROR / WARN rows in `arena_server` in 15 min (a deploy's restart makes about ten) | the server or the relay is in trouble | Arena: server health, Latest errors and warnings |
| `server_panics` | any `panic` row in `arena_server` in 15 min | the server panicked | Logs, stream `arena_server`, `body = 'panic'` (`reason`, `location`) |
| `missed_ticks` | over 60 `arena_server_missed_ticks_total` in 15 min | ticks run a whole tick late and are caught up back to back | Arena: server health, Behind the wall clock |
| `tick_resyncs` | any `arena_server_resyncs_total` in 15 min | the server fell over 250 ms behind and restarted its tick schedule | as above; the plist's `ProcessType` (above) |
| `tick_late` | `arena_server_late_seconds_max` over 0.1 s in 15 min | a tick landed over 100 ms after its time | as above |
| `disk_data` | `~/services/arena/observe/data` over 5 GB on disk | the store outgrows the kiosk | `du -sh`, `--streams` (above); lower the retention |
| `disk_free` | under 10 GB free on its volume | the kiosk's disk is filling | `df -h ~` |

A metric stream that does not exist yet (the server has not sent that counter) passes as "no data". The thresholds
are constants at the top of `watchdog.rs`, tested there (`cargo test -p arena-telemetry`).

The server-health dashboard shows them: Alerts: each check's latest result (failing first), and Alerts: failures over
time (failing runs per hour and check; 12 is a whole hour). In SQL:

```sql
SELECT "check", status, value, message FROM arena_alerts WHERE status = 'fail' ORDER BY _timestamp DESC
```

**Silence** a check's notifications: put its name on a line of `~/services/arena/observe/watchdog-silence` (`all`
for every check, `#` for a comment); it is still checked and written, with `silenced` true. Delete the line to
unsilence. Stop the watchdog altogether with `launchctl bootout gui/$(id -u)/io.aiocean.arena-watchdog`.

**Install or update** it:

```
cargo build --release -p arena-telemetry --example watchdog
install -m 755 target/release/examples/watchdog ~/services/arena/observe/watchdog
cp ops/observe/io.aiocean.arena-watchdog.plist ~/Library/LaunchAgents/
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.aiocean.arena-watchdog.plist   # bootout first if loaded
launchctl kickstart gui/$(id -u)/io.aiocean.arena-watchdog                                 # a run now
```

`watchdog --dry` prints the checks and writes nothing. `ARENA_WATCHDOG_DATA`, `_STATE`, `_SILENCE` and `_LOG` move its
files (tests and trials).

**Once a destination exists**, add in OpenObserve (Alerts, with the destination) the two that should reach the owner
away from the kiosk, and keep the watchdog for the rest:

- errors in `arena_server` above zero in an hour: `SELECT count(*) AS n FROM arena_server WHERE severity = 'ERROR'`,
  every 60 minutes over 60 minutes, fire when `n > 0`;
- no `match.start` from `arena-server` in ten minutes: `SELECT count(*) AS n FROM arena_match WHERE body =
  'match.start' AND service_name = 'arena-server'`, every 5 minutes over 10 minutes, fire when `n < 1`.

A webhook destination could also take the watchdog's flips (a scheduled alert on `arena_alerts WHERE flipped`).

## Queries

In the UI (Logs, pick the stream, SQL mode) or `cargo run -p arena-telemetry --example query -- "<sql>" [--hours 24]
[--type logs|traces|metrics]`:

```sql
-- live matches: started in the last ten minutes and not ended
SELECT match_id, min(_timestamp) AS started FROM arena_match WHERE body = 'match.start'
  AND service_name = 'arena-server' AND match_id NOT IN (SELECT match_id FROM arena_match WHERE body = 'match.end')
  GROUP BY match_id ORDER BY started DESC
-- errors per service and hour
SELECT service_name, histogram(_timestamp, '1 hour') AS hour, count(*) AS n FROM arena_server
  WHERE severity IN ('ERROR', 'WARN') GROUP BY service_name, hour ORDER BY hour
-- win rate per seat
SELECT result, count(*) AS n FROM arena_match WHERE body = 'match.end' GROUP BY result
-- win rate per gem (balance runs)
SELECT id, win, games, damage_per_game FROM arena_ladder WHERE body = 'balance.row' AND kind = 'gem' ORDER BY win DESC
-- skill use and damage in real fights
SELECT skill, count(*) AS hits, sum(damage) AS damage, avg(damage) AS per_hit FROM arena_duel_events
  WHERE body = 'hit' GROUP BY skill ORDER BY damage DESC
SELECT skill, count(*) AS casts FROM arena_duel_events WHERE body = 'cast' GROUP BY skill ORDER BY casts DESC
-- bot faults and timeouts (out of fuel)
SELECT kind, call, count(*) AS n FROM arena_bot WHERE body = 'bot.fault' GROUP BY kind, call
SELECT runtime, bot, sum(decisions) AS decisions, sum(think_p95_ms * decisions) / sum(decisions) AS p95_ms,
  max(think_max_ms) AS most_ms, max(fuel_max) AS fuel_most FROM arena_bot WHERE body = 'bot.end' AND runtime IS NOT NULL
  GROUP BY runtime, bot
-- the smoke test's runs: timings beside the machine's load
SELECT at, result, attempt, retry_reason, game_start_seconds, first_frame_seconds, load_1, load_5, load_15
  FROM arena_smoke WHERE body = 'smoke.run' ORDER BY _timestamp DESC
-- the server falling behind its clock (metrics)
SELECT max(value) AS worst FROM arena_server_late_seconds_max
```

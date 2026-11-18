# The arena's telemetry

Telemetry is made at the source with `tracing` and written by `crates/arena-telemetry` as JSON lines into local files,
for agents debugging the arena. There is no store, no collector, no network, no dashboard: the files and a query tool.

## Where it goes

A process calls `arena_telemetry::init("<service>", Options { .. })` once in `main` and keeps the guard. It always
prints to stderr (`RUST_LOG`); it writes files only when given a directory:

| Who | How it is told |
|---|---|
| any process | `ARENA_TELEMETRY_DIR=<dir>` in its environment |
| the tools (ladder, balance, duel, cap) | `--telemetry-dir DIR`, or the variable |
| the smoke test | `--telemetry` (the variable's dir, else `~/services/arena/telemetry`), or the variable |
| the watchdog | always: the variable's dir, else `~/services/arena/telemetry` |

On the kiosk the directory is `~/services/arena/telemetry`. Unset, nothing is written, and the per-event targets
(`arena::duel`, `arena::metrics`) are off at their call sites, so the duel observer costs nothing.

- **Files**: `<dir>/<service>/<YYYY-MM-DD>.jsonl`, a file a service and UTC day (rotation is the name). Two processes
  of one service append to one file, each a batch of whole lines at a time.
- **Off the hot path**: a record is built where the event is raised and handed over a bounded channel (65 536
  records) to a writer thread, which appends once a second. A full channel drops the record and counts it, never
  waits: the count is `Guard::dropped`, a `telemetry.dropped` warning line (`dropped`, `dropped_total`) and a line on
  stderr at exit. The guard's drop writes what is left; a process killed by a signal loses its last second (and its
  metrics' last ten).
- **Retention**: at each start and hourly, a process deletes the files of days older than 14 (today counted), then
  the oldest days until all the files take under 2 GB (`arena_telemetry::Retention`).

| Service | What |
|---|---|
| `arena-server` | the live match server: its matches (`arena::match`, `arena::duel`), its spans and metrics, its health |
| `arena-bot` | a bot as the arena runs it (target `arena::bot`, whichever process runs it): `bot.end`, `bot.fault`, `bot.broken` |
| `arena-ladder` | the sim, balance, cap and ladder runs (`arena::ladder`), and their matches through the observer |
| `arena-smoke` | the smoke test of the live site after each deploy: `smoke.run` |
| `arena-watchdog` | the watchdog's checks: `watchdog.check` |

## A line

One line a log record, a finished span, or a metric series over an interval. The keys come in this order, `ts` first
(always 27 characters, so a reader finds a line's time without parsing it); the event's own fields are under
`attrs`, never at the top, so an event's `kind` or `name` never meets the line's:

```json
{"ts":"2026-11-18T10:00:00.123456Z","kind":"log","service":"arena-server","name":"match.start","level":"INFO","target":"arena::match","trace_id":"…32 hex…","span_id":"…16 hex…","attrs":{"match_id":"live-1795…","seat":0,"bot":"house","runtime":"onnx","build":"8756445a1b2c"}}
{"ts":"…","kind":"span","service":"arena-server","name":"round","level":"INFO","target":"arena::match","trace_id":"…","span_id":"…","parent_id":"…","start":"…","end":"…","duration_ms":41250.3,"attrs":{"round":1}}
{"ts":"…","kind":"metric","service":"arena-server","name":"arena_casts_total","type":"counter","interval_s":10.0,"value":3,"attrs":{"skill":"lotus"}}
{"ts":"…","kind":"metric","service":"arena-server","name":"arena_step_seconds","type":"histogram","interval_s":10.0,"count":600,"sum":0.031,"min":0.00002,"max":0.0004,"zero":0,"buckets":[[-125,3],[-124,40]],"attrs":{}}
```

| Key | What |
|---|---|
| `ts` | UTC, microseconds: a log's time, a span's end, a metric interval's end |
| `kind` | `log`, `span` or `metric` |
| `service` | the file it is in |
| `name` | a log's message (`match.start`, `hit`, `tick schedule resynced`), a span's name, a metric's name |
| `level` | `ERROR`, `WARN`, `INFO` (logs and spans) |
| `target` | the event's target: `arena::match`, `arena::duel`, `arena::bot`, `arena::ladder`, else the module (`arena_server`) |
| `trace_id`, `span_id` | the span a log was raised in (a match's `match` span and its `round`s); a span's own, and its `parent_id` |
| `start`, `end`, `duration_ms` | a span's |
| `type`, `interval_s` | a metric's: `counter` (and `value`, the interval's sum) or `histogram` (`count`, `sum`, `min`, `max`, `zero`: values of 0 or less, `buckets`) |
| `attrs` | the event's, span's or metric's own fields: strings, numbers, booleans; NaN and infinities are `null` |

A histogram's buckets are `[i, n]`: `n` values in `(2^(i/8), 2^((i+1)/8)]`, eight to each doubling, so a percentile
read from them is within about 4.5% (`arena_telemetry::hist`); the query tool merges them across lines. The first line
a process writes is `telemetry.start` (`version`, `git`: the commit, `host`, `pid`).

### The events

Each with `match_id` and `round`, inside a `match` span and a `round` span (`crates/arena-telemetry/src/duel.rs`):

| Target | Events (`name`) and their `attrs` |
|---|---|
| `arena::match` | `match.start`, one per seat: `match_id`, `seed`, `seat`, `bot`, `foe`, `actives` (`skill[support+support],…`), `supports`, `keystones`, `root`, `life`, `qi` (null when `qi_uncapped`: Cổ Thần Quyết has no cap), `qi_uncapped`; from the live server also `runtime` (onnx for the house AI, builtin for brain, first, aggressive, turtle) and `build` (the server's commit). `round.start`: `round`, `wins_0`, `wins_1`. `round.end`: `winner`, `winner_bot`, `seconds`, `life_0`, `life_1`. `forfeit` (WARN): `seat`, `bot`. `match.end`: `result` (seat0, seat1, draw, cut, abandoned, server_stopped, connection_closed), `winner_bot`, `cut`, `rounds`, `wins_0`, `wins_1`, `seconds` (fight time), `ticks`, `wall_seconds`, `bot_0`, `bot_1` |
| `arena::duel` | `cast`: `who`, `bot`, `skill`, `skill_name`, `tags`, `kinds`, `basic`, `qi`. `hit`: `by`, `bot`, `skill`, `skill_name`, `tags`, `kinds`, `damage`, `share` (of full life), `how` (plain, counter, punish), `crit`, `launch`, `ranged`, `power`, `ailments` (laid this step), `foe_life`. `stopped`: `by`, `skill`, `how` (guard, barrier, ward, dodge, clash), `chip`. `broken`: `who`, `skill`. `dot` (shown every half second, or a curse's strike): `by`, `damage`, `share`, `cause` (burn, bleed, burn+bleed, curse). `heal`, `ward`: `by`, `amount`. `barrier`, `unyielding`: `by`. `ko`: `winner`, `life_0`, `life_1`, `seconds`. `status`: `who`, `status` (guard, meditate, fly, sealed, rooted, no_guard, silenced, barrier, down), `on`. `ailment`: `who`, `ailment`, `stacks`, `before`. `curse`: `who`, `curse`, `on`. `unit.spawn`: `unit`, `by`, `kind`, `skill`, `x`; `unit.act`, `unit.hurt`, `unit.die`. `snapshot` (each fighter, each second of fight time): `who`, `bot`, `time`, `life`, `life_max`, `shield`, `sense`, `qi`, `x`, `y`, `ailments` (`burn:2,soak:1`), `curses`, `units`, `act` |
| `arena_server` (the server's own) | `arena-server starting` (`build`, `rules`), `new match`, `match done` (`ticks`, `groups`, `frames`, `wall_seconds`, `missed`, `resyncs`), `match stopped` (`why`), `stopping` (`signal`), `tick schedule resynced` (WARN, `late_ms`), `relay subscription` (`watched`), `server stopped` (ERROR, `error`), `panic` (ERROR, `reason`, `location`), moq warnings |
| `arena::bot` (service `arena-bot`) | each with `runtime` (wasm, onnx) and `bot` (`house`, `onnx-<hash>`, `module-<hash>`, or the ladder's `<file>-<hash>`). `bot.fault`: `call` (start, round, decide), `kind` (trap, out_of_fuel; error for ONNX), `fault`, `faults` (the first ten of an instance, then every thousandth). `bot.broken`: a module that will not instantiate. `bot.end` (an instance's totals, when it is dropped): `decisions`, `matches`, `faults`, `think_mean_ms`, `think_p50_ms`, `think_p95_ms`, `think_p99_ms`, `think_max_ms`, `fuel_mean`, `fuel_p50`, `fuel_p95`, `fuel_max`, `fuel_most`, `fuel_cap` (the fuel fields null for ONNX; the percentiles from the host's own log-linear histogram, `crates/arena-bot/src/meter.rs`) |
| `arena::ladder` | each with `run_id` or `match_id`. `ladder.row`: `row`, `win`, `seat0`, `seat1`, `round_seconds`, `timeouts`, `top`, `attack_share`, `distance`. `ladder.skill`: `row`, `seat`, `skill`, `casts`, `landed`, `damage`, `damage_share`, `guarded`, `chip`, `barrier`, `ward`, `dodged`, `clashed`, `broken`, `punished`, `finals`. `ladder.seat`. `balance.run`; `balance.row`: `kind` (gem, support, keystone), `id`, `tier`, `picks`, `games`, `win`, `used_rate`, `damage_per_game`, `casts`, `hits`. `balance.cap`. `sim.match`, `sim.round`, `sim.skill` |
| `arena_smoke` (service `arena-smoke`) | `smoke.run`, one a run: `run_id`, `at` (its review page's name), `url`, `result` (pass, fail, error), `check_page`, `check_errors`, `check_live_json`, `check_live`, `check_hud`, `check_boxes`, `check_advances`, `check_build`, `check_module` (each PASS, FAIL or none), `failed`, `game_start_seconds`, `first_frame_seconds`, `load_1`, `load_5`, `load_15`, `attempt`, `retried`, `retry_reason`, `deployed_commit`, `deployed_dirty`, `server_build` (`unknown` without the server's files), `page_rules`, `server_rules`, `build_warning`, `module_hash`, `module_warning`, `errors`, `seconds`, `mbps`, `status_line` |

Metrics (`kind` metric), filed under the process's service: `arena_matches_total`, `arena_rounds_total`,
`arena_casts_total` (`skill`), `arena_damage_total` (`skill`, `kind`), `arena_step_seconds` (a duel step's wall
time), `arena_server_publish_seconds` (a tick stepped and written to the track), `arena_server_late_seconds` (how far
a tick runs behind the wall clock), `arena_server_missed_ticks_total`, `arena_server_resyncs_total` (more than 250 ms
behind: the tick schedule restarted from now), `arena_bot_think_seconds`, `arena_bot_fuel` (both with `runtime` and
`bot`; fuel for modules only), `arena_bot_faults_total` (`call`, `kind`, `runtime`, `bot`); from the smoke test
`arena_smoke_game_start_seconds`, `arena_smoke_first_frame_seconds`, `arena_smoke_runs_total` (`result`).

## Queries

`cargo run -p arena-telemetry --example query -- …` (`--release` for a big range). It streams the files in time
order across services (one line of each file held at a time), and finds the start of a range by a binary search in
the first day's file, so the last hour of a big day costs what an hour does.

| Flag | What |
|---|---|
| `--dir DIR` | else `ARENA_TELEMETRY_DIR`, else `~/services/arena/telemetry` |
| `--service S[,S]` | those services' files only |
| `--since 2h` / `--from T` / `--to T` | a range: `90s`, `15m`, `2h`, `1d`, `2w`; times in UTC, `2026-11-18` or `2026-11-18T10:05` |
| `--kind log\|span\|metric` | |
| `--name N[,N]` | the line's `name`, `*` a wildcard (`match.*`) |
| `--match ID` | a match's lines (`match_id`) or a run's (`run_id`) |
| `--where K=V` | several; `K=a\|b` either, `*` a wildcard, `K!=V`, `K>N`, `K<N`, `K>=N`, `K<=N`. `K` is an attribute, else the line's own field (`level`, `service`, `type`, `duration_ms`); `attrs.K` and `.K` say which |
| `--fields K,K` | print only those (and the time, service, kind and name) |
| `--json` | the lines as they are |
| `--count` | how many |
| `--group-by K[,K]` | a count a group, largest first |
| `--stats FIELD` | count, sum, mean, p50, p95, p99, max of a number (per group with `--group-by`); `value` over histogram lines merges their buckets: the percentiles of every sample |
| `--limit N` / `--last N` | the first or the last `N` lines |
| `--files` | each file and its size, and the total |

The common questions:

```sh
q() { cargo run -q --release -p arena-telemetry --example query -- "$@"; }

# what happened in match X: every event, in order (spans and the bot's lines included)
q --match live-1795512345678-3
q --match live-1795512345678-3 --kind log --name 'match.*,round.*,ko,forfeit'      # only its outline

# live matches started in the last hour, and their results
q --service arena-server --since 1h --name match.start --fields match_id,seat,bot,runtime,build
q --service arena-server --since 1h --name match.end --fields match_id,result,winner_bot,rounds,seconds,bot_0,bot_1
q --service arena-server --since 1h --name match.end --group-by result

# win rate by pairing: the house AI against each mind (wins counted by winner_bot; draws and cuts have none)
q --service arena-server --since 1d --name match.end --group-by bot_0,bot_1,winner_bot
q --service arena-server --since 1d --name match.end --group-by bot_0,bot_1,result

# tick lateness and step time percentiles
q --service arena-server --since 1h --name arena_server_late_seconds --stats value
q --service arena-server --since 1h --name arena_step_seconds,arena_server_publish_seconds --stats value --group-by name
q --service arena-server --since 1d --name arena_server_missed_ticks_total,arena_server_resyncs_total --stats value --group-by name

# think time p50/p95/p99 by runtime and bot (every decision, from the metric's buckets); and each instance's own
q --since 1d --name arena_bot_think_seconds --stats value --group-by runtime,bot
q --service arena-bot --since 1d --name bot.end --fields runtime,bot,decisions,think_p50_ms,think_p95_ms,think_p99_ms,think_max_ms,fuel_max

# bot faults by kind (and call)
q --service arena-bot --since 1d --name bot.fault --group-by kind,call
q --since 1d --name arena_bot_faults_total --stats value --group-by kind,call

# smoke timings over time, and smoke results
q --service arena-smoke --name smoke.run --fields at,result,attempt,game_start_seconds,first_frame_seconds,load_1,mbps
q --service arena-smoke --name smoke.run --last 10 --fields at,result,failed,retry_reason,deployed_commit,server_build,build_warning
q --service arena-smoke --since 2w --name smoke.run --group-by result

# errors and warnings in the last day (the server's own; drop --service for every process)
q --service arena-server --since 1d --where 'level=ERROR|WARN' --group-by level,name
q --since 1d --where 'level=ERROR|WARN' --last 30

# the watchdog's last results (ten checks a run), and its failures
q --service arena-watchdog --name watchdog.check --last 10 --fields check,status,value,detail
q --service arena-watchdog --since 1d --name watchdog.check --where status=fail --group-by check

# also: skill use and damage in real fights, a balance run's rows, the disk
q --service arena-server --since 1d --name hit --stats damage --group-by skill
q --service arena-ladder --name balance.row --where kind=gem --fields id,tier,win,games,damage_per_game
q --files
```

A line is plain JSON, so `rg` on the files works too: `rg -N '"match_id":"live-1795…"' ~/services/arena/telemetry/arena-server/2026-11-18.jsonl`.

### The live server (io.aiocean.arena-live)

In the plist's `EnvironmentVariables`, then reload it. The plist also sets `ProcessType` to `Interactive`: without
it launchd throttles the job's timers and the ticks fall up to 200 ms behind (`arena_server_late_seconds`):

```xml
<key>ARENA_TELEMETRY_DIR</key><string>/Users/firegroup/services/arena/telemetry</string>
<!-- beside EnvironmentVariables, not inside it -->
<key>ProcessType</key><string>Interactive</string>
```

A change to `ProcessType` needs the job reloaded (`launchctl bootout gui/$(id -u)/io.aiocean.arena-live`, then
`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.aiocean.arena-live.plist`); `kickstart -k` only restarts it.

The tools: `cargo run --release -p arena-sim --example duel -- 7 --telemetry-dir DIR`, `--example balance -- --seeds
20 --pairs 200 --telemetry-dir DIR`, `cargo run --release -p arena-bot --features host,onnx --example ladder --
--seeds 150 --telemetry-dir DIR`, and the smoke test after a deploy, `cargo run --release -p arena-smoke -- --review
~/services/arena/site/review/smoke --telemetry`. `cargo run --release -p arena-telemetry --example observe` fights
brain matches through the observer, as a live match is reported; `ARENA_TELEMETRY_DIR=… cargo run --release -p
arena-server --example tick_bench -- steps --matches 3 --observe` writes live matches' telemetry without a relay.

## Backend and frontend are separate

Game events come only from the backend, where a match runs: arena-server, and the sim and ladder tools. The browser
never reports game events. **Browser telemetry is deferred**: it will come with the two-way relay path (the browser
publishing on MoQ) once players act live, as service `arena-web`. The only links between the two sides will be
`match_id` and the viewer's session id.

## The watchdog

`crates/arena-telemetry/examples/watchdog.rs`, run every five minutes by the launchd job `io.aiocean.arena-watchdog`
(`ops/telemetry/io.aiocean.arena-watchdog.plist`). It reads the live server's files (the last fifteen minutes of
`<dir>/arena-server/`). Each run:

- writes one `watchdog.check` line a check to `<dir>/arena-watchdog/<day>.jsonl` (`check`, `status` ok / fail /
  unknown, `ok`, `value`, `threshold`, `detail`: what was measured, `flipped`, `silenced`);
- appends one line to `~/Library/Logs/game-factory/arena-alerts.log` (the failing checks and why; the job's own
  stderr is `arena-watchdog.log` beside it);
- raises a macOS notification for each check that has just turned from ok to failing (best-effort; the last state of
  each is kept in `~/services/arena/watchdog/watchdog-state.json`, so a check failing for an hour notifies once).

It always exits 0. The windows count back from the run.

| Check | Fails when | Means | Where to look |
|---|---|---|---|
| `telemetry` | the server's newest file was last written over 600 s ago (or it has none) | the server is down or writes nothing (its plist lost `ARENA_TELEMETRY_DIR`), so the other server checks are blind | `ls -l ~/services/arena/telemetry/arena-server/`, `launchctl print gui/$(id -u)/io.aiocean.arena-live` |
| `live_match_start` | no `match.start` from `arena-server` in 600 s | the live server is down or stuck between matches | `~/Library/Logs/game-factory/arena-live.log`; `q --service arena-server --since 1h --name 'match.*'` |
| `live_snapshot` | the newest live `snapshot` is older than 120 s | no match is being fought (one is written a second of fight) | as above |
| `server_errors` | over 20 ERROR / WARN lines of the server's own (not a match's) in 15 min (a deploy's restart makes about ten) | the server or the relay is in trouble | `q --service arena-server --since 15m --where 'level=ERROR\|WARN'` |
| `server_panics` | any `panic` line in 15 min | the server panicked | `q --service arena-server --since 1h --name panic --fields reason,location` |
| `missed_ticks` | over 60 `arena_server_missed_ticks_total` in 15 min | ticks run a whole tick late and are caught up back to back | `q --service arena-server --since 1h --name arena_server_late_seconds --stats value` |
| `tick_resyncs` | any `arena_server_resyncs_total` in 15 min | the server fell over 250 ms behind and restarted its tick schedule | as above; the plist's `ProcessType` (above) |
| `tick_late` | `arena_server_late_seconds`'s max over 0.1 s in 15 min | a tick landed over 100 ms after its time | as above |
| `disk_data` | the telemetry dir over 2 GB on disk | the retention is not keeping it in bounds | `q --files`; `du -sh ~/services/arena/telemetry` |
| `disk_free` | under 10 GB free on its volume | the kiosk's disk is filling | `df -h ~` |

A metric the server has not written in the window (no tick was missed) passes as "no data"; a server check whose
files cannot be read is `unknown` and keeps its last state. The thresholds are constants at the top of `watchdog.rs`,
tested there (`cargo test -p arena-telemetry`).

**Silence** a check's notifications: put its name on a line of `~/services/arena/watchdog/watchdog-silence` (`all`
for every check, `#` for a comment); it is still checked and written, with `silenced` true. Delete the line to
unsilence. Stop the watchdog altogether with `launchctl bootout gui/$(id -u)/io.aiocean.arena-watchdog`.

**Install or update** it:

```
cargo build --release -p arena-telemetry --example watchdog
mkdir -p ~/services/arena/watchdog
install -m 755 target/release/examples/watchdog ~/services/arena/watchdog/watchdog
cp ops/telemetry/io.aiocean.arena-watchdog.plist ~/Library/LaunchAgents/
launchctl bootout gui/$(id -u)/io.aiocean.arena-watchdog 2>/dev/null
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.aiocean.arena-watchdog.plist
launchctl kickstart gui/$(id -u)/io.aiocean.arena-watchdog                                 # a run now
```

`watchdog --dry` prints the checks and writes nothing. `ARENA_TELEMETRY_DIR` moves what it reads and writes;
`ARENA_WATCHDOG_STATE`, `_SILENCE` and `_LOG` move its own files (tests and trials).

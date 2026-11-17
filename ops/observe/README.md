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
arena-bot --features host,onnx --example ladder -- --seeds 150 --telemetry-otlp`. `cargo run --release -p
arena-telemetry --example observe` fights brain matches through the observer, as a live match is reported.

### The live server (io.aiocean.arena-live)

Add to the plist's `EnvironmentVariables`, then reload it; the password stays in the file:

```xml
<key>ARENA_OTLP_ENDPOINT</key><string>http://127.0.0.1:5080/api/default</string>
<key>ARENA_OTLP_AUTH_FILE</key><string>/Users/firegroup/projects/game-factory/ops/arena/observe.env</string>
```

## Backend and frontend are separate

Game events come only from the backend, where a match runs: arena-server, and the sim and ladder tools. The browser
never reports game events. **Browser telemetry is deferred**: it will come with the two-way relay path (the browser
publishing on MoQ) once players act live, as service `arena-web` in its own stream. The only links between the two
sides will be `match_id` and the viewer's session id.

## Services and streams

| service.name | What |
|---|---|
| `arena-server` | the live match server: its matches and its health |
| `arena-bot` | the bot host (wasmtime): think time, fuel, faults (filed under this name whichever process hosts the bot) |
| `arena-ladder` | the sim, balance and ladder runs |

Log streams (OpenObserve type `logs`). Every row has `_timestamp`, `body` (the event's name), `severity`,
`service_name`, `service_version`, `vcs_revision` (the commit), `host_name`, `trace_id` and `span_id` (the match's
trace), and the event's own fields:

| Stream | Events (`body`) and fields |
|---|---|
| `arena_match` | `match.start`, one per seat: `match_id`, `seed`, `seat`, `bot`, `foe`, `actives` (`skill[support+support],…`), `supports`, `keystones`, `root`, `life`, `qi` (the qi bar; null when `qi_uncapped`: Cổ Thần Quyết has no cap), `qi_uncapped`. `round.start`: `round`, `wins_0`, `wins_1`. `round.end`: `winner`, `winner_bot`, `seconds`, `life_0`, `life_1`. `forfeit`: `seat`, `bot`. `match.end`: `result` (seat0, seat1, draw, cut, abandoned), `winner_bot`, `cut`, `rounds`, `wins_0`, `wins_1`, `seconds` (fight time), `ticks`, `wall_seconds`, `bot_0`, `bot_1` |
| `arena_duel_events` | each with `match_id`, `round`. `cast`: `who`, `bot`, `skill`, `skill_name`, `tags`, `kinds`, `basic`, `qi`. `hit`: `by`, `bot`, `skill`, `skill_name`, `tags`, `kinds`, `damage`, `share` (of full life), `how` (plain, counter, punish), `crit`, `launch`, `ranged`, `power`, `ailments` (laid this step), `foe_life`. `stopped`: `by`, `skill`, `how` (guard, barrier, ward, dodge, clash), `chip`. `broken`: `who`, `skill`. `dot` (damage over time, shown every half second (`DOT_SHOWN`), or a curse's strike): `by`, `damage`, `share`, `cause` (burn, bleed, burn+bleed, curse). `heal`, `ward`: `by`, `amount`. `barrier`, `unyielding`: `by`. `ko`: `winner`, `life_0`, `life_1`, `seconds`. `status`: `who`, `status` (guard, meditate, fly, sealed, rooted, no_guard, silenced, barrier, down), `on` (after holding a quarter second). `ailment`: `who`, `ailment`, `stacks`, `before`. `curse`: `who`, `curse`, `on`. `unit.spawn`: `unit`, `by`, `kind`, `skill`, `x`; `unit.act`, `unit.hurt`, `unit.die`. `snapshot` (each fighter, each second of fight time): `who`, `bot`, `time`, `life`, `life_max`, `shield`, `sense`, `qi`, `x`, `y`, `ailments` (`burn:2,soak:1`), `curses`, `units`, `act` |
| `arena_server` | the server's own: `new match`, `match done` (`ticks`, `groups`, `frames`, `wall_seconds`), `relay subscription` (`watched`: the relay took or dropped the track; the server sees the relay, not each viewer), `server stopped` (`error`), `panic` (`reason`, `location`), moq warnings |
| `arena_bot` | `bot.fault`: `call` (start, round, decide), `kind` (trap, out_of_fuel), `fault`, `faults` (the first ten of an instance, then every thousandth). `bot.broken`: a module that will not instantiate. `bot.end` (an instance's totals): `decisions`, `faults`, `think_mean_ms`, `fuel_mean`, `fuel_most`, `fuel_cap` |
| `arena_ladder` | each with `run_id` or `match_id`. `ladder.row`: `row`, `win`, `seat0`, `seat1`, `round_seconds`, `timeouts`, `top`, `attack_share`, `distance`. `ladder.skill`: `row`, `seat`, `skill`, `casts`, `landed`, `damage`, `damage_share`, `guarded`, `chip`, `barrier`, `ward`, `dodged`, `clashed`, `broken`, `punished`, `finals`. `ladder.seat`. `balance.run`; `balance.row`: `kind` (gem, support, keystone), `id`, `tier`, `picks`, `games`, `win`, `used_rate`, `damage_per_game`, `casts`, `hits`. `sim.match` (by seat: loadout, `won`, `damage`), `sim.round`, `sim.skill` |

Traces (type `traces`, streams `arena_server`, `arena_ladder`): a `match` span (`match_id`, `seed`) with a `round`
span for each round; the events above carry its `trace_id`.

Metrics (type `metrics`, one stream each; a histogram also has `_bucket`, `_count`, `_sum`, `_min`, `_max`):
`arena_matches_total`, `arena_rounds_total`, `arena_casts_total` (`skill`), `arena_damage_total` (`skill`, `kind`),
`arena_step_seconds` (a duel step's wall time), `arena_server_publish_seconds` (a tick stepped and written to the
track), `arena_server_late_seconds` (how far a tick runs behind the wall clock), `arena_bot_think_seconds`,
`arena_bot_fuel`, `arena_bot_faults_total` (`call`, `kind`).

Metrics are sent as deltas (since 2026-11-18; `aggregation_temporality` = `AGGREGATION_TEMPORALITY_DELTA`): each point
is its ten seconds' own counts, so `sum(value)` over any range is what happened in it, and a histogram's `_max` is its
interval's. The few cumulative points from before are left out by the dashboards.

## Dashboards

Open https://arena-obs.aiocean.io, log in with the root user's email and password from
`~/projects/game-factory/ops/arena/observe.env` (never paste them anywhere), then Dashboards, folder `default`:

| Dashboard | What it shows |
|---|---|
| Arena: live matches | matches in progress, played, mean length and rounds; matches per hour, results (seat0, seat1, cut, abandoned), match length, rounds per match, the latest 50 matches |
| Arena: game events | casts per skill, damage per skill (dots by cause) and per kind, hits by how, blows stopped by how, ailments laid, units spawned / acted / hurt / killed, KOs per round, events per minute |
| Arena: server health | step time (buckets; mean and most per minute), publish time, how far behind the wall clock, errors and warnings per hour, relay subscription changes, panics, server starts, the latest errors |
| Arena: balance | the latest balance run (keystones, gems, supports: win, damage a game) and ladder run (seat win rates), sim wins by seat |
| Arena: bots | think time p50 and p95, think and fuel buckets, fuel per instance, faults by kind and call, forfeits |

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

There are no alerts yet: OpenObserve sends them to a destination (email or a webhook) and none is set up. With one,
the two to add are errors per hour in `arena_server` above zero, and no `match.start` from `arena-server` in ten
minutes.

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
SELECT max(fuel_most) AS most, avg(think_mean_ms) AS think_ms FROM arena_bot WHERE body = 'bot.end'
-- the server falling behind its clock (metrics)
SELECT max(value) AS worst FROM arena_server_late_seconds_max
```

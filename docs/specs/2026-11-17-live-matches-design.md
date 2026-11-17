# Live matches: the server fights, the page watches

## Intent

The owner, 2026-11-17: the game and its files are heavy; matches should run on a server, and the page should watch the
matches being fought, live. The link between them is WebTransport carrying postcard, built as Remit's is
(`~/projects/remit`: `crates/link`, `crates/link-quic`, `crates/client-web/src/browser.rs`, ADR-0072 decision 3,
ADR-0095).

What the page loads today, measured on `site/dist` (2026-11-17): the game's wasm 18.4 MB (5.9 MB gzipped), the house
bot module 13.7 MB (3.6 MB gzipped; its ONNX model is 0.8 MB, the rest is tract), the sheets 12 MB of PNG. The bot
module is fetched only after the game has started, then admitted, compiled and instantiated twice before the first
tick. With the server deciding, the page loads no bot at all.

## The shape

- **The server runs every match**: the rules (`arena-duel`) and both corners' bots natively (the house AI on tract,
  `arena-bot` `onnx`; a submitted module in wasmtime, `arena-bot` `host`, metered as today). One match is one task.
  Today each match seats the house AI against a built-in mind (brain, first mind, aggressive, turtle) in turn, the
  house AI's seat and the looks drawn from the seed (`arena_server::match_run::Pairing`).
- **The page plays the match back**: the rules are deterministic from a seed, so the server sends the seed and each
  corner's decisions as they are made, a few bytes each, and the page's own copy of the rules steps the same fight and
  draws it. Nothing of the fight's state crosses the link.
- **Live**: the server keeps a match at the pace a watcher sees it (below), no more than `LEAD` (1 s) ahead of the
  wall clock; a page plays `DELAY` (0.3 s) behind what it has received, so a late packet never stalls the picture.
- **Joining late**: a watcher who comes in mid-match is sent the start and every decision so far; the page steps the
  rules to the present without drawing (thousands of ticks, a fraction of a second) and plays on live.
- **Replays**: a finished match is its start and its decisions; the server keeps them, and a replay is the same stream
  sent at once. This is the tournament's "replays anyone can check" (`docs/tournament.md`).

## Two rule changes first (the owner's call: ladder before and after, `docs/rules.md` in the same change)

1. **One arena for every screen.** `arena-game/src/rules.rs` `config` takes the arena's walls, ground and sky from the
   window's `Layout`, and the fight starts over when the window changes size: the same seed fights differently on
   another screen, which the docs already say it does not. The arena becomes constants of the rules (`data.rs`, sized
   from today's widest layout: `SPAN_MAX` 420 world pixels); the camera fits it to the window. A bot reads distances
   over the arena's width, so the house AI is retrained if the width it trained on changes.
2. **The fight's pace is a rule.** The feel stops the fight for a hit-stop and slows it after a knockout
   (`feel/fight.rs` `Feel::pace`, from the duel's `Out` events): the page's fight runs slower than the clock. A server
   that draws nothing would run ahead of every watcher by every stop. The pace (the stop and slow-motion seconds an
   `Out` sets) moves into `arena-duel` as a pure function of the events; the server waits it out, the page's feel
   reads it from the rules instead of working it out. The screen's part (shake, flashes, the dusk) stays in the feel.

## The link

As Remit's, cut to what a watcher needs.

- **Crates.**
  - `arena-link`: the protocol, pure (no I/O; frames in, frames out, the time given), building for wasm32 and
    natively; the messages; postcard frames behind a 4-byte length; a `Transport` trait a driver implements.
  - `arena-link-quic`: the driver on `web-transport-quinn` (server, and a native client for tests and a CLI watcher).
  - The browser driver is in `arena-game` (`web.rs` beside it): the browser's WebTransport through `web-transport`
    (moq-dev), which needs `--cfg=web_sys_unstable_apis` in `.cargo/config.toml` for the wasm build.
  - `arena-server`: the binary; runs the matches, accepts sessions, keeps replays.
- **One session per page**, at `/live`.
  - **Control stream**, the first bidirectional one, opened by the page: `Hello { rules }` (a hash of the rules'
    build) → `Welcome` or the session closed with `NewRules` (the page reloads itself: a page of another build would
    step another fight). Then `Watch { match }` / `Leave { match }` from the page, `Ping`/`Pong` both ways.
  - **The feed**: one server-opened unidirectional stream: the matches being fought and the latest finished
    (`Matches`), changes as they happen.
  - **One server-opened unidirectional stream per match watched**: `Start { match, seed, corners, past }` (`past`: the
    decisions so far), then `Decision { tick, corner, choice }` (`arena_mind::Choice`, already serde behind a feature),
    `Forfeit { tick, corner }` (a module that trapped or ran out of fuel), `End { winner }`. In order within the
    stream; matches are independent, so one slow match holds up no other.
  - Datagrams are not used: a lost decision is a different fight.
  - **Falling behind**: a watcher whose stream's unsent bytes pass a bound is told `Behind` on the control stream and
    the session is closed; the page dials again and is sent the match from its start (Remit's guarantee 4).
- **Postcard rules** (ADR-0095): types on the link have no `flatten`, no `skip_serializing_if`, no internally tagged
  enum; a round-trip test encodes one value of each and reads it back equal.

## On the page

- A seat that plays a log: `seat::Replay` in `arena-duel`, whose `think` answers the decision logged for this tick and
  `forfeit` the logged forfeit. The duel is otherwise the same `Duel`; the game's `bots.rs` seats two of them.
- The page no longer fetches `bots/policy.wasm`, and the game's wasm drops `arena-bot` (its admission and its
  WebAssembly glue) and gains `arena-link`, `web-transport` and postcard.
- A player's bot: dropped on the page, the module is sent to the server, admitted there as a submission is, and fights
  as a match the page then watches. The page's "it runs here, in your browser, and nothing is uploaded" is rewritten.
- The loader (with the real progress of `site/assets/arena.js`, a separate change) gains a step: connecting to the
  arena.

## Certificates and the host

- WebTransport is HTTP/3 over UDP with TLS at the binary. Cloudflare's tunnel, which serves arena.aiocean.io today
  (`game-factory/backend/arena/README.md`), carries HTTP and WebSocket to a public hostname, never UDP (Cloudflare's
  tunnel FAQ; UDP only on private networks through WARP). The server's Mac has no static address.
- **Candidate: Cloudflare's MoQ relay** (Media over QUIC, beta since 2026-07-31, free in the beta; drafts 14, 16, 18,
  FETCH from 18). The server dials out to the relay and publishes; pages subscribe over WebTransport at Cloudflare's
  edge, with Cloudflare's certificate. No port opened, no static address, the fan-out to watchers is the relay's. The
  link above maps onto MoQ: a namespace per season, a track per match, the feed a track; a decision an object; a late
  joiner fetches the match's past (FETCH) or each group opens with the log so far. The control stream's checks (the
  rules' hash) move into the feed. Proof P0 below decides it.
- **Otherwise**: a small server with a fixed address runs `arena-server` (or relays for the Mac), with a UDP port open.
  A port forwarded at home with dynamic DNS is the last choice: it shows the home address, and fails under CGNAT.
- As Remit does: the server makes a self-signed ECDSA P-256 certificate valid less than 14 days at each start and
  before the last expires, and serves `live.json` (where to dial, the certificate's hash) over HTTPS beside the page;
  the page dials with `serverCertificateHashes`. No public certificate to keep.
- Browsers: Chrome, Firefox; Safari only from 26.4 (Remit's reading, caniuse 2026-11-10). Whether Safari takes
  `serverCertificateHashes` is unverified; a page without WebTransport says so.

## Proofs before building on them

| | What | Proved by |
|---|---|---|
| P0 | Cloudflare's MoQ relay carries a match | A Rust publisher on this Mac and a page in Chrome, both through one relay and its two tokens: the page receives a match's decisions live, a page joining late gets the past, the wasm client builds for the page. Refuted by any of these failing; then a server with a fixed address |
| P1 | The page's fight is the server's | A native test runs a match with two bots, logs it, replays it with two `Replay` seats and compares the duel's view every tick; the page's test checks the same log against a golden (as `house_fight.json` is today) |
| P2 | The pace is the same on both ends | The same test compares the pace function's seconds per tick |
| P3 | A browser reaches the server | On this Mac: Chrome and Firefox open a session with the certificate hash, over loopback, then over the public UDP port |
| P4 | A late joiner catches up | A page that joins at 60 s plays on within `DELAY` of one that watched from the start |

### P0 so far (2026-11-17)

- The crates: `moq-net` 0.3.23 and `moq-tokio` 0.19.37 (moq-dev, the makers of `web-transport`) speak moq-lite and
  IETF drafts 14 to 22, negotiated by ALPN. Cloudflare's own IETF crate is `moq-transport` (cloudflare/moq-rs), native
  only.
- Native, both ways: `moq-tokio`'s `clock` example connects to Cloudflare's open relay
  (`https://draft-14.cloudflare.mediaoverquic.com`) as `moq-transport-14`, publisher and subscriber.
- In the browser: `moq-net` over `web-transport-wasm` 0.6.0 (the last on `web-transport-trait` 0.4, which `moq-net`
  takes) builds for wasm32 with the IETF path, and in Chromium connects to the same relay as `Ietf(Draft14)`
  (`site/spike/moq/browser`). 2.5 MB after `wasm-opt -Oz`, 0.9 MB gzipped, on its own.
- **P0 passed, end to end (2026-11-17).** A relay was provisioned with the account's "MoQ Edit" API token (relay
  `arena`, uid `774cf1c6…`; `ops/arena/moq-relay.env` holds a publish and a subscribe JWT). Through the draft-18 relay:
  the native publisher and subscriber ran as `moq-transport-18`; the browser subscriber connected as `Ietf(Draft18)`
  and logged each frame live while the native publisher ran. Spike kept at `site/spike/moq` (its README has the steps).
- The open draft-14 relay is not enough: it never answers `SUBSCRIBE_NAMESPACE`, so a `moq-net` subscriber (which finds
  broadcasts by their announcements) never discovers the match. A provisioned draft-16/18 relay announces namespaces and
  works.

## As built (2026-11-17)

The design above, with what the build changed. Everything runs end to end: `io.aiocean.arena-live` fights on the
kiosk, Cloudflare's relay carries it, and the home page draws it.

- **The link is MoQ, not our own streams.** The relay is a MoQ relay, so its model is the carrier: a match is a
  broadcast `arena/match/<id>` with one track `fight`; a page lists matches by the announcements under `arena/match`.
  The control stream, the feed and the `Hello` of "The link" are not built: the rules check rides in every keyframe
  (`Snapshot.rules` against `arena_link::RULES_VERSION`; a page on other rules is told to reload).
- **Frames** (`arena_link::Frame`, postcard): `Keyframe(Snapshot)` opens every group (seed, arena, corners, every
  decision and forfeit so far, the ticks fought); then `Decision`, `Forfeit`, `Through(tick)`, `End`. `Act` is the
  postcard mirror of `arena_mind::Choice` (whose own serde is internally tagged, so postcard cannot carry it).
- **`Through` was not in the design.** A tick with no decision is a real "nothing", and a page cannot tell it from a
  decision still in flight, so the server marks every `THROUGH_EVERY` (6) ticks how far everything has been sent, and
  the page steps only that far (`rules::DELAY_TICKS`, 18 ticks, behind it). This also answers the pace problem of rule
  change 2 without changing a rule: the feel's hit-stops make the page fall behind, and it runs at double pace when
  more than `CATCH_UP_TICKS` behind, and jumps (undrawn) when more than `JUMP_TICKS` behind or joining late.
- **The arena is in the keyframe.** Rule change 1 is not made either: the server fights in the arena a 1280x720 window
  lays out (`arena_server::match_run::ARENA`), the keyframe carries it, and the page builds its rules on it instead of
  on its window. A page on a window of another size draws that arena where its layout puts the world (the camera
  fitting it is still to do).
- **Seats**: `arena_duel::seat::Recorder` wraps the server's seats and logs each `think`; `seat::Replay` plays a log
  that keeps growing (`Replay::live`). A tick is a count of the seat's own `think` calls, the same on both ends.
- **The page** (`arena-game` `live.rs`): `live(url)` (wasm) subscribes through `moq-net` 0.3 over
  `web-transport-wasm` 0.6, folds frames (`Watching`), and the rules seat two `Replay`s on its logs. With no match in
  8 s the page seats the house bot and fights on its own; a player dropping a bot also turns watching off. `?seed=`
  in the address plays that seed on the page, as before. The site writes `live.json` from `ARENA_LIVE_URL` (the
  relay's URL with a subscribe-only token), never from the repo. The wasm build needs `--cfg=web_sys_unstable_apis`
  (`site/build.sh`, CI).
- **Proofs**: P1, `arena-duel` `tests/replay.rs` (a recorded fight replays tick for tick) and `arena-server`'s unit
  test (the wire snapshot replays the fought match); P3 and P4 by `arena-watch`, the native watcher, which joins a
  live match on the relay, rebuilds it from its frames and checks its replay ends where the server's did, and by the
  page itself, which joins mid-match and follows into the next.
- **Weight**: the game's wasm is 6.6 MB gzipped with the live path (5.9 MB before); it still carries `arena-bot` for
  the page's own fights, and the house module is fetched only when the page fights on its own.

## Order

1. The two rule changes (the owner's call), measured with the ladder before and after.
2. `arena-link` with its tests over an in-memory transport; `seat::Replay` and P1, P2.
3. `arena-link-quic`, `arena-server`; P3.
4. The page: the browser driver, the replay seats, the watch list; P4. Then `arena-bot` leaves the game's wasm.
5. Players' bots sent to the server.

`CLAUDE.md`'s table gains the protocol, the server and its driver; `docs/bot.md` and the home page say where a bot runs.

## Open

- **The host**: Cloudflare's MoQ relay if P0 holds; else a small server with a fixed address. The owner's call.
- Which matches run when nobody uploads: the house against itself on a loop, with a new seed each time, as the home
  page shows now.

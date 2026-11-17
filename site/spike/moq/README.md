# P0 spike: live matches over Cloudflare's MoQ relay

Proves the transport of `docs/specs/2026-11-17-live-matches-design.md`: the server (native, behind NAT, no static
IP) publishes a match outbound to a Cloudflare MoQ relay, and a browser subscribes over WebTransport and plays it
live. Result 2026-11-17: **passed, end to end, on IETF draft-18.**

## Relay

Made with the account's "MoQ Edit" API token (`ops/arena/cloudflare-moq.env` in game-factory, chmod 600):

    POST /accounts/{acc}/moq/relays            {"name":"arena"}   -> relay uid 774cf1c6…
    POST /accounts/{acc}/moq/relays/{uid}/tokens  {"operations":[...]}  -> a JWT per token

The publish and subscribe JWTs are in `ops/arena/moq-relay.env`. A token rides in the URL path:
`https://draft-18.cloudflare.mediaoverquic.com/<JWT>`. Free during the beta. Revoke a token with
`DELETE .../tokens/{jti}`.

## Browser (`browser/`)

A ~40-line Rust `cdylib`: `moq-net` over `web-transport-wasm` 0.6.0 (the last on `web-transport-trait` 0.4, which
moq-net takes), the browser's WebTransport. 2.5 MB after `wasm-opt -Oz`, 0.9 MB gzipped, on its own.
`.cargo/config.toml` sets `--cfg=web_sys_unstable_apis` and the wasm getrandom backend.

    cargo build --release --target wasm32-unknown-unknown
    wasm-bindgen --target web --no-typescript --out-dir web target/.../moqwasm.wasm
    # serve web/ and open ?url=<relay>/<SUB_JWT>&b=<broadcast>&t=seconds

In Chromium it connected as `Ietf(Draft18)` and logged each frame live while the native publisher ran: the end of
P0. The draft-14 open relay is not enough (it never answers SUBSCRIBE_NAMESPACE, so a moq-net subscriber never
discovers the broadcast); a provisioned draft-16/18 relay does.

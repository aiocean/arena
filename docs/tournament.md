# The tournament

Players train bots, submit them as bot modules (most built from an ONNX model), and the arena plays them against each other to find the strongest.
This is the design; what is not built yet is listed in `docs/roadmap.md`.

## One season, while we test

The arena runs a single season for now, the test one, on this repo's `main`. Nothing is frozen yet: the rules (every
number in `crates/arena-duel/src/data.rs` and in the damage system, `crates/arena-calc`, the frame data and the boxes
measured from the sprite sheets, `crates/arena-duel/src/frames_data.rs`, so a redrawn clip too, the bar), what a bot sees and does (`docs/bot.md`) and the
house AI may still change, and a change may mean a bot needs retraining. There are no season tags while the arena is
in testing; the release workflow (`.github/workflows/release.yml`, a tag `arena-s<N>`) is kept, dormant, for when
seasons start.

What a bot depends on, and what a later season would freeze: the rules; the 2055 inputs, the 29 acts, the legality
rules and the timing; the bot ABI (`arena-bot-4`), the module features it accepts and its budgets (`docs/bot.md`,
"The bot module"). The model inside a bot is never part of it: any size, any shape, retrained at will.

## What a player submits

One bot module (`.wasm`), plus a name (`docs/bot.md`, "What a player submits"; an ONNX model becomes one with
`bots/onnx`). Nothing else runs: the arena runs the module in its own copy of the rules, on its own machines, in a
sandbox that can reach nothing outside it.

Why a module: it can do nothing but answer a decision; it runs the same bits on every machine and in the browser, so
anyone can replay a fight; and its cost is capped by its fuel, so no bot buys reflexes with a bigger machine. It leaves
the model, its size, its memory and how it picks to the player.

## Checks on submission

Run them yourself first, the way the arena runs them:

```sh
cargo run --release -p arena-bot --features host --example check -- BOT.wasm
```

A bot module enters the season only if:

1. it is admitted: a valid module of the accepted features, no imports, the exports with their signatures, within
   size and memory (`arena_bot::admit` names what is wrong). The arena writes the fuel counter into it; that module is
   the bot from then on, and its SHA-256 is its identity;
2. it answers: `arena_abi()` is 3, and 32 fixed decisions from real fights, played twice in fresh instances, give the
   same acts both times within fuel;
3. it is good enough to be worth a slot (below).

Good enough: over a fixed set of seeds, both ways, in the balanced style, it wins at least 75% against the brain (65%
in each seat) and at least 80% against the first mind, as the ladder measures.

A module with the same hash as one already entered is refused.

## How a match is played

- A **match** between bots A and B is a set of seeds; each seed is played **both ways**: A in seat 0 and B in seat
  1, then swapped, 3 rounds each. Each seed draws both seats' loadouts, so a seat's edge and a seed's lucky loadout
  cancel out, and the bot that wins is the bot that plays both sides of each pair of loadouts better. The fighters'
  looks (Vương Lâm, Thất Thải Đạo Nhân) are drawn apart from the seats, and each seed is fought in both (`ladder::looks_of`).
- A match is one sequence: a bot module keeps one instance, and its memory, through every fight of it, so a
  match runs on one machine, in the order its seeds are listed. Each fight's act log is published, so any one fight
  can be checked alone by playing its log.
- Round limit 120 s; a round past it is cut: it ends that fight, and its unplayed rounds count for neither bot.
- The seeds are drawn by the arena and published only after the season ends, so nobody trains against them. They
  never overlap the training seeds (1,000,000 and up) or the ladder's (1-150).
- Each bot thinks with its own seeded dice, so a match is fully decided by: the seed list, file A, file B and the
  season's rules.

## Ranking

1. **Round robin.** Every pair plays a match. A bot's score is its share of rounds won. With many entries, a Swiss
   system pairs bots of similar score instead, a fixed number of rounds.
2. **Rating.** Elo (or TrueSkill) over every match, shown beside the score.
3. **Final.** The top 8 play a single-elimination bracket on a fresh, larger seed set, both ways.

The season's winner is the final's winner.

## Replays anyone can check

A fight is three things: a seed, two bot modules and a commit of this repo. Anyone with the repo at that commit can play it
again and get the same fight, frame for frame, natively or in the browser. The arena publishes, for each match, the
seeds and the hashes of both files; the results are checkable without trusting the arena.

## What a player sees

- Their bot's results per opponent and per seat, with the ladder's telemetry (what each skill did, punishes,
  counters).
- Any fight replayed on the page, both bots seated by their files.

## Live matches

The home page shows matches as they are fought: `crates/arena-server` fights the house AI against the one before
it (the old one always Vương Lâm, the new one Thất Thải Đạo Nhân) and
publishes each match's frames to a MoQ relay (`crates/arena-link`), and the page replays them from the decisions. The
page and the server must run one rules build, `arena_link::RULES_VERSION`; a page on other rules says so and reloads.
`arena-watch` checks a published match by replaying it. The server rests a few seconds between matches while the page
shows the result, and the board under the bars shows the match's number, the round and the score between Vương Lâm
and Thất Thải Đạo Nhân since the server started.

# The tournament

Players train bots, submit them as bot modules (most built from an ONNX model), and the arena plays them against each other to find the strongest.
This is the design; what is not built yet is listed in `docs/roadmap.md`.

## Seasons

A season freezes everything a bot depends on:

- the rules: every number in `crates/pixfx-duel/src/data.rs`, the frame data, the bar;
- what a bot sees and does: the 354 inputs, the 30 acts, the legality rules, the timing (`docs/bot.md`);
- the bot ABI (`arena-bot-1`), the module features it accepts and its budgets (`docs/bot.md`, "The bot module").

The model inside a bot is not part of a season: any size, any shape, retrained at will.

A season is a tag on this repo (`arena-s1`, ...). A balance change or a new input waits for the next season, because
it would make every submitted bot stale. A bot from one season does not play in another.

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
2. it answers: `arena_abi()` is 1, and 32 fixed decisions from real fights, played twice in fresh instances, give the
   same acts both times within fuel;
3. it is good enough to be worth a slot (below).

Good enough: over a fixed set of seeds, both ways, in the balanced style, it wins at least 75% against the brain (65%
as each school) and at least 80% against the first mind, as the ladder measures.

A module with the same hash as one already entered is refused.

## How a match is played

- A **match** between bots A and B is a set of seeds; each seed is played **both ways**: A as Thunder and B as
  Frost, then swapped, 3 rounds each. So a school's edge cancels out, and the bot that wins is the bot that plays both
  schools better.
- A match is one sequence: a bot module keeps one instance, and its memory, through every fight of it, so a
  match runs on one machine, in the order its seeds are listed. Each fight's act log is published, so any one fight
  can be checked alone by playing its log.
- Round limit 120 s; a cut round is a loss for both.
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

A fight is three things: a seed, two bot modules and a season tag. Anyone with the repo at that tag can play it
again and get the same fight, frame for frame, natively or in the browser. The arena publishes, for each match, the
seeds and the hashes of both files; the results are checkable without trusting the arena.

## What a player sees

- Their bot's results per opponent and per school, with the ladder's telemetry (what each skill did, punishes,
  counters).
- Any fight replayed on the page, both bots seated by their files.

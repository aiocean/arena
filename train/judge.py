"""Judge how trained policies fight, the way a spectator would: what code can count of how a fight looks (flicker,
ducks, take-offs, time in the air, casts), and what a decision model (TypeSafe's Jev, through OpenRouter's Decisions
API) reads in it, given the rules: 3-second clips (jittery? purposeless flight? skilled? watchable?), whole rounds
(deliberate, risky, dull, which pattern), and single moves set beside the policy's own critic. Several models are
fought on the same seeds and compared on one review page. Nothing here feeds a reward: it is for reading, and for the
coach (`.claude/skills/arena-coach`).

    uv run python judge.py ../../arena-train/train/runs/abi7a/u1200.pt runs/coach-calm/last.pt \\
        --names house calm --env-file ~/projects/game-factory/ops/.env --out runs/judge-coach \\
        --review ~/services/arena/site/review/jev-coach

Each model fights `--rounds` rounds against the house AI (`../crates/arena-duel/weights/policy.onnx`, as the live
matches do), the brain, the aggressive and the turtle, sampling its acts as the house bot does. Each decision keeps
its moment in words (`VecEnv.describe`, `arena_duel::words`), the act, the policy's probability for it, the critic's
value and the advantage worked out as PPO does (GAE over the round's rewards, paid with the run's terms). The first
model's moves are also judged without the rules, to show what the rules change.

The key is `OPENROUTER_API_KEY`, from the environment or the `--env-file`; it is never printed.
"""

import argparse
import concurrent.futures as cf
import html
import json
import os
import pathlib
import statistics
import time
import urllib.error
import urllib.request

import numpy as np
import torch
from torch.distributions import Categorical

import arena_duel as pd
from export import load

HERE = pathlib.Path(__file__).parent
HOUSE = HERE / "../crates/arena-duel/weights/policy.onnx"
URL = "https://openrouter.ai/api/alpha/decisions"
MODEL = "typesafe/jev-1.14"
OPPONENTS = ["policy", "brain", "aggressive", "turtle"]
# a move undone within this long, with another between, is a flicker (the trainer's `--flicker`, `train::FLICKER`)
FLICKER = 1.0
CLIP, AFTER = 3.0, 1.5

# What Jev is told of the rules before every question (docs/rules.md, RULES 25). Only what the rules say.
RULES = """How this duel works (a 2D side-view spell duel between two cultivators):
- The arena is 880 px wide with walls. Distances are between the fighters' centres; standing they are at least about
  85 px apart. Life and qi are shown as 0-100%.
- Every skill has a wind-up (its blow comes out at the end) and a recovery. A blow that lands during a wind-up breaks
  the cast and does 1.4x. Casting is slow on purpose: a good fighter casts when the foe cannot answer (it is
  recovering, stunned, knocked down or meditating) or from where the foe cannot punish it.
- Range bands: a skill thrown at the foe can only be begun from its band of distances: near skills (palms, bursts,
  close blasts) 0-192 px, mid skills (seals, areas landed on the foe) 112-400 px, far skills (bolts, banners)
  224-640 px. Outside its band a skill shows not ready. Spacing decides which skills can be used.
- Qi pays for casts. A jump or a take-off into flight costs 8 qi; flying costs 8 qi a second and stops qi
  regeneration; on the ground qi refills 8 a second.
- Meditation: sitting down takes 0.4 s and getting up 0.4 s, and in both it cannot act. Sitting refills qi faster (up
  to 21 a second) but the extra only starts after a whole first second, in half-second blocks. A blow on a meditating
  fighter does 1.5x and breaks it. A short sit gains nothing; sitting where the foe can strike is dangerous.
- Ducking lowers the body (228 px tall standing, 160 ducking): a fighter is hit where its body is drawn, so a bolt
  flying at chest height can pass over a ducking one. A ducking fighter cannot act. Ducking does nothing against
  areas or fields on the ground.
- Guard stops blasts, bolts and orbs from the front only (not areas or fields), lets 20% through and costs 8 qi a
  blow held.
- A fighter in the air is hit where its body is; areas that hit only a foe on the floor miss it.
"""

CLIP_QUESTIONS = {
    "jittery": {
        "type": "noul",
        "instructions": "Does Me look jittery or indecisive in this clip to a spectator: switching back and forth "
        "(duck, stand, duck; walk in, walk out; sit, stand) with no purpose the situation explains?",
    },
    "aimless_flight": {
        "type": "noul",
        "instructions": "Does Me take to the air or stay in the air in this clip without a purpose (not dodging "
        "something, not reaching a better spot to cast)? If Me does not fly in the clip, answer no.",
    },
    "skilled": {
        "type": "score",
        "instructions": "How much does Me look like a skilled duelist in this clip?",
        "criteria": [
            "clumsy or random: moves that serve no purpose",
            "ordinary",
            "skilled and intentional: every move has a reason",
        ],
    },
    "watchable": {
        "type": "score",
        "instructions": "How good is this clip to watch for a spectator?",
        "criteria": ["dull or confusing", "fine", "exciting, readable exchanges"],
    },
}

ROUND_QUESTIONS = {
    "deliberate": {
        "type": "score",
        "instructions": "How deliberate is Me's casting in this round?",
        "criteria": [
            "spams spells whenever they are ready, with no regard to timing",
            "some casts timed, many thrown at random",
            "most casts are timed to an opening",
        ],
    },
    "risk": {
        "type": "score",
        "instructions": "How much real risk does Me take in this round?",
        "criteria": [
            "plays it safe: keeps away, rarely commits to a long cast or to meditating near the foe",
            "commits to some exchanges",
            "takes real, committed risks: long casts and meditation within the foe's reach",
        ],
    },
    "pattern": {
        "type": "choice",
        "instructions": "Which describes how Me fights in this round?",
        "criteria": {
            "spammer": "casts constantly, whatever the foe is doing",
            "turtle": "mostly guards, ducks or waits for the foe",
            "kiter": "keeps its distance, walks or flies away and shoots from range",
            "brawler": "presses in close and trades blows",
            "duelist": "patient: waits for openings, then commits",
            "aimless": "no clear plan: walks back and forth, wastes its turns",
        },
    },
    "dull": {
        "type": "noul",
        "instructions": "Would a spectator find this round dull: long stretches where nothing happens, or the same "
        "move over and over?",
    },
}

MOVE_QUESTIONS = {
    "quality": {
        "type": "choice",
        "instructions": "Judge My choice for this exact moment, by the rules above.",
        "criteria": {
            "strong": "a strong move: it punishes an opening, dodges a real threat, or sets up a hit",
            "fine": "a reasonable move, nothing special",
            "mistake": "a clear mistake: it walks into a blow, wastes qi or a cast that cannot land, or misses an "
            "obvious opening",
        },
    },
}


def api_key(env_file):
    """The OpenRouter key, from the environment or a `KEY=value` file; never printed."""
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key and env_file:
        for line in pathlib.Path(env_file).expanduser().read_text().splitlines():
            if line.startswith("OPENROUTER_API_KEY="):
                key = line.split("=", 1)[1].strip().strip('"').strip("'")
    if not key:
        raise SystemExit("OPENROUTER_API_KEY is not set (nor in --env-file)")
    return key


def decide(key, state, questions, tries=4):
    """One Decisions request: `(answers, seconds, cost)`."""
    body = json.dumps({"model": MODEL, "state": state, "questions": questions}).encode()
    for attempt in range(tries):
        req = urllib.request.Request(URL, body, {"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
        t0 = time.monotonic()
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                out = json.loads(r.read())
            return out["answers"], time.monotonic() - t0, float(out.get("usage", {}).get("cost", 0.0))
        except urllib.error.HTTPError as e:
            if e.code not in (429, 500, 502, 503, 524, 529) or attempt == tries - 1:
                raise RuntimeError(f"decisions: HTTP {e.code}: {e.read()[:300]!r}") from None
        except (urllib.error.URLError, TimeoutError):
            if attempt == tries - 1:
                raise
        time.sleep(1.5 * (attempt + 1))


def short(words):
    """An act's words without its aim: `cast fireball`, `duck`."""
    return words.split(" at ")[0].split(" aimed")[0]


def fight(policy, args):
    """Every round's decisions: a list of rounds, each `{opponent, seat, seed, won, moves: [...]}`."""
    n = args.rounds
    env = pd.VecEnv(n, min(n, os.cpu_count() or 4))
    env.set_model(0, HOUSE.read_bytes())
    rounds = []
    for i in range(n):
        opp = OPPONENTS[i % len(OPPONENTS)]
        seed = args.seed + i
        env.reset(i, seed, i % 2, 1, 1, opp, 0 if opp == "policy" else -1, args.limit)
        rounds.append({"opponent": opp, "seat": i % 2, "seed": seed, "won": 0, "moves": []})
    live = np.ones(n, dtype=bool)
    gen = torch.Generator().manual_seed(args.seed)
    while live.any():
        obs, mask = env.observe()
        words = env.describe()
        with torch.no_grad():
            logits, vlog = policy(torch.from_numpy(obs), torch.from_numpy(mask))
            probs = torch.softmax(logits, -1)
            act = torch.multinomial(probs, 1, generator=gen).squeeze(1)
            value = policy.value_of(vlog)
        ent = Categorical(probs=probs).entropy()
        for i in np.nonzero(live)[0]:
            moment, acts, (t, hp, foe_hp, qi, foe_qi, gap, flying, air) = words[i]
            a = int(act[i])
            rounds[i]["moves"].append(
                {
                    "t": round(t, 2),
                    "hp": hp,
                    "foe_hp": foe_hp,
                    "qi": qi,
                    "foe_qi": foe_qi,
                    "gap": gap,
                    "flying": flying,
                    "air": air,
                    "act": a,
                    "act_words": acts[a],
                    "p": float(probs[i, a]),
                    "entropy": float(ent[i]),
                    "value": float(value[i]),
                    "moment": moment,
                }
            )
        r, d, won, _ = env.step(
            act.numpy().astype(np.int64), args.shaping, args.whiff, args.whiff_far, args.variety, args.idle, args.land
        )
        r = r - np.float32(args.time_cost)
        for i in np.nonzero(live)[0]:
            rounds[i]["moves"][-1]["reward"] = float(r[i])
            if d[i]:
                live[i] = False
                rounds[i]["won"] = int(won[i])
    for rd in rounds:
        gae, nxt = 0.0, 0.0
        for m in reversed(rd["moves"]):
            delta = m["reward"] + args.gamma * nxt - m["value"]
            gae = delta + args.gamma * args.lam * gae
            m["advantage"] = gae
            nxt = m["value"]
    return rounds


def look(moves):
    """What code can count of how a stretch of decisions looks, per minute where it is a rate."""
    if len(moves) < 2:
        return {}
    secs = max(moves[-1]["t"] - moves[0]["t"], 1e-6)
    per_min = 60.0 / secs
    names = [short(m["act_words"]) for m in moves]
    changes = sum(a != b for a, b in zip(names, names[1:]))
    flicker = 0
    for i in range(2, len(moves)):
        a = moves[i]["act"]
        plain = a != 0 and not names[i].startswith("cast")
        if plain and moves[i - 2]["act"] == a and moves[i - 1]["act"] != a and moves[i]["t"] - moves[i - 2]["t"] < FLICKER:
            flicker += 1
    takeoffs = sum(1 for m in moves if m["act_words"] == "fly up" and not m["flying"])
    dt = np.diff([m["t"] for m in moves] + [moves[-1]["t"]])
    airborne = float(sum(d for d, m in zip(dt, moves) if m["flying"] or m["air"] > 4)) / secs
    casts = sum(1 for n in names if n.startswith("cast"))
    return {
        "seconds": round(secs, 1),
        "changes_per_s": round(changes / secs, 2),
        "flicker_per_min": round(flicker * per_min, 1),
        "duck_per_min": round(names.count("duck") * per_min, 1),
        "takeoffs_per_min": round(takeoffs * per_min, 1),
        "air_share": round(airborne, 3),
        "meditate_per_min": round(sum(1 for n in names if "meditate" in n) * per_min, 1),
        "casts_per_min": round(casts * per_min, 1),
        "skills_used": len({n for n in names if n.startswith("cast")}),
    }


def line(m, after=False):
    flight = " (flying)" if m["flying"] else " (in the air)" if m["air"] > 4 else ""
    return (
        f"{'  after ' if after else ''}{m['t']:.1f}s life {m['hp']:.0f} qi {m['qi']:.0f} | foe life {m['foe_hp']:.0f}"
        f" | gap {m['gap']:.0f}px{flight}: {m['act_words']}"
    )


def round_log(rd, limit=28000):
    """A round as text: its outcome, what it cast, and the decisions, runs of one act merged."""
    cast = sorted({short(m["act_words"])[5:] for m in rd["moves"] if m["act_words"].startswith("cast ")})
    head = [
        RULES,
        "A whole round, from Me's side. Each line: time, Me's life and qi, the foe's life, the gap, and what Me chose.",
        "Skills rotate through a bar during the round; Me cast: " + (", ".join(cast) or "nothing") + ".",
        f"Outcome: Me {'won' if rd['won'] == 1 else 'lost' if rd['won'] == -1 else 'neither won (time ran out)'}.",
    ]
    rows, run = [], None
    for m in rd["moves"]:
        if run and run["act_words"] == m["act_words"]:
            run["n"] += 1
            run["end"] = m["t"]
            continue
        if run:
            rows.append(run)
        run = {**m, "n": 1, "end": m["t"]}
    if run:
        rows.append(run)
    body = [line(r) + (f" x{r['n']} until {r['end']:.1f}s" if r["n"] > 1 else "") for r in rows]
    text = "\n".join(head + body)
    return text if len(text) <= limit else text[:limit] + "\n(log cut short)"


def clip_text(rd, start):
    """`CLIP` seconds of a round from move `start`, every decision, then `AFTER` seconds of what followed."""
    moves = rd["moves"]
    t0 = moves[start]["t"]
    inside = [m for m in moves[start:] if m["t"] < t0 + CLIP]
    after = [m for m in moves[start + len(inside) :] if m["t"] < t0 + CLIP + AFTER]
    first = moves[start]["moment"]
    return (
        RULES
        + "\nThe clip starts here:\n"
        + first
        + "\n\nMe's decisions in the clip, one a line (time, life, qi, foe's life, gap, choice):\n"
        + "\n".join(line(m) for m in inside)
        + "\n\nWhat followed (not part of the clip, to show what came of it):\n"
        + ("\n".join(line(m, True) for m in after) or "  the round ended")
    ), inside


def pick_clips(rounds, k, rng):
    """`k` clip starts spread over the rounds, each at least 2 s in and a whole clip before the end."""
    starts = []
    for ri, rd in enumerate(rounds):
        ms = rd["moves"]
        if not ms:
            continue
        end = ms[-1]["t"] - CLIP
        starts += [(ri, mi) for mi, m in enumerate(ms) if 2.0 <= m["t"] <= end]
    idx = rng.choice(len(starts), size=min(k, len(starts)), replace=False)
    return sorted(starts[j] for j in idx)


def pick_moves(rounds, k, rng):
    """`k` moves to judge: the most and least advantaged acts that are not a plain wait, and a random share."""
    pool = [(ri, mi) for ri, rd in enumerate(rounds) for mi, m in enumerate(rd["moves"]) if m["act"] != 0]
    pool.sort(key=lambda x: rounds[x[0]]["moves"][x[1]]["advantage"])
    third = k // 3
    chosen = set(pool[:third]) | set(pool[-third:])
    rest = [x for x in pool if x not in chosen]
    for j in rng.permutation(len(rest))[: k - len(chosen)]:
        chosen.add(rest[j])
    return sorted(chosen)


def spearman(a, b):
    if len(a) <= 2:
        return float("nan")
    ra, rb = np.argsort(np.argsort(a)), np.argsort(np.argsort(b))
    return float(np.corrcoef(ra, rb)[0, 1])


def ask_all(key, jobs, workers):
    """Run `(tag, state, questions)` jobs; `{tag: answers}`, latencies, cost, failures."""
    got, lat, cost, failed = {}, [], 0.0, 0
    with cf.ThreadPoolExecutor(workers) as pool:
        futures = {pool.submit(decide, key, s, q): tag for tag, s, q in jobs}
        for f in cf.as_completed(futures):
            try:
                answers, secs, c = f.result()
            except Exception as e:  # one failed question is reported, not fatal
                failed += 1
                print(f"failed {futures[f]}: {e}")
                continue
            got[futures[f]] = answers
            lat.append(secs)
            cost += c
    return got, lat, cost, failed


def judge_model(name, checkpoint, args, key, rules_ab):
    policy = load(checkpoint)
    t0 = time.monotonic()
    rounds = fight(policy, args)
    print(f"{name}: fought {len(rounds)} rounds, {sum(len(r['moves']) for r in rounds)} decisions in {time.monotonic() - t0:.1f}s")
    rng = np.random.default_rng(args.seed)
    clips = pick_clips(rounds, args.clips, rng)
    moves = pick_moves(rounds, args.moves, rng)
    jobs = [(("round", ri), round_log(rd), ROUND_QUESTIONS) for ri, rd in enumerate(rounds) if rd["moves"]]
    clip_rows = {}
    for ri, mi in clips:
        text, inside = clip_text(rounds[ri], mi)
        clip_rows[(ri, mi)] = {"round": ri, "start": mi, "text": text, "look": look(inside)}
        jobs.append((("clip", ri, mi), text, CLIP_QUESTIONS))
    for ri, mi in moves:
        m = rounds[ri]["moves"][mi]
        state = f"{m['moment']}\nMy choice: {m['act_words']}."
        jobs.append((("move", ri, mi), RULES + "\n" + state, MOVE_QUESTIONS))
        if rules_ab:
            jobs.append((("bare", ri, mi), state, MOVE_QUESTIONS))
    got, lat, cost, failed = ask_all(key, jobs, args.workers)

    all_moves = [m for rd in rounds for m in rd["moves"]]
    result = {
        "name": name,
        "checkpoint": str(checkpoint),
        "look": look_rounds(rounds),
        "won": {o: round(np.mean([r["won"] == 1 for r in rounds if r["opponent"] == o]), 2) for o in OPPONENTS},
        "requests": len(lat),
        "failed": failed,
        "latency_p50": round(statistics.median(lat), 3) if lat else None,
        "latency_p90": round(float(np.quantile(lat, 0.9)), 3) if lat else None,
        "cost_usd": round(cost, 5),
        "decisions": len(all_moves),
    }
    # rounds
    style = []
    for ri, rd in enumerate(rounds):
        j = got.get(("round", ri), {})
        rd["judge"] = j
        style.append(
            {
                "i": ri,
                "opponent": rd["opponent"],
                "won": rd["won"],
                **look(rd["moves"]),
                "deliberate": j.get("deliberate", {}).get("score"),
                "risk": j.get("risk", {}).get("score"),
                "pattern": j.get("pattern", {}).get("choice"),
                "dull": j.get("dull", {}).get("noul"),
            }
        )
    result["style"] = style
    result["patterns"] = {p: sum(1 for s in style if s["pattern"] == p) for p in ROUND_QUESTIONS["pattern"]["criteria"]}
    # clips
    clip_out = []
    for key_, row in clip_rows.items():
        j = got.get(("clip", *key_))
        if not j:
            continue
        clip_out.append(
            {
                **row,
                "opponent": rounds[row["round"]]["opponent"],
                "jittery": j.get("jittery", {}).get("noul"),
                "aimless_flight": j.get("aimless_flight", {}).get("noul"),
                "skilled": j.get("skilled", {}).get("score"),
                "watchable": j.get("watchable", {}).get("score"),
            }
        )
    result["clips"] = clip_out

    def mean(k):
        v = [c[k] for c in clip_out if c[k] is not None]
        return round(float(np.mean(v)), 3) if v else None

    result["jev"] = {
        "jittery": mean("jittery"),
        "jittery_share": round(float(np.mean([c["jittery"] >= 0.7 for c in clip_out])), 3) if clip_out else None,
        "aimless_flight": mean("aimless_flight"),
        "skilled": mean("skilled"),
        "watchable": mean("watchable"),
        "deliberate": round(float(np.mean([s["deliberate"] for s in style if s["deliberate"] is not None])), 3),
        "dull": round(float(np.mean([s["dull"] for s in style if s["dull"] is not None])), 3),
    }
    # does Jev's clip verdict follow what code counts? (if only that, it adds nothing to the counts)
    result["jev_vs_counts"] = {
        "jittery~flicker": round(spearman([c["jittery"] for c in clip_out], [c["look"].get("flicker_per_min", 0) for c in clip_out]), 3),
        "jittery~changes": round(spearman([c["jittery"] for c in clip_out], [c["look"].get("changes_per_s", 0) for c in clip_out]), 3),
        "aimless_flight~takeoffs": round(
            spearman([c["aimless_flight"] for c in clip_out], [c["look"].get("takeoffs_per_min", 0) for c in clip_out]), 3
        ),
        "skilled~casts": round(spearman([c["skilled"] for c in clip_out], [c["look"].get("casts_per_min", 0) for c in clip_out]), 3),
    }
    # moves against the critic, with the rules and (first model) without
    judged = []
    for ri, mi in moves:
        m = rounds[ri]["moves"][mi]
        j = got.get(("move", ri, mi))
        if not j:
            continue
        q = j["quality"].get("probabilities", {})
        m["p_strong"], m["p_mistake"] = q.get("strong", 0.0), q.get("mistake", 0.0)
        b = got.get(("bare", ri, mi))
        if b:
            qb = b["quality"].get("probabilities", {})
            m["bare_good"] = qb.get("strong", 0.0) - qb.get("mistake", 0.0)
        judged.append(m)
    good = [m["p_strong"] - m["p_mistake"] for m in judged]
    adv = [m["advantage"] for m in judged]
    result["moves"] = {
        "judged": len(judged),
        "rules_vs_critic": round(spearman(good, adv), 3),
        "bare_vs_critic": round(spearman([m["bare_good"] for m in judged if "bare_good" in m], [m["advantage"] for m in judged if "bare_good" in m]), 3)
        if rules_ab
        else None,
    }
    lo, hi = (np.quantile(adv, 0.25), np.quantile(adv, 0.75)) if adv else (0, 0)
    result["disagree"] = [
        {k: m[k] for k in ("t", "act_words", "p", "advantage", "p_strong", "p_mistake", "moment")}
        | {"why": "Jev: mistake; critic: better than expected"}
        for m in judged
        if m["p_mistake"] >= 0.6 and m["advantage"] >= hi
    ] + [
        {k: m[k] for k in ("t", "act_words", "p", "advantage", "p_strong", "p_mistake", "moment")}
        | {"why": "Jev: strong; critic: worse than expected"}
        for m in judged
        if m["p_strong"] >= 0.6 and m["advantage"] <= lo
    ]
    return result, rounds


def look_rounds(rounds):
    """`look` over every round, each weighted by its length."""
    looks = [look(rd["moves"]) for rd in rounds if len(rd["moves"]) > 2]
    secs = sum(lk["seconds"] for lk in looks)
    return {k: round(sum(lk[k] * lk["seconds"] for lk in looks) / secs, 3) for k in looks[0] if k != "seconds"} | {
        "seconds": round(secs, 1)
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("checkpoints", nargs="+", help=".pt checkpoints (their critics are needed)")
    ap.add_argument("--names", nargs="+", help="a short name for each checkpoint")
    ap.add_argument("--rounds", type=int, default=32)
    ap.add_argument("--clips", type=int, default=80, help="3-second clips to judge, per model")
    ap.add_argument("--moves", type=int, default=90, help="single moves to judge, per model")
    ap.add_argument("--seed", type=int, default=9001)
    ap.add_argument("--limit", type=float, default=90.0)
    ap.add_argument("--env-file")
    ap.add_argument("--out", default="runs/judge")
    ap.add_argument("--review", help="a folder to write the review page into (a notes.html there is folded in)")
    ap.add_argument("--workers", type=int, default=8)
    # the pay abi6b and abi7a trained on (docs/training.md), so the advantages are their critics' own
    ap.add_argument("--gamma", type=float, default=0.995)
    ap.add_argument("--lam", type=float, default=0.95)
    ap.add_argument("--shaping", type=float, default=0.667)
    ap.add_argument("--whiff", type=float, default=0.01)
    ap.add_argument("--whiff-far", type=float, default=0.02)
    ap.add_argument("--variety", type=float, default=0.5)
    ap.add_argument("--idle", type=float, default=0.004)
    ap.add_argument("--land", type=float, default=0.04)
    ap.add_argument("--time-cost", type=float, default=0.0005)
    args = ap.parse_args()
    names = args.names or [pathlib.Path(c).parent.name for c in args.checkpoints]
    if len(names) != len(args.checkpoints):
        raise SystemExit("one name a checkpoint")
    key = api_key(args.env_file)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    results = []
    for i, (name, ck) in enumerate(zip(names, args.checkpoints)):
        res, rounds = judge_model(name, ck, args, key, rules_ab=i == 0)
        (out / f"{name}.rounds.json").write_text(json.dumps(rounds))
        results.append(res)
        brief = {k: res[k] for k in ("name", "look", "won", "jev", "jev_vs_counts", "moves", "patterns", "cost_usd")}
        print(json.dumps(brief, indent=1))
    (out / "summary.json").write_text(json.dumps(results, indent=1))
    if args.review:
        folder = pathlib.Path(args.review).expanduser()
        review(folder, results)
        print(f"review page: {folder}/index.html")


def review(folder, results):
    """The comparison page: what code counts, what Jev reads, each model's worst clips and disagreements."""
    folder.mkdir(parents=True, exist_ok=True)
    e = html.escape

    def f(x, n=2):
        if x is None:
            return "–"
        if isinstance(x, float):
            return "nan" if x != x else f"{x:.{n}f}"
        return e(str(x))

    look_keys = list(results[0]["look"])
    head = "".join(f"<th>{e(r['name'])}</th>" for r in results)

    def table(title, rows):
        body = "".join(f"<tr><td>{e(k)}</td>{''.join(f'<td>{f(v)}</td>' for v in vals)}</tr>" for k, vals in rows)
        return f"<table><tr><th>{e(title)}</th>{head}</tr>{body}</table>"

    looks = table("what code counts (all rounds)", [(k, [r["look"][k] for r in results]) for k in look_keys])
    won = table("rounds won", [(o, [r["won"][o] for r in results]) for o in OPPONENTS])
    jev = table("what Jev reads (clips 0-1 noul / 0-2 score; rounds)", [(k, [r["jev"][k] for r in results]) for k in results[0]["jev"]])
    pats = table("Jev's pattern of each round", [(k, [r["patterns"][k] for r in results]) for k in results[0]["patterns"]])
    vs = table("Jev against the counts (Spearman over clips)", [(k, [r["jev_vs_counts"][k] for r in results]) for k in results[0]["jev_vs_counts"]])
    mv = table("Jev's single moves against the critic (Spearman)", [(k, [r["moves"][k] for r in results]) for k in results[0]["moves"]])
    cost = sum(r["cost_usd"] for r in results)
    reqs = sum(r["requests"] for r in results)
    p50 = statistics.median([r["latency_p50"] for r in results if r["latency_p50"]])

    sections = []
    for r in results:
        worst = sorted(r["clips"], key=lambda c: -(c["jittery"] or 0))[:6]
        best = sorted(r["clips"], key=lambda c: -(c["skilled"] or 0))[:3]

        def clip_html(c, tag):
            lk = c["look"]
            return (
                f"<details><summary>{tag} round {c['round']} vs {e(c['opponent'])}: jittery {f(c['jittery'])}, aimless "
                f"flight {f(c['aimless_flight'])}, skilled {f(c['skilled'])}, watchable {f(c['watchable'])} | counted: "
                f"flicker {lk.get('flicker_per_min', 0)}/min, changes {lk.get('changes_per_s', 0)}/s</summary>"
                f"<pre>{e(c['text'].split('The clip starts here:')[1])}</pre></details>"
            )

        dis = "".join(
            f"<details><summary>{e(d['why'])}: {e(d['act_words'])} at {d['t']}s (advantage {d['advantage']:+.2f}, Jev strong "
            f"{d['p_strong']:.2f} / mistake {d['p_mistake']:.2f})</summary><pre>{e(d['moment'])}</pre></details>"
            for d in r["disagree"][:10]
        )
        sections.append(
            f"<h2>{e(r['name'])} <small>{e(r['checkpoint'])}</small></h2>"
            f"<h3>Jev's most jittery clips</h3>{''.join(clip_html(c, 'jittery') for c in worst)}"
            f"<h3>Jev's most skilled clips</h3>{''.join(clip_html(c, 'skilled') for c in best)}"
            f"<h3>Disagreements with the critic ({len(r['disagree'])})</h3>{dis or '<p>none</p>'}"
        )
    notes = (folder / "notes.html").read_text() if (folder / "notes.html").exists() else ""
    page = f"""<!doctype html><meta charset=utf-8><title>Coach: how the policies look</title>
<style>body{{font:14px system-ui;max-width:1200px;margin:2em auto;padding:0 1em;background:#111;color:#ddd}}
table{{border-collapse:collapse;margin:1em 0}}td,th{{border:1px solid #333;padding:3px 8px;text-align:right}}
td:first-child,th:first-child{{text-align:left}}th{{background:#222}}pre{{white-space:pre-wrap;font-size:12px}}
h2{{margin-top:2.5em}}summary{{cursor:pointer;margin:.3em 0}}small{{color:#888;font-weight:normal}}</style>
<h1>Coach: how the policies look</h1>
<p>{len(results)} models, each fought on the same seeds against the house AI, the brain, the aggressive and the turtle.
Jev (<code>{MODEL}</code>) was given the rules (<code>train/judge.py</code>, <code>RULES</code>) and asked about 3-second
clips, whole rounds and single moves: {reqs} requests, p50 {p50:.2f} s, ${cost:.4f} in all.</p>
{notes}
<h2>Side by side</h2>{looks}{won}{jev}{pats}{vs}{mv}
<p>A Jev verdict that only follows a count adds nothing a count does not; one that goes its own way is either seeing
something the counts miss or wrong. One move's advantage is one noisy sample.</p>
{''.join(sections)}
<h2>Doubts</h2><ul>
<li>Jev reads words (<code>arena_duel::words</code>), not pictures: frame data, hitboxes and what each of the foe's
skills does are not in them.</li>
<li>The policy samples its acts; other seeds fight other rounds. Win shares here are a few rounds each: the ladder
(150 seeds) is the measure of strength.</li>
<li>Jev's probabilities are calibrated on its makers' data, not on duels; the owner's own labels are the check.</li></ul>
"""
    (folder / "index.html").write_text(page)


if __name__ == "__main__":
    main()

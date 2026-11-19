"""Ask a decision model (TypeSafe's Jev, through OpenRouter's Decisions API) about a trained policy's fights: how each
round reads as a duel (its style), and which single moves it calls mistakes or strong where the policy's own critic
says the opposite (the disagreements). Nothing here trains anything or feeds a reward: it writes what it found, for a
person to read.

    uv run python judge.py ../../arena-train/train/runs/abi6b/u1200.pt --rounds 24 --moves 240 \\
        --env-file ~/projects/game-factory/ops/.env --out runs/judge-abi6b \\
        --review ~/services/arena/site/review/jev-train

The policy fights `--rounds` rounds against the house AI (`../crates/arena-duel/weights/policy.onnx`, as the live
matches do), the brain, the aggressive and the turtle, sampling its acts as the house bot does. Each decision keeps
the moment in words (`VecEnv.describe`), the act, the policy's probability for it, the critic's value and the
advantage worked out as PPO does (GAE over the round's rewards, paid with the run's terms: `--whiff` and the rest).

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

# The questions, in one place. A round's log is judged as a whole; a move with its moment.
ROUND_QUESTIONS = {
    "deliberate": {
        "type": "score",
        "instructions": "How deliberate is Me's casting in this round of a spell duel? Casting is slow and leaves the "
        "caster open, so a good duelist casts at openings: the foe recovering, out of action, or about to walk into "
        "the spell.",
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
        "instructions": "Judge My choice for this exact moment of a spell duel. Casting takes a wind-up and a "
        "recovery during which the caster is open; meditating regains qi but leaves the meditator defenceless.",
        "criteria": {
            "strong": "a strong move: it punishes an opening, dodges a real threat, or sets up a hit",
            "fine": "a reasonable move, nothing special",
            "mistake": "a clear mistake: it walks into a blow, wastes qi or a cast that cannot land, or misses an "
            "obvious opening",
        },
    },
    "intent": {
        "type": "choice",
        "instructions": "What does My choice look like?",
        "criteria": {
            "deliberate": "a considered, timed decision",
            "panic": "a reflexive or panicked reaction",
            "passive": "stalling, avoiding the fight",
            "opportunistic": "seizing an opening the foe gave",
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
        req = urllib.request.Request(
            URL, body, {"Authorization": f"Bearer {key}", "Content-Type": "application/json"}
        )
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
            moment, acts, (t, hp, foe_hp, qi, foe_qi, gap) = words[i]
            a = int(act[i])
            rounds[i]["moves"].append(
                {
                    "t": round(t, 2),
                    "hp": hp,
                    "foe_hp": foe_hp,
                    "qi": qi,
                    "foe_qi": foe_qi,
                    "gap": gap,
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


def numbers(rd):
    """What code can count in a round, to set beside the judge's words."""
    moves = rd["moves"]
    length = max(moves[-1]["t"] - moves[0]["t"], 1e-6) if moves else 1e-6
    casts = [m for m in moves if m["act_words"].startswith("cast ")]
    acts = [m["act"] for m in moves]
    counts = np.bincount(acts, minlength=pd.ACTIONS) / max(len(acts), 1)
    return {
        "seconds": round(length, 1),
        "decisions": len(moves),
        "casts_per_min": round(60 * len(casts) / length, 1),
        "skills_used": len({m["act_words"].split(" at ")[0].split(" aimed")[0] for m in casts}),
        "meditations": sum(1 for m in moves if "meditate" in m["act_words"]),
        "top_act_share": round(float(counts.max()), 2),
    }


def round_log(rd, limit=28000):
    """A round as text for the judge: its outcome, the loadout, and the decisions, runs of one act merged."""
    cast = sorted({m["act_words"].split(" at ")[0].split(" aimed")[0][5:] for m in rd["moves"] if m["act_words"].startswith("cast ")})
    head = [
        "A round of a 1-on-1 spell duel between two cultivators, from Me's side. Each line: time, Me's life and qi, "
        "the foe's life, the gap between them in pixels (the arena is about 220px wide), and what Me chose.",
        "Skills rotate through a bar during the round; Me cast: " + (", ".join(cast) or "nothing") + ".",
        f"Outcome: Me {'won' if rd['won'] == 1 else 'lost' if rd['won'] == -1 else 'neither won (time ran out)'}.",
    ]
    lines, run = [], None
    for m in rd["moves"]:
        if run and run["act_words"] == m["act_words"]:
            run["n"] += 1
            run["end"] = m["t"]
            continue
        if run:
            lines.append(run)
        run = {**m, "n": 1, "end": m["t"]}
    if run:
        lines.append(run)
    body = []
    for r in lines:
        span = f"{r['t']:.1f}s" if r["n"] == 1 else f"{r['t']:.1f}-{r['end']:.1f}s"
        times = "" if r["n"] == 1 else f" x{r['n']}"
        body.append(
            f"{span} life {r['hp']:.0f} qi {r['qi']:.0f} | foe life {r['foe_hp']:.0f} | gap {r['gap']:.0f}: "
            f"{r['act_words']}{times}"
        )
    text = "\n".join(head + body)
    return text if len(text) <= limit else text[:limit] + "\n(log cut short)"


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
    ra, rb = np.argsort(np.argsort(a)), np.argsort(np.argsort(b))
    return float(np.corrcoef(ra, rb)[0, 1]) if len(a) > 2 else float("nan")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("checkpoint", help="a .pt checkpoint (its critic is needed)")
    ap.add_argument("--rounds", type=int, default=24)
    ap.add_argument("--moves", type=int, default=240, help="moves to judge one by one")
    ap.add_argument("--seed", type=int, default=9001)
    ap.add_argument("--limit", type=float, default=90.0)
    ap.add_argument("--env-file")
    ap.add_argument("--out", default="runs/judge")
    ap.add_argument("--review", help="a folder to write the review page into")
    ap.add_argument("--workers", type=int, default=8)
    # the pay abi6b trained on (docs/training.md), so the advantages are its critic's own
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
    key = api_key(args.env_file)
    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)

    policy = load(args.checkpoint)
    t0 = time.monotonic()
    rounds = fight(policy, args)
    print(f"fought {len(rounds)} rounds, {sum(len(r['moves']) for r in rounds)} decisions in {time.monotonic() - t0:.1f}s")

    rng = np.random.default_rng(args.seed)
    picks = pick_moves(rounds, args.moves, rng)
    jobs = [("round", ri, None, round_log(rd), ROUND_QUESTIONS) for ri, rd in enumerate(rounds) if rd["moves"]]
    for ri, mi in picks:
        m = rounds[ri]["moves"][mi]
        jobs.append(("move", ri, mi, f"{m['moment']}\nMy choice: {m['act_words']}.", MOVE_QUESTIONS))
    latencies, cost, failed = [], 0.0, 0
    with cf.ThreadPoolExecutor(args.workers) as pool:
        futures = {pool.submit(decide, key, j[3], j[4]): j for j in jobs}
        for f in cf.as_completed(futures):
            kind, ri, mi, state, _ = futures[f]
            try:
                answers, secs, c = f.result()
            except Exception as e:  # one failed question is reported, not fatal
                failed += 1
                print(f"failed {kind} {ri}/{mi}: {e}")
                continue
            latencies.append(secs)
            cost += c
            if kind == "round":
                rounds[ri]["judge"] = answers
                rounds[ri]["log"] = state
            else:
                rounds[ri]["moves"][mi]["judge"] = answers

    for rd in rounds:
        rd["numbers"] = numbers(rd)
    judged = [m for rd in rounds for m in rd["moves"] if "judge" in m]
    for m in judged:
        q = m["judge"].get("quality", {}).get("probabilities", {})
        m["p_mistake"], m["p_strong"] = q.get("mistake", 0.0), q.get("strong", 0.0)
    advs = np.array([m["advantage"] for m in judged])
    lo, hi = (np.quantile(advs, 0.25), np.quantile(advs, 0.75)) if len(advs) else (0, 0)
    disagree = [
        {**m, "why": "Jev: mistake; critic: better than expected"}
        for m in judged
        if m["p_mistake"] >= 0.6 and m["advantage"] >= hi
    ] + [
        {**m, "why": "Jev: strong; critic: worse than expected"}
        for m in judged
        if m["p_strong"] >= 0.6 and m["advantage"] <= lo
    ]
    summary = {
        "checkpoint": args.checkpoint,
        "rounds": len(rounds),
        "decisions": sum(len(r["moves"]) for r in rounds),
        "requests": len(latencies),
        "failed": failed,
        "latency_p50": round(statistics.median(latencies), 3) if latencies else None,
        "latency_p90": round(float(np.quantile(latencies, 0.9)), 3) if latencies else None,
        "cost_usd": round(cost, 6),
        "judged_moves": len(judged),
        "spearman_good_vs_advantage": round(
            spearman([m["p_strong"] - m["p_mistake"] for m in judged], list(advs)), 3
        ),
        "spearman_good_vs_policy_p": round(
            spearman([m["p_strong"] - m["p_mistake"] for m in judged], [m["p"] for m in judged]), 3
        ),
        "disagreements": len(disagree),
    }
    rounds_style = []
    for i, rd in enumerate(rounds):
        j = rd.get("judge", {})
        rounds_style.append(
            {
                "i": i,
                "opponent": rd["opponent"],
                "seat": rd["seat"],
                "seed": rd["seed"],
                "won": rd["won"],
                **rd["numbers"],
                "deliberate": j.get("deliberate", {}).get("score"),
                "risk": j.get("risk", {}).get("score"),
                "pattern": j.get("pattern", {}).get("choice"),
                "pattern_p": j.get("pattern", {}).get("probabilities"),
                "dull": j.get("dull", {}).get("noul"),
            }
        )
    # what the judge's style scores track among the numbers code can count
    def corr(a, b):
        pairs = [(x, y) for x, y in zip(a, b) if x is not None and y is not None]
        return round(spearman([p[0] for p in pairs], [p[1] for p in pairs]), 3) if len(pairs) > 2 else None

    for score in ("deliberate", "risk", "dull"):
        for num in ("casts_per_min", "meditations", "top_act_share", "skills_used"):
            summary[f"spearman_{score}_vs_{num}"] = corr(
                [r[score] for r in rounds_style], [r[num] for r in rounds_style]
            )
    (out / "rounds.json").write_text(json.dumps(rounds))
    (out / "summary.json").write_text(json.dumps({"summary": summary, "style": rounds_style}, indent=1))
    print(json.dumps(summary, indent=1))
    if args.review:
        review(pathlib.Path(args.review).expanduser(), summary, rounds_style, rounds, judged, disagree)
        print(f"review page: {args.review}/index.html")


def review(folder, summary, style, rounds, judged, disagree):
    """The review page: the numbers, each round's style beside what code counts, and the disagreements."""
    folder.mkdir(parents=True, exist_ok=True)
    e = html.escape

    def f(x, n=2):
        return "–" if x is None else f"{x:.{n}f}" if isinstance(x, float) else e(str(x))

    rows = "".join(
        f"<tr><td>{r['i']}</td><td>{e(r['opponent'])}</td><td>{r['seat']}</td>"
        f"<td>{'won' if r['won'] == 1 else 'lost' if r['won'] == -1 else 'cut'}</td><td>{r['seconds']}</td>"
        f"<td>{r['casts_per_min']}</td><td>{r['skills_used']}</td><td>{r['meditations']}</td><td>{r['top_act_share']}</td>"
        f"<td>{f(r['deliberate'])}</td><td>{f(r['risk'])}</td><td>{f(r['pattern'])}</td><td>{f(r['dull'])}</td></tr>"
        for r in style
    )

    def move_row(m, why=""):
        j = m["judge"]
        intent = j.get("intent", {}).get("choice", "–")
        return (
            f"<tr><td>{m['t']}</td><td>{e(m['act_words'])}</td><td>{m['p']:.2f}</td><td>{m['advantage']:+.3f}</td>"
            f"<td>{m['p_strong']:.2f}</td><td>{m['p_mistake']:.2f}</td><td>{e(intent)}</td><td>{e(why)}</td>"
            f"<td><details><summary>moment</summary><pre>{e(m['moment'])}</pre></details></td></tr>"
        )

    dis = "".join(move_row(m, m["why"]) for m in disagree) or "<tr><td colspan=9>none</td></tr>"
    worst = sorted(judged, key=lambda m: -m["p_mistake"])[:15]
    worst_rows = "".join(move_row(m) for m in worst)
    logs = "".join(
        f"<details><summary>round {i} vs {e(rd['opponent'])}</summary><pre>{e(rd.get('log', ''))}</pre></details>"
        for i, rd in enumerate(rounds)
    )
    s = summary
    corr_rows = "".join(
        f"<tr><td>{k.removeprefix('spearman_').replace('_vs_', ' vs ')}</td><td>{f(v)}</td></tr>"
        for k, v in s.items()
        if k.startswith("spearman_") and "_vs_" in k
    )
    page = f"""<!doctype html><meta charset=utf-8><title>Jev judges the house AI</title>
<style>body{{font:14px system-ui;max-width:1200px;margin:2em auto;padding:0 1em;background:#111;color:#ddd}}
table{{border-collapse:collapse;margin:1em 0}}td,th{{border:1px solid #333;padding:3px 6px;vertical-align:top}}
th{{background:#222}}pre{{white-space:pre-wrap;max-width:700px;font-size:12px}}h2{{margin-top:2em}}</style>
<h1>Jev judges the house AI ({e(pathlib.Path(s['checkpoint']).parent.name)}/{e(pathlib.Path(s['checkpoint']).name)})</h1>
<p>TypeSafe's Jev (a decision model on OpenRouter, <code>{MODEL}</code>) read {s['rounds']} rounds the policy fought
({s['decisions']} decisions) and {s['judged_moves']} single moves. {s['requests']} requests, {s['failed']} failed,
latency p50 {f(s['latency_p50'])} s / p90 {f(s['latency_p90'])} s, cost ${s['cost_usd']:.4f}. Nothing here feeds the
training: it is for reading.</p>
<h2>Does Jev agree with the critic?</h2>
<p>Spearman between Jev's "strong − mistake" and the critic's advantage (GAE, the run's own pay):
<b>{f(s['spearman_good_vs_advantage'])}</b>; with the policy's own probability for the act:
<b>{f(s['spearman_good_vs_policy_p'])}</b>. Near 0 means Jev's sense of a good move has nothing to do with what wins
here.</p>
<h2>Style of each round</h2>
<p>Jev's scores (deliberate and risk 0 to 2, dull the probability of yes) beside what code counts. If a Jev score
only follows a count (casts a minute, top act share), it adds nothing a count does not.</p>
<table><tr><th>#</th><th>vs</th><th>seat</th><th>result</th><th>s</th><th>casts/min</th><th>skills</th>
<th>meditations</th><th>top act share</th><th>deliberate</th><th>risk</th><th>pattern</th><th>dull</th></tr>{rows}</table>
<table><tr><th>Jev score vs count (Spearman over rounds)</th><th>ρ</th></tr>{corr_rows}</table>
<h2>Disagreements ({len(disagree)})</h2>
<p>Moves Jev calls a mistake (p ≥ 0.6) that the critic rates in its top quarter, and moves Jev calls strong that the
critic rates in its bottom quarter. One move's advantage is noisy: read the moment before believing either side.</p>
<table><tr><th>t</th><th>act</th><th>policy p</th><th>advantage</th><th>Jev strong</th><th>Jev mistake</th>
<th>intent</th><th>why listed</th><th></th></tr>{dis}</table>
<h2>Jev's 15 surest mistakes</h2>
<table><tr><th>t</th><th>act</th><th>policy p</th><th>advantage</th><th>Jev strong</th><th>Jev mistake</th>
<th>intent</th><th></th><th></th></tr>{worst_rows}</table>
<h2>What Jev read</h2>{logs}
<h2>Doubts</h2><ul>
<li>Jev knows nothing of this game but the words it is given (<code>arena_duel::words</code>): frame data, hitboxes and
what the foe's skills do are not in them.</li>
<li>One move's advantage is one sample of a noisy return; the critic itself is an estimate.</li>
<li>The policy samples its acts, so another seed fights other rounds.</li>
<li>Jev's probabilities are said to be calibrated on its makers' data, not on duels.</li></ul>
"""
    (folder / "index.html").write_text(page)


if __name__ == "__main__":
    main()

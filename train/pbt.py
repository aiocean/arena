"""Population-based training: a few `ppo.py` runs side by side, each with its own reward terms, entropy and learning
rate; every generation the ones that win least take a copy of the ones that win most, and their values are perturbed
(`docs/specs/2026-11-19-model-improvement-design.md`, 4).

    uv run python pbt.py --run pbt1 --init ../crates/arena-duel/weights/policy.onnx \
        [--members 4] [--generations 12] [--every 100] [--threads 8] -- --shaping-until 3.0 --pfsp 2 --mix ...

Member i is the run `<run>/m<i>` (`runs/<run>/m<i>/`, its log and league as any run's), trained with `--seed`
`seed + i`. Member 0 starts on today's recipe, the others on values drawn about it. A generation is `--every`
updates of every member at once (`--threads` shared out among them); then each member fights every other member, the
brain and the house AI over the evaluation seeds, both ways, and its fitness is its share of the rounds it won
(winning, not the shaped reward). The bottom quarter copies a top-quarter member's checkpoint and league, keeps its
own training seeds, and perturbs the copied values (each times 0.8 or 1.25, one in five drawn again).

The state is `runs/<run>/pbt.json`, so the command run again goes on where it stopped; each generation is a line of
`runs/<run>/pbt.jsonl` (the values, the fitness, who copied whom), and its best member is kept as
`runs/<run>/best/g<NNN>.pt` for the pool. Arguments after `--` go to every member's `ppo.py`.
"""

import argparse
import json
import math
import pathlib
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

import numpy as np
import torch

import arena_duel as pd
from pool import onnx_of

HERE = pathlib.Path(__file__).parent
# each value: today's recipe (the house AI's) and the range a fresh draw comes from (log-uniform)
SPACE = {
    "lr": (1e-4, 5e-5, 2e-4),
    "ent": (0.002, 0.0008, 0.005),
    "idle": (0.004, 0.001, 0.012),
    "land": (0.04, 0.01, 0.12),
    "whiff": (0.01, 0.003, 0.03),
    "whiff-far": (0.02, 0.005, 0.05),
    "variety": (0.5, 0.1, 1.0),
    "time-cost": (0.0005, 0.0001, 0.002),
}
EVAL_SEEDS = (500_000, 500_020)
REFERENCES = ["brain", "house"]


def draw(rng, key):
    _, lo, hi = SPACE[key]
    return float(math.exp(rng.uniform(math.log(lo), math.log(hi))))


def perturb(rng, hp, resample=0.2):
    """Each value times 0.8 or 1.25, kept in twice its range; one in five drawn again."""
    out = {}
    for k, v in hp.items():
        _, lo, hi = SPACE[k]
        v = draw(rng, k) if rng.random() < resample else v * float(rng.choice([0.8, 1.25]))
        out[k] = float(min(max(v, lo / 2), hi * 2))
    return out


def flags(hp):
    """A member's values as `ppo.py` arguments (the entropy falls to three quarters of its value over the run)."""
    out = []
    for k, v in hp.items():
        out += ["--ent", f"{v:.6g}", f"{0.75 * v:.6g}"] if k == "ent" else [f"--{k}", f"{v:.6g}"]
    return out


def fitness(bots, threads, seeds=EVAL_SEEDS):
    """Each member's share of the rounds it won against every other member and the references."""
    names = list(bots)
    members = [n for n in names if n not in REFERENCES]
    pairs = [(a, b) for i, a in enumerate(members) for b in names[i + 1 :]]  # members come first in `names`

    def game(p):
        return p, pd.bout(bots[p[0]], bots[p[1]], seeds=seeds)

    won = {m: 0 for m in members}
    played = {m: 0 for m in members}
    with ThreadPoolExecutor(threads) as ex:
        for (a, b), r in ex.map(game, pairs):
            for m, w, l in ((a, r["won"], r["lost"]), (b, r["lost"], r["won"])):
                if m in won:
                    won[m] += w
                    played[m] += w + l
    return {m: round(won[m] / max(played[m], 1), 4) for m in members}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--init", required=True, help="the network every member starts from (.pt or .onnx)")
    ap.add_argument("--members", type=int, default=4)
    ap.add_argument("--generations", type=int, default=12)
    ap.add_argument("--every", type=int, default=100, help="updates a generation (a multiple of 25)")
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("ppo", nargs=argparse.REMAINDER, help="after --: arguments for every member's ppo.py")
    args = ap.parse_args()
    extra = [a for a in args.ppo if a != "--"]
    root = HERE / "runs" / args.run
    root.mkdir(parents=True, exist_ok=True)
    state_file = root / "pbt.json"
    rng = np.random.default_rng(args.seed)
    if state_file.exists():
        state = json.loads(state_file.read_text())
    else:
        hps = [{k: v[0] for k, v in SPACE.items()}] + [
            {k: draw(rng, k) for k in SPACE} for _ in range(args.members - 1)
        ]
        state = {"generation": 0, "members": [{"hp": hp, "seed": args.seed * 100 + i} for i, hp in enumerate(hps)]}
    total = args.generations * args.every
    per = max(1, args.threads // len(state["members"]))
    house = bytes(pd.HOUSE)

    while state["generation"] < args.generations:
        g = state["generation"]
        procs = []
        for i, m in enumerate(state["members"]):
            run = f"{args.run}/m{i}"
            last = root / f"m{i}" / "last.pt"
            cmd = [sys.executable, "ppo.py", "--run", run, "--updates", str(total), "--stop-at", str((g + 1) * args.every)]
            cmd += ["--seed", str(m["seed"]), "--threads", str(per)]
            cmd += ["--resume", str(last)] if last.exists() else ["--init", args.init]
            (root / f"m{i}").mkdir(exist_ok=True)
            log = open(root / f"m{i}" / "stdout.log", "a")
            procs.append(subprocess.Popen(cmd + flags(m["hp"]) + extra, cwd=HERE, stdout=log, stderr=subprocess.STDOUT))
        codes = [p.wait() for p in procs]
        if any(codes):
            raise SystemExit(f"generation {g}: a member failed ({codes}); see runs/{args.run}/m*/stdout.log")

        bots = {f"m{i}": onnx_of(root / f"m{i}" / "last.pt") for i in range(len(state["members"]))}
        fit = fitness({**bots, "brain": "brain", "house": house}, args.threads)
        order = sorted(fit, key=lambda m: -fit[m])
        best = order[0]
        (root / "best").mkdir(exist_ok=True)
        shutil.copy(root / best / "last.pt", root / "best" / f"g{g + 1:03d}.pt")

        q = max(1, len(order) // 4)
        copied = {}
        for loser in order[-q:]:
            winner = order[int(rng.integers(0, q))]
            li, wi = int(loser[1:]), int(winner[1:])
            mine = torch.load(root / loser / "last.pt", map_location="cpu", weights_only=True)
            ck = torch.load(root / winner / "last.pt", map_location="cpu", weights_only=True)
            ck["next_seed"] = mine["next_seed"]  # its own training seeds go on
            torch.save(ck, root / loser / "last.pt")
            shutil.rmtree(root / loser / "league", ignore_errors=True)
            shutil.copytree(root / winner / "league", root / loser / "league")
            state["members"][li]["hp"] = perturb(rng, state["members"][wi]["hp"])
            copied[loser] = winner

        line = {
            "generation": g + 1,
            "update": (g + 1) * args.every,
            "fitness": fit,
            "best": best,
            "copied": copied,
            "hp": {f"m{i}": m["hp"] for i, m in enumerate(state["members"])},
        }
        print(json.dumps(line), flush=True)
        with open(root / "pbt.jsonl", "a") as f:
            f.write(json.dumps(line) + "\n")
        state["generation"] = g + 1
        state_file.write_text(json.dumps(state, indent=1))
    print("PBT-DONE", flush=True)


if __name__ == "__main__":
    main()

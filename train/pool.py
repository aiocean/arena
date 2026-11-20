"""The pool: models kept on disk, every pair fought over the pool's seeds, and their rating
(`docs/specs/2026-11-19-model-improvement-design.md`, 1).

    uv run python pool.py init                                  # the built-in minds and the house AI
    uv run python pool.py add runs/abi6b/u800.pt runs/abi6b/u1200.pt [--name NAME]
    uv run python pool.py play [--threads 8]                    # every pair not yet fought
    uv run python pool.py table [--json]                        # ratings, the Nash mixture, the cycles
    uv run python pool.py remove NAME

The pool is `runs/pool/` (`--dir` for another): `members.json`, the models under `models/`, the games in
`games.json`. A game is a pair fought over the seeds 500,000 to 500,039 both ways, 3 rounds a fight (`--seeds` at
`init`), apart from the ladder's seeds (1-150) and the trainer's (1,000,000 and up). When the rules change (the
fingerprint: the observation's width and two short reference fights) the games are fought again.

The rating is a Bradley-Terry fit of all the games at once, on the Elo scale with the brain at 1000, with its
standard error; a cut round counts for nobody. `nash` is the member's payoff against the Nash mixture of the
meta-game (its rate against each member, less the rate against it, at most 0); a cycle is three members that beat
one another round, each by 55% or more.
"""

import argparse
import hashlib
import itertools
import json
import pathlib
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed

import numpy as np
import torch

import arena_duel as pd
from net import Policy, _initializers, from_onnx, to_onnx, without_priv
from widen import current, widen

HERE = pathlib.Path(__file__).parent
POOL = HERE / "runs" / "pool"
SEEDS = (500_000, 500_040)
BUILTIN = ["brain", "first", "aggressive", "turtle"]
ANCHOR = "brain"
ELO = 400 / np.log(10)


def load_policy(path):
    """A checkpoint's (`.pt`) or an ONNX model's policy at today's observation: one of a narrower observation (an
    older ABI) is widened with the new inputs at zero, so it plays as it did."""
    path = pathlib.Path(path)
    if path.suffix == ".pt":
        ck = torch.load(path, map_location="cpu", weights_only=True)
        model, size = without_priv(ck["model"]), ck.get("size", {})
    else:
        data = path.read_bytes()
        found = _initializers(data)
        p = from_onnx(data, found["inp.weight"].shape[1], found["pi.weight"].shape[0])
        model = {k: v for k, v in p.state_dict().items()}
        size = dict(width=p.width, hidden=p.blocks[0].up.out_features, blocks=len(p.blocks))
    old, acts = model["inp.weight"].shape[1], model["pi.weight"].shape[0]
    if (old, acts) != (pd.OBS, pd.ACTIONS):
        model = widen(model, old, pd.OBS, acts, pd.ACTIONS)
    policy = Policy(pd.OBS, pd.ACTIONS, **size)
    policy.load_state_dict(model)
    return policy.eval()


def onnx_of(path):
    """A model's ONNX bytes at today's observation."""
    path = pathlib.Path(path)
    if path.suffix == ".onnx":
        return current(path.read_bytes())
    return to_onnx(load_policy(path))


def fingerprint():
    """What the pool's games depend on: the observation, the acts and two short fights of the built-in minds."""
    fights = [pd.bout("brain", b, seeds=(SEEDS[0], SEEDS[0] + 2)) for b in ("first", "aggressive")]
    blob = json.dumps([pd.OBS, pd.ACTIONS, fights], sort_keys=True)
    return f"{pd.OBS}x{pd.ACTIONS}-{hashlib.sha256(blob.encode()).hexdigest()[:12]}"


def name_of(path):
    """A checkpoint's default name: its run and its file, `abi6b/u1200`."""
    path = pathlib.Path(path)
    return f"{path.parent.name}/{path.stem}"


class Pool:
    def __init__(self, root=POOL):
        self.root = pathlib.Path(root)
        self.members = self._read("members.json", {})
        self.state = self._read("games.json", {"fingerprint": None, "seeds": list(SEEDS), "games": {}})
        self.lock = threading.Lock()

    def _read(self, name, default):
        f = self.root / name
        return json.loads(f.read_text()) if f.exists() else default

    def save(self):
        self.root.mkdir(parents=True, exist_ok=True)
        (self.root / "members.json").write_text(json.dumps(self.members, indent=1, sort_keys=True))
        (self.root / "games.json").write_text(json.dumps(self.state, indent=1, sort_keys=True))

    @property
    def seeds(self):
        return tuple(self.state["seeds"])

    def init(self, seeds=SEEDS):
        """The built-in minds and the house AI as shipped (named by its hash: a new house AI is a new member)."""
        if list(seeds) != self.state["seeds"]:
            self.state["games"] = {}  # games over other seeds do not count
            self.state["seeds"] = list(seeds)
        for b in BUILTIN:
            self.members.setdefault(b, {"file": None, "source": "built in"})
        house = bytes(pd.HOUSE)
        self.add_bytes(f"house@{hashlib.sha256(house).hexdigest()[:8]}", house, "weights/policy.onnx")

    def add_bytes(self, name, data, source):
        models = self.root / "models"
        models.mkdir(parents=True, exist_ok=True)
        file = f"{name.replace('/', '_')}.onnx"
        (models / file).write_bytes(data)
        self.members[name] = {"file": file, "source": str(source), "sha": hashlib.sha256(data).hexdigest()[:12]}
        self.save()
        return name

    def add(self, path, name=None):
        return self.add_bytes(name or name_of(path), onnx_of(path), path)

    def remove(self, name):
        self.members.pop(name)
        self.state["games"] = {k: v for k, v in self.state["games"].items() if name not in k.split("\t")}
        self.save()

    def bot(self, name):
        """What `arena_duel.bout` takes for a member: a built-in's name or a model's bytes."""
        file = self.members[name]["file"]
        return name if file is None else (self.root / "models" / file).read_bytes()

    def model_files(self):
        return [self.root / "models" / m["file"] for m in self.members.values() if m["file"]]

    def missing(self):
        """The pairs not yet fought on these rules; all of them once the rules have changed."""
        fp = fingerprint()
        if self.state["fingerprint"] != fp:
            self.state = {"fingerprint": fp, "seeds": self.state["seeds"], "games": {}}
            self.save()
        names = sorted(self.members)
        return [p for p in itertools.combinations(names, 2) if "\t".join(p) not in self.state["games"]]

    def play(self, threads=8, log=print):
        """Fight every missing pair, `threads` at once; each game is saved as it ends."""
        pairs = self.missing()
        log(f"{len(pairs)} games to fight over seeds {self.seeds[0]}-{self.seeds[1] - 1}, both ways")

        def game(pair):
            r = pd.bout(self.bot(pair[0]), self.bot(pair[1]), seeds=self.seeds)
            return pair, [r["won"], r["lost"], r["cut"]]

        with ThreadPoolExecutor(threads) as ex:
            for k, f in enumerate(as_completed([ex.submit(game, p) for p in pairs])):
                (a, b), g = f.result()
                with self.lock:
                    self.state["games"][f"{a}\t{b}"] = g
                    self.save()
                log(f"[{k + 1}/{len(pairs)}] {a} vs {b}: {g[0]}-{g[1]} ({g[2]} cut)")

    def wins(self):
        """The names, and `w[i, j]` the rounds member i won against member j."""
        names = sorted(self.members)
        at = {n: i for i, n in enumerate(names)}
        w = np.zeros((len(names), len(names)))
        for key, (won, lost, _) in self.state["games"].items():
            a, b = key.split("\t")
            if a in at and b in at:
                w[at[a], at[b]] += won
                w[at[b], at[a]] += lost
        return names, w

    def table(self):
        names, w = self.wins()
        elo, se = bradley_terry(w, names.index(ANCHOR) if ANCHOR in names else 0)
        a = payoff(w)
        mix = nash(a)
        vs_nash = a @ mix
        rows = [
            {
                "name": n,
                "elo": round(float(elo[i]), 1),
                "se": round(float(se[i]), 1),
                "nash": round(float(vs_nash[i]), 3),
                "mix": round(float(mix[i]), 3),
                "rate": round(float(w[i].sum() / max(w[i].sum() + w[:, i].sum(), 1)), 3),
                "source": self.members[n]["source"],
            }
            for i, n in enumerate(names)
        ]
        rows.sort(key=lambda r: -r["elo"])
        return {
            "fingerprint": self.state["fingerprint"],
            "seeds": self.seeds,
            "rows": rows,
            "cycles": [[names[i] for i in c] for c in cycles(a)],
        }


def bradley_terry(w, anchor=0, smooth=0.5, iters=100):
    """Bradley-Terry strengths of `w` (`w[i, j]` wins of i over j) on the Elo scale, member `anchor` at 1000, and
    their standard errors. Each pair that met gets `smooth` wins each way, so a member that never lost stays finite;
    a member that met nobody is at 1000 with an infinite error."""
    n = len(w)
    met = (w + w.T) > 0
    w = w + smooth * met
    games = w + w.T
    r = np.zeros(n)
    free = np.array([i != anchor and met[i].any() for i in range(n)])
    hess = np.eye(n)
    for _ in range(iters):
        p = 1 / (1 + np.exp(r[None, :] - r[:, None]))  # p[i, j]: i beats j
        grad = (w - games * p).sum(1)
        q = games * p * (1 - p)
        hess = np.diag(q.sum(1)) - q  # minus the log-likelihood's Hessian
        if not free.any():
            break
        step = np.linalg.solve(hess[np.ix_(free, free)], grad[free])
        r[free] += step
        if np.abs(step).max() < 1e-9:
            break
    se = np.full(n, np.inf)
    se[anchor] = 0.0
    if free.any():
        se[free] = np.sqrt(np.diag(np.linalg.inv(hess[np.ix_(free, free)]))) * ELO
    return 1000 + r * ELO, se


def payoff(w):
    """The meta-game: `a[i, j]` member i's share of the rounds against j less j's, in [-1, 1], 0 where they never met."""
    games = w + w.T
    return np.divide(w - w.T, games, out=np.zeros_like(w), where=games > 0)


def nash(a, iters=20000):
    """A Nash mixture of the symmetric zero-sum game `a`, by regret matching (the average of the strategies)."""
    n = len(a)
    regret, avg = np.zeros(n), np.zeros(n)
    for _ in range(iters):
        pos = np.maximum(regret, 0)
        p = pos / pos.sum() if pos.sum() > 0 else np.full(n, 1 / n)
        avg += p
        u = a @ p
        regret += u - p @ u
    avg /= avg.sum()
    avg[avg < 1e-3] = 0
    return avg / avg.sum()


def cycles(a, edge=0.1):
    """The triples that beat one another round, each by `edge` or more (0.1: 55% of the rounds)."""
    n = len(a)
    beats = a >= edge
    found = []
    for i, j, k in itertools.combinations(range(n), 3):
        if beats[i, j] and beats[j, k] and beats[k, i]:
            found.append((i, j, k))
        elif beats[j, i] and beats[k, j] and beats[i, k]:
            found.append((i, k, j))
    return found


def print_table(t):
    print(f"pool: rules {t['fingerprint']}, seeds {t['seeds'][0]}-{t['seeds'][1] - 1} both ways\n")
    print(f"{'member':<28} {'elo':>7} {'±2se':>6} {'rate':>6} {'nash':>7} {'mix':>5}")
    for r in t["rows"]:
        two = "inf" if r["se"] == float("inf") else f"{2 * r['se']:.0f}"
        print(f"{r['name']:<28} {r['elo']:>7.0f} {two:>6} {r['rate']:>6.3f} {r['nash']:>7.3f} {r['mix']:>5.2f}")
    print(f"\ncycles (each beats the next by 55% or more): {len(t['cycles'])}")
    for c in t["cycles"][:10]:
        print("  " + " > ".join(c) + f" > {c[0]}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=str(POOL))
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("init")
    s.add_argument("--seeds", type=int, nargs=2, default=list(SEEDS))
    s = sub.add_parser("add")
    s.add_argument("paths", nargs="+")
    s.add_argument("--name", help="the member's name (one path only); its run and file by default")
    s = sub.add_parser("play")
    s.add_argument("--threads", type=int, default=8)
    s = sub.add_parser("table")
    s.add_argument("--json", action="store_true")
    s = sub.add_parser("remove")
    s.add_argument("name")
    args = ap.parse_args()
    pool = Pool(args.dir)
    if args.cmd == "init":
        pool.init(tuple(args.seeds))
        print(f"{len(pool.members)} members in {pool.root}")
    elif args.cmd == "add":
        if args.name and len(args.paths) > 1:
            raise SystemExit("--name names one path")
        for p in args.paths:
            print("added", pool.add(p, args.name))
    elif args.cmd == "play":
        pool.play(args.threads)
    elif args.cmd == "table":
        t = pool.table()
        print(json.dumps(t, indent=1)) if args.json else print_table(t)
    elif args.cmd == "remove":
        pool.remove(args.name)


if __name__ == "__main__":
    main()

"""Who the learner fights: its latest self, its past selves, the brain, the first mind, the scripted fighters, the
models of other folders (a pool, another run's league) and a target (`docs/specs/2026-11-19-model-improvement-design.md`,
3). Snapshots are handed to the envs as ONNX models under an id (`VecEnv.set_model`).

With a `dir` each snapshot is also written there (`<n>.onnx`), and `restore` reads them back, so a resumed run keeps
its past selves and another run can fight them. With `pfsp` k > 0 a past self or a pool model is drawn with weight
(1 - p)^k, p the learner's recent share of wins against it: what it always beats stops taking fights."""

import pathlib
from collections import defaultdict, deque

import numpy as np

from net import to_onnx
from widen import current

# opponent shares: latest self, a past self, the brain, the first mind, the scripted pair (either, drawn), a pool
# model, the target, and each script alone (to lean on one the policy is weak against)
MIX = {
    "latest": 0.35,
    "past": 0.25,
    "brain": 0.15,
    "first": 0.10,
    "scripted": 0.15,
    "pool": 0.0,
    "target": 0.0,
    "aggressive": 0.0,
    "turtle": 0.0,
}
OLD_ORDER = ["latest", "past", "brain", "first", "scripted"]  # five numbers of `--mix`, as before
STYLES = 3
POOL_IDS, TARGET_IDS = 1_000_000, 2_000_000  # model ids of pool models and targets, apart from the snapshots'


def parse_mix(items):
    """`--mix` as the shares in `MIX`'s order: five numbers (latest, past, brain, first, scripted) or `kind=share`
    pairs (kinds left out get 0)."""
    if items is None:
        return None
    if all("=" not in x for x in items):
        if len(items) != len(OLD_ORDER):
            raise ValueError(f"--mix: {len(OLD_ORDER)} numbers ({' '.join(OLD_ORDER)}) or kind=share pairs")
        shares = dict(zip(OLD_ORDER, map(float, items)))
    else:
        shares = {}
        for x in items:
            k, _, v = x.partition("=")
            if k not in MIX:
                raise ValueError(f"--mix: no kind {k!r} ({', '.join(MIX)})")
            shares[k] = float(v)
    return [shares.get(k, 0.0) for k in MIX]


class League:
    def __init__(self, env, rng, limit=90.0, seed=1_000_000, seat0=0.5, mix=None, dir=None, pfsp=0.0, pools=(), target=None):
        """`seat0`: the share of fights in which the learner sits in seat 0 (the left); seat 1 the rest. `mix`: the
        opponent shares in `MIX`'s order, `MIX` itself when None. `dir`: where snapshots are kept. `pools`: folders
        whose `*.onnx` are the pool's models; `target`: a model, or a folder whose newest `*.onnx` is the target."""
        self.env, self.rng, self.limit, self.seat0 = env, rng, limit, seat0
        self.snapshots = []  # model ids, oldest first
        self.next_seed = seed  # training fights never use the ladder's seeds (1-150)
        self.kinds = list(MIX)
        self.p = np.array(mix if mix is not None else [MIX[k] for k in self.kinds], dtype=float)
        self.p /= self.p.sum()
        self.dir = pathlib.Path(dir) if dir else None
        self.pfsp = pfsp
        self.pools = [pathlib.Path(d) for d in pools]
        self.target_path = pathlib.Path(target) if target else None
        self.pool = {}  # model id -> its file
        self.seen = {}  # a pool file -> (its mtime, its model id)
        self.target = None  # (file, mtime, model id)
        self.results = defaultdict(lambda: deque(maxlen=200))  # opponent key -> recent outcomes (1 won)

    def snapshot(self, policy):
        """Freeze the policy as it is now: the latest self from here on; kept in `dir` if there is one."""
        data = to_onnx(policy)
        wid = len(self.snapshots)
        self.env.set_model(wid, data)
        self.snapshots.append(wid)
        if self.dir:
            self.dir.mkdir(parents=True, exist_ok=True)
            tmp = self.dir / f".{wid:05d}.tmp"
            tmp.write_bytes(data)
            tmp.rename(self.dir / f"{wid:05d}.onnx")

    def restore(self):
        """Seat the snapshots kept in `dir` as the past selves (a resumed run); returns how many."""
        if not self.dir or not self.dir.exists():
            return 0
        for f in sorted(self.dir.glob("*.onnx")):
            wid = len(self.snapshots)
            self.env.set_model(wid, current(f.read_bytes()))
            self.snapshots.append(wid)
        return len(self.snapshots)

    def refresh(self):
        """Read the pool's folders and the target again: a new or changed file is seated under a fresh id."""
        for d in self.pools:
            for f in sorted(d.glob("*.onnx")) if d.exists() else []:
                mtime = f.stat().st_mtime
                if self.seen.get(f, (None,))[0] == mtime:
                    continue
                wid = POOL_IDS + len(self.seen)
                try:
                    self.env.set_model(wid, current(f.read_bytes()))
                except ValueError:
                    continue  # a model of another observation: not this run's opponent
                old = self.seen.get(f)
                if old:
                    self.pool.pop(old[1], None)
                self.seen[f] = (mtime, wid)
                self.pool[wid] = f
        t = self.target_path
        if t is None:
            return
        files = sorted(t.glob("*.onnx")) if t.is_dir() else [t] if t.exists() else []
        if not files:
            return
        f = files[-1]
        mtime = f.stat().st_mtime
        if self.target and self.target[:2] == (f, mtime):
            return
        wid = TARGET_IDS + (self.target[2] - TARGET_IDS + 1 if self.target else 0)
        self.env.set_model(wid, current(f.read_bytes()))
        self.target = (f, mtime, wid)

    def weight(self, key):
        """PFSP's weight for an opponent: (1 - p)^k, p the learner's recent share of wins against it (a prior of one
        win and one loss); 1 when PFSP is off."""
        if self.pfsp <= 0:
            return 1.0
        r = self.results[key]
        p = (sum(r) + 1) / (len(r) + 2)
        return max((1 - p) ** self.pfsp, 1e-3)

    def pick(self, keys):
        """One of `keys`, by PFSP's weights."""
        w = np.array([self.weight(k) for k in keys])
        return keys[int(self.rng.choice(len(keys), p=w / w.sum()))]

    def record(self, key, won):
        """The learner won (True) or not a fight against the opponent `key` (`reset`'s fourth answer)."""
        self.results[key].append(float(won))

    def reset(self, i):
        """Start env i on a fresh fight against an opponent drawn from the mix; returns the opponent's kind, the
        learner's style, its seat (0 left, 1 right) and the opponent's key (for `record`)."""
        kind = self.kinds[self.rng.choice(len(self.kinds), p=self.p)]
        if kind == "past" and len(self.snapshots) < 2 or kind == "pool" and not self.pool or kind == "target" and not self.target:
            kind = "latest"
        wid = -1
        if kind == "latest":
            name, wid = "policy", self.snapshots[-1]
            key = f"self:{wid}"
        elif kind == "past":
            key = self.pick([f"self:{w}" for w in self.snapshots[:-1]])
            name, wid = "policy", int(key.split(":")[1])
        elif kind == "pool":
            by_key = {f"pool:{f.parent.name}/{f.stem}": w for w, f in sorted(self.pool.items())}
            key = self.pick(list(by_key))
            name, wid = "policy", by_key[key]
        elif kind == "target":
            name, wid, key = "policy", self.target[2], "target"
        elif kind == "scripted":
            name = ["aggressive", "turtle"][int(self.rng.integers(0, 2))]
            kind = key = name
        else:
            name = key = kind
        seed = self.next_seed
        self.next_seed += 1
        learner = 0 if self.rng.random() < self.seat0 else 1
        style, foe_style = (int(x) for x in self.rng.integers(0, STYLES, 2))
        self.env.reset(i, seed, learner, style, foe_style, name, wid, self.limit)
        return kind, style, learner, key

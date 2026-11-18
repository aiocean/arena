"""Who the learner fights: its latest self, its past selves, the brain, the first mind and the scripted fighters.
Snapshots are handed to the envs as ONNX models under an id (`VecEnv.set_model`)."""

import numpy as np

from net import to_onnx

# opponent shares: latest self, a past self, the brain, the first mind, the scripted pair
MIX = {"latest": 0.35, "past": 0.25, "brain": 0.15, "first": 0.10, "scripted": 0.15}
STYLES = 3


class League:
    def __init__(self, env, rng, limit=90.0, seed=1_000_000, seat0=0.5, mix=None):
        """`seat0`: the share of fights in which the learner sits in seat 0 (the left); seat 1 the rest. `mix`: the
        opponent shares in `MIX`'s order (latest, past, brain, first, scripted), `MIX` itself when None."""
        self.env, self.rng, self.limit, self.seat0 = env, rng, limit, seat0
        self.snapshots = []  # model ids, oldest first
        self.next_seed = seed  # training fights never use the ladder's seeds (1-150)
        self.kinds = list(MIX)
        self.p = np.array(mix if mix is not None else [MIX[k] for k in self.kinds], dtype=float)
        self.p /= self.p.sum()

    def snapshot(self, policy):
        """Freeze the policy as it is now: the latest self from here on."""
        wid = len(self.snapshots)
        self.env.set_model(wid, to_onnx(policy))
        self.snapshots.append(wid)

    def reset(self, i):
        """Start env i on a fresh fight against an opponent drawn from the mix; returns the opponent's name, the
        learner's style and its seat (0 left, 1 right)."""
        kind = self.kinds[self.rng.choice(len(self.kinds), p=self.p)]
        wid = -1
        if kind == "latest" or (kind == "past" and len(self.snapshots) < 2):
            name, wid = "policy", self.snapshots[-1]
            kind = "latest"
        elif kind == "past":
            name, wid = "policy", self.snapshots[int(self.rng.integers(0, len(self.snapshots) - 1))]
        elif kind == "scripted":
            name = ["aggressive", "turtle"][int(self.rng.integers(0, 2))]
            kind = name
        else:
            name = kind
        seed = self.next_seed
        self.next_seed += 1
        learner = 0 if self.rng.random() < self.seat0 else 1
        style, foe_style = (int(x) for x in self.rng.integers(0, STYLES, 2))
        self.env.reset(i, seed, learner, style, foe_style, name, wid, self.limit)
        return kind, style, learner

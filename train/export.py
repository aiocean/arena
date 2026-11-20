"""Write a policy as the ONNX model the arena ships (`crates/arena-duel/weights/policy.onnx`: the house AI, the
model `bots/onnx` builds into the house bot module) and the check its Rust test reads: 32 observations from real
fights, their masks, and PyTorch's logits for them.

    uv run python export.py runs/<run>/last.pt                               # a checkpoint
    uv run python export.py ../crates/arena-duel/weights/policy.onnx         # re-check a model as it is
"""

import argparse
import json
import pathlib

import numpy as np
import torch

import arena_duel as pd
from net import Policy, from_onnx, to_onnx, without_priv
from widen import current, widen

HERE = pathlib.Path(__file__).parent
HOUSE = HERE / "../crates/arena-duel/weights/policy.onnx"
CHECK = HERE / "../crates/arena-duel/tests/fixtures/policy_check.json"


def observations(n=32, seed=424242):
    """n observations, with their masks, from fights a few seconds in against each opponent."""
    env = pd.VecEnv(n, 4)
    opponents = ["brain", "first", "aggressive", "turtle"]
    for i in range(n):
        env.reset(i, seed + i, i % 2, i % 3, (i + 2) % 3, opponents[i % 4], -1, 90.0)
    rng = np.random.default_rng(seed)
    for _ in range(int(rng.integers(20, 40))):
        obs, mask = env.observe()
        actions = np.where(mask, rng.random(mask.shape), -1).argmax(1).astype(np.int64)
        env.step(actions, 0.0)
    return env.observe()


def load(path):
    """A checkpoint's policy (at the size it was trained) or an ONNX model's."""
    path = pathlib.Path(path)
    if path.suffix == ".pt":
        ck = torch.load(path, map_location="cpu", weights_only=True)
        policy = Policy(pd.OBS, pd.ACTIONS, **ck.get("size", {}))
        model = without_priv(ck["model"])
        old, acts = model["inp.weight"].shape[1], model["pi.weight"].shape[0]
        if (old, acts) != (pd.OBS, pd.ACTIONS):  # an older ABI's checkpoint, widened: it plays as it did
            model = widen(model, old, pd.OBS, acts, pd.ACTIONS)
        policy.load_state_dict(model)
    else:
        policy = from_onnx(current(path.read_bytes()), pd.OBS, pd.ACTIONS)
    return policy.eval()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("source")
    ap.add_argument("--out", default=str(HOUSE), help="the ONNX model's path")
    ap.add_argument("--check", default=str(CHECK))
    args = ap.parse_args()
    policy = load(args.source)
    data = to_onnx(policy)
    pathlib.Path(args.out).write_bytes(data)
    obs, mask = observations()
    with torch.no_grad():
        logits, _ = policy(torch.from_numpy(obs), torch.ones(mask.shape, dtype=torch.bool))
    cases = [
        {"obs": [float(x) for x in o], "mask": [bool(m) for m in k], "logits": [float(x) for x in l]}
        for o, k, l in zip(obs, mask, logits.numpy())
    ]
    pathlib.Path(args.check).parent.mkdir(parents=True, exist_ok=True)
    pathlib.Path(args.check).write_text(json.dumps({"source": pathlib.Path(args.source).name, "cases": cases}))
    print(f"wrote {args.out} ({len(data)} bytes) and {args.check} ({len(cases)} cases)")


if __name__ == "__main__":
    main()

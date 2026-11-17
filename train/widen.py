"""Carry a policy over to a wider observation: an ABI that appends inputs keeps the old ones at their indices, so the
old network's first layer is copied into the new one's first columns and the new columns start at zero. With the new
inputs at zero the widened policy acts exactly as the old one did; training then learns to use them.

    uv run python widen.py runs/abi2b/u1510.pt runs/abi3/init.pt --old-obs 305     # a checkpoint (.pt)

The output is a checkpoint `ppo.py --init` reads (its network and size; a fresh optimiser and schedule).
"""

import argparse
import pathlib

import torch

import arena_duel as pd
from net import Policy


def widen(model, old_obs, new_obs):
    """The state dict `model` (a `Policy` of `old_obs` inputs) as one of `new_obs` inputs, the new columns zero."""
    if new_obs < old_obs:
        raise ValueError(f"a wider observation has more inputs: {new_obs} < {old_obs}")
    out = dict(model)
    w = model["inp.weight"]
    if w.shape[1] != old_obs:
        raise ValueError(f"the model reads {w.shape[1]} inputs, not {old_obs}")
    wide = torch.zeros(w.shape[0], new_obs, dtype=w.dtype)
    wide[:, :old_obs] = w
    out["inp.weight"] = wide
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src", help="a ppo.py checkpoint (.pt) of the old observation")
    ap.add_argument("out")
    ap.add_argument("--old-obs", type=int, required=True)
    args = ap.parse_args()
    ck = torch.load(args.src, map_location="cpu", weights_only=True)
    size = ck.get("size", {})
    model = widen(ck["model"], args.old_obs, pd.OBS)
    policy = Policy(pd.OBS, pd.ACTIONS, **size)
    policy.load_state_dict(model)
    old = Policy(args.old_obs, pd.ACTIONS, **size)
    old.load_state_dict(ck["model"])
    # the check: on random observations with the new inputs zero, both give the same logits
    torch.manual_seed(0)
    x = torch.randn(256, args.old_obs)
    mask = torch.ones(256, pd.ACTIONS, dtype=torch.bool)
    with torch.no_grad():
        a, _ = old(x, mask)
        b, _ = policy(torch.cat([x, torch.zeros(256, pd.OBS - args.old_obs)], 1), mask)
    gap = (a - b).abs().max().item()
    if gap > 1e-5:
        raise SystemExit(f"the widened policy differs from the old one by {gap}")
    pathlib.Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    torch.save({"model": policy.state_dict(), "size": size}, args.out)
    print(f"wrote {args.out}: {args.old_obs} -> {pd.OBS} inputs, logits equal within {gap:.2e}")


if __name__ == "__main__":
    main()

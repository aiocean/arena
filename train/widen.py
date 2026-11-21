"""Carry a policy over to a wider observation and more acts: an ABI that appends inputs or acts keeps the old ones at
their indices, so the old network's first layer is copied into the new one's first columns (the new columns start at
zero) and its act head into the first rows (a new act's row starts at zero with a bias `NEW_ACT` below the rest).
With the new inputs at zero the widened policy gives the old acts exactly the old logits and almost never picks a new
one; training then learns to use them.

    uv run python widen.py runs/abi2b/u1510.pt runs/abi3/init.pt --old-obs 305                    # inputs only
    uv run python widen.py ../crates/arena-duel/weights/policy.onnx runs/abi5/init.pt --old-obs 1889 --old-actions 27

The source is a checkpoint (.pt) or an ONNX model `to_onnx` wrote (its value head then starts fresh). The output is
a checkpoint `ppo.py --init` reads (its network and size; a fresh optimiser and schedule).
"""

import argparse
import pathlib

import torch

import arena_duel as pd
from net import Policy, _initializers, from_onnx, size_of, to_onnx, without_priv

# a new act's logit below the old ones' bias: e^-20 of the mass, so the widened policy plays as it did
NEW_ACT = 20.0


def widen(model, old_obs, new_obs, old_actions=None, new_actions=None):
    """The state dict `model` (a `Policy` of `old_obs` inputs and `old_actions` acts) as one of `new_obs` inputs and
    `new_actions` acts: the new input columns zero, the new acts' rows zero with a bias `NEW_ACT` below the lowest."""
    if new_obs < old_obs:
        raise ValueError(f"a wider observation has more inputs: {new_obs} < {old_obs}")
    out = dict(model)
    w = model["inp.weight"]
    if w.shape[1] != old_obs:
        raise ValueError(f"the model reads {w.shape[1]} inputs, not {old_obs}")
    wide = torch.zeros(w.shape[0], new_obs, dtype=w.dtype)
    wide[:, :old_obs] = w
    out["inp.weight"] = wide
    old_actions = old_actions or model["pi.weight"].shape[0]
    new_actions = new_actions or old_actions
    if new_actions < old_actions:
        raise ValueError(f"more acts, not fewer: {new_actions} < {old_actions}")
    pw, pb = model["pi.weight"], model["pi.bias"]
    if pw.shape[0] != old_actions:
        raise ValueError(f"the model gives {pw.shape[0]} acts, not {old_actions}")
    rows = torch.zeros(new_actions, pw.shape[1], dtype=pw.dtype)
    rows[:old_actions] = pw
    bias = torch.full((new_actions,), float(pb.min()) - NEW_ACT, dtype=pb.dtype)
    bias[:old_actions] = pb
    out["pi.weight"], out["pi.bias"] = rows, bias
    return out


def sizes(data):
    """An ONNX model's inputs and acts (`to_onnx`'s layout)."""
    found = _initializers(data)
    return found["inp.weight"].shape[1], found["pi.weight"].shape[0]


def current(data):
    """ONNX bytes at today's observation and acts: a model of an older ABI widened (it plays as it did), today's
    as it is."""
    try:
        obs, actions = sizes(data)
    except Exception:  # not a model `to_onnx` wrote: handed on as it is, for the arena to judge
        return data
    if (obs, actions) == (pd.OBS, pd.ACTIONS):
        return data
    p = from_onnx(data, obs, actions)
    size = size_of(p)
    policy = Policy(pd.OBS, pd.ACTIONS, **size)
    policy.load_state_dict(widen(p.state_dict(), obs, pd.OBS, actions, pd.ACTIONS))
    return to_onnx(policy)


def source(path, old_obs, old_actions):
    """A checkpoint's state dict and size, or an ONNX model's (its value head fresh)."""
    path = pathlib.Path(path)
    if path.suffix == ".pt":
        ck = torch.load(path, map_location="cpu", weights_only=True)
        return without_priv(ck["model"]), ck.get("size", {})
    p = from_onnx(path.read_bytes(), old_obs, old_actions)
    size = size_of(p)
    return p.state_dict(), size


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src", help="a ppo.py checkpoint (.pt) or an ONNX model of the old observation")
    ap.add_argument("out")
    ap.add_argument("--old-obs", type=int, required=True)
    ap.add_argument("--old-actions", type=int, default=pd.ACTIONS, help="the old model's acts (ABI 4: 27)")
    args = ap.parse_args()
    state, size = source(args.src, args.old_obs, args.old_actions)
    model = widen(state, args.old_obs, pd.OBS, args.old_actions, pd.ACTIONS)
    policy = Policy(pd.OBS, pd.ACTIONS, **size)
    policy.load_state_dict(model)
    old = Policy(args.old_obs, args.old_actions, **size)
    old.load_state_dict(state)
    # the check: on random observations with the new inputs zero, the old acts' logits are the same and the new
    # acts' far below
    torch.manual_seed(0)
    x = torch.randn(256, args.old_obs)
    with torch.no_grad():
        a, _ = old(x, torch.ones(256, args.old_actions, dtype=torch.bool))
        b, _ = policy(
            torch.cat([x, torch.zeros(256, pd.OBS - args.old_obs)], 1), torch.ones(256, pd.ACTIONS, dtype=torch.bool)
        )
    gap = (a - b[:, : args.old_actions]).abs().max().item()
    if gap > 1e-5:
        raise SystemExit(f"the widened policy differs from the old one by {gap}")
    if pd.ACTIONS > args.old_actions:
        new_mass = torch.softmax(b, -1)[:, args.old_actions :].sum(-1).max().item()
        if new_mass > 1e-4:
            raise SystemExit(f"the new acts take {new_mass} of the mass")
    pathlib.Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    torch.save({"model": policy.state_dict(), "size": size}, args.out)
    print(f"wrote {args.out}: {args.old_obs} -> {pd.OBS} inputs, {args.old_actions} -> {pd.ACTIONS} acts, logits equal within {gap:.2e}")


if __name__ == "__main__":
    main()

"""Widening a policy to more inputs keeps what it does: with the new inputs at zero it gives the same logits."""

import torch

import arena_duel as pd
from net import Policy, from_onnx, to_onnx
from widen import current, sizes, widen


def test_a_widened_policy_acts_as_the_old_one_on_the_old_inputs():
    torch.manual_seed(3)
    old = Policy(20, 7, width=16, hidden=32, blocks=1)
    new = Policy(31, 7, width=16, hidden=32, blocks=1)
    new.load_state_dict(widen(old.state_dict(), 20, 31))
    x = torch.randn(64, 20)
    mask = torch.ones(64, 7, dtype=torch.bool)
    with torch.no_grad():
        a, va = old(x, mask)
        b, vb = new(torch.cat([x, torch.zeros(64, 11)], 1), mask)
    assert torch.allclose(a, b, atol=1e-6)
    assert torch.allclose(va, vb, atol=1e-6)
    # the new inputs start with no weight at all
    assert new.inp.weight[:, 20:].abs().sum() == 0


def test_an_abi_4_model_plays_as_it_did_once_widened():
    torch.manual_seed(3)
    old = Policy(1889, 27).eval()
    data = current(to_onnx(old))
    assert sizes(data) == (pd.OBS, pd.ACTIONS)
    new = from_onnx(data, pd.OBS, pd.ACTIONS).eval()
    x = torch.randn(64, 1889)
    with torch.no_grad():
        a, _ = old(x, torch.ones(64, 27, dtype=torch.bool))
        b, _ = new(torch.cat([x, torch.zeros(64, pd.OBS - 1889)], 1), torch.ones(64, pd.ACTIONS, dtype=torch.bool))
    assert (a - b[:, :27]).abs().max() < 1e-4
    assert torch.softmax(b, -1)[:, 27:].sum(-1).max() < 1e-4


def test_a_model_of_today_is_handed_on_as_it_is():
    data = to_onnx(Policy(pd.OBS, pd.ACTIONS).eval())
    assert current(data) == data
    assert current(b"not a model") == b"not a model"

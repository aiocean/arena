"""Widening a policy to more inputs keeps what it does: with the new inputs at zero it gives the same logits."""

import torch

from net import Policy
from widen import widen


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

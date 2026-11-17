"""The trainer's network: any size, written to and read from ONNX, and a critic trained by HL-Gauss."""

import numpy as np
import pytest
import torch

import pixfx_duel as pd
from net import Policy, from_onnx, hl_gauss, load_onnx, to_onnx


def test_forward_shapes_and_mask():
    p = Policy(pd.OBS, pd.ACTIONS)
    obs = torch.randn(5, pd.OBS)
    mask = torch.zeros(5, pd.ACTIONS, dtype=torch.bool)
    mask[:, 0] = True
    mask[2, 7] = True
    logits, value = p(obs, mask)
    assert logits.shape == (5, pd.ACTIONS) and value.shape == (5, p.bins)
    probs = torch.softmax(logits, -1)
    assert torch.all(probs[~mask] < 1e-12)
    assert torch.allclose(probs.sum(-1), torch.ones(5))


def test_hl_gauss_targets_sum_to_one_and_centre_on_the_return():
    p = Policy(pd.OBS, pd.ACTIONS)
    target = hl_gauss(torch.tensor([-1.0, 0.0, 0.37, 1.4]), p.support, p.sigma)
    assert torch.allclose(target.sum(-1), torch.ones(4), atol=1e-5)
    centres = (p.support[:-1] + p.support[1:]) / 2
    assert torch.allclose((target * centres).sum(-1), torch.tensor([-1.0, 0.0, 0.37, 1.4]), atol=0.02)


def test_an_onnx_model_round_trips_at_any_size():
    for size in [{}, dict(width=64, hidden=96, blocks=3)]:
        p = Policy(pd.OBS, pd.ACTIONS, **size)
        q = from_onnx(to_onnx(p), pd.OBS, pd.ACTIONS)
        assert [t.shape for t in q.parameters()] == [t.shape for t in p.parameters()]
        obs = torch.randn(3, pd.OBS)
        mask = torch.ones(3, pd.ACTIONS, dtype=torch.bool)
        assert torch.equal(p(obs, mask)[0], q(obs, mask)[0])


def test_a_model_of_another_size_is_refused():
    with pytest.raises(ValueError, match="tensor"):
        load_onnx(Policy(pd.OBS, pd.ACTIONS), to_onnx(Policy(pd.OBS, pd.ACTIONS, width=64)))


def test_the_house_ai_loads():
    q = from_onnx(pd.HOUSE, pd.OBS, pd.ACTIONS)
    logits, _ = q(torch.zeros(1, pd.OBS), torch.ones(1, pd.ACTIONS, dtype=torch.bool))
    assert np.isfinite(logits.detach().numpy()).all()

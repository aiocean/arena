"""The trainer's network: any size, written to and read from ONNX, and a critic trained by HL-Gauss."""

import numpy as np
import pytest
import torch

import arena_duel as pd
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


# the observation's blocks and their lengths (`crates/arena-mind/src/policy/observe.rs`, `BLOCKS`, ABI 5)
BLOCKS = [
    ("Me", 17), ("Foe", 37), ("Threats", 48), ("Slots", 198), ("Tag", 5), ("My cast", 11), ("Foe's cast", 11),
    ("Slot builds", 888), ("Effects", 226), ("Units", 96), ("Foe's slots", 204), ("Why", 54), ("Foe's bands", 24),
    ("Buffs", 20), ("In play", 50), ("My acts", 90), ("Foe's acts", 76),
    ("My older acts", 150), ("Foe's older acts", 76), ("Foe's kinds", 18),
]


def test_each_entity_encoder_reads_whole_blocks_of_the_observation():
    """Every range an entity reads is a block of `observe.rs`, tiled exactly by the entity's copies."""
    import arena_duel as pd
    from net import entities_of, entity_index

    assert sum(n for _, n in BLOCKS) == pd.OBS
    start, at = {}, 0
    for name, n in BLOCKS:
        start[at] = n
        at += n
    for name, copies, ranges in entities_of(pd.OBS):
        idx = entity_index(copies, ranges)
        assert idx.shape == (copies, sum(n for _, n, _ in ranges))
        for first, n, stride in ranges:
            assert n == stride and start.get(first) == copies * n, (name, first)
        assert len(set(idx.flatten().tolist())) == idx.numel()


def test_a_policy_with_entity_encoders_round_trips_through_onnx():
    """`Policy(entities=...)`: its ONNX export loads back at its size (`from_onnx`, `size_of`) with the same logits."""
    import torch

    import arena_duel as pd
    from net import Policy, from_onnx, size_of, to_onnx

    torch.manual_seed(0)
    p = Policy(pd.OBS, pd.ACTIONS, width=64, hidden=96, blocks=3, entities=16)
    q = from_onnx(to_onnx(p), pd.OBS, pd.ACTIONS)
    assert size_of(q) == size_of(p) == {"width": 64, "hidden": 96, "blocks": 3, "entities": 16}
    x, m = torch.randn(5, pd.OBS), torch.ones(5, pd.ACTIONS, dtype=torch.bool)
    assert torch.allclose(p(x, m)[0], q(x, m)[0])


def test_a_policy_with_attention_round_trips_through_onnx_and_tract():
    """`Policy(attention=heads)`: the entity codes attend to one another; the ONNX export (heads in its metadata) loads
    back at its size with the same logits, and the arena's own runner (tract, `VecEnv.set_model`) takes it."""
    import torch

    import arena_duel as pd
    from net import Policy, from_onnx, size_of, to_onnx

    torch.manual_seed(0)
    p = Policy(pd.OBS, pd.ACTIONS, width=64, hidden=96, blocks=2, entities=16, attention=4)
    with torch.no_grad():
        p.ent.place.normal_()  # not the zeros it starts at, so the round trip is tested
    data = to_onnx(p)
    q = from_onnx(data, pd.OBS, pd.ACTIONS)
    assert size_of(q) == size_of(p) == {"width": 64, "hidden": 96, "blocks": 2, "entities": 16, "attention": 4}
    x, m = torch.randn(5, pd.OBS), torch.ones(5, pd.ACTIONS, dtype=torch.bool)
    assert torch.allclose(p(x, m)[0], q(x, m)[0], atol=1e-5)
    env = pd.VecEnv(2, 1)
    env.set_model(9, data)  # tract refuses a graph it cannot run

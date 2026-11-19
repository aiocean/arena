"""PBT's values: perturbed within their bounds, and handed to ppo.py as its flags."""

import numpy as np

from pbt import SPACE, flags, perturb


def test_perturbed_values_stay_within_twice_their_range():
    rng = np.random.default_rng(3)
    hp = {k: v[0] for k, v in SPACE.items()}
    for _ in range(200):
        hp = perturb(rng, hp)
        for k, v in hp.items():
            assert SPACE[k][1] / 2 <= v <= SPACE[k][2] * 2


def test_flags_name_ppo_arguments_and_the_entropy_falls_to_three_quarters():
    f = flags({"lr": 1e-4, "ent": 0.002, "whiff-far": 0.02})
    assert f == ["--lr", "0.0001", "--ent", "0.002", "0.0015", "--whiff-far", "0.02"]

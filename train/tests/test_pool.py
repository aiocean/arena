"""The pool's rating, its meta-game's Nash mixture and its cycles, on made-up games."""

import numpy as np

from pool import Pool, bradley_terry, cycles, nash, payoff


def games(strengths, n=4000, seed=0):
    """Rounds between members of the given Elo strengths, each pair `n` rounds."""
    rng = np.random.default_rng(seed)
    k = len(strengths)
    w = np.zeros((k, k))
    for i in range(k):
        for j in range(i + 1, k):
            p = 1 / (1 + 10 ** ((strengths[j] - strengths[i]) / 400))
            won = rng.binomial(n, p)
            w[i, j], w[j, i] = won, n - won
    return w


def test_bradley_terry_finds_the_strengths_with_the_anchor_at_1000():
    truth = [1000, 1200, 900, 1400]
    elo, se = bradley_terry(games(truth), anchor=0)
    assert elo[0] == 1000 and se[0] == 0
    assert np.all(np.abs(elo - truth) < 3 * se[1:].max() + 1)
    assert np.all(se[1:] < 15)


def test_a_member_that_never_lost_has_a_finite_rating():
    w = np.array([[0.0, 0.0], [30.0, 0.0]])
    elo, se = bradley_terry(w, anchor=0)
    assert np.isfinite(elo[1]) and elo[1] > 1300 and np.isfinite(se[1])


def test_a_member_that_met_nobody_is_unknown():
    w = np.zeros((3, 3))
    w[0, 1], w[1, 0] = 10, 10
    elo, se = bradley_terry(w, anchor=0)
    assert se[2] == np.inf and elo[2] == 1000


def test_rock_paper_scissors_is_one_cycle_and_its_nash_mixture_is_even():
    w = np.array([[0, 80, 20], [20, 0, 80], [80, 20, 0]], dtype=float)
    a = payoff(w)
    assert len(cycles(a)) == 1
    assert np.allclose(nash(a), 1 / 3, atol=0.02)


def test_a_transitive_pool_has_no_cycle_and_its_nash_is_the_strongest():
    a = payoff(games([1000, 1200, 1400]))
    assert cycles(a) == []
    mix = nash(a)
    assert mix[2] > 0.99
    assert np.all(a @ mix <= 1e-9)


def test_two_pools_writing_at_once_keep_each_others_games_and_members(tmp_path):
    """The cycle and a standing exploiter share a pool: each one's save joins what the other wrote."""
    a, b = Pool(tmp_path), Pool(tmp_path)
    for p in (a, b):
        p.state["fingerprint"] = "fp"
    a.members["x"] = {"file": None, "source": "test"}
    a.state["games"]["brain\tx"] = [1, 2, 0]
    a.save()
    b.members["y"] = {"file": None, "source": "test"}
    b.state["games"]["brain\ty"] = [3, 4, 0]
    b.save()
    c = Pool(tmp_path)
    assert {"x", "y"} <= set(c.members)
    assert c.state["games"] == {"brain\tx": [1, 2, 0], "brain\ty": [3, 4, 0]}
    a.remove("y")  # a removal is kept through the join
    c = Pool(tmp_path)
    assert "y" not in c.members and "brain\ty" not in c.state["games"]
    assert "x" in c.members


def test_a_pool_on_new_rules_does_not_take_the_old_games(tmp_path):
    a, b = Pool(tmp_path), Pool(tmp_path)
    a.state["fingerprint"] = "old"
    a.state["games"]["brain\tx"] = [1, 2, 0]
    a.save()
    b.state = {"fingerprint": "new", "seeds": b.state["seeds"], "games": {}}
    b.save()
    assert Pool(tmp_path).state["games"] == {}

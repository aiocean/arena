import numpy as np

from league import League


class Recorder:
    """Stands in for the VecEnv: keeps the learner's seat (0 left, 1 right) of every reset."""

    def __init__(self):
        self.corners = []

    def set_model(self, wid, data):
        pass

    def reset(self, i, seed, learner, style, foe_style, name, wid, limit):
        self.corners.append(learner)


def seat0_share(fights=4000, **kw):
    """The share of `fights` fresh fights in which the learner sits in seat 0."""
    env = Recorder()
    league = League(env, np.random.default_rng(7), **kw)
    league.snapshots = [0]
    for i in range(fights):
        league.reset(i)
    return env.corners.count(0) / fights


def test_the_learner_sits_in_seat_0_in_the_share_it_is_given():
    assert abs(seat0_share(seat0=0.75) - 0.75) < 0.03


def test_the_learner_sits_in_each_seat_half_the_time_by_default():
    assert abs(seat0_share() - 0.5) < 0.03

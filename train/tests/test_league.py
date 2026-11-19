import numpy as np

from league import League, parse_mix


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


def test_mix_takes_five_numbers_in_the_old_order_or_named_shares():
    from league import MIX

    assert parse_mix(["0.2", "0.15", "0.5", "0.05", "0.1"]) == [0.2, 0.15, 0.5, 0.05, 0.1, 0.0, 0.0, 0.0, 0.0]
    named = parse_mix(["target=1", "brain=0.5"])
    assert named == [0.5 if k == "brain" else 1.0 if k == "target" else 0.0 for k in MIX]


class Models(Recorder):
    """A Recorder that also keeps the models it was handed and the opponents it was given."""

    def __init__(self):
        super().__init__()
        self.models, self.foes = {}, []

    def set_model(self, wid, data):
        self.models[wid] = data

    def reset(self, i, seed, learner, style, foe_style, name, wid, limit):
        super().reset(i, seed, learner, style, foe_style, name, wid, limit)
        self.foes.append(wid)


def test_snapshots_are_kept_and_a_resumed_league_fights_them_again(tmp_path, monkeypatch):
    import league as lg

    monkeypatch.setattr(lg, "to_onnx", lambda policy: f"model {policy}".encode())
    first = lg.League(Models(), np.random.default_rng(0), dir=tmp_path)
    for k in range(3):
        first.snapshot(k)
    env = Models()
    again = lg.League(env, np.random.default_rng(0), dir=tmp_path)
    assert again.restore() == 3 and env.models[2] == b"model 2"


def test_a_script_can_be_fought_alone():
    env = Models()
    league = League(env, np.random.default_rng(4), mix=parse_mix(["aggressive=1"]))
    league.snapshots = [0]
    assert {league.reset(i)[0] for i in range(50)} == {"aggressive"}


def test_pfsp_draws_the_opponents_it_loses_to(tmp_path):
    league = League(Models(), np.random.default_rng(1), mix=parse_mix(["past=1"]), pfsp=2)
    league.snapshots = [0, 1, 2]
    for _ in range(100):
        league.record("self:0", True)  # always beaten
        league.record("self:1", False)  # always beats the learner
    keys = [league.reset(i)[3] for i in range(2000)]
    assert keys.count("self:1") > 20 * max(keys.count("self:0"), 1)


def test_pool_and_target_models_are_read_from_their_folders(tmp_path):
    pool, target = tmp_path / "pool", tmp_path / "main"
    pool.mkdir(), target.mkdir()
    (pool / "a.onnx").write_bytes(b"a")
    (pool / "b.onnx").write_bytes(b"b")
    (target / "00000.onnx").write_bytes(b"old")
    (target / "00001.onnx").write_bytes(b"new")
    env = Models()
    league = League(env, np.random.default_rng(2), mix=parse_mix(["pool=1", "target=1"]), pools=[pool], target=target)
    league.snapshots = [0]
    league.refresh()
    assert sorted(env.models[w] for w in league.pool) == [b"a", b"b"]
    assert env.models[league.target[2]] == b"new"
    kinds = {league.reset(i)[0] for i in range(200)}
    assert kinds == {"pool", "target"}

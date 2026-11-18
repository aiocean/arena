"""The batched duel environments: random legal play ends every episode with a round's outcome, and replays."""

import numpy as np
import arena_duel as pd


def random_episodes(n, seed, steps=3000, whiff=0.0, env=None):
    env = env or pd.VecEnv(n, 4)
    for i in range(n):
        env.reset(i, seed + i, i % 2, i % 3, (i + 1) % 3, ["brain", "first", "aggressive", "turtle"][i % 4], -1, 90.0)
    rng = np.random.default_rng(seed)
    finals = np.zeros(n, dtype=np.float32)
    ended = np.zeros(n, dtype=bool)
    rewards = []
    for _ in range(steps):
        obs, mask = env.observe()
        assert obs.shape == (n, pd.OBS) and obs.dtype == np.float32
        assert mask.shape == (n, pd.ACTIONS) and mask.dtype == bool
        assert mask[:, 0].all(), "waiting is always legal"
        logits = rng.random((n, pd.ACTIONS))
        actions = np.where(mask, logits, -1.0).argmax(axis=1).astype(np.int64)
        reward, done, won, length = env.step(actions, 0.0, whiff)
        rewards.append(reward.copy())
        newly = done & ~ended
        finals[newly] = reward[newly]
        ended |= done
        if ended.all():
            break
    return ended, finals, np.array(rewards)


def test_random_legal_play_finishes_episodes():
    ended, finals, _ = random_episodes(16, 1)
    assert ended.all()
    assert np.all(np.abs(np.abs(finals) - 1.0) < 0.05), finals


def test_same_seeds_same_rewards():
    a = random_episodes(8, 7)[2]
    b = random_episodes(8, 7)[2]
    assert np.array_equal(a, b)


def test_casts_are_counted_by_kind_and_whiffs_cost_what_is_asked():
    env = pd.VecEnv(8, 4)
    free = random_episodes(8, 3, env=env)[2]
    casts = env.casts()
    assert len(casts) == 11 and env.casts() == [(0, 0, 0, 0.0)] * 11, "casts() takes what it reports"
    n, far, whiffs, damage = (sum(c[i] for c in casts) for i in range(4))
    assert n > 0 and 0 < whiffs <= n and far <= n and damage > 0, casts
    costly = random_episodes(8, 3, whiff=0.01)[2]
    assert free.sum() - costly.sum() > 0.01 * whiffs * 0.5


def test_the_log_reads_the_casts():
    from ppo import KINDS, cast_stats

    casts = [(0, 0, 0, 0.0)] * len(KINDS)
    casts[0] = (10, 6, 5, 20.0)  # blades: 10 cast, 6 out of reach, 5 landed nothing
    casts[3] = (10, 0, 5, 40.0)  # bolts
    s = cast_stats(casts)
    assert s["casts"] == 20 and s["whiff"] == 0.5 and s["out_of_reach"] == 0.6 and s["dmg_cast"] == 3.0
    assert s["melee_out_of_reach"] == 0.6 and s["melee_whiff"] == 0.5


def test_a_policy_opponent_from_an_onnx_model():
    env = pd.VecEnv(2, 1)
    env.set_model(3, pd.HOUSE)
    env.reset(0, 1, 0, 1, 1, "policy", 3, 90.0)
    env.reset(1, 2, 1, 1, 0, "policy", 3, 90.0)
    obs, _ = env.observe()
    assert np.isfinite(obs).all()
    for _ in range(3000):
        _, done, _, _ = env.step(np.zeros(2, dtype=np.int64), 0.0)
        if done.all():
            break
    assert done.all()


def test_unknown_opponent_or_model_is_an_error():
    env = pd.VecEnv(1, 1)
    for args in [("nobody", -1), ("policy", 99)]:
        try:
            env.reset(0, 1, 0, 1, 1, args[0], args[1], 90.0)
        except ValueError:
            continue
        raise AssertionError(f"{args} accepted")


def test_a_model_that_is_not_a_policy_is_refused():
    env = pd.VecEnv(1, 1)
    try:
        env.set_model(0, b"not a model")
    except ValueError:
        return
    raise AssertionError("accepted")


def test_a_bot_module_is_not_run_from_python():
    env = pd.VecEnv(1, 1)
    try:
        env.set_model(0, b"\0asm\x01\0\0\0")
    except ValueError as e:
        assert "ONNX" in str(e)
        return
    raise AssertionError("accepted")

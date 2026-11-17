"""Two bots fight from Python, and an ONNX policy is measured against the entry bar."""

import pytest

import arena_duel as pd

HOUSE = pd.HOUSE


def test_a_bout_between_an_onnx_policy_and_a_built_in_bot_counts_every_round():
    r = pd.bout(HOUSE, "brain", seeds=(1, 3))
    assert r["won"] + r["lost"] + r["cut"] >= 2 * 2 * 2
    assert 0.0 <= r["as_seat0"] <= 1.0 and 0.0 <= r["as_seat1"] <= 1.0
    assert r == pd.bout(HOUSE, "brain", seeds=(1, 3))
    assert r == pd.bout("house", "brain", seeds=(1, 3))


def test_built_in_names_fight_each_other():
    assert set(pd.BOTS) == {"brain", "first", "aggressive", "turtle"}
    r = pd.bout("brain", "turtle", seeds=(1, 2))
    assert r["won"] + r["lost"] > 0


def test_a_bad_bot_is_refused_before_any_fight():
    with pytest.raises(ValueError, match="no bot"):
        pd.bout("champion", "brain", seeds=(1, 2))
    with pytest.raises(ValueError):
        pd.bout(b"not a model", "brain", seeds=(1, 2))


@pytest.mark.skip(
    reason="the house AI was trained on the old rules: it clears the bar again once retrained on the damage system "
    "(docs/specs/2026-11-17-damage-system-design.md, 13 and 16)"
)
def test_the_house_ai_clears_the_entry_bar():
    bar = pd.entry_bar(HOUSE, seeds=(1, 7))
    assert bar["passed"], bar
    assert bar["vs_brain"]["win_rate"] >= 0.75

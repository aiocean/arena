"""Two bots fight from Python, and a weights file is measured against the entry bar."""

from pathlib import Path

import pytest

import pixfx_duel as pd

SHIPPED = (Path(__file__).parents[2] / "crates/pixfx-duel/weights/policy.bin").read_bytes()


def test_a_bout_between_a_weights_file_and_a_built_in_bot_counts_every_round():
    r = pd.bout(SHIPPED, "brain", seeds=(1, 3))
    assert r["won"] + r["lost"] + r["cut"] >= 2 * 2 * 2
    assert 0.0 <= r["as_thunder"] <= 1.0 and 0.0 <= r["as_frost"] <= 1.0
    assert r == pd.bout(SHIPPED, "brain", seeds=(1, 3))


def test_built_in_names_fight_each_other():
    assert set(pd.BOTS) == {"brain", "first", "aggressive", "turtle"}
    r = pd.bout("brain", "turtle", seeds=(1, 2))
    assert r["won"] + r["lost"] > 0


def test_a_bad_bot_is_refused_before_any_fight():
    with pytest.raises(ValueError, match="no bot"):
        pd.bout("champion", "brain", seeds=(1, 2))
    with pytest.raises(ValueError):
        pd.bout(b"not a weights file", "brain", seeds=(1, 2))


def test_the_shipped_weights_clear_the_entry_bar():
    bar = pd.entry_bar(SHIPPED, seeds=(1, 7))
    assert bar["passed"], bar
    assert bar["vs_brain"]["win_rate"] >= 0.75

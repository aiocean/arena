"""The trainer's schedule: learning rate and entropy fall over the run, shaping fades to 0 (or is off)."""

import pathlib
import shutil

from ppo import schedule


def test_schedule_starts_full_and_ends_at_its_floors():
    lr, ent, shaping = schedule(0, 100, lr=3e-4, ent=(0.01, 0.002), shaping_until=0.6)
    assert (lr, ent, shaping) == (3e-4, 0.01, 1.0)
    lr, ent, shaping = schedule(100, 100, lr=3e-4, ent=(0.01, 0.002), shaping_until=0.6)
    assert lr == 0.0 and abs(ent - 0.002) < 1e-12 and shaping == 0.0


def test_shaping_until_zero_turns_shaping_off():
    for u in (0, 50, 99):
        assert schedule(u, 100, lr=1e-4, ent=(0.003, 0.002), shaping_until=0.0)[2] == 0.0


def test_an_asymmetric_critic_trains_and_its_policy_exports_as_any_other(tmp_path):
    """`--privileged`: the value head also reads `VecEnv.privileged`; the checkpoint loads as a plain policy."""
    import subprocess
    import sys

    import torch

    import arena_duel as pd
    from export import load

    run = "test-priv"
    out = pathlib.Path(__file__).parent.parent / "runs" / run
    cmd = [sys.executable, "ppo.py", "--run", run, "--updates", "2", "--envs", "8", "--steps", "16", "--threads", "2"]
    subprocess.run(cmd + ["--privileged"], cwd=pathlib.Path(__file__).parent.parent, check=True)
    ck = torch.load(out / "last.pt", map_location="cpu", weights_only=True)
    assert any(k.startswith("priv.") for k in ck["model"])
    policy = load(out / "last.pt")
    assert policy.priv is None and policy.inp.in_features == pd.OBS
    env = pd.VecEnv(2, 1, privileged=True)
    env.reset(0, 5, 0, 1, 1, "brain", -1, 30.0)
    assert env.privileged().shape == (2, pd.PRIVILEGED)
    assert env.privileged()[0].any() and not env.privileged()[1].any()
    shutil.rmtree(out)


def test_a_run_with_search_imitates_the_lookahead_and_logs_it():
    """`--search`: some decisions are searched (`VecEnv.lookahead_many`) and the policy imitates the target."""
    import json
    import subprocess
    import sys

    here = pathlib.Path(__file__).parent.parent
    out = here / "runs" / "test-search-unit"
    shutil.rmtree(out, ignore_errors=True)
    cmd = [sys.executable, "ppo.py", "--run", "test-search-unit", "--updates", "10", "--envs", "8", "--steps", "16"]
    subprocess.run(cmd + ["--threads", "2", "--search", "4", "--search-depth", "3"], cwd=here, check=True)
    line = json.loads((out / "log.jsonl").read_text().splitlines()[-1])
    assert line["search_kl"] >= 0 and 0 <= line["search_agree"] <= 1
    shutil.rmtree(out)

"""`distill.py`: a student learns the teacher's acts and leaves a checkpoint `ppo.py --init` reads."""

import json
import os
import pathlib
import shutil
import subprocess
import sys


def test_a_short_distillation_writes_a_checkpoint_at_its_size():
    import torch

    import arena_duel as pd
    from net import Policy

    here = pathlib.Path(__file__).parent.parent
    run = f"test-distill-{os.getpid()}"
    out = here / "runs" / run
    shutil.rmtree(out, ignore_errors=True)
    try:
        cmd = [sys.executable, "distill.py", "--run", run, "--updates", "5", "--envs", "8", "--steps", "8"]
        cmd += ["--threads", "2", "--width", "32", "--hidden", "48", "--blocks", "2", "--entities", "8"]
        subprocess.run(cmd, cwd=here, check=True)
        ck = torch.load(out / "last.pt", map_location="cpu", weights_only=True)
        assert ck["size"] == {"width": 32, "hidden": 48, "blocks": 2, "entities": 8}
        Policy(pd.OBS, pd.ACTIONS, **ck["size"]).load_state_dict(ck["model"])
        line = json.loads((out / "log.jsonl").read_text().splitlines()[-1])
        assert line["update"] == 5 and line["kl"] >= 0 and 0 <= line["agree"] <= 1
    finally:
        shutil.rmtree(out, ignore_errors=True)

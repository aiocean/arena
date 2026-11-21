"""Start a new network by imitating a trained one (`docs/plans/2026-11-20-smarter-ai.md`, D): a bigger brain learns
first to play as the house AI does, then `ppo.py --init` trains it on.

    uv run python distill.py --run abi5-big-init --entities 32 --width 192 --hidden 256 --blocks 3 [--updates 300]

Each update rolls `--envs` fights `--steps` decisions against the league's opponents (the teacher itself among
them, as `latest`). A decision is played by the teacher, or by the student with a share rising to `--student` (so
the student also sees the places its own mistakes lead to); either way the teacher's logits over the legal acts are
the target (KL), and the student's critic learns the fight's returns (GAE, HL-Gauss as in `ppo.py`). Writes
runs/<run>/last.pt (a checkpoint `ppo.py --init` reads, its size inside) and runs/<run>/log.jsonl, with how often
the student's top act is the teacher's.
"""

import argparse
import json
import pathlib
import time
from collections import defaultdict, deque

import numpy as np
import torch
from torch.distributions import Categorical

import arena_duel as pd
from league import League, parse_mix
from net import Policy, from_onnx, hl_gauss, size_of
from widen import current

HERE = pathlib.Path(__file__).parent
HOUSE = HERE / "../crates/arena-duel/weights/policy.onnx"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--teacher", default=str(HOUSE), help="an ONNX model (the house AI's by default)")
    ap.add_argument("--width", type=int, default=192)
    ap.add_argument("--hidden", type=int, default=256)
    ap.add_argument("--blocks", type=int, default=3)
    ap.add_argument("--entities", type=int, default=32)
    ap.add_argument("--updates", type=int, default=300)
    ap.add_argument("--envs", type=int, default=256)
    ap.add_argument("--steps", type=int, default=64)
    ap.add_argument("--epochs", type=int, default=2)
    ap.add_argument("--minibatches", type=int, default=8)
    ap.add_argument("--lr", type=float, default=3e-4)
    ap.add_argument("--gamma", type=float, default=0.995)
    ap.add_argument("--lam", type=float, default=0.95)
    ap.add_argument("--student", type=float, default=0.5, help="the share of decisions the student plays, at the end")
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--mix", nargs="+", default=["latest=0.2", "brain=0.35", "first=0.1", "scripted=0.1", "aggressive=0.25"])
    args = ap.parse_args()

    torch.manual_seed(args.seed)
    rng = np.random.default_rng(args.seed)
    out = HERE / "runs" / args.run
    out.mkdir(parents=True, exist_ok=True)
    teacher_bytes = current(pathlib.Path(args.teacher).read_bytes())
    teacher = from_onnx(teacher_bytes, pd.OBS, pd.ACTIONS).eval()
    size = dict(width=args.width, hidden=args.hidden, blocks=args.blocks, entities=args.entities)
    student = Policy(pd.OBS, pd.ACTIONS, **size)
    opt = torch.optim.Adam(student.parameters(), lr=args.lr, eps=1e-5)

    n, T = args.envs, args.steps
    env = pd.VecEnv(n, args.threads)
    league = League(env, rng, seed=50_000_000 + args.seed * 1_000_000, mix=parse_mix(args.mix), dir=out / "league")
    league.snapshot(teacher)  # `latest`: the teacher plays itself
    kinds = [league.reset(i) for i in range(n)]

    obs_b = torch.zeros(T, n, pd.OBS)
    mask_b = torch.zeros(T, n, pd.ACTIONS, dtype=torch.bool)
    rew_b, done_b, val_b = torch.zeros(T, n), torch.zeros(T, n), torch.zeros(T, n)
    results = defaultdict(lambda: deque(maxlen=400))
    log = open(out / "log.jsonl", "a")
    tick = time.time()
    for update in range(args.updates):
        share = args.student * min(1.0, update / max(1, args.updates // 2))
        student.eval()
        for t in range(T):
            obs, mask = env.observe()
            o, m = torch.from_numpy(obs), torch.from_numpy(mask)
            with torch.no_grad():
                t_logits, _ = teacher(o, m)
                s_logits, vlog = student(o, m)
                by_student = torch.from_numpy(rng.random(n) < share)
                a = torch.where(by_student, Categorical(logits=s_logits).sample(), Categorical(logits=t_logits).sample())
            obs_b[t], mask_b[t], val_b[t] = o, m, student.value_of(vlog)
            r, d, won, _length = env.step(a.numpy().astype(np.int64), 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            rew_b[t], done_b[t] = torch.from_numpy(r), torch.from_numpy(d.astype(np.float32))
            for i in np.nonzero(d)[0]:
                kind = kinds[i][0]
                league.record(kinds[i][3], won[i] == 1)
                results[kind].append(float(won[i] == 1))
                kinds[i] = league.reset(int(i))
        with torch.no_grad():
            obs, mask = env.observe()
            _, vlog = student(torch.from_numpy(obs), torch.from_numpy(mask))
            nxt = student.value_of(vlog)
        adv, gae = torch.zeros(T, n), torch.zeros(n)
        for t in reversed(range(T)):
            live = 1.0 - done_b[t]
            v_next = nxt if t == T - 1 else val_b[t + 1]
            gae = rew_b[t] + args.gamma * v_next * live - val_b[t] + args.gamma * args.lam * live * gae
            adv[t] = gae
        ret = (adv + val_b).reshape(-1)
        O, M = obs_b.reshape(-1, pd.OBS), mask_b.reshape(-1, pd.ACTIONS)
        with torch.no_grad():
            target = torch.cat([teacher(O[i : i + 4096], M[i : i + 4096])[0] for i in range(0, len(O), 4096)])
            target = target.log_softmax(-1)

        student.train()
        stats = defaultdict(float)
        B = T * n
        for _ in range(args.epochs):
            perm = torch.randperm(B)
            for idx in perm.chunk(args.minibatches):
                logits, vlog = student(O[idx], M[idx])
                logp = logits.log_softmax(-1)
                p_t = target[idx].exp()
                kl = (p_t * (target[idx] - logp)).masked_fill(~M[idx], 0.0).sum(-1).mean()
                vloss = -(hl_gauss(ret[idx], student.support, student.sigma) * vlog.log_softmax(-1)).sum(-1).mean()
                loss = kl + 0.5 * vloss
                opt.zero_grad()
                loss.backward()
                torch.nn.utils.clip_grad_norm_(student.parameters(), 0.5)
                opt.step()
                stats["kl"] += kl.item()
                stats["v"] += vloss.item()
                stats["agree"] += (logits.argmax(-1) == target[idx].argmax(-1)).float().mean().item()
        k = args.epochs * args.minibatches
        if (update + 1) % 5 == 0 or update + 1 == args.updates:
            line = {
                "update": update + 1,
                "sps": round(5 * B / (time.time() - tick)),
                "student_share": round(share, 3),
                **{f: round(v / k, 4) for f, v in stats.items()},
                **{f"win/{o}": round(float(np.mean(v)), 3) for o, v in sorted(results.items())},
            }
            print(json.dumps(line), flush=True)
            log.write(json.dumps(line) + "\n")
            log.flush()
            tick = time.time()
        if (update + 1) % 25 == 0 or update + 1 == args.updates:
            torch.save({"model": student.state_dict(), "size": size_of(student)}, out / "last.pt")


if __name__ == "__main__":
    main()

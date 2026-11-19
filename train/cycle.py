"""The improvement cycle on one command: train, rate every candidate in the pool, measure the best few, write the
report (`docs/specs/2026-11-19-model-improvement-design.md`, 5). It ships nothing.

    uv run python cycle.py --run c25 [--from ../crates/arena-duel/weights/policy.onnx] [--pbt | --updates 1200]
        [--generations 12] [--threads 8] [--exploit 200] [--review DIR] [-- ppo.py arguments]

1. Train from `--from` (the house AI by default; widened first when the observation grew): a PBT population
   (`--pbt`, `pbt.py`, every generation's best kept) or one run (`ppo.py --keep-every 100`). A run already trained
   is not trained again (`--skip-train`, or its state says it is done).
2. Add every kept checkpoint, and the house AI, to the pool and fight the missing games (`pool.py`).
3. Take the `--top` best-rated candidates; measure each against the entry bar on the ladder's seeds and head to
   head against the house AI over the pool's seeds, and with `--exploit N` its exploitability after N updates of a
   best response (`exploit.py`). Each best response joins the pool (`<run>-x-<candidate>/best`): the next cycle's
   league fights it (`--pool runs/pool/models`), so the holes found are the ones the next model must close.
4. The pick: the best-rated candidate that clears the bar. `runs/<run>/report.json` and `report.md`; with
   `--review DIR` also `DIR/index.html`, a review page.

Exporting the pick (`export.py`), the house AI's tests and the module's re-pinning stay a session's steps
(`CLAUDE.md`, "Traps"), once the owner has seen the report.
"""

import argparse
import html
import json
import pathlib
import subprocess
import sys

import torch

import arena_duel as pd
from pool import POOL, Pool, load_policy, print_table

HERE = pathlib.Path(__file__).parent
HOUSE = HERE / "../crates/arena-duel/weights/policy.onnx"


def train(args, out, extra):
    """Train and return the candidates' checkpoints, oldest first."""
    init = out / "init.pt"
    if not init.exists():
        p = load_policy(args.source)
        size = dict(width=p.width, hidden=p.blocks[0].up.out_features, blocks=len(p.blocks))
        torch.save({"model": p.state_dict(), "size": size}, init)
    if args.pbt:
        cmd = [sys.executable, "pbt.py", "--run", args.run, "--init", str(init), "--threads", str(args.threads)]
        cmd += ["--generations", str(args.generations), "--every", str(args.every), "--members", str(args.members)]
        if not args.skip_train:
            subprocess.run(cmd + ["--", *extra], cwd=HERE, check=True)
        return sorted((out / "best").glob("g*.pt"))
    if not args.skip_train:
        cmd = [sys.executable, "ppo.py", "--run", args.run, "--updates", str(args.updates), "--keep-every", "100"]
        cmd += ["--threads", str(args.threads)]
        cmd += ["--resume", str(out / "last.pt")] if (out / "last.pt").exists() else ["--init", str(init)]
        subprocess.run(cmd + extra, cwd=HERE, check=True)
    return sorted(out.glob("u*.pt"), key=lambda f: int(f.stem[1:]))


def measure(pool, name, house, args):
    """A candidate's bar, its record against the house AI, and its exploitability when asked."""
    bot = pool.bot(name)
    bar = pd.entry_bar(bot)
    vs_house = pd.bout(bot, house, seeds=pool.seeds)
    m = {"name": name, "bar": bar, "vs_house": vs_house}
    if args.exploit:
        run = f"{args.run}-x-{name.replace('/', '-')}"
        file = pool.root / "models" / pool.members[name]["file"]
        cmd = [sys.executable, "exploit.py", str(file), "--run", run, "--updates", str(args.exploit), "--pool", str(pool.root)]
        cmd.append("--pool-add")
        subprocess.run(cmd + ["--threads", str(args.threads)], cwd=HERE, check=True)
        m["exploitability"] = json.loads((HERE / "runs" / run / "exploit.json").read_text())["exploitability"]
    return m


def pct(x):
    return f"{100 * x:.1f}%"


def markdown(report):
    t, lines = report["table"], []
    lines.append(f"# Cycle {report['run']}\n")
    lines.append(f"From `{report['from']}`; rules `{t['fingerprint']}`; pool seeds {t['seeds'][0]}-{t['seeds'][1] - 1}.\n")
    pick = report["pick"]
    lines.append(f"**Pick: {pick}**\n" if pick else "**No candidate clears the bar.**\n")
    lines.append("| candidate | elo | ±2se | nash | bar | vs brain (seat 0 / 1) | vs first | vs house | exploitability |")
    lines.append("|---|---|---|---|---|---|---|---|---|")
    rows = {r["name"]: r for r in t["rows"]}
    for m in report["measured"]:
        r, b = rows[m["name"]], m["bar"]
        vb, vf = b["vs_brain"], b["vs_first"]
        x = m.get("exploitability")
        lines.append(
            f"| {m['name']} | {r['elo']:.0f} | {2 * r['se']:.0f} | {r['nash']:.3f} | {'pass' if b['passed'] else 'fail'} "
            f"| {pct(vb['win_rate'])} ({pct(vb['as_seat0'])} / {pct(vb['as_seat1'])}) | {pct(vf['win_rate'])} "
            f"| {pct(m['vs_house']['win_rate'])} | {'' if x is None else f'{x:+.3f}'} |"
        )
    lines.append("\n## The pool\n")
    lines.append("| member | elo | ±2se | rate | nash | mix |")
    lines.append("|---|---|---|---|---|---|")
    for r in t["rows"]:
        lines.append(f"| {r['name']} | {r['elo']:.0f} | {2 * r['se']:.0f} | {r['rate']:.3f} | {r['nash']:.3f} | {r['mix']:.2f} |")
    lines.append(f"\nCycles (each beats the next by 55% or more): {len(t['cycles'])}")
    lines += [f"- {' > '.join(c)} > {c[0]}" for c in t["cycles"][:20]]
    return "\n".join(lines) + "\n"


def review_page(md):
    """The report as a plain page: its markdown tables as HTML tables."""
    body, table = [], None
    for line in md.splitlines():
        if line.startswith("|"):
            cells = [html.escape(c.strip()) for c in line.strip("|").split("|")]
            if set("".join(cells)) <= set("-"):
                continue
            tag = "th" if table is None else "td"
            table = table or []
            table.append("<tr>" + "".join(f"<{tag}>{c}</{tag}>" for c in cells) + "</tr>")
            continue
        if table is not None:
            body.append("<table>" + "".join(table) + "</table>")
            table = None
        if line.startswith("# "):
            body.append(f"<h1>{html.escape(line[2:])}</h1>")
        elif line.startswith("## "):
            body.append(f"<h2>{html.escape(line[3:])}</h2>")
        elif line.strip():
            text = html.escape(line.lstrip("- ")).replace("**", "")
            body.append(f"<p>{text}</p>")
    if table is not None:
        body.append("<table>" + "".join(table) + "</table>")
    style = "body{font:15px system-ui;margin:2em auto;max-width:72em;padding:0 1em}table{border-collapse:collapse}"
    style += "td,th{border:1px solid #ccc;padding:.25em .6em;text-align:right}td:first-child,th:first-child{text-align:left}"
    return f"<!doctype html><meta charset=utf-8><title>Cycle</title><style>{style}</style>" + "".join(body)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True)
    ap.add_argument("--from", dest="source", default=str(HOUSE), help="the model to start from (.pt or .onnx)")
    ap.add_argument("--pbt", action="store_true", help="train a PBT population instead of one run")
    ap.add_argument("--updates", type=int, default=1200, help="one run's updates")
    ap.add_argument("--generations", type=int, default=12)
    ap.add_argument("--every", type=int, default=100)
    ap.add_argument("--members", type=int, default=4)
    ap.add_argument("--threads", type=int, default=8)
    ap.add_argument("--skip-train", action="store_true")
    ap.add_argument("--top", type=int, default=3, help="candidates measured against the bar")
    ap.add_argument("--exploit", type=int, default=0, help="updates of a best response against each measured one")
    ap.add_argument("--pool", default=str(POOL))
    ap.add_argument("--review", help="a folder for the review page (index.html)")
    ap.add_argument("ppo", nargs=argparse.REMAINDER, help="after --: arguments for ppo.py")
    args = ap.parse_args()
    extra = [a for a in args.ppo if a != "--"]
    out = HERE / "runs" / args.run
    out.mkdir(parents=True, exist_ok=True)

    candidates = train(args, out, extra)
    pool = Pool(args.pool)
    pool.init(pool.seeds)  # the built-ins, and the house AI as shipped now
    names = [pool.add(c, f"{args.run}/{c.stem}") for c in candidates]
    pool.play(args.threads)
    table = pool.table()
    print_table(table)

    ranked = [r["name"] for r in table["rows"] if r["name"] in names][: args.top]
    house = bytes(pd.HOUSE)
    measured = [measure(pool, n, house, args) for n in ranked]
    passed = [m["name"] for m in measured if m["bar"]["passed"]]
    report = {"run": args.run, "from": args.source, "table": table, "measured": measured}
    report["pick"] = passed[0] if passed else None
    (out / "report.json").write_text(json.dumps(report, indent=1))
    md = markdown(report)
    (out / "report.md").write_text(md)
    if args.review:
        page = pathlib.Path(args.review)
        page.mkdir(parents=True, exist_ok=True)
        (page / "index.html").write_text(review_page(md))
    print(md)


if __name__ == "__main__":
    main()

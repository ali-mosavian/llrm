#!/usr/bin/env python3
"""BC against the front end on whole programs: compare_programs.py [--family qb45] [--group G] [--kind gap|leak] [--show N]"""
import argparse, collections, json, subprocess
from pathlib import Path

HERE = Path(__file__).parent
ap = argparse.ArgumentParser()
ap.add_argument("--family", default="qb45")
ap.add_argument("--group")
ap.add_argument("--kind", default="leak")
ap.add_argument("--show", type=int, default=0)
ap.add_argument("--results", default="bc_programs.json")
ap.add_argument("--match")
args = ap.parse_args()
index = {}
for line in (HERE / "programs.jsonl").read_text().splitlines():
    j = json.loads(line); index[j["id"]] = j
bc = json.loads((HERE / args.results).read_text())
binary = HERE.parent / "target/release/checkfiles"
files = [str(HERE / "programs" / f"p{i:05d}.bas") for i in index]
out = subprocess.run([str(binary), args.family, *files], capture_output=True, text=True).stdout
ours = {}
for row in out.splitlines():
    path, cls, msg = (row.split("\t") + ["", ""])[:3]
    ours[int(Path(path).stem[1:])] = (cls, msg)
counts = collections.Counter(); rows = collections.defaultdict(list)
for i, item in index.items():
    refused = bc[str(i)][args.family]
    cls, msg = ours[i]
    if args.group and item["group"] != args.group:
        continue
    kind = ("agree-ok" if cls == "ok" else "gap") if refused is None else ("leak" if cls == "ok" else "agree-refuse")
    counts[(item["group"], kind)] += 1
    rows[kind].append((item, refused, cls, msg))
kinds = ["agree-ok", "agree-refuse", "gap", "leak"]
groups = sorted({g for g, _ in counts})
print(f"{'group':10}" + "".join(f"{k:>14}" for k in kinds))
for g in groups:
    print(f"{g:10}" + "".join(f"{counts[(g, k)]:>14}" for k in kinds))
print(f"{'total':10}" + "".join(f"{sum(counts[(g, k)] for g in groups):>14}" for k in kinds))
if args.show:
    shown = 0
    for item, refused, cls, msg in rows[args.kind]:
        if args.match and args.match not in item["why"]:
            continue
        text = " | ".join((HERE / "programs" / f"p{item['id']:05d}.bas").read_text().splitlines())
        print(f"[{item['group']}] {item['why'][:50]:50} BC={refused} ours={cls}:{msg[:50]}\n      {text[:150]}")
        shown += 1
        if shown >= args.show:
            break

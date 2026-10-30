#!/usr/bin/env python3
"""BC against the front end, per statement and per family.

    compare.py [--show N] [--family qb45|pds71|vbdos] [--group G] [--kind gap|leak|unsupported]
"""

import argparse
import collections
import json
from pathlib import Path

HERE = Path(__file__).parent
ap = argparse.ArgumentParser()
ap.add_argument("--show", type=int, default=0)
ap.add_argument("--family", default="qb45")
ap.add_argument("--group")
ap.add_argument("--kind", default="gap")
ap.add_argument("--results", default="bc_results.json")
ap.add_argument("--pattern")
args = ap.parse_args()

catalog = {}
for line in (HERE / "catalog.jsonl").read_text().splitlines():
    item = json.loads(line)
    catalog[item["id"]] = item
bc = json.loads((HERE / args.results).read_text())
ours = {}
for line in (HERE / f"ours_{args.family}.tsv").read_text().splitlines():
    i, cls, message = (line.split("\t") + [""])[:3]
    ours[int(i)] = (cls, message)

counts = collections.Counter()
rows = collections.defaultdict(list)
for sid, item in catalog.items():
    if str(sid) not in bc:
        continue
    if args.group and item["group"] != args.group:
        continue
    refused = bc[str(sid)][args.family]
    if refused and set(refused) <= {"Statement Ignored"}:
        refused = None  # a warning (TRON/TROFF without /D)
    if refused == ["CRASH"]:
        continue  # the oracle died: no verdict
    cls, message = ours[sid]
    if refused is None:
        kind = "agree-ok" if cls == "ok" else ("unsupported" if cls == "unsupported" else "gap")
    else:
        kind = "leak" if cls == "ok" else ("agree-refuse" if cls in ("syntax", "semantic") else "unsupported-refused")
    counts[(item["group"], kind)] += 1
    rows[kind].append((item, refused, cls, message))

groups = sorted({g for g, _ in counts})
kinds = ["agree-ok", "agree-refuse", "gap", "leak", "unsupported", "unsupported-refused"]
print(f"{'group':12}" + "".join(f"{k:>20}" for k in kinds))
for g in groups:
    print(f"{g:12}" + "".join(f"{counts[(g, k)]:>20}" for k in kinds))
print(f"{'total':12}" + "".join(f"{sum(counts[(g, k)] for g in groups):>20}" for k in kinds))
if args.show:
    print()
    for item, refused, cls, message in rows[args.kind][: args.show]:
        if args.pattern and args.pattern not in item["pattern"]:
            continue
        print(f"[{item['group']}] {item['text']:60} BC={refused} ours={cls}: {message[:80]}")

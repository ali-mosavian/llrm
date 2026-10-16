#!/usr/bin/env python3
"""Compare llrm-c's objects with the reference build's: publics, then code and data bytes.

    compare.py OURS_DIR REF_DIR      (each an inventory.py --out directory)
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import omf  # noqa: E402


def load(directory):
    return {(r["dir"], r["obj"]): r for r in map(json.loads, open(Path(directory) / "results.jsonl"))}


def compare(ours, ref):
    """Rows (dir, obj, publics_same, code_ours, code_ref, data_ours, data_ref) of objects both built."""
    a, b = load(ours), load(ref)
    rows = []
    for key, x in a.items():
        y = b.get(key)
        if x["status"] != "ok" or not y or y["status"] != "ok":
            continue
        p = omf.read(Path(ours) / "obj" / key[0] / f"{key[1]}.obj")
        q = omf.read(Path(ref) / "obj" / key[0] / f"{key[1]}.obj")
        rows.append((*key, p["publics"] == q["publics"], p["code"], q["code"], p["data"], q["data"], p, q))
    return rows


if __name__ == "__main__":
    rows = compare(*sys.argv[1:3])
    same = [r for r in rows if r[2]]
    print(f"both built {len(rows)}; same publics {len(same)}")
    print("code ours/ref", sum(r[3] for r in rows), sum(r[4] for r in rows), "data", sum(r[5] for r in rows), sum(r[6] for r in rows))
    for r in rows:
        if not r[2]:
            print("PUBLICS", r[0], r[1], sorted(r[7]["publics"] ^ r[8]["publics"])[:4])

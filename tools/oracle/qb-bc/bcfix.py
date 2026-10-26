#!/usr/bin/env python3
"""Ask BC again about the (file, family) pairs whose verdict was UNPLACED: bcfix.py programs|real"""
import json, os, re, sys
from pathlib import Path
HERE = Path(__file__).parent
CORPUS = HERE.parents[2] / "crates/frontends/qb/lalrspike/corpus"
os.environ["BCRAW"] = "1"
import bcprobe
from tqdm import tqdm
TOOLS = Path(os.environ['LLRM_PRIVATE_TOOLS'])
DIRS = {"qb45": TOOLS / "qb45", "pds71": TOOLS / "pds71", "vbdos": TOOLS / "vbdos"}
kind = sys.argv[1]
target = CORPUS / ("bc_programs.json" if kind == "programs" else "real_bc.json")
data = json.loads(target.read_text())
def path_of(key):
    return CORPUS / "programs" / f"p{int(key):05d}.bas" if kind == "programs" else __import__("real").path_of(key)
pairs = [(key, fam) for key, v in data.items() for fam, m in v.items() if m == ["UNPLACED"]]
print(len(pairs), "pairs", file=sys.stderr)
roots = {}
for key, fam in pairs:
    root = Path(os.environ['MGL_DIR']) if kind == "real" and str(key).startswith("mgl:") else None
    roots.setdefault(root, []).append((key, fam))
for root, group in roots.items():
    tasks = [(DIRS[fam], path_of(key)) for key, fam in group]
    with tqdm(total=len(tasks), unit="job", file=sys.stderr, mininterval=10) as bar:
        raw = bcprobe.probe(tasks, root, bar.update)
    for (key, fam) in group:
        text = raw[(DIRS[fam], path_of(key))]
        if text == "timeout" or "Severe" not in text:
            data[key][fam] = ["CRASH"]
            continue
        found, loose = bcprobe.errors(text)
        messages = [m.replace("\x07", "").strip() for _, _, _, m in found] + loose
        if not messages and int(re.search(r"(\d+)\s+Severe", text).group(1)):
            messages = ["UNPLACED"]
        data[key][fam] = messages or None
target.write_text(json.dumps(data))
print("remaining UNPLACED:", sum(1 for v in data.values() for m in v.values() if m == ["UNPLACED"]), file=sys.stderr)

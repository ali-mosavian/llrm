#!/usr/bin/env python3
"""BC (all three families) on the real programs, then the front end against it.

    real.py run [SLICE]      ask BC about the next SLICE files not yet in real_bc.json (resumable)
    real.py compare [--family F] [--show N] [--kind gap|leak]
"""
import argparse, collections, json, os, subprocess, sys
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent))
REPO = HERE.parents[3]
TOOLS = Path("/home/user/llrm-private-tools")
MGL = Path("/home/user/ali-mosavian/mgl")
FAMILIES = {"qb45": TOOLS / "qb45", "pds71": TOOLS / "pds71", "vbdos": TOOLS / "vbdos"}
RESULTS = HERE / "real_bc.json"
EXT = {".bas", ".bi"}


def files():
    """[(path, root)]: root is where BC must run for $INCLUDEs to resolve (mgl), else None."""
    out = {}
    tracked = subprocess.run(["git", "-C", str(REPO), "ls-files"], capture_output=True, text=True).stdout.splitlines()
    for line in tracked:
        if Path(line).suffix.lower() in EXT and "lalrspike/corpus/" not in line:
            out[REPO / line] = None
    for base in (TOOLS / "gor", TOOLS / "qb45", TOOLS / "pds71", TOOLS / "vbdos"):
        for path in sorted(base.rglob("*")):
            if path.suffix.lower() in EXT and path.is_file():
                out.setdefault(path, None)
    for path in sorted(MGL.rglob("*")):
        if path.suffix.lower() in EXT and path.is_file():
            out[path] = MGL
    return sorted(out.items(), key=lambda kv: str(kv[0]))


def run(limit):
    os.environ["BCRAW"] = "1"
    os.environ.setdefault("DOSBOX_BIN", "/home/user/dosbox-x-fork/src/dosbox-x")
    import bcprobe
    from tqdm import tqdm
    done = json.loads(RESULTS.read_text()) if RESULTS.exists() else {}
    todo = [(p, r) for p, r in files() if str(p) not in done][:limit]
    print(f"{len(done)} done, {len(todo)} in this slice", file=sys.stderr)
    for root in (None, MGL):
        batch = [p for p, r in todo if r == root]
        tasks = [(d, p) for p in batch for d in FAMILIES.values()]
        if not tasks:
            continue
        with tqdm(total=len(tasks), unit="job", file=sys.stderr, mininterval=10) as bar:
            raw = bcprobe.probe(tasks, root, bar.update)
        results = {}
        for (directory, path), text in raw.items():
            family = bcprobe.compiler(directory)[0]
            if text == "timeout" or "Severe" not in text:
                messages = ["CRASH"]
            else:
                found, loose = bcprobe.errors(text)
                messages = [m.replace("\x07", "").strip() for _, _, _, m in found] + loose
                import re
                if not messages and int(re.search(r"(\d+)\s+Severe", text).group(1)):
                    messages = ["UNPLACED"]
            results.setdefault(str(path), {})[family] = messages or None
        done.update(results)
        RESULTS.write_text(json.dumps(done))


def compare(args):
    bc = json.loads(RESULTS.read_text())
    paths = sorted(bc)
    binary = HERE.parent / "target/release/checkfiles"
    out = subprocess.run([str(binary), args.family, *paths], capture_output=True, text=True).stdout
    ours = {}
    for row in out.splitlines():
        path, cls, msg = (row.split("\t") + ["", ""])[:3]
        ours[path] = (cls, msg)
    counts = collections.Counter()
    rows = collections.defaultdict(list)
    for path in paths:
        refused = bc[path].get(args.family)
        if refused == ["CRASH"]:
            counts["crash"] += 1
            continue
        cls, msg = ours.get(path, ("?", ""))
        kind = ("agree-ok" if cls == "ok" else "gap") if refused is None else ("leak" if cls == "ok" else "agree-refuse")
        counts[kind] += 1
        rows[kind].append((path, refused, cls, msg))
    print(dict(counts))
    for path, refused, cls, msg in rows[args.kind][: args.show]:
        print(f"{path.replace(str(REPO) + '/', '')}\n    BC={refused}\n    ours={cls}: {msg[:160]}")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd")
    ap.add_argument("limit", nargs="?", type=int, default=100)
    ap.add_argument("--family", default="qb45")
    ap.add_argument("--kind", default="gap")
    ap.add_argument("--show", type=int, default=0)
    a = ap.parse_args()
    run(a.limit) if a.cmd == "run" else compare(a)

#!/usr/bin/env python3
"""Where the real BC.EXE and the LALR grammar disagree.

    bcdiff.py [--root DIR] BC_DIR[,BC_DIR...] FILE.bas...      # one BC_DIR per family

BC accepts, grammar refuses: a gap in the grammar. BC refuses, grammar
accepts: usually a semantic error the later passes own, but read the list.
"""

import subprocess
import sys
from pathlib import Path

from tqdm import tqdm

from bcprobe import compiler, probe

HERE = Path(__file__).parent


def mine_verdicts(files):
    binary = HERE / "target" / "debug" / "qbone"
    subprocess.run(["cargo", "build", "-q", "--manifest-path", str(HERE / "Cargo.toml"), "--bin", "qbone"], check=True)
    out = subprocess.run([str(binary), *map(str, files)], capture_output=True, text=True).stdout
    return {line.partition(": ")[0]: line.partition(": ")[2] for line in out.splitlines()}


def main():
    args, root = sys.argv[1:], None
    if args[:1] == ["--root"]:
        root, args = Path(args[1]), args[2:]
    dirs = [Path(one) for one in args[0].split(",")]  # BC_DIR[,BC_DIR...], one per family
    files = [Path(f) for f in args[1:]]
    mine = mine_verdicts(files)
    tasks = [(d, f) for f in files for d in dirs]
    with tqdm(total=len(tasks), unit="job", desc="BC", file=sys.stderr) as bar:
        bc = probe(tasks, root, bar.update)
    accepted = {compiler(d)[0]: 0 for d in dirs}
    gaps, lax, both_ok, both_refused = [], [], 0, 0
    for f in files:
        by = {compiler(d)[0]: bc[(d, f)] for d in dirs}
        for name, verdict in by.items():
            accepted[name] += verdict.startswith("ok")
        who = [name for name, verdict in by.items() if verdict.startswith("ok")]
        m = mine[str(f)]
        if who and m == "ok":
            both_ok += 1
        elif who:
            gaps.append(f"{f} [{','.join(who)}]: {m[:110]}")
        elif m == "ok":
            lax.append(f"{f}: " + "; ".join(f"{n}: {v[:60]}" for n, v in by.items()))
        else:
            both_refused += 1
    print("BC accepts: " + ", ".join(f"{n} {c}" for n, c in accepted.items()) + f"  (of {len(files)})")
    print(f"some BC accepts and the grammar does too {both_ok}; neither {both_refused}")
    print(f"some BC accepts, grammar refuses: {len(gaps)}")
    print("\n".join("  " + l for l in gaps))
    print(f"every BC refuses, grammar accepts: {len(lax)}")
    print("\n".join("  " + l for l in lax))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Where the real BC.EXE and the LALR grammar disagree.

    bcdiff.py [--root DIR] BC_DIR[,BC_DIR...] FILE.bas...      # one BC_DIR per family

BC accepts, grammar refuses: a gap in the grammar. BC refuses, grammar
accepts: usually a semantic error the later passes own, but read the list.
"""

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent


def verdicts(command, files):
    out = subprocess.run(command + [str(f) for f in files], stdout=subprocess.PIPE, text=True).stdout
    result = {}
    for line in out.splitlines():
        name, _, rest = line.partition(": ")
        result.setdefault(name, []).append(rest)
    return result


def main():
    args, root = sys.argv[1:], []
    if args[:1] == ["--root"]:
        root, args = args[:2], args[2:]
    families = {Path(one).name or one: one for one in args[0].split(",")}  # BC_DIR[,BC_DIR...]
    files = [Path(f) for f in args[1:]]
    cargo = ["cargo", "run", "-q", "--manifest-path", str(HERE / "Cargo.toml"), "--bin", "qbone", "--"]
    mine = verdicts(cargo, files)
    bc = {name: verdicts([sys.executable, str(HERE / "bcprobe.py"), *root, bc_dir], files) for name, bc_dir in families.items()}
    accepted = {name: 0 for name in families}
    gaps, lax, both_ok, both_refused = [], [], 0, 0
    for f in files:
        by = {name: got[str(f)][0] for name, got in bc.items()}
        for name, verdict in by.items():
            accepted[name] += verdict.startswith("ok")
        who = [name for name, verdict in by.items() if verdict.startswith("ok")]
        m = mine[str(f)][0]
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


main()

#!/usr/bin/env python3
"""Where the real BC.EXE and the LALR grammar disagree.

    bcdiff.py [--root DIR] BC_DIR FILE.bas...

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
    bc_dir, files = args[0], [Path(f) for f in args[1:]]
    cargo = ["cargo", "run", "-q", "--manifest-path", str(HERE / "Cargo.toml"), "--bin", "qbone", "--"]
    cargo = ["cargo", "run", "-q", "--manifest-path", str(HERE / "Cargo.toml"), "--bin", "qbone", "--"]
    bc_ok_mine_refused, bc_refused_mine_ok, both_ok, both_refused = [], [], 0, 0
    bc = verdicts([sys.executable, str(HERE / "bcprobe.py"), *root, bc_dir], files)
    mine = verdicts(cargo, files)
    for f in files:
        b, m = bc[str(f)][0], mine[str(f)][0]
        if b.startswith("ok") and m == "ok":
            both_ok += 1
        elif b.startswith("ok"):
            bc_ok_mine_refused.append(f"{f}: {m[:110]}")
        elif m == "ok":
            bc_refused_mine_ok.append(f"{f}: BC: {b[:400]}")
        else:
            both_refused += 1
    print(f"both accept {both_ok}, both refuse {both_refused}")
    print(f"BC accepts, grammar refuses: {len(bc_ok_mine_refused)}")
    print("\n".join("  " + l for l in bc_ok_mine_refused))
    print(f"BC refuses, grammar accepts: {len(bc_refused_mine_ok)}")
    print("\n".join("  " + l for l in bc_refused_mine_ok))


main()

#!/usr/bin/env python3
"""Where the real BC.EXE and the LALR grammar disagree.

    bcdiff.py BC_DIR FILE.bas...

BC accepts, grammar refuses: a gap in the grammar. BC refuses, grammar
accepts: usually a semantic error the later passes own, but read the list.
"""

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent


def verdicts(command, files):
    out = subprocess.run(command + [str(f) for f in files], capture_output=True, text=True).stdout
    result = {}
    for line in out.splitlines():
        name, _, rest = line.partition(": ")
        result.setdefault(name, []).append(rest)
    return result


def main():
    bc_dir, files = sys.argv[1], [Path(f) for f in sys.argv[2:]]
    cargo = ["cargo", "run", "-q", "--manifest-path", str(HERE / "Cargo.toml"), "--bin", "qbone", "--"]
    bc_ok_mine_refused, bc_refused_mine_ok, both_ok, both_refused = [], [], 0, 0
    for start in range(0, len(files), 80):
        chunk = files[start : start + 80]
        names = {}
        for f in chunk:
            names.setdefault(f.name, []).append(f)
        assert all(len(v) == 1 for v in names.values()), "file names in a batch must differ"
        bc = verdicts([sys.executable, str(HERE / "bcprobe.py"), bc_dir], chunk)
        mine = verdicts(cargo, chunk)
        for f in chunk:
            b, m = bc[f.name][0], mine[f.name][0]
            if b.startswith("ok") and m == "ok":
                both_ok += 1
            elif b.startswith("ok"):
                bc_ok_mine_refused.append(f"{f}: {m[:110]}")
            elif m == "ok":
                bc_refused_mine_ok.append(f"{f}: BC: {b[:110]}")
            else:
                both_refused += 1
    print(f"both accept {both_ok}, both refuse {both_refused}")
    print(f"BC accepts, grammar refuses: {len(bc_ok_mine_refused)}")
    print("\n".join("  " + l for l in bc_ok_mine_refused))
    print(f"BC refuses, grammar accepts: {len(bc_refused_mine_ok)}")
    print("\n".join("  " + l for l in bc_refused_mine_ok))


main()

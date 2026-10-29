#!/usr/bin/env python3
"""Static instruction count of llrm-cc against the pin's `llc` on the same IR, per program.

    tools/pairs/pincount.py LLRM_CC [--src DIR] [--level O2] [--jobs N] [--env K=V ...] [--json OUT]

For each `DIR/*.c` (default `ROWS_WORK/base/b`): llrm-cc compiles it for m32 (`-march=i486 -mabi=sysv --isel=dag`) with `--dump`,
the pin's `llc` (pin.py, explicit m32 target) compiles the dump's last IR (the `switchconv` stage, the one handed to the backend), and
both outputs are counted: the pin's lines that start with a tab and a lowercase letter, ours the indented lines that are not
directives (`align`, `db`, `dd`, `dw`, `dq`, `org`) or comments. llc runs with `-disable-lsr` (`--pin-args`). Prints the totals, then the programs furthest above the pin.
`--env` sets variables for llrm-cc only (`LLRM_SRC_ORDER=1`). The IR is ours, so a gap is the backend's, not the optimizer's.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402

FLAGS = ["-m32", "-mabi=sysv", "-march=i486", "--isel=dag"]
DIRECTIVES = ("align", "db ", "dd ", "dw ", "dq ", "org ")


def count_pin(asm: str) -> int:
    return sum(1 for line in asm.split("\n") if re.match(r"^\t[a-z]", line))


def count_ours(asm: str) -> int:
    return sum(1 for line in asm.split("\n") if line.startswith("    ") and not line.strip().startswith((";", ".", *DIRECTIVES)))


def one(binary: str, source: Path, level: str, env: dict, work: Path, target, pin_args: list[str]) -> tuple[str, int | None, int | None]:
    name = source.stem
    dump = work / name
    out = work / f"{name}.s"
    done = subprocess.run([binary, "--dump", str(dump), *FLAGS, f"-{level}", "-S", "-o", str(out), str(source)], capture_output=True, text=True, env={**env, "LLRM_CC_TARGET": "x86_32-wat"}, timeout=600)
    ll = pin.backend_ir(dump)
    if done.returncode or ll is None or not out.exists():
        shutil.rmtree(dump, ignore_errors=True)
        return name, None, None
    theirs = pin.llc([f"-{level}", "-filetype=asm", "-o", "-", *pin_args], ll.read_text(), target)
    ours = count_ours(out.read_text())
    shutil.rmtree(dump, ignore_errors=True)
    out.unlink()
    return name, ours, (count_pin(theirs.stdout) if theirs.returncode == 0 else None)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("binary")
    parser.add_argument("--src", default=str(Path(os.environ.get("ROWS_WORK", Path.home() / "scratch/rows")) / "base/b"))
    parser.add_argument("--level", default="O2")
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--env", nargs="*", default=[])
    parser.add_argument("--pin-args", nargs="*", default=["-disable-lsr"], help="llc's extra arguments; llc runs loop strength reduction on our IR, which ours has already done in the IR walk (x_reverse: 51 with it, 53 without), so the default leaves it out")
    parser.add_argument("--json", type=Path)
    args = parser.parse_args()
    env = dict(os.environ, **dict(kv.split("=", 1) for kv in args.env))
    target = pin.target("m32", optsize=args.level == "Os")
    # the dump is thousands of small files: on disk, never on tmpfs
    with tempfile.TemporaryDirectory(dir=Path.home() / "scratch") as tmp:
        sources = sorted(Path(args.src).glob("*.c"))
        with ThreadPoolExecutor(args.jobs) as pool:
            rows = list(pool.map(lambda s: one(args.binary, s, args.level, env, Path(tmp), target, args.pin_args), sources))
    both = [(name, ours, theirs) for name, ours, theirs in rows if ours is not None and theirs is not None]
    failed = [name for name, ours, theirs in rows if ours is None or theirs is None]
    ours, theirs = sum(r[1] for r in both), sum(r[2] for r in both)
    print(pin.header(target))
    print(f"{len(both)} programs: ours {ours}, pin {theirs} ({ours - theirs:+d}, {100 * (ours / theirs - 1):+.1f}%); failed: {failed or 'none'}")
    for name, o, t in sorted(both, key=lambda r: r[2] - r[1])[:12]:
        print(f"  {name:20} ours {o:5} pin {t:5} {o - t:+d}")
    if args.json:
        args.json.write_text(json.dumps({name: {"ours": o, "pin": t} for name, o, t in both}, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Final code bytes per module at m16: the old selector, the DAG path and the pin's llc on the same IR.

    tools/dagisel/m16_bytes.py LLRM_C [--src DIR] [--level=-Os] [--jobs N] [--out FILE]

Each program is compiled by `LLRM_C -m16 -march=i386 LEVEL` with `--isel=old` and `--isel=dag` (bytes of LEDATA in the OMF
object, as tools/rows/qcport.py counts them); the last IR the dump writes goes through llrm's dialect fixes (difftest.normalise)
to the pin's `llc -O2 -frame-pointer=all -filetype=obj` for the code16 target, and `size -A` gives its .text.

Two cautions. The OMF count includes initialised data, the pin's figure is .text only: the pin column is for modules without data.
The pin chooses 32-bit register forms in code16 (prefix bytes), so "closer to the pin" is not a goal for bytes; the rows to read are
`bigger`: the DAG path emitting more bytes than the old selector.
"""
from __future__ import annotations

import argparse
import glob
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "pairs"))
sys.path.insert(0, str(HERE.parent / "difftest"))
import difftest  # noqa: E402
import pin  # noqa: E402


def emitted(path: Path) -> int:
    """Bytes of LEDATA in an OMF object: code and initialised data as emitted."""
    data = path.read_bytes()
    i = total = 0
    while i < len(data):
        kind, size = data[i], data[i + 1] | data[i + 2] << 8
        if kind in (0xA0, 0xA1):
            body = data[i + 3 : i + 3 + size - 1]
            skip = 1 if body[0] < 0x80 else 2
            total += len(body) - skip - (4 if kind == 0xA1 else 2)
        i += 3 + size
    return total


def pin_text(size_output: str) -> int:
    for line in size_output.splitlines():
        if line.startswith(".text"):
            return int(line.split()[1])
    return 0


def one(llrm_c: str, source: Path, level: str) -> tuple[str, int | None, int | None, int | None, str]:
    """(program, pin, old, dag, why-not)."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        sizes = {}
        for mode in ("old", "dag"):
            obj, dump = tmp / f"{mode}.obj", tmp / f"d_{mode}"
            done = subprocess.run(
                [llrm_c, "-m16", "-march=i386", level, f"--isel={mode}", "--dump", str(dump), "-o", str(obj), str(source)],
                capture_output=True, text=True, timeout=600,
            )
            if done.returncode != 0 or not obj.exists():
                return source.stem, None, None, None, f"{mode} compile failed"
            sizes[mode] = emitted(obj)
        irs = sorted(glob.glob(str(tmp / "d_dag" / "[0-9]*.ll")))
        if not irs:
            return source.stem, None, sizes["old"], sizes["dag"], "no IR"
        text = difftest.normalise(Path(irs[-1]).read_text())
        target = pin.target("m16", optsize=level == "-Os")
        pinned = tmp / "pin.o"
        done = pin.llc(["-O2", "-frame-pointer=all", "-filetype=obj", "-o", str(pinned)], text, target)
        if done.returncode != 0:
            return source.stem, None, sizes["old"], sizes["dag"], "pin llc failed: " + (done.stderr.strip().splitlines() or ["?"])[0][:60]
        text_size = pin_text(subprocess.run(["size", "-A", str(pinned)], capture_output=True, text=True).stdout)
        return source.stem, text_size, sizes["old"], sizes["dag"], ""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("llrm_c")
    parser.add_argument("--src", required=True)
    parser.add_argument("--level", default="-O2", choices=["-O2", "-Os"])
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    parser.add_argument("--out", default="")
    args = parser.parse_args()
    sources = sorted(Path(args.src).glob("*.c"))
    with ThreadPoolExecutor(args.jobs) as pool:
        rows = list(pool.map(lambda s: one(args.llrm_c, s, args.level), sources))
    done = [r for r in rows if r[1] is not None]
    skipped = [r for r in rows if r[1] is None]
    farther = [r for r in done if abs(r[3] - r[1]) > abs(r[2] - r[1])]
    print(f"{len(done)} programs compared, {len(skipped)} not compared (a tool failed)")
    print(f"bytes: pin {sum(r[1] for r in done)}  old {sum(r[2] for r in done)}  dag {sum(r[3] for r in done)}")
    bigger = sorted((r for r in done if r[3] > r[2]), key=lambda r: r[2] - r[3])
    print(f"{len(bigger)} programs where the DAG path emits more bytes than the old selector, {sum(1 for r in done if r[3] < r[2])} fewer")
    for r in bigger[:10]:
        print(f"  {r[0]}: old {r[2]} dag {r[3]} (+{r[3] - r[2]})")
    lines = [f"{p}\tpin {a}\told {b}\tdag {c}\t{'bigger' if (b is not None and c is not None and c > b) else ''}{why}" for p, a, b, c, why in rows]
    if args.out:
        Path(args.out).write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

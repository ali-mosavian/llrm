#!/usr/bin/env python3
"""Final code bytes per QCport module at m32: the old selector, the DAG path and the pin's llc on the same IR.

    tools/dagisel/m32_bytes.py LLRM_CC [--level=-Os] [--jobs N] [--out FILE]

As m16_bytes.py, for `-m32 -march=i486 -mabi=sysv`: each module is compiled with `--isel=old` and `--isel=dag` (bytes of LEDATA, as
tools/rows/qcport.py counts them); the last IR the dag dump writes goes to the pin's `llc -O2 -filetype=obj` for the i486 target
(`optsize` on every function at -Os), and `size -A` gives its .text + .data + .rodata, the same quantity as the OMF count (code and initialised data). No frame-pointer flag: the pin frees ebp at m32.
Sources and headers are $QCPORT_SRC and $QCPORT_INC (the stubbed tree, ~/scratch/qcstub/{src,inc}).
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
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent / "pairs"))
sys.path.insert(0, str(HERE.parent / "difftest"))
sys.path.insert(0, str(HERE))
import difftest  # noqa: E402
import llrmbin  # noqa: E402
import pin  # noqa: E402
from m16_bytes import emitted, pin_text  # noqa: E402

MODS = ("host", "render", "model", "game", "sound", "ui")
FLAGS = ["-c", *llrmbin.c_args(32), "-mabi=sysv"]


def pin_loaded(size_output: str) -> int:
    """Code and initialised data, as the OMF count has them: .text, .data and .rodata sections (not bss, unwind or notes)."""
    total = 0
    for line in size_output.splitlines():
        name = line.split()[0] if line.split() else ""
        if name.startswith((".text", ".data", ".rodata")):
            total += int(line.split()[1])
    return total


def one(llrm_cc: str, source: Path, level: str, includes: list[str]) -> tuple[str, int | None, int | None, int | None, str]:
    """(module, pin, old, dag, why-not)."""
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        sizes = {}
        for mode in ("old", "dag"):
            obj, dump = tmp / f"{mode}.obj", tmp / f"d_{mode}"
            done = subprocess.run(
                [llrm_cc, *FLAGS, level, f"--isel={mode}", "--dump", str(dump), *includes, "-o", str(obj), str(source)],
                capture_output=True, text=True, timeout=600, env={**os.environ, **llrmbin.cc_environment()},
            )
            if done.returncode != 0 or not obj.exists():
                return source.stem, None, None, None, f"{mode} compile failed"
            sizes[mode] = emitted(obj)
        irs = sorted(glob.glob(str(tmp / "d_dag" / "[0-9]*.ll")))
        if not irs:
            return source.stem, None, sizes["old"], sizes["dag"], "no IR"
        text = difftest.normalise(Path(irs[-1]).read_text())
        pinned = tmp / "pin.o"
        done = pin.llc(["-O2", "-filetype=obj", "-o", str(pinned)], text, pin.target("m32", optsize=level == "-Os"))
        if done.returncode != 0:
            return source.stem, None, sizes["old"], sizes["dag"], "pin llc failed: " + (done.stderr.strip().splitlines() or ["?"])[0][:60]
        text_size = pin_loaded(subprocess.run(["size", "-A", str(pinned)], capture_output=True, text=True).stdout)
        return source.stem, text_size, sizes["old"], sizes["dag"], ""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("llrm_cc")
    parser.add_argument("--level", default="-Os", choices=["-O2", "-Os"])
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    parser.add_argument("--out", default="")
    args = parser.parse_args()
    src = Path(os.environ.get("QCPORT_SRC", Path.home() / "scratch/qcstub/src"))
    inc = os.environ.get("QCPORT_INC", str(Path.home() / "scratch/qcstub/inc"))
    includes = [f for d in (*MODS, "qgl") for f in ("-I", str(src / d))] + ["-I", inc]
    sources = sorted(p for d in MODS for p in (src / d).glob("*.c"))
    with ThreadPoolExecutor(args.jobs) as pool:
        rows = list(pool.map(lambda s: one(args.llrm_cc, s, args.level, includes), sources))
    done = [r for r in rows if r[1] is not None]
    print(f"{len(done)} modules compared, {len(rows) - len(done)} not compared (a tool failed)")
    print(f"bytes at {args.level}: pin {sum(r[1] for r in done)}  old {sum(r[2] for r in done)}  dag {sum(r[3] for r in done)}")
    lines = [f"{p}\tpin {a}\told {b}\tdag {c}\t{why}" for p, a, b, c, why in rows]
    if args.out:
        Path(args.out).write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

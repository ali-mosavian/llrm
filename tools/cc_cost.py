#!/usr/bin/env python3
"""What llrm-cc costs to run and what it emits, beside the pin's clang, for chosen C programs.

    tools/cc_cost.py [--dist DIR] [--programs jpeg_fixed,stft_fixed] [--configs m32-O2,m32-Os,m16-Os]

DIR holds a built llrm-cc (default target/release). One row per program and config:

    ours Minstr  - user-space instructions of the whole `llrm-cc -c` run, clang front end and llrm-llc
                   included (`perf stat -e instructions:u`, children inherited)
    pin Minstr   - `clang -cc1 -O2 ... -emit-obj` of the pinned clang: the compile a user would run (m32 only). Its
                   bytes are not printed: it optimises the C itself and inlines and folds differently from llrm-cc's IR.
    pin B        - `.text` of the pin on llrm-cc's own IR: clang -S -emit-llvm -disable-llvm-passes (the front end
                   llrm-cc runs, with -march: without it clang targets pentium4 +sse2; datalayout rewritten to llrm's table) -> pin `opt default<O2>` -> pin `llc -O2`.
                   Its instruction count is not printed: it round-trips the IR through text twice.
    ratio        - ours / pin
    ours B, pin B - `.text` bytes of the two objects (m32 ELF via `size -A`), alignment padding included

These are llrm-cc numbers: never mix them with llrm-c's (tools/compile-cost.py, tools/sizes.py). m16 has no pin
column (the m16 pin is the separate clang build in LLRM_CLANG_M16) and no size column (OMF).
Needs ~/scratch/ow-src (LLRM_OW), the pin in ~/scratch/llvm-build (LLRM_PIN_CLANG), perf. A failed compile stops the run.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import elfsize  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
HOME = Path.home()
PIN = Path(os.environ.get("LLRM_PIN_CLANG", HOME / "scratch/llvm-build/bin/clang"))
DATALAYOUT = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"  # targets.toml [x86_32-linux]
OW = os.environ.get("LLRM_OW", str(HOME / "scratch/ow-src"))

# config -> (LLRM_CC_TARGET, -march, level)
CONFIGS = {
    "m32-O2": ("x86_32-linux", "i486", "-O2"),
    "m32-Os": ("x86_32-linux", "i486", "-Os"),
    "m16-Os": ("x86_16-dos", "i386", "-Os"),
}


def instructions(argv: list[str], env: dict[str, str]) -> int:
    """User-space instructions of one process tree; exit 77-style failure if perf cannot count."""
    with tempfile.NamedTemporaryFile("r") as out:
        run = subprocess.run(
            ["perf", "stat", "-x,", "-e", "instructions:u", "-o", out.name, "--", *argv],
            env=env, capture_output=True, text=True,
        )
        if run.returncode != 0:
            sys.exit(f"failed: {' '.join(argv)}\n{run.stderr[-800:]}")
        for line in out.read().splitlines():
            fields = line.split(",")
            if len(fields) > 2 and fields[2].startswith("instructions"):
                if not fields[0].isdigit():
                    sys.exit("perf cannot count instructions here")
                return int(fields[0])
    sys.exit("perf printed no instruction count")


def pin_ir_command(src: Path, level: str, march: str, includes: list[str]) -> list[str]:
    """The pin's front end as llrm-cc runs it. `-march` is required: without it clang targets pentium4 +sse2."""
    return [str(PIN), "-ffp-contract=off", "-target", "i486-unknown-linux-gnu", "-fno-pic", "-fno-stack-protector",
            "-fno-asynchronous-unwind-tables", f"-march={march}", level, "-fno-builtin", *includes, "-S", "-emit-llvm",
            "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone"]


def pin_same_ir(src: Path, level: str, march: str, includes: list[str], tmp: Path, env: dict[str, str]) -> tuple[int, Path]:
    """The pin on llrm-cc's IR: front end, `opt`, `llc`. Returns (instructions of the three, object)."""
    stem = src.stem
    ll, opt_ll, obj = tmp / f"{stem}.pin.ll", tmp / f"{stem}.pin.opt.ll", tmp / f"{stem}.pin.o"
    bins = PIN.parent
    total = instructions([*pin_ir_command(src, level, march, includes), "-o", str(ll), str(src)], env)
    text = ll.read_text()
    ll.write_text(re.sub(r'target datalayout = "[^"]*"', f'target datalayout = "{DATALAYOUT}"', text, count=1))
    total += instructions(
        [str(bins / "opt"), "-passes=default<O2>", "-S", "-o", str(opt_ll), str(ll)], env)
    total += instructions(
        [str(bins / "llc"), "-O2", "-mtriple=i486-unknown-linux-gnu", f"-mcpu={march}", "-filetype=obj", "-o", str(obj),
         str(opt_ll)], env)
    return total, obj


def text_bytes(obj: Path) -> int:
    out = subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True, check=True).stdout
    if not any(elfsize.is_text(line.split()[0]) for line in out.splitlines() if line.split()):
        sys.exit(f"{obj} has no .text")
    return elfsize.text_bytes(out)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dist", default=str(ROOT / "target/release"))
    ap.add_argument("--programs", default="jpeg_fixed,stft_fixed,nbody,matmul,crc")
    ap.add_argument("--configs", default="m32-O2,m32-Os,m16-Os")
    args = ap.parse_args()
    cc = Path(args.dist) / "llrm-cc"
    posix = ROOT / "runtime/shared/posix"
    isystem = PIN.parent.parent / "lib/clang/24/include"
    print(f"{'program':<12} {'config':<7} {'ours M':>9} {'pin M':>9} {'ratio':>6} {'ours B':>8} {'pin B':>8}")
    with tempfile.TemporaryDirectory() as tmp:
        for name in args.programs.split(","):
            src = ROOT / "bench" / name / f"{name}.c"
            for config in args.configs.split(","):
                target, march, level = CONFIGS[config]
                env = dict(os.environ, LLRM_CC_TARGET=target, LLRM_OW=OW, LLRM_PIN_CLANG=str(PIN))
                extra = ["-I", str(posix)]
                ours_obj = Path(tmp) / f"{name}.{config}.ours.o"
                ours = instructions(
                    [str(cc), level, "-fno-builtin", f"-march={march}", "--isel=dag", *extra, "-I", str(src.parent),
                     "-c", str(src), "-o", str(ours_obj)], env)
                pin = pin_bytes = None
                if target == "x86_32-linux":
                    pin_obj = Path(tmp) / f"{name}.{config}.cc1.o"
                    pin = instructions(
                        [str(PIN), "-cc1", "-triple", "i486-unknown-linux-gnu", "-target-cpu", march, level,
                         "-fno-builtin", "-internal-isystem", str(isystem), "-I", str(posix), "-I", str(src.parent),
                         "-emit-obj", "-o", str(pin_obj), str(src)], env)
                    _, same_ir = pin_same_ir(src, level, march, ["-I", str(posix), "-I", str(src.parent)], Path(tmp), env)
                    pin_bytes = text_bytes(same_ir)
                ob = text_bytes(ours_obj) if target == "x86_32-linux" else None
                fmt = lambda v, d=1: "-" if v is None else f"{v:.{d}f}"
                print(f"{name:<12} {config:<7} {ours / 1e6:>9.1f} {fmt(pin and pin / 1e6):>9} "
                      f"{fmt(pin and ours / pin, 2):>6} {ob if ob is not None else '-':>8} "
                      f"{pin_bytes if pin_bytes is not None else '-':>8}")


if __name__ == "__main__":
    main()

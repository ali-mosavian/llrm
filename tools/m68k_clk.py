#!/usr/bin/env python3
"""Qemu-traced clocks and code bytes of C programs: ours against gcc, per CPU and level. The O2 judge of `m68k_worse_cells.py` needs clocks, which the matrix leaves empty.

m68k_clk.py [--cc LLRM_CC] [--cpus 68000,68020] [--levels O1,O2,Os] [--jobs N] FILE.c...

Each program is linked with the support objects of `m68k_exec_sweep`, run under qemu-m68k and priced by the `cycles` example (CYCLES_CPUS).
An empty clocks cell is unmeasured: no row, a bound (`N*`, unknown loop count) or uncovered code (`?`); never a number.
Environment: GCC_M68K (the cross toolchain root), LLRM_CC, CYCLES (the built `cycles` example; default under CARGO_TARGET_DIR).
"""
import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))


def parse_clocks(stdout):
    """The clocks cell of the first `| NAME.exe | insns | clk | clk/call |` row. `None` (unmeasured) when there is no such row, the cell is empty, or it is a bound (`N*`: unknown loop count) or uncovered (`?`): a bound is not a count."""
    for line in stdout.splitlines():
        if line.startswith("| ") and ".exe" in line:
            cells = [cell.strip() for cell in line.strip("|").split("|")]
            cell = cells[2] if len(cells) > 2 else ""
            return cell if cell.isdigit() else None
    return None


def price(cycles, exe, cpu, gcc_root):
    run = subprocess.run(
        [cycles, "--exe", str(exe)],
        env={**os.environ, "CYCLES_CPUS": cpu, "GCC_M68K": gcc_root},
        capture_output=True,
        text=True,
        timeout=900,
    )
    return parse_clocks(run.stdout)


def main():
    import m68k_exec_sweep as sweep
    import llrmbin
    import o0_sweep

    ap = argparse.ArgumentParser()
    ap.add_argument("--cc", default=os.environ.get("LLRM_CC"))
    ap.add_argument("--cpus", default="68000,68020")
    ap.add_argument("--levels", default="O1,O2,Os")
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("files", nargs="+")
    args = ap.parse_args()
    gcc_root = os.environ["GCC_M68K"]
    target = os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))
    cycles = os.environ.get("CYCLES", f"{target}/release/examples/cycles")
    gcc = f"{gcc_root}/usr/bin/m68k-linux-gnu-gcc"
    size = f"{gcc_root}/usr/bin/m68k-linux-gnu-size"
    print("cpu\tprogram\tlevel\tours_B\tours_clk\tgcc_B\tgcc_clk")
    for cpu in args.cpus.split(","):
        support = sweep.build_support(args.cc, gcc_root, cpu, sweep.cache_dir(args.cc), args.jobs)
        for name in args.files:
            path = Path(name).resolve()
            for level in args.levels.split(","):
                cells = {}
                with tempfile.TemporaryDirectory() as tmp:
                    for side in ("ours", "gcc"):
                        obj, exe = Path(tmp) / f"{side}.o", Path(tmp) / f"{side}.exe"
                        if side == "ours":
                            env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": f"m68k-{cpu}"}
                            cmd = [args.cc, f"-{level}", "-w", "-c", *o0_sweep.flags_of(path.read_text(), f"m68k-{cpu}"),
                                   "-I", str(ROOT / "runtime/shared/posix"), "-o", str(obj), str(path)]
                        else:
                            env = sweep.tool_env(gcc_root)
                            cmd = [gcc, f"-B{gcc_root}/usr/lib/gcc-cross/m68k-linux-gnu/15/", f"-B{gcc_root}/usr/m68k-linux-gnu/bin/",
                                   "-ffreestanding", "-nostdlib", "-w", "-msoft-float", f"-mcpu={cpu}", f"-{level}",
                                   "-isystem", str(ROOT / "runtime/shared/posix"), "-c", str(path), "-o", str(obj)]
                        if subprocess.run(cmd, capture_output=True, text=True, env=env).returncode:
                            cells[side] = ("", "")
                            continue
                        link = subprocess.run([gcc, f"-m{cpu}", "-msoft-float", "-nostdlib", "-static", "-ffreestanding", "-o", str(exe), str(obj), *support],
                                              capture_output=True, text=True, env=sweep.tool_env(gcc_root))
                        if link.returncode:
                            cells[side] = ("", "")
                            continue
                        text = subprocess.run([size, str(obj)], capture_output=True, text=True, env=sweep.tool_env(gcc_root)).stdout
                        cells[side] = (text.split("\n")[1].split()[0], price(cycles, exe, cpu, gcc_root) or "")
                print("\t".join([cpu, path.stem, level, *cells["ours"], *cells["gcc"]]), flush=True)


if __name__ == "__main__":
    main()

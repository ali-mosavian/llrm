#!/usr/bin/env python3
"""Qemu-traced clocks and code bytes of C programs: ours against gcc, per CPU and level. The O2 judge of `m68k_worse_cells.py` needs clocks, which the matrix leaves empty.

m68k_clk.py [--cc LLRM_CC] [--cpus 68000,68020] [--levels O1,O2,Os] [--jobs N] FILE.c...

Each program is linked with the support objects of `m68k_exec_sweep`, run under qemu-m68k and priced by the `cycles` example (CYCLES_CPUS).
The support objects (compiler-rt archive) are built by the compiler under test and linked into BOTH the ours and the gcc executables: cells that call the library (soft float, multiply, divide) compare user code only, and library codegen differences against gcc are invisible.
An empty clocks cell is unmeasured: no row or uncovered code (`?`); never a number.
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


def sources_stamp(root):
    """The stamp `crates/target/llrm-m68k/build.rs` bakes into `cycles` (`cycles --stamp`): FNV-1a 64 over the relative path and text of every file under the crate's `src` and `examples`, in path order. A binary whose stamp differs was built from other sources."""
    crate = Path(root) / "crates/target/llrm-m68k"
    names = sorted(
        (str(path.relative_to(crate)).replace(os.sep, "/"), path)
        for sub in ("src", "examples")
        for path in (crate / sub).rglob("*")
        if path.is_file()
    )
    value = 0xCBF29CE484222325
    for name, path in names:
        for chunk in (name.encode(), b"\0", path.read_bytes(), b"\0"):
            for byte in chunk:
                value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{value:016x}"


def verify_cycles(cycles, root, target_dir):
    """Refuse a `cycles` binary that is not this checkout's: outside its target dir (another session's build priced a whole census from before #10557: `>=` where the tip says `<=`), or built from other `llrm-m68k` sources (stale)."""
    path = Path(cycles).resolve()
    allowed = [Path(target_dir).resolve(), (Path(root) / "target").resolve()]
    if not any(path.is_relative_to(one) for one in allowed):
        raise SystemExit(f"m68k_clk: CYCLES {path} is outside this checkout's target dir ({allowed[0]}): build it here (cargo build -p llrm-m68k --example cycles)")
    out = subprocess.run([str(path), "--stamp"], capture_output=True, text=True)
    if out.returncode or out.stdout.strip() != sources_stamp(root):
        raise SystemExit(f"m68k_clk: CYCLES {path} was built from other llrm-m68k sources (stamp {out.stdout.strip() or 'none'}, this checkout {sources_stamp(root)}): rebuild it")


def gcc_is_comparable(source):
    """False where gcc's column is not like-for-like: a program naming `long double` (gcc's is 12-byte extended, ours and the pin's 8-byte double; the corpus matrix leaves the column empty). Reading them, long_double_layout -Os was +435% bytes."""
    import m68k_corpus_matrix

    return not m68k_corpus_matrix.long_double_abi_mismatch(source)


def library_bytes(cc, gcc_root, cpu, level, obj):
    """The .text bytes of the compiler-rt members `obj` pulls (as `m68k_program_size.py`: the program's code alone hides a libcall one side makes and the other inlines, shift64_variable -Os). None when the link fails."""
    import m68k_exec_sweep as sweep
    import m68k_program_size as program_size

    crt, archive, stubs = program_size.support(cc, gcc_root, cpu, level, sweep.cache_dir(cc))
    mapfile = Path(f"{obj}.map")
    link = subprocess.run(program_size.link_command(f"{gcc_root}/usr/bin/m68k-linux-gnu-", cpu, obj, Path(f"{obj}.lib.exe"), mapfile, crt, archive, stubs),
                          capture_output=True, text=True, env=sweep.tool_env(gcc_root))
    if link.returncode:
        return None
    return sum(program_size.members_of(program_size.text_bytes(mapfile.read_text())).values())


def loaded_bytes(size_output):
    """The loaded footprint of `size`'s Berkeley row: text (code and read-only data) plus data. The text column alone left out `.data`, so a static table
    that is not const (word_compare_branches, 56 B) was in neither side's bytes."""
    columns = size_output.split("\n")[1].split()
    return str(int(columns[0]) + int(columns[1]))


def parse_clocks(stdout):
    """The clocks cell of the first `| NAME.exe | insns | clk | clk/call |` row. `None` (unmeasured) when there is no such row, the cell is empty, or it is uncovered (`?`). A bound is kept as a bound, never as a count: `>=N` (`N*`: a shift count or loop count at its lower bound) and `<=N` (only multiplies and divides at their published maximum)."""
    for line in stdout.splitlines():
        if line.startswith("| ") and ".exe" in line:
            cells = [cell.strip() for cell in line.strip("|").split("|")]
            cell = cells[2] if len(cells) > 2 else ""
            if cell.endswith("*") and cell[:-1].isdigit():
                return f">={cell[:-1]}"
            return cell if cell.isdigit() or (cell.startswith("<=") and cell[2:].isdigit()) else None
    return None


def cycles_env(cpu, gcc_root):
    """`cycles --exe` traces on the qemu CPU its path names (`m68020` for a path containing 68020, else `m68000`): the temp exe's path names none, so a 68020 program ran on a 68000 and stopped at its first 68020-only instruction (gcc's, 23 instructions of u64_divide_wide_divisor against ours's 137258), pricing a handful of clocks."""
    return {**os.environ, "CYCLES_CPUS": cpu, "CYCLES_QEMU_CPU": f"m{cpu}", "GCC_M68K": gcc_root}


def price(cycles, exe, cpu, gcc_root):
    run = subprocess.run(
        [cycles, "--exe", str(exe)],
        env=cycles_env(cpu, gcc_root),
        capture_output=True,
        text=True,
        timeout=900,
    )
    return parse_clocks(run.stdout)


def same_output(runs):
    """Whether the two columns' runs printed and exited alike. A pair that differs ran different programs (a gcc exe that stops early prices a handful of instructions): its clocks are not compared."""
    return len(runs) == 2 and runs[0] == runs[1]


def run_once(exe, cpu):
    """(returncode, stdout) of the program under qemu, or None when it does not finish."""
    try:
        done = subprocess.run(["qemu-m68k", "-cpu", f"m{cpu}", str(exe)], capture_output=True, text=True, timeout=60, stdin=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        return None
    return (done.returncode, done.stdout)


def gcc_command(gcc, gcc_root, cpu, level, path, obj, pcrel, flags=()):
    """gcc's compile line; `pcrel` puts its data on ours's pc-relative model (`-mpcrel`), so the clocks of the two columns share one data model.
    `flags` are the program's own `// flags:` in gcc's spelling (ValueError for one gcc cannot take): ours was built with `-fno-inline-functions`
    and gcc without, so licm_graph_callee_footprint priced 188244 against 77282 clocks, gcc having inlined the call the program is about.
    Hosted, as ours is since #10561: `-ffreestanding` kept gcc from forming memset/memcpy calls, so ours called the harness stub where gcc inlined its fills."""
    import external_flags

    return [gcc, f"-B{gcc_root}/usr/lib/gcc-cross/m68k-linux-gnu/15/", f"-B{gcc_root}/usr/m68k-linux-gnu/bin/",
            "-nostdlib", "-w", *external_flags.gcc_float("m68k"), *external_flags.flags("none", (), 15), *(["-mpcrel"] if pcrel else []), f"-mcpu={cpu}", f"-{level}",
            *external_flags.GCC_DIALECT, *external_flags.gcc_source_defines(Path(path).read_text() if Path(path).is_file() else ""), *external_flags.gcc_program_flags(flags), *(["-isystem", os.environ["M68K_GCC_HEADERS"]] if os.environ.get("M68K_GCC_HEADERS") else []), "-isystem", str(ROOT / "runtime/shared/posix"), "-c", str(path), "-o", str(obj)]


def main():
    import m68k_exec_sweep as sweep
    import llrmbin
    import o0_sweep

    ap = argparse.ArgumentParser()
    ap.add_argument("--cc", default=os.environ.get("LLRM_CC"))
    ap.add_argument("--cpus", default="68000,68020")
    ap.add_argument("--levels", default="O1,O2,Os")
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--gcc-pcrel", action="store_true", help="gcc -mpcrel: ours's data model")
    ap.add_argument("--libs-only", action="store_true", help="bytes and library bytes only: no link, no run, no clocks (a card's lib columns without its hours of qemu)")
    ap.add_argument("files", nargs="+")
    args = ap.parse_args()
    gcc_root = os.environ["GCC_M68K"]
    target = os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))
    cycles = os.environ.get("CYCLES", f"{target}/release/examples/cycles")
    if not args.libs_only:
        verify_cycles(cycles, ROOT, target)
    gcc = f"{gcc_root}/usr/bin/m68k-linux-gnu-gcc"
    size = f"{gcc_root}/usr/bin/m68k-linux-gnu-size"
    print("cpu\tprogram\tlevel\tours_B\tours_clk\tgcc_B\tgcc_clk\tours_lib\tgcc_lib")
    for cpu in args.cpus.split(","):
        support = sweep.build_support(args.cc, gcc_root, cpu, sweep.cache_dir(args.cc), args.jobs)
        for name in args.files:
            path = Path(name).resolve()
            for level in args.levels.split(","):
                cells, runs, libs = {}, {}, {}
                with tempfile.TemporaryDirectory() as tmp:
                    for side in ("ours", "gcc"):
                        obj, exe = Path(tmp) / f"{side}.o", Path(tmp) / f"{side}.exe"
                        if side == "ours":
                            env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": f"m68k-{cpu}"}
                            cmd = [args.cc, f"-{level}", "-w", "-c", *o0_sweep.flags_of(path.read_text(), f"m68k-{cpu}"),
                                   "-I", str(ROOT / "runtime/shared/posix"), "-o", str(obj), str(path)]
                        else:
                            env = sweep.tool_env(gcc_root)
                            if not gcc_is_comparable(path.read_text()):
                                cells[side] = ("", "")
                                continue
                            try:
                                cmd = gcc_command(gcc, gcc_root, cpu, level, path, obj, args.gcc_pcrel, o0_sweep.flags_of(path.read_text(), f"m68k-{cpu}"))
                            except ValueError:
                                cells[side] = ("", "")
                                continue
                        if subprocess.run(cmd, capture_output=True, text=True, env=env).returncode:
                            cells[side] = ("", "")
                            continue
                        if args.libs_only:
                            library = library_bytes(args.cc, gcc_root, cpu, level, obj)
                            libs[side] = "" if library is None else str(library)
                            cells[side] = (loaded_bytes(subprocess.run([size, str(obj)], capture_output=True, text=True, env=sweep.tool_env(gcc_root)).stdout), "")
                            continue
                        link = subprocess.run([gcc, f"-m{cpu}", *external_flags.gcc_float("m68k"), "-nostdlib", "-static", "-ffreestanding", "-o", str(exe), str(obj), *support],
                                              capture_output=True, text=True, env=sweep.tool_env(gcc_root))
                        if link.returncode:
                            cells[side] = ("", "")
                            continue
                        text = subprocess.run([size, str(obj)], capture_output=True, text=True, env=sweep.tool_env(gcc_root)).stdout
                        runs[side] = run_once(exe, cpu)
                        library = library_bytes(args.cc, gcc_root, cpu, level, obj)
                        libs[side] = "" if library is None else str(library)
                        cells[side] = (loaded_bytes(text), price(cycles, exe, cpu, gcc_root) or "")
                    if not args.libs_only and not same_output([runs.get("ours"), runs.get("gcc")]):
                        cells = {side: (cell[0], "") for side, cell in cells.items()}  # bytes stay, clocks of unequal runs do not
                print("\t".join([cpu, path.stem, level, *cells.get("ours", ("", "")), *cells.get("gcc", ("", "")), libs.get("ours", ""), libs.get("gcc", "")]), flush=True)


if __name__ == "__main__":
    main()

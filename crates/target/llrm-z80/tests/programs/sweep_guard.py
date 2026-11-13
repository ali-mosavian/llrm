"""The row-count guard of the size/clock sweep: a program that produced no `ours` row is named and fails the run.

The sweep once printed `112 programs 0 failed` with 3 rows missing (2026-11-09): a program whose build or link
never reached a result left no FAILED row either, so the tally counted only the rows it was shown.
"""

import os
import subprocess

# The library's own run checks: f64 divide and multiply against the host FPU on ucsim_z80, and the link-size bounds of the split members.
# They sat in no runner (2026-11-13): a wrong f64 divide (3.0/1.0 = 4) failed tests/z80/f64_div_clk.sh for a day unseen.
LIBRARY_CHECKS = (
    "runtime/compiler-rt/tests/z80/f64_div_clk.sh",
    "runtime/compiler-rt/tests/z80/f64_mul_clk.sh",
    "runtime/compiler-rt/tests/z80/mul64_check.sh",
    "runtime/compiler-rt/tests/z80_f64_split_test.sh",
)


def failed_library_checks(root, llc, checks=LIBRARY_CHECKS):
    """`{script: last line of its output}` for each of `checks` (under `root`) that exits non-zero with `llc` as the compiler under test."""
    failed = {}
    for script in checks:
        done = subprocess.run(
            ["bash", os.path.join(root, script)], env={**os.environ, "LLRM_Z80_LLC": llc}, capture_output=True, text=True
        )
        if done.returncode:
            lines = (done.stdout + done.stderr).strip().splitlines()
            failed[script] = lines[-1] if lines else f"exit {done.returncode}"
    return failed


def incomparable(source):
    """`WHY; twin NAME` for a program whose SDCC columns are not a bar, from its `// bar: not comparable (WHY); twin NAME` line,
    or None. SDCC compiles a different program when the two compilers lay the same struct out differently
    (volatile_bitfield, 2026-11-12: `f:15` at bit 3 under clang's packed layout, at a byte under SDCC's: +223 B read as a gap)."""
    for line in source.splitlines():
        if line.startswith("// bar: not comparable"):
            return line.removeprefix("// bar: not comparable").strip().removeprefix("(").replace("); twin ", "; twin ").rstrip(")")
    return None


def dropped(expected, results, required=lambda tag: tag == "ours", excused=()):
    """`{program: reason}` for every program of `expected` without a row whose column `required` accepts (`ours`).

    `excused`: programs whose missing row has a recorded reason (an SDCC column the compiler rejects is a gap
    in the bar, listed, not a dropped row). `results`: `(name, [(tag, bytes, clocks), ...])` as the sweep collects them. A program the pool never
    returned and one that returned no `ours` row are both dropped; `reasons` of the sweep refine the message.
    """
    have = {name for name, rows in results if any(required(tag) for tag, _, _ in rows)}
    return {name: "NOROW" for name in expected if name not in have and name not in excused}


def summary(expected, results, reasons=None, required=lambda tag: tag == "ours", excused=()):
    """`(line, ok)`: the sweep's last line and whether it passed. The line names the dropped programs."""
    reasons = reasons or {}
    lost = dropped(expected, results, required, excused)
    failed = {name for name, rows in results for tag, _, _ in rows if tag == "FAILED"} | set(lost)
    have = {name for name, rows in results if any(required(tag) for tag, _, _ in rows)}
    line = f"{len(expected)} programs {len(failed)} failed; rows {len(have & set(expected))} of {len(expected)}"
    if failed:
        line += "; no row: " + " ".join(f"{n}({reasons.get(n, lost.get(n, 'FAILED'))})" for n in sorted(failed))
    return line, not failed


def read_gcc_column(path):
    """`{name: (bytes, clocks)}` of the gcc-z80 fork's run of the corpus (`run.py CC1 LEVEL OUT.tsv`, rows `name\tgcc\tbytes\tclocks`)."""
    column = {}
    for line in open(path):
        name, tag, size, clocks = line.rstrip("\n").split("\t")[:4]
        if tag == "gcc":
            column[name] = (int(size), int(clocks))
    return column


def add_gcc_row(rows, gcc_cell, not_a_bar):
    """`rows` with the fork's `gcc` row beside them: a bar of its own where SDCC gives none (13 of 36 cells, 2026-11-13) and one more
    column where it does (the bar is the lowest column per metric). Not for a program that computes a different program under
    the fork (its `double` is 32 bits, as SDCC's: `incomparable`)."""
    if gcc_cell is None or not_a_bar or any(tag == "gcc" for tag, _, _ in rows):
        return rows
    return rows + [("gcc", *gcc_cell)]


def has_bar(rows):
    """Whether any column besides ours bars the program."""
    return any(tag.startswith("sdcc") or tag == "gcc" for tag, _, _ in rows)


def softfloat_library_missing(environ, sdcc_only, no_softfloat):
    """Why a sweep must not start, or None. Our float and double programs link `__adddf3` and its kin from the soft-float library
    (`Z80_LIBC_EXTRA`); without it 13 of 121 programs fail to link and read as 'no row' (2026-11-12)."""
    if sdcc_only or no_softfloat or environ.get("Z80_LIBC_EXTRA"):
        return None
    return (
        "Z80_LIBC_EXTRA is unset: the float and double programs will not link. Build the library with "
        "runtime/compiler-rt/tests/z80_lib.sh OUTDIR (LLRM_Z80_LLC=the compiler under test) and export Z80_LIBC_EXTRA=OUTDIR, "
        "or pass --no-softfloat to leave them out."
    )

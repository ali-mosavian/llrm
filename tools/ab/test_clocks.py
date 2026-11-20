"""The instruction count of a cell is one number whatever the binary is called and whatever shell runs it."""
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

import clocks

PROGRAM = "int main(void) { return 0; }\n"


def build(directory: Path) -> Path:
    source = directory / "t.c"
    source.write_text(PROGRAM)
    exe = directory / "t"
    subprocess.run(["gcc", "-static", "-no-pie", "-o", str(exe), str(source)], check=True)
    return exe


def naive(exe: Path, env: dict) -> int:
    """What the matrix read before: the binary where it was built, in the caller's environment."""
    done = subprocess.run(["perf", "stat", "-x,", "-e", "instructions:u", str(exe)], capture_output=True, text=True, env=env)
    return min(int(line.split(",")[0]) for line in done.stderr.splitlines() if "instructions" in line)


def test_the_count_does_not_depend_on_the_name_or_the_environment():
    """`tag.ours` and `tag.clang` of one program read 56 instructions apart per 8 characters of name, and a 300-byte
    environment variable added 235: ties with the best of gcc and clang flipped from run to run on no code change."""
    work = Path(tempfile.mkdtemp(prefix="clk"))
    try:
        exe = build(work)
        short, long = work / "a", work / "a_much_longer_name_for_the_same_binary"
        shutil.copy2(exe, short)
        shutil.copy2(exe, long)
        loud = {**os.environ, "CLOCKS_FIXTURE_PADDING": "x" * 300}
        assert naive(short, os.environ) != naive(long, loud), "the fixture must show the drift it guards against"
        assert clocks.count(short) == clocks.count(long)
    finally:
        shutil.rmtree(work, ignore_errors=True)


def cell(ours, gcc, clang):
    return {"ours": {"instr": ours}, "gcc": {"instr": gcc}, "clang": {"instr": clang}}


def test_a_tie_that_moves_by_noise_is_neither_gained_nor_lost():
    """switch_header_marker_loop read 2441 then 2423 against a best of 2426 on one compiler: a counted cell lost and regained."""
    base = {"tie": cell(2441, 2422, 2427), "flat": cell(1000, 1000, 1010)}
    new = {"tie": cell(2423, 2426, 2427), "flat": cell(1004, 1000, 1010)}
    assert clocks.verdicts(base, new) == {"gained": [], "lost": [], "moved": [], "failed": []}


def test_a_move_of_more_than_one_percent_across_the_bar_is_gained_or_lost():
    base = {"win": cell(1000, 950, 980), "regress": cell(900, 950, 980)}
    new = {"win": cell(900, 950, 980), "regress": cell(1000, 950, 980)}
    assert clocks.verdicts(base, new) == {"gained": ["win"], "lost": ["regress"], "moved": ["regress", "win"], "failed": []}


def test_a_cell_that_falls_with_both_bar_columns_is_not_lost():
    """merge_icmps_fields: ours 2680 to 2579 (-3.8%) while gcc fell 2708 to 2607 and clang 2697 to 2569 on the same compilers.
    It read as a counted cell lost, on drift of the instrument and no code change."""
    base = {"merge_icmps_fields": cell(2680, 2708, 2697)}
    new = {"merge_icmps_fields": cell(2579, 2607, 2569)}
    assert clocks.verdicts(base, new) == {"gained": [], "lost": [], "moved": [], "failed": []}


def test_a_move_larger_than_the_drift_still_counts():
    base = {"win": cell(1000, 950, 980)}
    new = {"win": cell(850, 960, 990)}
    assert clocks.verdicts(base, new) == {"gained": ["win"], "lost": [], "moved": ["win"], "failed": []}


def test_a_program_with_an_lp64_expectation_is_checked_against_it():
    """`call_keeps.out` is the ILP32 output; x86_64 prints `call_keeps.lp64.out`. Read the wrong one, ours, gcc and clang all
    came out "not ok" (42 of 211 cells at -O2) and the cell was never counted."""
    work = Path(tempfile.mkdtemp(prefix="clk"))
    try:
        source = work / "p.c"
        source.write_text(PROGRAM)
        assert clocks.expected_output(source) is None
        source.with_suffix(".out").write_text("ilp32\n")
        assert clocks.expected_output(source) == "ilp32\n"
        source.with_suffix(".lp64.out").write_text("lp64\n")
        assert clocks.expected_output(source) == "lp64\n"
    finally:
        shutil.rmtree(work, ignore_errors=True)


FLAGGED = "// flags: -O2 -fno-inline-functions\nint f(void) { return 1; }\n"


def test_the_bar_columns_are_the_matrixs_not_the_distros():
    """gcc -m64 on a distro adds endbr64 per function and a canary per array frame: instructions the bar does not owe
    (the size matrix drops them in `external_flags`; the clocks columns did not, so gcc read high and ours looked better)."""
    lines = clocks.compile_lines("/dist", FLAGGED, "O2")
    for column in ("gcc", "clang"):
        assert "-fcf-protection=none" in lines[column] and "-fno-stack-protector" in lines[column], column


def test_every_column_takes_the_programs_own_flags():
    """40 programs say `-fno-inline-functions`; ours, gcc and clang were all built without it."""
    lines = clocks.compile_lines("/dist", FLAGGED, "O2")
    assert "-fno-inline-functions" in lines["ours"] and "-fno-inline-functions" in lines["clang"]
    assert "-fno-inline" in lines["gcc"]


def test_a_program_for_another_target_is_not_an_x86_64_cell():
    """`// targets: dos` programs were built and counted as x86_64 cells."""
    assert not clocks.is_cell("// targets: dos\nint main(void) { return 0; }\n")
    assert clocks.is_cell(FLAGGED)


def test_a_program_gcc_cannot_take_has_no_gcc_column():
    clang_only = "// flags: -fsanitize=stack\nint main(void) { return 0; }\n"
    assert clocks.compile_lines("/dist", clang_only, "O2")["gcc"] is None


def test_every_column_has_the_same_unwind_tables():
    """The size matrix's bars drop `.eh_frame` (`-fno-asynchronous-unwind-tables`; it is not code bytes). Ours emits it, and a
    program with `.eh_frame` runs 560 more instructions of runtime start-up and exit than one without, on identical code
    (value_of_valueless: same 10-instruction main, 137,834 against 137,260): the bars read 25-34% under ours on that alone,
    127 cells of the first v2 census flipped from counted to not."""
    lines = clocks.compile_lines("/dist", FLAGGED, "O2")
    for column in ("gcc", "clang"):
        assert "-fno-asynchronous-unwind-tables" not in lines[column], column


def test_a_program_ours_no_longer_builds_is_failed_not_unchanged():
    """The v2i64 multiply row made llc refuse scroll, quicksort and dot (CannotSelect): their `ours` count read None, the
    verdicts skipped them and the A/B said "no loss" while dot lost 97% once it built."""
    cell = lambda ours: {"ours": {"instr": ours}, "gcc": {"instr": 100}, "clang": {"instr": 100}}
    base, new = {"a": cell(100), "b": cell(100)}, {"a": cell(None), "b": cell(100)}
    assert clocks.verdicts(base, new)["failed"] == ["a"]

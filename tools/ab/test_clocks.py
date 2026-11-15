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
    assert clocks.verdicts(base, new) == {"gained": [], "lost": [], "moved": []}


def test_a_move_of_more_than_one_percent_across_the_bar_is_gained_or_lost():
    base = {"win": cell(1000, 950, 980), "regress": cell(900, 950, 980)}
    new = {"win": cell(900, 950, 980), "regress": cell(1000, 950, 980)}
    assert clocks.verdicts(base, new) == {"gained": ["win"], "lost": ["regress"], "moved": ["regress", "win"]}

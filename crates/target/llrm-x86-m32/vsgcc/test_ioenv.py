"""Regression tests of the file programs' instrument: each name says what went wrong and what it produced."""
import subprocess
import struct
from pathlib import Path

import pytest

import harness
import ioenv
import programs
import wrap


@pytest.mark.parametrize("name", programs.FILE_PROGRAMS)
def test_a_file_program_builds_under_gcc_through_wrap_py(name, tmp_path):
    """stft, grep and jpeg were UNMEASURED at m32: their `#include "../bench_io.h"` reaches llrm_os.h (`#pragma aux`), which gcc
    and clang cannot take (both predefine `__INT_MAX__`, so they took the Watcom branch), and no column of the table was built."""
    source = tmp_path / f"{name}.c"
    source.write_text(wrap.wrapped(name, (harness.BENCH / name / f"{name}.c").read_text()))
    assert "#include" not in "".join(l for l in source.read_text().splitlines() if "bench_io" in l)
    done = subprocess.run(["gcc", "-m32", "-w", "-fsyntax-only", str(source)], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr[:400]


def test_a_kernel_copy_of_a_bench_program_finds_the_data_its_original_names():
    """x_jpeg_single named `photo.jpg` beside the kernel, where it is not: harness.py died with FileNotFoundError and no cell
    of the program had a row (x87 session, 11-11)."""
    source = Path(__file__).parent / "kernels/x_jpeg_single/x_jpeg_single.c"
    files, stdin = ioenv.inputs(source)
    assert list(files) == ["PHOTO.JPG"] and files["PHOTO.JPG"][:2] == b"\xff\xd8" and stdin is None


class Memory:
    """The part of a unicorn handle the file calls use."""

    def __init__(self):
        self.bytes = bytearray(4096)

    def mem_read(self, at, size):
        return bytes(self.bytes[at:at + size])

    def mem_write(self, at, data):
        self.bytes[at:at + len(data)] = data


def test_a_missing_file_opens_below_zero_and_is_counted_not_passed_over():
    """m68k and arm64 sweeps ran stft and jpeg with no input file: the program returned -1 early, every column agreed on nothing."""
    uc, files = Memory(), ioenv.Files({"SIGNAL": b"ab"}, None)
    uc.mem_write(100, b"NOSUCH\0")
    assert struct.unpack("<i", struct.pack("<I", files.call(uc, "vs_open_read", [100, 0, 0, 0])))[0] < 0
    assert files.summary()["failed_opens"] == 1


def test_a_file_reads_in_chunks_a_created_file_keeps_what_was_written_and_handle_one_is_the_console():
    uc, files = Memory(), ioenv.Files({"IN": b"hello world"}, b"stdin!")
    uc.mem_write(100, b"IN\0OUT\0")
    reader = files.call(uc, "vs_open_read", [100, 0, 0, 0])
    assert files.call(uc, "vs_read", [reader, 200, 8, 0]) == 8 and uc.mem_read(200, 8) == b"hello wo"
    assert files.call(uc, "vs_read", [reader, 200, 8, 0]) == 3 and files.call(uc, "vs_read", [reader, 200, 8, 0]) == 0
    writer = files.call(uc, "vs_create", [103, 0, 0, 0])
    uc.mem_write(300, b"xyz")
    assert files.call(uc, "vs_write", [writer, 300, 3, 0]) == 3 and files.call(uc, "vs_write", [1, 300, 2, 0]) == 2
    assert files.call(uc, "input_read", [400, 4, 0, 0]) == 4 and uc.mem_read(400, 4) == b"stdi"
    assert files.call(uc, "vs_close", [reader, 0, 0, 0]) == 0 and files.summary()["written"]["OUT"][0] == 3 and bytes(files.stdout) == b"xy"


def test_grep_is_fed_the_cut_the_default_bench_times_and_checked_against_its_ci_answer():
    """Fed all 10 MB it would reach the 300M-instruction cap, and `grep.out` is the answer for the whole file, not the cut."""
    stdin = ioenv.inputs(harness.BENCH / "grep" / "grep.c")[1]
    assert len(stdin) == 1048576 and harness.expected_file("grep").name == "grep.ci.out" and harness.expected_file("stft_fixed").name == "stft_fixed.out"


@pytest.fixture(scope="module")
def fib_built(tmp_path_factory):
    """A work directory of its own with the stub and gcc's `fib` alone (what the cap test runs)."""
    import os
    import sys
    sys.path.insert(0, str(harness.REPO / "tools"))
    import linkrecipe
    work = tmp_path_factory.mktemp("vsgcc-fib")
    done = subprocess.run(["gcc", "-m32", "-c", str(harness.Path(__file__).parent / "stub.s"), "-o", str(work / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    done = subprocess.run(["ld", "-m", linkrecipe.ld_emulation("x86-m32"), "-static", "-e", "0", "-Ttext=0x8000", "-o", str(work / "stub.elf"), str(work / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    previous = harness.OUT
    harness.OUT = work
    harness.record_stub(work)
    build = subprocess.run([str(harness.Path(__file__).parent / "build.sh"), "fib"], capture_output=True, text=True, env={**os.environ, "VSGCC_WORK": str(work)})
    if "FAIL gcc" in build.stdout or not (work / "b" / "fib.gccO2.elf").exists():
        pytest.skip("gcc's fib did not build here")
    yield work
    harness.OUT = previous


def test_a_run_stopped_by_the_instruction_cap_is_an_error_not_a_short_count(fib_built):
    """`emu_start(count=limit)` returns quietly at the cap: a 10 MB grep would have been priced for the first 300M instructions, or fewer, as if it had finished."""
    assert harness.run("fib", "gccO2")["clocks"] > 0
    with pytest.raises(AssertionError, match="cap"):
        harness.run("fib", "gccO2", limit=50)


XPREC = """extern void report(long value);
volatile double half = 1.5;
long bench_xprec(void)
{
    double d = 1.0;
    int i;
    long first = (long)half;    /* fnstcw / fldcw / fistp / fldcw: the word it restores is the one it saved */
    for (i = 0; i < 1000; ++i)
        d += 1e-9;
    return (long)((d - 1.0) * 1e9 + 0.5) + first - 1;
}
int main(void)
{
    report(bench_xprec());
    return 0;
}
"""


def test_x87_code_runs_in_extended_precision_as_a_process_starts(fib_built):
    """A fresh unicorn CPU holds an x87 control word of 0, and the float-to-integer conversion saves and restores it (`fnstcw; fldcw; fistp;
    fldcw`): from the first conversion on every x87 result was rounded to 24 bits. stft_single wrote 1 bin of 65,919 one low in all 16
    cells, its checksum 204598972 against the 78803551 of the same gcc object run natively, and read `ok: false`."""
    import os
    import sys
    sys.path.insert(0, str(harness.REPO / "tools"))
    import linkrecipe
    source = fib_built / "b" / "xprec.c"
    source.write_text(XPREC)
    build = subprocess.run(["gcc", "-m32", "-O1", "-march=i486", "-fno-pic", "-c", "-o", str(fib_built / "b" / "xprec.gccO1.o"), str(source)], capture_output=True, text=True)
    assert build.returncode == 0, build.stderr
    link = subprocess.run(["ld", "-m", linkrecipe.ld_emulation("x86-m32"), "-static", "-e", "0", "-Ttext=0x10000", f"--just-symbols={fib_built / 'stub.elf'}", "-o",
                           str(fib_built / "b" / "xprec.gccO1.elf"), str(fib_built / "b" / "xprec.gccO1.o")], capture_output=True, text=True)
    assert link.returncode == 0, link.stderr
    assert harness.run("xprec", "gccO1")["reports"] == [1000]

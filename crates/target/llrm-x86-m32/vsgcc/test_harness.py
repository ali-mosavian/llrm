"""Regression tests of the instrument: each name says what went wrong and what it produced."""
import json
from pathlib import Path

import pytest

import harness

BENCH = harness.BENCH
HERE = Path(__file__).parent
RESULTS = harness.OUT / "results.jsonl"   # a run of run.sh, if there was one: read before any test points OUT elsewhere


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    """A work directory of its own with what these tests run: the stub and the gcc and clang objects of scroll, fib and x_ll_div."""
    import os
    import subprocess
    import sys
    sys.path.insert(0, str(harness.REPO / "tools"))
    import linkrecipe
    work = tmp_path_factory.mktemp("vsgcc")
    env = {**os.environ, "VSGCC_WORK": str(work)}
    (work / "o").mkdir()
    (work / "b").mkdir()
    done = subprocess.run(["gcc", "-m32", "-c", str(HERE / "stub.s"), "-o", str(work / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    done = subprocess.run(["ld", "-m", linkrecipe.ld_emulation("x86-m32"), "-static", "-e", "0", "-Ttext=0x8000", "-o", str(work / "stub.elf"), str(work / "stub.o")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    previous = harness.OUT
    harness.OUT = work
    harness.record_stub(work)
    for prog in ("scroll", "fib", "x_ll_div"):
        done = subprocess.run([str(HERE / "build.sh"), prog], capture_output=True, text=True, env=env)
        assert done.returncode == 0 and "FAIL" not in done.stdout, (prog, done.stdout, done.stderr)
    yield work
    harness.OUT = previous


def expected(prog):
    return [int(x) for x in (BENCH / prog / f"{prog}.out").read_text().split()]


def test_memmove_stand_in_copies_overlap_backwards(built):
    """The stub's memmove copied forwards: gcc/clang scroll (loops turned into memmove) reported 32636400, not 32634864."""
    for v in ("gccO2", "clangO2"):
        assert harness.run("scroll", v)["reports"] == expected("scroll")


def test_a_divide_gcc_sends_to_libgcc_is_counted_in_its_code(built):
    """x_ll_div read 508 B for gcc -O2 against llrm's 1,174 B (2.31x, the table's worst code row): the i64 divides gcc calls
    are libgcc's, and the count was of its object, which leaves them out; linked, it is 1,477 B and llrm is 0.79x."""
    import subprocess
    obj = subprocess.run(["size", "-A", str(harness.OUT / "b" / "x_ll_div.gccO2.o")], capture_output=True, text=True).stdout
    object_text = sum(int(l.split()[1]) for l in obj.splitlines() if l.startswith(".text"))
    assert harness.code_bytes("x_ll_div", "gccO2") > object_text + 500, object_text


def test_kernel_inlined_into_main_is_not_counted_as_zero(built):
    """gcc inlined bench_fib into main: the region was never entered and fib read 0 instructions for gcc."""
    assert harness.run("fib", "gccO2")["ins"] > 0


@pytest.mark.skipif(not RESULTS.exists(), reason="reads the results of a run: run crates/target/llrm-x86-m32/vsgcc/run.sh first, or it checks nothing")
def test_every_result_reproduces_the_benchmark_output():
    for line in RESULTS.read_text().splitlines():
        r = json.loads(line)
        assert r["ok"], (r["prog"], r["variant"], r["got"])


def test_build_script_flags_are_accepted_by_llrm_c():
    """build.sh/ctime.py passed the removed --target: every llrm build failed with exit 2 and run.sh printed no tables."""
    import subprocess
    if not harness.LLRM.exists():
        pytest.skip(f"no llrm-c at {harness.LLRM}")
    out = subprocess.run([str(harness.LLRM), "-m32", "-O2", "-march=i486", "-fno-inline-functions", "-o", "/dev/null", str(BENCH / "sieve/sieve.c")], capture_output=True, text=True)
    assert out.returncode == 0, out.stderr
    here = Path(__file__).parent
    assert "--target" not in (here / "build.sh").read_text() + (here / "ctime.py").read_text()


def test_a_multiply_costs_what_its_multiplier_is():
    """The 486's MUL and IMUL were priced at their least, 13 clocks, whatever the multiplier: gcc's reciprocal `mul` by
    0xCCCCCCCD (42 clocks by the data sheet) read as cheap, and frames' `/ 10` as 2.2x for llrm's `div`."""
    assert harness.multiply_clocks(0, False) == 13
    assert harness.multiply_clocks(5, True) == 13
    assert harness.multiply_clocks(0xFFFF, False) == 26
    assert harness.multiply_clocks(25173, True) == 10 + 15
    assert harness.multiply_clocks(0xCCCCCCCD, False) == 42
    assert harness.multiply_clocks(0xFFFFFFFF, True) == 15      # -1: n = 5 for a negative multiplier


def test_llrm_is_built_in_the_convention_gcc_uses_and_its_report_is_read_from_the_stack(built):
    """The table put llrm's watcom register arguments beside gcc's stack ones: part of every ratio was the ABI. build.sh passes
    -mabi=sysv and records it; the harness reads `report`'s value where that convention puts it."""
    assert harness.built_abi() == "sysv"
    result = harness.run("fib", "llrm")
    assert result["reports"] == expected("fib"), result["reports"]
    listing = subprocess_text([str(harness.LLRM), "-m32", "-mabi=sysv", "-O2", "-march=i486", "-S", "-o", "/dev/stdout", str(BENCH / "fib/fib.c")])
    assert "_fib proc" in listing and "fib_ proc" not in listing, "sysv names are undecorated"


def subprocess_text(argv):
    import subprocess
    done = subprocess.run(argv, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return done.stdout


def test_enter_costs_what_the_486_charges_for_it():
    """`enter` fell to the default one clock: a frame opened with it read 3 clocks cheaper than push/mov/sub's 3 when it is
    14 (Intel 240440-002), and the -Os rows that used it looked as fast as they were small."""
    from iced_x86 import Decoder
    enter = next(iter(Decoder(32, bytes([0xC8, 0x08, 0x00, 0x00]))))
    assert harness.cost(enter, True, True, None) == 14


def test_every_x_kernel_has_its_source_and_its_self_check():
    """The kernels live beside the harness, not in bench/: a lookup in bench/ alone found neither file."""
    names = sorted(p.name for p in harness.KERNELS.iterdir())
    assert len(names) == 38 and all(n.startswith("x_") for n in names)
    for n in names:
        assert (harness.source_dir(n) / f"{n}.c").is_file() and harness.source_dir(n) == harness.KERNELS / n
        assert harness.source_dir(n).joinpath(f"{n}.out").read_text().split()

"""Regression tests of the instrument: each name says what went wrong and what it produced."""
import json
from pathlib import Path

import pytest

import harness

pytestmark = pytest.mark.skipif(not (harness.OUT / "results.jsonl").exists(), reason="run crates/target/llrm-x86-code32/vsgcc/run.sh first")

BENCH = harness.BENCH


def expected(prog):
    return [int(x) for x in (BENCH / prog / f"{prog}.out").read_text().split()]


def test_memmove_stand_in_copies_overlap_backwards():
    """The stub's memmove copied forwards: gcc/clang scroll (loops turned into memmove) reported 32636400, not 32634864."""
    for v in ("gccO2", "clangO2"):
        assert harness.run("scroll", v)["reports"] == expected("scroll")


def test_kernel_inlined_into_main_is_not_counted_as_zero():
    """gcc inlined bench_fib into main: the region was never entered and fib read 0 instructions for gcc."""
    assert harness.run("fib", "gccO2")["ins"] > 0


def test_every_result_reproduces_the_benchmark_output():
    for line in (harness.OUT / "results.jsonl").read_text().splitlines():
        r = json.loads(line)
        assert r["ok"], (r["prog"], r["variant"], r["got"])

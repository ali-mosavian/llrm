import os
import re
import sys
from pathlib import Path

import pytest

sys.path.insert(0, os.path.dirname(__file__))
import exec as ex  # noqa: E402
import gen  # noqa: E402

pytestmark = pytest.mark.skipif(not os.path.exists(ex.CLANG), reason="the pin's clang is not built")
FAKE = [sys.executable, str(Path(__file__).parent / "fixtures/exec/wrong_callee_cc.py")]


def setup(ours):
    return ex.Setup("/nonexistent", "O2", "x86_64-linux", ours)


def test_a_compiler_that_calls_the_wrong_callee_differs_on_some_generated_program():
    """Cost of the bug it stands for (#6146): a sibling call to h ran f. A harness that reads 0 differ here is blind."""
    s = setup(FAKE)
    found = [(seed, shape) for seed in range(6) for shape in gen.SHAPES if ex.verdict(s, gen.emit(seed, shape))[0] == "differ"]
    assert len(found) >= 4, found


def test_the_reference_compiler_against_itself_never_differs():
    s = setup([ex.CLANG, "-m64"])
    for seed in range(4):
        for shape in gen.SHAPES:
            assert ex.verdict(s, gen.emit(seed, shape))[0] == "match", (seed, shape)


def test_a_difference_reduces_to_fewer_lines_that_still_differ_and_are_defined():
    s = setup(FAKE)
    text = next(t for t in (gen.emit(seed, "ladder") for seed in range(20)) if ex.verdict(s, t)[0] == "differ")
    small = ex.reduce(s, text)
    assert len(small.splitlines()) < len(text.splitlines())
    assert ex.verdict(s, small)[0] == "differ" and ex.defined(s, small)


def test_the_driver_defines_every_callee_and_the_replay_command_names_the_seed():
    text = gen.emit(2, "ladder")
    assert all(f"int {name}(int a, int b)" in ex.driver(text) for name in ex.callees(text)) and ex.callees(text)
    import argparse

    args = argparse.Namespace(level="O2", target="x86_64-linux", bin="B")
    assert "replay 7 sibling" in ex.replay_command(args, 7, "sibling")


def test_the_unfixed_tailmerge_binary_differs_on_the_recorded_programs_and_the_fixed_one_on_none():
    """Positive control on the real compiler: LLRM_DIFFTEST_BIN is a release dir, LLRM_DIFFTEST_EXPECT `prefix` or `fixed`.
    prefix (release/next without #6146): the programs of fixtures/exec/prefix_differs.txt differ, no others."""
    bin_dir, expect = os.environ.get("LLRM_DIFFTEST_BIN"), os.environ.get("LLRM_DIFFTEST_EXPECT")
    if not bin_dir or expect not in ("prefix", "fixed"):
        pytest.skip("LLRM_DIFFTEST_BIN and LLRM_DIFFTEST_EXPECT=prefix|fixed name a built compiler")
    s = ex.Setup(bin_dir, "O2", "x86_64-linux")
    found = {f"{seed} {shape}" for seed in range(10) for shape in gen.SHAPES if ex.verdict(s, gen.emit(seed, shape))[0] == "differ"}
    recorded = set((Path(__file__).parent / "fixtures/exec/prefix_differs.txt").read_text().split("\n")) - {""}
    assert found == (recorded if expect == "prefix" else set()), sorted(found ^ recorded)


def test_the_vendored_sibling_call_files_differ_only_on_the_unfixed_binary():
    """The files w10 found (a4 b17 b21 d2 d3 d4, ladder_336) differ on release/next without #6146; the fixed binary matches all."""
    bin_dir, expect = os.environ.get("LLRM_DIFFTEST_BIN"), os.environ.get("LLRM_DIFFTEST_EXPECT")
    if not bin_dir or expect not in ("prefix", "fixed"):
        pytest.skip("LLRM_DIFFTEST_BIN and LLRM_DIFFTEST_EXPECT=prefix|fixed name a built compiler")
    s = ex.Setup(bin_dir, "O2", "x86_64-linux")
    differ = set()
    for path in sorted((Path(__file__).parent / "fixtures").glob("*.c")):
        text = path.read_text()
        if re.search(r"\b(?:void|int)\s+g\((?:int\s+\w+)(?:\s*,\s*int\s+\w+)?\)", text) and ex.verdict(s, text)[0] != "match":
            differ.add(path.stem)
    assert differ == ({"a4", "b17", "b21", "d2", "d3", "d4", "ladder_336"} if expect == "prefix" else set()), differ

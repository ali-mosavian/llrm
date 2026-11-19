"""A runtime routine that calls itself through a libcall is a stack overflow, not a size."""
import os
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import m68k_exec_sweep as sweep  # noqa: E402
import m68k_runtime_guard as guard  # noqa: E402

CC = os.environ.get("LLRM_BIN")
GCC = os.environ.get("GCC_M68K")

# int32.c at -O1: the 16-bit path of udivmod is a call to __udivsi3
LOOP = """udivmod:
\tmovem.l\t%d2, -(%sp)
\tjsr\t__udivsi3
\trts
__udivsi3:
\tjsr\tudivmod
\trts
"""
SELF = "f:\n\tbsr\tf\n\trts\n"
FLAT = "udivmod:\n\tdivu\t%d1, %d0\n\trts\n__udivsi3:\n\tjsr\tudivmod\n\trts\n"
HOISTED = "g:\n\tlea\th, %a2\n\trts\nh:\n\tjsr\t(g,%pc)\n\trts\n"


def test_two_routines_that_call_each_other_are_a_cycle():
    """nbody_fixed and div_constants_short crashed at -O1: udivmod -> __udivsi3 -> udivmod, stack overflow, SIGSEGV."""
    assert guard.cycles(guard.call_graph(LOOP)) == [["__udivsi3", "udivmod"]]


def test_a_routine_that_calls_itself_is_a_cycle():
    assert guard.cycles(guard.call_graph(SELF)) == [["f"]]


def test_a_call_chain_without_a_return_edge_is_not():
    assert guard.cycles(guard.call_graph(FLAT)) == []


def test_a_hoisted_call_target_counts_as_a_call():
    assert guard.cycles(guard.call_graph(HOISTED)) == [["g", "h"]]


needs_compiler = pytest.mark.skipif(not CC or not GCC, reason="LLRM_BIN and GCC_M68K are not set")


@needs_compiler
def test_the_runtime_calls_nothing_back_at_the_levels_it_is_built_for():
    assert guard.runtime_cycles(CC, "68000", "O2") == []
    assert guard.runtime_cycles(CC, "68000", "Os") == []


@needs_compiler
def test_the_runtime_at_O1_is_refused_to_run(tmp_path):
    """Premise: -O1 leaves `udivmod` calling `__udivsi3` (the pin's -O1 does not narrow either); the sweep must refuse that archive."""
    assert guard.runtime_cycles(CC, "68000", "O1")
    with pytest.raises(SystemExit, match="calls itself"):
        sweep.build_support(CC, GCC, "68000", tmp_path, 2, "O1")

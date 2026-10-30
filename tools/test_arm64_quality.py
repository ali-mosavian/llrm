import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import arm64_quality as q  # noqa: E402


def dis(*lines, start=0):
    return "\n".join(f"{start + 4 * n:8x}:\t{l}" for n, l in enumerate(lines))


def run(*lines, name="_f"):
    return q.check_and_measure(q.parse_disasm(dis(*lines)), [(0, name)], 4 * len(lines))


def test_loop_depth_weights_nested_loops_by_ten_per_level():
    """Without weights a hot inner loop counted like straight-line code."""
    m = run(
        "mov w0, #0x0",       # 0  depth 0
        "mov w1, #0x0",       # 4  outer header
        "add w1, w1, #0x1",   # 8  inner header
        "cmp w1, #0x9",       # c
        "b.lt 0x8",           # 10 inner back-edge
        "cmp w0, #0x9",       # 14
        "b.lt 0x4",           # 18 outer back-edge
        "ret",                # 1c
    )
    assert m["exec"] == 1 + 1 * 10 + 3 * 100 + 2 * 10 + 1


def test_loop_exec_exact():
    m = run("nop", "add w0, w0, #0x1", "cmp w0, #0x9", "b.lt 0x4", "ret")
    assert m["exec"] == 1 + 3 * 10 + 1


def test_ldp_is_one_memory_op():
    m = run("ldp x0, x1, [x2]", "stp x0, x1, [x3]", "ret")
    assert m["mem"] == 2 and m["insns"] == 3


def test_prologue_epilogue_pairs_are_saves_not_spills():
    m = run(
        "stp x29, x30, [sp, #-0x10]!",
        "mov x29, sp",
        "ldp x29, x30, [sp], #0x10",
        "ret",
    )
    assert (m["saves"], m["spills"]) == (2, 0)


def test_stack_slot_traffic_is_a_spill():
    m = run("sub sp, sp, #0x20", "str w0, [sp, #0xc]", "ldur w1, [x29, #-0x4]", "add sp, sp, #0x20", "ret")
    assert (m["saves"], m["spills"]) == (0, 2)


def test_callee_saved_store_mid_function_is_a_spill():
    m = run("add w0, w0, #0x1", "str x19, [sp, #0x8]", "add w0, w0, #0x1", "ret")
    assert (m["saves"], m["spills"]) == (0, 1)


def test_non_stack_memory_is_not_a_spill():
    m = run("ldr w0, [x1]", "str w0, [x2, #0x4]", "ret")
    assert (m["mem"], m["spills"], m["saves"]) == (2, 0, 0)


def test_call_adds_callee_estimate_weighted_by_call_site():
    text = dis("add w0, w0, #1", "cmp w0, #9", "bl 0x14", "b.lt 0x0", "ret", "nop", "ret")
    ins = q.parse_disasm(text)
    m = q.check_and_measure(ins, [(0, "_f"), (0x14, "_g")], 28)
    assert m["exec"] == 4 * 10 + 1 + 10 * 2


def test_recursion_is_capped():
    text = dis("nop", "bl 0x0", "ret")
    m = q.check_and_measure(q.parse_disasm(text), [(0, "_f")], 12)
    assert m["exec"] == 3


def test_relocated_call_resolves_by_symbol():
    text = dis("nop", "bl 0x4", "ret", "ret") 
    lines = text.splitlines()
    lines.insert(2, "\t\t0000000000000004:  ARM64_RELOC_BRANCH26\t_g")
    m = q.check_and_measure(q.parse_disasm("\n".join(lines)), [(0, "_f"), (0xc, "_g")], 16)
    assert m["exec"] == 3 + 1


def test_data_in_text_is_reported_not_miscounted():
    """A literal-pool word breaks size/4 == insns; it must say DATA, not return a count."""
    with pytest.raises(q.Unmeasurable) as e:
        run("ldr x0, 0x8", "ret", ".long\t0x3ff00000")
    assert e.value.kind == "DATA"


def test_parse_size_mismatch_fails_loudly():
    with pytest.raises(q.Unmeasurable) as e:
        q.check_and_measure(q.parse_disasm(dis("ret")), [(0, "_f")], 8)
    assert e.value.kind == "PARSE"


def test_empty_disassembly_fails_loudly():
    with pytest.raises(q.Unmeasurable):
        q.check_and_measure([], [], 0)


def test_mutual_recursion_chain_does_not_multiply_loop_weights():
    """gcc's clones of a recursive fib called each other from loops; the estimate reached 4.8e12 for a 321-insn object."""
    lines = ["add w0, w0, #1", "bl 0x10", "b.lt 0x0", "ret",
             "add w0, w0, #1", "bl 0x20", "b.lt 0x10", "ret",
             "add w0, w0, #1", "bl 0x0", "b.lt 0x20", "ret"]
    m = q.check_and_measure(q.parse_disasm(dis(*lines)), [(0, "_f"), (0x10, "_g"), (0x20, "_h")], 48)
    assert m["exec"] == 3 * 31


def test_without_unrolling_adds_each_compilers_own_switch():
    """gcc's -O2 completely peeled small loops and clang unrolled by cost, so a static count compared unrolling policy."""
    clang = q.without_unrolling(["/p/bin/clang", "-O2", "-x", "ir", "-c", "a.ll"])
    assert clang[-1] == "-fno-unroll-loops"
    ours = q.without_unrolling(["/t/llrm-arm64-llc", "-O2", "a.ll"])
    assert ours[-2:] == ["-fno-unroll-loops", "-fno-peel-loops"]
    gcc = q.without_unrolling(["gcc-13", "-O2", "-c", "a.c"])
    assert "max-completely-peeled-insns=0" in gcc and "-fno-peel-loops" in gcc
    assert q.without_unrolling(["clang", "--version"]) == ["clang", "--version"], "other commands unchanged"

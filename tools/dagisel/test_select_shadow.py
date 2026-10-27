"""select_shadow.py's buckets: every defined function lands in exactly one."""

import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import select_shadow as tool  # noqa: E402

IR = "define i32 @a() {\n ret i32 0\n}\ndefine i32 @b() {\n ret i32 0\n}\n"


def func(unselected=None):
    return {"blocks": 1, "built": 1, "machine": Counter({"RET": 1}), "unselected": Counter(unselected or {})}


def test_m16_has_no_oracle_so_a_clean_function_selects():
    rows = tool.rows_of("p", {"a": func(), "b": func({"GlobalAddress": 1})}, {}, None, IR)
    assert {f: b for _, f, b in rows} == {"a": "selects", "b": "unselected GlobalAddress"}


def test_a_function_missing_from_the_output_is_still_a_row():
    rows = tool.rows_of("p", {"a": func()}, {}, None, IR)
    assert len(rows) == 2


def test_a_quoted_function_name_is_the_name_the_shadow_prints():
    """m16 names like `_f@3` are quoted in the IR; the quotes made 87 functions "missing from the output"."""
    assert tool.defined('define i16 @"_f@3"(i16 %0) {\n ret i16 0\n}\n') == ["_f@3"]


def test_an_extract_subreg_is_the_copy_the_pin_prints():
    """The emitter turns `EXTRACT_SUBREG` into `COPY`: x_dct's `movzx` of a low byte was `differs: ours only EXTRACT_SUBREG`."""
    ours = {"a": Counter({"MOVZX32rr8": 1, "EXTRACT_SUBREG": 1, "RET": 1})}
    pinned = {"a": Counter({"MOVZX32rr8": 1, "COPY": 1, "RET": 1})}
    rows = tool.rows_of("p", {"a": {**func(), "machine": ours["a"]}}, {}, pinned, "define i32 @a() {\n ret i32 0\n}\n")
    assert [b for _, _, b in rows] == ["equal"]


def test_the_example_expands_memset_pattern_as_the_driver_does(tmp_path):
    """`scroll` in the m16 shadow was refused for `intrinsic call llvm.experimental.memset.pattern`: the driver expands it, the example did not."""
    import os
    import subprocess

    import pytest

    binary = os.environ.get("SELECT_SHADOW")
    if not binary:
        pytest.skip("SELECT_SHADOW names a built select_shadow example")
    ir = tmp_path / "p.ll"
    ir.write_text(
        'target datalayout = "e-p:16:16-i32:16-i64:16-f80:16-n8:16:32"\n'
        "define void @f(ptr %p, i16 %n) {\nentry:\n"
        "  call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr %p, i16 4660, i16 %n, i1 false)\n  ret void\n}\n"
        "declare void @llvm.experimental.memset.pattern.p0.i16.i16(ptr, i16, i16, i1)\n"
    )
    out = subprocess.run([binary, str(ir)], capture_output=True, text=True, env={**os.environ, "M16": "1"}).stdout
    assert "REFUSED" not in out, out


def test_a_select_pseudo_is_the_jcc_the_pins_inserter_makes():
    """The pin's finalize-isel has run EmitLoweredSelect (a JCC_1 diamond); our selection still has CMOV_GR32, which the
    emitter expands the same way: a function of one select was `differs: ours only CMOV_GR32` (x_heapsort, x_isqrt)."""
    machine = Counter({"CMOV_GR32": 1, "ADD32rr": 2, "RET": 1})
    ours = {"blocks": 1, "built": 1, "machine": machine, "unselected": Counter()}
    pinned = {"a": Counter({"JCC_1": 1, "ADD32rr": 2, "RET": 1})}
    rows = tool.rows_of("p", {"a": ours}, {}, pinned, "define i32 @a() {\n ret i32 0\n}\n")
    assert [b for _, _, b in rows] == ["equal"]


def test_a_cascade_of_selects_is_one_diamond():
    machine = Counter({"CMOV_GR32": 3, "RET": 1})
    assert tool.expanded_selects(machine, Counter({"JCC_1": 1, "RET": 1})) == Counter({"JCC_1": 1, "RET": 1})


def test_a_float_to_int_store_is_the_control_word_switch_the_pin_makes():
    machine = Counter({"FP64_TO_INT32_IN_MEM": 1, "RET": 1})
    pin = Counter({"FNSTCW16m": 1, "MOVZX32rm16": 1, "OR32ri": 1, "MOV16mr": 1, "FLDCW16m": 2, "IST_Fp32m64": 1, "RET": 1})
    assert tool.expanded_fp_to_int(machine) == pin


def test_a_function_equal_only_through_the_fp_to_int_rule_says_so():
    machine = Counter({"FP64_TO_INT32_IN_MEM": 1, "RET": 1})
    pin = {"f": Counter({"FNSTCW16m": 1, "MOVZX32rm16": 1, "OR32ri": 1, "MOV16mr": 1, "FLDCW16m": 2, "IST_Fp32m64": 1, "RET": 1})}
    shadow = {"f": {"blocks": 1, "built": 1, "machine": machine, "unselected": Counter()}}
    rows = tool.rows_of("p", shadow, {}, pin, "define i32 @f() {\n ret i32 0\n}\n")
    assert [b for _, _, b in rows] == [tool.EQUAL_FP]


def test_the_pin_reads_llrms_switch_and_fp80_spellings():
    """`switch table` and `x86_fp80 0x<double bits>` made llc fail on 15 of 576 functions (jpeg, switch_peeled, double_to_uint64), which the
    equal count silently left out."""
    out = tool.pin_ir("switch table i32 %5, label %8 [\n]\n%a = fptoui x86_fp80 0x43EFFFFFFFFFFFFF to i64\n%b = fadd x86_fp80 0x3FF0000000000000, %c")
    assert out.startswith("switch i32 %5")
    assert "x86_fp80 0xK403EFFFFFFFFFFFFF800" in out
    assert "x86_fp80 0xK3FFF8000000000000000," in out


def test_the_failure_bucket_names_the_error_not_the_caret():
    assert tool.first_error("llc: <stdin>:18:10: error: expected type\n  switch table\n         ^\n") == "llc: <stdin>:18:10: error: expected type"

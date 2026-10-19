"""The inline-decision comparison on a hand-made module: a call LLVM refuses over a threshold that llrm inlined shows as llrm-only; at the default
threshold both inline it and the module is all agree."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import difftest  # noqa: E402
import inlinediff  # noqa: E402

needs_opt = pytest.mark.skipif(not (difftest.LLVM_BIN / "opt").is_file(), reason="no opt at LLVM_BIN")

CHAIN = "".join(f"  %a{i} = add i32 %a{i - 1}, 1\n" for i in range(1, 31))
BEFORE = (
    "define i32 @tiny(i32 %x) {\nentry:\n  %r = add i32 %x, 1\n  ret i32 %r\n}\n"
    f"define i32 @mid(i32 %x) {{\nentry:\n  %a0 = add i32 %x, 1\n{CHAIN}  ret i32 %a30\n}}\n"
    "define i32 @main(i32 %x) {\nentry:\n  %t = call i32 @tiny(i32 %x)\n  %m = call i32 @mid(i32 %t)\n  ret i32 %m\n}\n"
)
# llrm inlined both calls
AFTER = "'tiny' inlined into 'main' with (cost=-30, threshold=225)\n'mid' inlined into 'main' with (cost=-15000, threshold=225)\n"


def classes(report: dict) -> dict:
    return {key: sum(n for k, n in report["by_class_and_size"].items() if k.startswith(key + "|")) for key in ("agree", "llrm-only", "llvm-only")}


@needs_opt
def test_a_call_over_the_threshold_that_llrm_inlined_is_llrm_only_with_llvms_cost():
    report = inlinediff.compare(BEFORE, AFTER, "inline", threshold=5)
    row = next(r for r in report["rows"] if r["callee"] == "mid")
    assert classes(report)["llrm-only"] == 1 and row["llrm_inlined"] == 1 and row["llvm_inlined"] == 0
    assert row["llvm"][0] == "too costly to inline", row["llvm"]
    assert row["callee_instructions"] > 30 and row["callee_called_once"]


@needs_opt
def test_at_the_default_threshold_both_inline_both_calls_and_nothing_differs():
    report = inlinediff.compare(BEFORE, AFTER, "inline")
    assert classes(report) == {"agree": 2, "llrm-only": 0, "llvm-only": 0}, report["rows"]
    tiny = next(r for r in report["rows"] if r["callee"] == "tiny")
    assert tiny["llvm"][1] > 0, tiny  # its threshold


def test_the_sites_of_a_module_are_counted_by_caller_and_callee():
    assert inlinediff.sites(BEFORE) == {("main", "tiny"): 1, ("main", "mid"): 1}


def test_the_stage_files_of_a_dump_are_the_second_pipeline_module_and_the_remarks(tmp_path):
    for name in ("03-stamp.ll", "04-pipeline.ll", "05-pipeline.ll", "06-pipeline.ll", "07-interprocedural.ll", "remarks.txt"):
        (tmp_path / name).write_text("")
    before, remarks = inlinediff.stage_files(tmp_path)
    assert (before.name, remarks.name) == ("05-pipeline.ll", "remarks.txt")


@needs_opt
def test_the_report_states_whether_llvm_both_inlined_and_declined():
    report = inlinediff.compare(BEFORE, AFTER, "inline", threshold=5)
    assert report["llvm_inlined"] == 1 and report["premise"] == "ok", "tiny inlined, mid refused: the reference said both"
    assert "premise (LLVM inlined 1 of 2 sites): ok" in inlinediff.table(report)
    alone = inlinediff.compare(BEFORE, AFTER, "inline")
    assert alone["premise"].startswith("PREMISE FAILED: LLVM inlined all 2 sites"), alone["premise"]


def test_quoted_names_of_m16_modules_are_read():
    """m16 names a function `@"_f@3"` (its convention's decoration): the reader took only bare names and stopped at the first quoted
    `define`, so no m16 module could be compared (TypeError in `functions`)."""
    text = 'define i16 @"_main@0"() addrspace(1) {\n  %1 = call cc 1005 addrspace(1) i16 @"_tiny@3"(i16 1)\n  ret i16 %1\n}\n'
    assert inlinediff.sites(text) == {("_main@0", "_tiny@3"): 1}


@needs_opt
def test_the_calls_of_a_caller_inlined_and_deleted_are_not_inlined_by_llrm():
    """Counting calls left after the stage read `outer` inlined into `main` and deleted as llrm inlining `outer`'s call of the declaration
    `ext`: 425 of QCport's 574 llrm-only sites named a callee with no body."""
    before = (
        "declare i32 @ext(i32)\n"
        "define internal i32 @outer(i32 %x) {\nentry:\n  %r = call i32 @ext(i32 %x)\n  ret i32 %r\n}\n"
        "define i32 @main(i32 %x) {\nentry:\n  %r = call i32 @outer(i32 %x)\n  ret i32 %r\n}\n"
    )
    remarks = "'outer' inlined into 'main' with (cost=-15000, threshold=225)\n"
    report = inlinediff.compare(before, remarks, "inline")
    assert classes(report)["llrm-only"] == 0, report["rows"]


def test_a_refusal_keeps_its_threshold():
    """The cost's optional tail took `, threshold=T` before the threshold group could: every refusal read threshold None."""
    _, refused = inlinediff.verdicts("'f' not inlined into 'g' because too costly to inline (cost=300, threshold=225)\n")
    assert refused[("g", "f")] == [("too costly to inline", "300", "225")]


@needs_opt
def test_opt_prices_with_the_target_given():
    """opt without a triple priced a scaled address as work: _clip_w in d_faces cost 365, refused, where i486 makes its 30
    `getelementptr i32, ptr, i32` free (215, inlined) as llrm's x86 model does. The target goes to opt."""
    loads = "".join(f"  %g{i} = getelementptr inbounds i32, ptr %p, i32 %i{i}\n  %v{i} = load i32, ptr %g{i}\n" for i in range(48))
    adds = "".join(f"  %s{i} = add i32 %s{i - 1}, %v{i}\n" for i in range(1, 48))
    params = ", ".join(f"i32 %i{i}" for i in range(48))
    callee = f"define internal i32 @sum(ptr %p, {params}) {{\nentry:\n{loads}  %s0 = add i32 %v0, 0\n{adds}  ret i32 %s47\n}}\n"
    args = ", ".join(f"i32 %x{i}" for i in range(48))
    caller = f"define i32 @main(ptr %p, {args}) {{\nentry:\n  %r = call i32 @sum(ptr %p, {args})\n  %t = call i32 @sum(ptr %p, {args})\n  %u = add i32 %r, %t\n  ret i32 %u\n}}\n"
    before = callee + caller
    generic = inlinediff.compare(before, "", "inline")
    targeted = inlinediff.compare(before, "", "inline", target=("i386-unknown-unknown", "i486"))
    assert generic["llvm_inlined"] == 0, generic["rows"]
    assert targeted["llvm_inlined"] == 2, targeted["rows"]


def test_the_oracle_targets_what_the_module_was_built_for():
    """A run without --mtriple asked opt for its generic TTI: every QCport agreement before #2746 was taken that way. The target
    is read off the module's data layout; a layout it does not know is refused, not priced generically."""
    m16 = 'target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-n8:16:32"\n'
    m32 = 'target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-n8:16:32-S32"\n'
    assert inlinediff.inferred_target(m16) == ("i386-unknown-unknown-code16", "i486")
    assert inlinediff.inferred_target(m32) == ("i386-unknown-unknown", "i486")
    with pytest.raises(SystemExit):
        inlinediff.inferred_target("define void @f() {\n  ret void\n}\n")

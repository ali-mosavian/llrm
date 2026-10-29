"""The inline-decision comparison on a hand-made module: a call LLVM refuses over a threshold that llrm inlined shows as llrm-only; at the default
threshold both inline it and the module is all agree."""

import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import difftest  # noqa: E402
import inlinediff  # noqa: E402

# Red, not skipped, where the gate requires the pin's tools (LLRM_REQUIRE_PINTOOLS): a missing opt then fails the test that needs it.
needs_opt = pytest.mark.skipif(not (difftest.LLVM_BIN / "opt").is_file() and not os.environ.get("LLRM_REQUIRE_PINTOOLS"), reason="no opt at LLVM_BIN")

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


@needs_opt
def test_a_reference_that_inlined_nothing_is_not_a_premise_even_where_llrm_disagrees():
    """`idioms.bas` -Os: the dump's `__main` was `internal` and called by nothing, so `opt` never visited it and inlined 0 of 123 sites while llrm
    inlined 6; the table said "premise ok" and the comparison read as llrm-only noise."""
    dead = BEFORE.replace("define i32 @main", "define internal i32 @main")
    report = inlinediff.compare(dead, AFTER, "inline")
    assert report["llvm_inlined"] == 0 and classes(report)["llrm-only"] == 2, report["rows"]
    assert report["premise"].startswith("PREMISE FAILED: LLVM inlined none of 2 sites"), report["premise"]


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

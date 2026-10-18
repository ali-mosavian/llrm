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
AFTER = BEFORE.split("define i32 @main")[0] + "define i32 @main(i32 %x) {\nentry:\n  %t = add i32 %x, 1\n  %m = add i32 %t, 31\n  ret i32 %m\n}\n"


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


def test_the_stage_files_of_a_dump_are_the_second_pipeline_module_and_the_interprocedural_one(tmp_path):
    for name in ("03-stamp.ll", "04-pipeline.ll", "05-pipeline.ll", "06-pipeline.ll", "07-interprocedural.ll"):
        (tmp_path / name).write_text("")
    before, after = inlinediff.stage_files(tmp_path)
    assert (before.name, after.name) == ("05-pipeline.ll", "07-interprocedural.ll")


@needs_opt
def test_the_report_states_whether_llvm_both_inlined_and_declined():
    report = inlinediff.compare(BEFORE, AFTER, "inline", threshold=5)
    assert report["llvm_inlined"] == 1 and report["premise"] == "ok", "tiny inlined, mid refused: the reference said both"
    assert "premise (LLVM inlined 1 of 2 sites): ok" in inlinediff.table(report)
    alone = inlinediff.compare(BEFORE, AFTER, "inline")
    assert alone["premise"].startswith("PREMISE FAILED: LLVM inlined all 2 sites"), alone["premise"]

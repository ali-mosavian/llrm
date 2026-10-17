"""`uv run --project tools python -m pytest tools/difftest/test_difftest.py`: the harness tests its own output on two hand-made inputs, one where
llrm's answer and LLVM's agree and one where they do not."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import difftest  # noqa: E402

needs_opt = pytest.mark.skipif(not (difftest.LLVM_BIN / "opt").is_file(), reason="no opt at LLVM_BIN")

# `%a` and `%b` are one value: gvn keeps one add.
INPUT = """define i32 @f(i32 %x) {
entry:
  %a = add i32 %x, 1
  %b = add i32 %x, 1
  %c = mul i32 %a, %b
  ret i32 %c
}
"""
AGREES = """define i32 @f(i32 %x) {
entry:
  %p = add i32 %x, 1
  %q = mul i32 %p, %p
  ret i32 %q
}
"""


@needs_opt
def test_an_answer_llvm_would_write_too_is_identical_whatever_its_values_are_called():
    got = difftest.compare(INPUT, AGREES, "gvn")["f"]
    assert got["identical"] and got["llrm_only"] == {} and got["llvm_only"] == {} and not got["unchanged_by_llvm"] and not got["unchanged_by_llrm"]


@needs_opt
def test_an_answer_that_kept_the_second_add_differs_and_the_histogram_says_what():
    got = difftest.compare(INPUT, INPUT, "gvn")["f"]
    assert not got["identical"] and got["llrm_only"] == {"add": 1} and got["llvm_only"] == {} and got["unchanged_by_llrm"] and not got["unchanged_by_llvm"]
    assert any(line.startswith("-") and "add" in line for line in got["hunk"]), got["hunk"]


@needs_opt
def test_llrms_dialect_is_rewritten_for_llvm_and_an_input_it_cannot_read_after_that_is_an_error_not_a_pass():
    text = "declare internal watcallcc i32 @g(i32)\ndefine cc1000 i32 @f(i32 %x) {\n  %r = call watcallcc i32 @g(i32 %x)\n  ret i32 %r\n}\n"
    assert "watcallcc" not in difftest.normalise(text) and "declare i32" in difftest.normalise(text)
    assert "error" not in difftest.compare(text, text, "gvn")
    assert difftest.compare("define i32 @f( {", "", "gvn")["error"].startswith("opt gvn: ")


def test_llrms_answer_to_a_run_is_the_next_runs_input_of_the_same_function(tmp_path):
    rows = [
        {"config": "c", "fn": "01", "pass": "fold", "run": 1, "file": "c/01/1.ll", "instructions": 3},
        {"config": "c", "fn": "01", "pass": "gvn", "run": 2, "file": "c/01/2.ll", "instructions": 3},
        {"config": "c", "fn": "01", "pass": "dse", "run": 3, "file": "c/01/3.ll", "instructions": 2},
        {"config": "c", "fn": "02", "pass": "gvn", "run": 1, "file": "c/02/1.ll", "instructions": 3},
    ]
    got = difftest.pairs(rows, "gvn", tmp_path)
    assert [(r["fn"], r["run"], p.name) for r, p in got] == [("01", 2, "3.ll")], "the last run of a function has no answer and fn 02 has one run"


def test_a_step_file_is_the_answer_of_its_step_and_its_input_is_the_nearest_slot_of_the_file_before(tmp_path):
    """The pass lists of a pipeline each count from 001 into one directory: 002-dead@1242 and 002-promote@1179 are different lists, and 003-earlycse@1180
    follows the one at slot 1179."""
    body = tmp_path / "f"
    body.mkdir()
    for name in ("1-r01-sroa@1179.ll", "2-r01-promote@1179.ll", "2-r01-dead@1242.ll", "2-r01-tailrec@688.ll", "3-r01-earlycse@1180.ll", "10-r01-gvn@7.ll"):
        (body / name).write_text("")
    got = difftest.dump_pairs(tmp_path, "earlycse")
    assert [(Path(r["file"]).name, p.name) for r, p in got] == [("2-r01-promote@1179.ll", "3-r01-earlycse@1180.ll")]
    assert difftest.dump_pairs(tmp_path, "sroa") == [], "the first step of a list has no input in the dump"


def test_a_pass_without_a_counterpart_is_refused_by_name(capsys, monkeypatch):
    monkeypatch.setattr(sys, "argv", ["difftest.py", "run", "ports"])
    assert difftest.main() == 2 and "ports has no counterpart" in capsys.readouterr().err

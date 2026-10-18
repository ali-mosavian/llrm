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


def test_the_rewrites_change_only_llrms_convention_words_and_internal_on_a_declaration():
    """The words are llrm's only as tokens of the code: not a name that contains one, a string, a comment, or `internal` on a definition."""
    text = (
        'declare internal watcallcc i32 @g(i32) ; the word watcallcc stays\n'
        'define internal cc1000 i32 @watcallcc_fn(i32 %x) {\n'
        '  %r = call sysvcc i32 @g(i32 %x), !note !"cc1000 sysvcc"\n'
        '  ret i32 %r\n'
        '}\n'
        '@s = constant [9 x i8] c"watcallcc"\n'
    )
    assert difftest.normalise(text) == (
        'declare i32 @g(i32) ; the word watcallcc stays\n'
        'define internal i32 @watcallcc_fn(i32 %x) {\n'
        '  %r = call i32 @g(i32 %x), !note !"cc1000 sysvcc"\n'
        '  ret i32 %r\n'
        '}\n'
        '@s = constant [9 x i8] c"watcallcc"\n'
    )


def test_two_layouts_of_one_control_flow_graph_are_identical():
    """A pass that moves blocks showed as a large hunk with equal histograms."""
    one = "define i32 @f(i1 %c) {\ne:\n  br i1 %c, label %a, label %b\na:\n  br label %m\nb:\n  br label %m\nm:\n  ret i32 1\n}\n"
    other = "define i32 @f(i1 %c) {\ne:\n  br i1 %c, label %a, label %b\nm:\n  ret i32 1\nb:\n  br label %m\na:\n  br label %m\n}\n"
    assert difftest.canonical(difftest.functions(one)["f"]) == difftest.canonical(difftest.functions(other)["f"])
    different = one.replace("br label %m\nb:", "br label %b\nb:")
    assert difftest.canonical(difftest.functions(one)["f"]) != difftest.canonical(difftest.functions(different)["f"])


@needs_opt
def test_llrms_answer_is_the_pass_run_alone_not_what_the_next_step_made_of_it(tmp_path):
    """The proxy (the next step's input) holds every pass that ran in between: with the replay the answer is the runner's output, whatever the next input is."""
    source = tmp_path / "in.ll"
    source.write_text(INPUT)
    runner = tmp_path / "llrm-pass"
    runner.write_text(f"#!/bin/bash\ncat <<'EOF'\n{AGREES}EOF\n")
    runner.chmod(0o755)
    leaked = tmp_path / "next.ll"
    leaked.write_text(INPUT.replace("mul i32 %a, %b", "mul i32 %a, 7"))  # a pass after gvn changed the body further
    row = {"config": "c-O2", "fn": "f", "pass": "gvn", "run": 1, "file": str(source)}
    proxy = difftest.analyse([(row, leaked)], tmp_path, "gvn")
    alone = difftest.analyse([(row, leaked)], tmp_path, "gvn", replay=runner)
    assert proxy["identical"] == 0 and alone["identical"] == 1, "the leak of the later pass is what the proxy compares"
    assert alone["proxy_equals_isolated"] == 0 and alone["answer"].startswith("the pass run alone") and proxy["answer"] == "next-input proxy"


def test_a_counterpart_names_the_cplusplus_it_is_translated_from():
    assert "GVN.cpp" in difftest.source_link("gvn") and "d1106deb71cc/llvm/lib/Transforms/Scalar/GVN.cpp" in difftest.source_link("gvn")
    assert all(passes in difftest.SOURCE for passes in set(difftest.COUNTERPART.values()))


def llrm_pass():
    import os

    for base in (os.environ.get("LLRM_BIN"), os.environ.get("CARGO_TARGET_DIR") and os.environ["CARGO_TARGET_DIR"] + "/release"):
        if base and (Path(base) / "llrm-pass").is_file():
            return Path(base) / "llrm-pass"
    return None


@pytest.mark.skipif(llrm_pass() is None, reason="llrm-pass is not built")
def test_llrm_pass_runs_the_one_pass_it_is_asked_for_and_no_other(tmp_path):
    """gvn takes the second add; fold would take `mul x, 1`: only the first happens when gvn is asked for."""
    import subprocess

    source = tmp_path / "in.mir"
    source.write_text(
        "define i32 @f(i32 %x) {\nentry:\n  %a = add i32 %x, 1\n  %b = add i32 %x, 1\n  %c = mul i32 %a, 1\n  %d = add i32 %b, %c\n  ret i32 %d\n}\n"
    )
    out = subprocess.run([str(llrm_pass()), "-O2", "gvn", str(source)], capture_output=True, text=True, check=True).stdout
    assert out.count(" add i32 %") == 2 and "mul i32" in out, out
    refused = subprocess.run([str(llrm_pass()), "-O2", "nosuch", str(source)], capture_output=True, text=True)
    assert refused.returncode == 2 and "no pass named nosuch" in refused.stderr


def test_a_run_with_a_check_switch_nobody_named_is_refused_before_it_replays(monkeypatch):
    """llvm-order-poc blamed callee-first order for a failure that came from LLRM_CHECK_* on in one run only: llrm-pass inherits the environment."""
    monkeypatch.setenv("LLRM_CHECK_ORDER", "1")
    monkeypatch.setattr(difftest, "pairs", lambda *a, **k: (_ for _ in ()).throw(AssertionError("replayed")))
    monkeypatch.setattr(sys, "argv", ["difftest.py", "run", "sroa", "--llrm-pass", "/x/llrm-pass"])
    with pytest.raises(SystemExit, match="LLRM_CHECK_ORDER=1"):
        difftest.main()
    monkeypatch.setattr(sys, "argv", ["difftest.py", "run", "sroa", "--allow-env", "LLRM_CHECK_ORDER"])
    monkeypatch.setattr(difftest, "manifest", lambda *a, **k: [])
    with pytest.raises(AssertionError, match="replayed"):
        difftest.main()


def test_llvm_bin_defaults_to_the_pinned_build_never_the_distros(monkeypatch):
    """difftest fell back to /usr/lib/llvm-20 when the pinned opt was missing: tables from LLVM 20.1.8 under the name of the plan's 24.0.0git."""
    import importlib

    monkeypatch.delenv("LLVM_BIN", raising=False)
    fresh = importlib.reload(difftest)
    try:
        assert fresh.LLVM_BIN == fresh.PINNED_BIN
    finally:
        monkeypatch.undo()
        importlib.reload(difftest)


HOIST = """define i32 @f(i32 %a, i32 %b, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %i2, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s2, %loop ]
  %inv = mul i32 %a, %b
  %s2 = add i32 %s, %inv
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s2
}
define i32 @g(i32 %x) {
entry:
  %y = add i32 %x, 1
  ret i32 %y
}
"""


@needs_opt
def test_licm_counts_name_the_function_and_the_event_and_a_loopless_function_has_none():
    """-stats prints nothing in the pinned opt (no LLVM_FORCE_ENABLE_STATS): the remarks are the counters licm's translation is judged by."""
    counts = difftest.licm_counts(HOIST)
    assert counts == {"f": {"Hoisted": 1}}, counts


def test_a_comparison_whose_reference_never_acted_or_never_declined_fails_its_premise():
    """memoryssa-port's lesson: an oracle that can give only one answer on the inputs proves nothing by agreeing."""
    assert difftest.premise(0, 40).startswith("PREMISE FAILED: LLVM changed none of 40")
    assert difftest.premise(40, 40).startswith("PREMISE FAILED: LLVM changed all 40")
    assert difftest.premise(7, 40) == "ok"
    assert difftest.premise(40, 40, differing=11) == "ok", "it disagreed with llrm somewhere: it can say more than one thing"
    assert difftest.premise(0, 1) == "ok", "one function cannot show both answers: not judged"


def test_inputs_of_reads_the_runs_of_other_passes_as_the_passes_own():
    """The replay manifest has no bdce rows: scalar-cleanup-port relabelled fold/algebraic/dead inputs in a private manifest."""
    rows = [{"config": "c", "fn": "f", "pass": "fold", "run": 1, "file": "a"}, {"config": "c", "fn": "f", "pass": "dead", "run": 2, "file": "b"}, {"config": "c", "fn": "f", "pass": "gvn", "run": 3, "file": "c"}]
    got = difftest.relabelled(rows, "bdce", ["fold", "dead"])
    assert [r["pass"] for r in got] == ["bdce", "bdce", "gvn"] and rows[0]["pass"] == "fold"
    assert difftest.relabelled(rows, "bdce", None) == rows

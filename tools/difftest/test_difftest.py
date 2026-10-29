"""`uv run --project tools python -m pytest tools/difftest/test_difftest.py`: the harness tests its own output on two hand-made inputs, one where
llrm's answer and LLVM's agree and one where they do not."""

import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import difftest  # noqa: E402

# Red, not skipped, where the gate requires the pin's tools (LLRM_REQUIRE_PINTOOLS): a missing opt then fails the test that needs it.
needs_opt = pytest.mark.skipif(not (difftest.LLVM_BIN / "opt").is_file() and not os.environ.get("LLRM_REQUIRE_PINTOOLS"), reason="no opt at LLVM_BIN")

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


def test_a_slot_name_selects_that_slot_alone_and_has_its_passes_counterpart(tmp_path):
    """`sroa@776` was not a name the oracle took: the translated slots of LLVM's order could not be asked for, only the old pass names."""
    body = tmp_path / "f"
    body.mkdir()
    for name in ("1-r01-gvn@5.ll", "2-r01-earlycse@649.ll", "2-r01-dead@5.ll", "3-r01-earlycse@1180.ll"):
        (body / name).write_text("")
    got = difftest.dump_pairs(tmp_path, "earlycse@1180")
    assert [(Path(r["file"]).name, r["pass"], p.name) for r, p in got] == [("2-r01-earlycse@649.ll", "earlycse@1180", "3-r01-earlycse@1180.ll")]
    assert difftest.dump_pairs(tmp_path, "earlycse@999") == []
    assert difftest.counterpart("sroa@776") == "sroa" and difftest.counterpart("loopdeletion@749") == "loop(loop-deletion)"


def test_no_input_compared_is_a_failed_premise_not_an_ok_one(tmp_path):
    """Eight slots read 'inputs 0 ... premise ok': dumps hold only the steps that changed a body, and a report over nothing said ok."""
    assert difftest.premise(0, 0).startswith("PREMISE FAILED")


def test_the_inputs_of_a_slot_are_every_run_changed_or_not(tmp_path):
    body = tmp_path / "cfg" / "01"
    body.mkdir(parents=True)
    for name in ("0003-in-sroa@1179.ll", "0007-in-sroa@776.ll", "0002-in-earlycse@1180.ll"):
        (body / name).write_text("")
    got = difftest.input_pairs(tmp_path / "cfg", "sroa@776")
    assert [(r["pass"], r["run"], Path(r["file"]).name, a) for r, a in got] == [("sroa@776", 7, "0007-in-sroa@776.ll", None)]


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
def test_llrms_answer_is_the_pass_run_alone_not_what_the_next_step_made_of_it(tmp_path, monkeypatch):
    """The proxy (the next step's input) holds every pass that ran in between: with the replay the answer is the runner's output, whatever the next input is."""
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "m32")  # these tests are about the i486 machine; the default target is m64 now
    source = tmp_path / "in.ll"
    source.write_text(INPUT)
    runner = tmp_path / "llrm-pass"
    runner.write_text(f"#!/bin/bash\necho 'llrm-pass: target x86-m32 cpu 486 tti X86 datalayout d' >&2\ncat <<'EOF'\n{AGREES}EOF\n")
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
    """gvn takes the second add; simplifycfg would fold `br i1 true`: only the first happens when gvn is asked for.

    The premise was `mul x, 1`, which gvn simplifies itself (simplifyInstruction), so the test failed once gvn was translated."""
    import subprocess

    source = tmp_path / "in.mir"
    source.write_text(
        "define i32 @f(i32 %x) {\nentry:\n  %a = add i32 %x, 1\n  %b = add i32 %x, 1\n  br i1 true, label %t, label %e\nt:\n  %d = add i32 %b, %a\n  ret i32 %d\ne:\n  ret i32 0\n}\n"
    )
    out = subprocess.run([str(llrm_pass()), "-O2", "gvn", str(source)], capture_output=True, text=True, check=True).stdout
    assert out.count(" add i32 %") == 2 and "br i1 true" in out, out
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


def test_llrm_only_attributes_reach_opt_as_strings():
    """A bare `threeway` on a declaration made opt refuse 9 QCport modules: "expected top-level entity"."""
    text = 'declare i16 @_strcmp(ptr, ptr) addrspace(1) threeway nearcode\n@s = constant [8 x i8] c"threeway"\n'
    assert difftest.normalise(text) == 'declare i16 @_strcmp(ptr, ptr) addrspace(1) "threeway" "nearcode"\n@s = constant [8 x i8] c"threeway"\n'


def test_every_opt_command_line_carries_llrms_triple_and_cpu(monkeypatch):
    """inlinediff priced 30 i32 GEPs at 5 each with opt's host default, 370 vs 215 at i486: a dump has no `target triple`, so opt costs for x86-64 unless told."""
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "m32")  # these tests are about the i486 machine; the default target is m64 now
    import inlinediff

    seen = []

    class Done:
        returncode, stdout, stderr = 0, "", ""

    monkeypatch.setattr(difftest.subprocess, "run", lambda command, *a, **k: (seen.append(list(command)), Done())[1])
    monkeypatch.setattr(inlinediff.subprocess, "run", lambda command, *a, **k: (seen.append(list(command)), Done())[1])
    difftest.opt("sroa", "")
    inlinediff.llvm_verdicts("", "inline", None)
    difftest.licm_counts("")
    assert len(seen) == 3 and all("-mtriple=i486-unknown-linux-gnu" in c and "-mcpu=i486" in c for c in seen), seen
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "m16")
    seen.clear()
    difftest.opt("sroa", "")
    assert "-mtriple=i386-unknown-unknown-code16" in seen[0] and "-mcpu=i486" in seen[0]


def test_the_m16_runs_read_a_datalayout_with_16_bit_pointers(monkeypatch):
    """opt used 32-bit pointers for m16 dumps: a pointer-width fold compared at the wrong width. The layout comes from the target's own datalayout.toml."""
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "m16")
    args = difftest.target_args()
    layout = next(a for a in args if a.startswith("--data-layout="))
    assert "p:16:16" in layout and "-mtriple=i386-unknown-unknown-code16" in args
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "m32")
    assert "p:32:32" in next(a for a in difftest.target_args() if a.startswith("--data-layout="))


def test_export_writes_input_both_answers_and_the_diff_of_a_differing_function(tmp_path, monkeypatch):
    """Owners with a 'unit tests only' rule cannot run the oracle: the differing bodies are written for them."""
    src = tmp_path / "in.ll"
    src.write_text("define i32 @f() {\n  ret i32 1\n}\n")
    row = {"config": "c", "fn": "01", "pass": "sroa", "run": 1, "file": str(src)}
    monkeypatch.setattr(difftest, "isolated", lambda replay, row, source: "define i32 @f() {\n  ret i32 2\n}\n")
    monkeypatch.setattr(difftest, "opt", lambda passes, text: type("R", (), {"returncode": 0, "stdout": "define i32 @f() {\n  ret i32 3\n}\n", "stderr": ""})())
    assert difftest.export([(row, None)], "sroa", Path("x"), tmp_path / "out") == 1
    folder = next((tmp_path / "out" / "sroa").iterdir())
    assert {p.name for p in folder.iterdir()} == {"input.ll", "llrm.ll", "opt.ll", "diff.txt"}
    assert "ret i32 2" in (folder / "diff.txt").read_text() and "ret i32 3" in (folder / "diff.txt").read_text()


def test_the_replay_gets_the_targets_layout_so_pointer_width_answers_are_the_targets(monkeypatch, tmp_path):
    """LoopIdiom formed `memset.p0.i64` where opt at i486 forms `.i32`: llrm-pass ran on the Neutral 64-bit layout and the dumps carry no `target datalayout`."""
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "m32")  # these tests are about the i486 machine; the default target is m64 now
    seen = []

    class Done:
        returncode, stdout, stderr = 0, "", ""

    def run(command, *a, **k):
        seen.append(list(command))
        mode = command[command.index("--mode") + 1]
        done = Done()
        done.stderr = f"llrm-pass: target x86-m{mode} cpu 486 tti X86 datalayout d\n"
        return done

    monkeypatch.setattr(difftest.subprocess, "run", run)
    src = tmp_path / "x.ll"
    src.write_text("")
    difftest.isolated(Path("llrm-pass"), {"config": "c-O2", "pass": "loopidiom@737"}, src)
    assert "--mode" in seen[0] and seen[0][seen[0].index("--mode") + 1] == "32", seen
    monkeypatch.setenv("LLRM_DIFFTEST_TARGET", "m16")
    seen.clear()
    difftest.isolated(Path("llrm-pass"), {"config": "c-O2", "pass": "loopidiom@737"}, src)
    assert seen[0][seen[0].index("--mode") + 1] == "16"


def test_a_neutral_llrm_side_is_refused_and_a_target_one_is_named():
    """unroll, licm, rotate, indvars and loopidiom counts were taken with llrm-pass on Neutral (unit costs) against opt at i486: matmul unrolled to 36 blocks, opt's 216."""
    ok = "llrm-pass: target x86-m32 cpu 486 tti X86 datalayout e-p:32:32\n"
    assert difftest.llrm_target(ok, "m32").startswith("x86-m32 cpu 486 tti X86")
    for stderr in ("llrm-pass: slot unroll@753\n", "llrm-pass: target neutral\n", ok.replace("tti X86", "tti none")):
        with pytest.raises(SystemExit, match="llrm side"):
            difftest.llrm_target(stderr, "m32")
    assert difftest.llrm_target("llrm-pass: target neutral\n", "host") == "neutral"


@pytest.mark.skipif(llrm_pass() is None, reason="llrm-pass is not built")
def test_llrm_pass_unrolls_matmul_on_the_targets_machine_not_neutral(tmp_path):
    """Neutral priced the unroll at unit cost and left matmul at 36 blocks; the i486 machine unrolls it to 216 as opt does."""
    import subprocess

    source = Path(__file__).parent / "fixtures/matmul-unroll-in.ll"

    def blocks(*mode):
        done = subprocess.run([str(llrm_pass()), "-O2", *mode, "--order", "unroll@753", str(source)], capture_output=True, text=True, check=True)
        return sum(1 for line in done.stdout.splitlines() if line.endswith(":") and " " not in line), done.stderr

    assert blocks()[0] == 36
    count, stderr = blocks("--mode", "32")
    assert count == 216 and "target x86-m32 cpu 486 tti X86 " in stderr


def test_a_failed_replay_names_the_panic_not_the_header_line_llrm_pass_printed_first():
    """Two bodies panicked in expand_iv.rs and difftest reported "llrm-pass: slot indvars@738 Simplify Old", the first stderr line."""
    stderr = (
        "llrm-pass: slot indvars@738 Simplify Old (LLVM: indvars)\nllrm-pass: target x86-m32 cpu 486 tti X86 datalayout d\n"
        "thread 'main' panicked at crates/opt/x.rs:74:22:\na SCEVable type, not Float(Float)\nnote: run with `RUST_BACKTRACE=1`\n"
    )
    assert difftest.failure_line(stderr) == "crates/opt/x.rs:74:22: a SCEVable type, not Float(Float)"
    assert difftest.failure_line("llrm-pass: target x\nno such file\n") == "no such file"


def test_a_cpu_llrm_does_not_price_for_is_refused_on_the_opt_side(monkeypatch):
    """The m16 oracle ran llrm at cpu 486 against opt at -mcpu=i386: two machines under one header."""
    monkeypatch.setattr(difftest.targetdesc, "targets", lambda: {"m16": {"mode": 16, "llvm_cpu": "i386", "tti": "X86"}})
    with pytest.raises(SystemExit, match="cpu 486 but opt is told -mcpu=i386"):
        difftest.llrm_target("llrm-pass: target x86-m16 cpu 486 tti X86 datalayout d\n", "m16")
    monkeypatch.setattr(difftest.targetdesc, "targets", lambda: {"m16": {"mode": 16, "llvm_cpu": "i486", "tti": "X86"}})
    assert difftest.llrm_target("llrm-pass: target x86-m16 cpu 486 tti X86 datalayout d\n", "m16").startswith("x86-m16 cpu 486")


def test_a_lifetime_marker_with_the_size_operand_reads_as_the_pins_without_it():
    """llrm's text gives `llvm.lifetime.start.p0(i64 4, ptr %p)`; opt's reader upgrades it to `(ptr %p)`. The matmul body of decide@1178 differed
    from `opt -passes=simplifycfg` on those two lines alone."""
    llrm = ["define void @f() {", "b0:", "  %p = alloca i32", "  call void @llvm.lifetime.start.p0(i64 4, ptr %p)", "  call void @llvm.lifetime.end.p0(i64 4, ptr %p)", "  ret void", "}"]
    opt = [line.replace("(i64 4, ptr", "(ptr") for line in llrm]
    assert difftest.canonical(llrm) == difftest.canonical(opt)
    assert difftest.canonical(["declare void @llvm.lifetime.start.p0(i64, ptr)"]) == difftest.canonical(["declare void @llvm.lifetime.start.p0(ptr)"])


def test_a_calls_attributes_are_not_the_code_whether_written_out_or_named_as_a_group():
    """`opt` prints `call void @bar() #4` where llrm writes `call void @bar() nounwind`: the phi-undef-loadstore bodies differed on those
    lines alone (and sink-cb-diff-attrs, hoist-cb-diff-attrs)."""
    llrm = ["define void @f() {", "b0:", "  tail call void @bar() nounwind", "  %r = call i32 @g(i32 1) nocallback nofree memory(none)", "  ret void", "}"]
    opt = [line.replace(" nounwind", " #4").replace(" nocallback nofree memory(none)", " #5") for line in llrm]
    assert difftest.canonical(llrm) == difftest.canonical(opt)
    kept = difftest.canonical(["define void @f() {", "b0:", "  %r = call i32 @g(i32 1)", "  ret void", "}"])
    assert any("call i32 @g(i32 1)" in line for line in kept)

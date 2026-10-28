"""LLRM_ISEL puts `--isel=` after llrm-cc, in the commands run_tests builds."""

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402


def test_the_variable_selects_the_selector_for_llrm_cc_only(monkeypatch):
    monkeypatch.setenv("LLRM_ISEL", "shadow")
    assert dosbatch.with_isel(["/b/llrm-cc", "a.c", "-O2"]) == ["/b/llrm-cc", "--isel=shadow", "a.c", "-O2"]
    assert dosbatch.with_isel(["/b/jwasm", "a.asm"]) == ["/b/jwasm", "a.asm"]


def test_a_qb_program_is_compiled_by_the_selected_selector(monkeypatch):
    """llrm-qb got no `--isel`: LLRM_ISEL=shadow on the 198 qb programs reported 'the DAG path selected no function (539 old)'."""
    monkeypatch.setenv("LLRM_ISEL", "shadow")
    assert dosbatch.with_isel(["/b/llrm-qb", "a.bas", "-o", "a.obj"]) == ["/b/llrm-qb", "--isel=shadow", "a.bas", "-o", "a.obj"]


def test_a_nib_program_is_compiled_by_the_selected_selector(monkeypatch):
    """llrm-nib got no `--isel`: LLRM_ISEL=shadow on the 74 examples reported 'the DAG path selected no function (1235 old)'."""
    monkeypatch.setenv("LLRM_ISEL", "shadow")
    assert dosbatch.with_isel(["/b/llrm-nib", "a.nib", "-o", "a.obj"]) == ["/b/llrm-nib", "--isel=shadow", "a.nib", "-o", "a.obj"]


def test_without_the_variable_nothing_changes(monkeypatch):
    monkeypatch.delenv("LLRM_ISEL", raising=False)
    assert dosbatch.with_isel(["/b/llrm-cc", "a.c"]) == ["/b/llrm-cc", "a.c"]


def test_a_c_program_itself_is_compiled_by_the_selected_selector(monkeypatch, tmp_path):
    """Only the `link:` helpers went through LLRM_ISEL: a run with LLRM_ISEL=dag passed 445 of 449 programs on the old selector."""
    import run_tests  # noqa: E402

    monkeypatch.setenv("LLRM_ISEL", "dag")
    source = tmp_path / "p.c"
    source.write_text("int main(void) { return 0; }\n")
    program = run_tests.Program(source=source, flags=["-m32"], known=None)
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: tmp_path)
    command = run_tests.compile_command(program, tmp_path / "p.obj")
    assert command[1] == "--isel=dag" and Path(command[0]).name == "llrm-cc", command


def test_a_selection_count_needs_the_selectors_report(tmp_path):
    """445 of 449 programs 'passed' under LLRM_ISEL=dag on the old selector: a count without the report that shows who selected is void."""
    import pytest

    report = tmp_path / "isel.txt"
    with pytest.raises(dosbatch.ProofMissing):
        dosbatch.isel_proof(report, "dag")
    report.write_text("")
    with pytest.raises(dosbatch.ProofMissing):
        dosbatch.isel_proof(report, "dag")
    report.write_text("f old\ng old\n")
    with pytest.raises(dosbatch.ProofMissing):
        dosbatch.isel_proof(report, "dag")
    assert dosbatch.isel_proof(report, "old") == (0, 2)
    report.write_text("f dag\ng old\nh dag\n")
    assert dosbatch.isel_proof(report, "dag") == (2, 1)


def test_real_mode_c_takes_the_selector_too(monkeypatch, tmp_path):
    """Real-mode C programs go through LLRM_ISEL like the rest (llrm-c kept them on the old selector)."""
    import run_tests  # noqa: E402

    monkeypatch.setenv("LLRM_ISEL", "dag")
    source = tmp_path / "p.c"
    source.write_text("int main(void) { return 0; }\n")
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: tmp_path)
    command = run_tests.compile_command(run_tests.Program(source=source, flags=[], known=None), tmp_path / "p.obj")
    assert command[1] == "--isel=dag", command


def test_a_clock_pair_names_the_excess_precision_of_a_basic_program_and_extra_flags(monkeypatch, tmp_path):
    """main compiled SINGLE as fast and the tip as standard, so a main-vs-tip clock row read the deliberate rounding as a slowdown (nbody_single.bas +29.6%).
    LLRM_EXCESS sets it on a BASIC program only; LLRM_EXTRA_FLAGS adds flags to every program (an ablation)."""
    import run_tests  # noqa: E402

    monkeypatch.setenv("LLRM_EXCESS", "fast")
    monkeypatch.setenv("LLRM_EXTRA_FLAGS", "-fno-tree-ch")
    basic = tmp_path / "p.bas"
    basic.write_text("PRINT 1\n")
    c = tmp_path / "p.c"
    c.write_text("int main(void) { return 0; }\n")
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: tmp_path)
    one = run_tests.compile_command(run_tests.Program(source=basic, flags=[], known=None), tmp_path / "p.obj")
    two = run_tests.compile_command(run_tests.Program(source=c, flags=[], known=None), tmp_path / "p.obj")
    assert "-fexcess-precision=fast" in one and "-fno-tree-ch" in one, one
    assert "-fexcess-precision=fast" not in two and "-fno-tree-ch" in two, two

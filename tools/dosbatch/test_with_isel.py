"""LLRM_ISEL puts `--isel=` after llrm-c, in the commands run_tests builds."""

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402


def test_the_variable_selects_the_selector_for_llrm_c_only(monkeypatch):
    monkeypatch.setenv("LLRM_ISEL", "shadow")
    assert dosbatch.with_isel(["/b/llrm-c", "a.c", "-O2"]) == ["/b/llrm-c", "--isel=shadow", "a.c", "-O2"]
    assert dosbatch.with_isel(["/b/jwasm", "a.asm"]) == ["/b/jwasm", "a.asm"]


def test_without_the_variable_nothing_changes(monkeypatch):
    monkeypatch.delenv("LLRM_ISEL", raising=False)
    assert dosbatch.with_isel(["/b/llrm-c", "a.c"]) == ["/b/llrm-c", "a.c"]


def test_a_c_program_itself_is_compiled_by_the_selected_selector(monkeypatch, tmp_path):
    """Only the `link:` helpers went through LLRM_ISEL: a run with LLRM_ISEL=dag passed 445 of 449 programs on the old selector."""
    import run_tests  # noqa: E402

    monkeypatch.setenv("LLRM_ISEL", "dag")
    source = tmp_path / "p.c"
    source.write_text("int main(void) { return 0; }\n")
    program = run_tests.Program(source=source, flags=["-m32"], known=None)
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: tmp_path)
    command = run_tests.compile_command(program, tmp_path / "p.obj")
    assert command[1] == "--isel=dag" and Path(command[0]).name == "llrm-c", command

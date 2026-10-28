"""`c_compile` names the C compiler a run builds its programs with: llrm-c, or llrm-cc where LLRM_C_FRONTEND says so."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402


def command(monkeypatch, frontend, target="dos16"):
    monkeypatch.setattr(dosbatch, "m_flag", lambda target: "-m16" if target == "dos16" else "-m32")
    monkeypatch.setattr(dosbatch, "target_bits", lambda target: 16 if target == "dos16" else 32)
    if frontend:
        monkeypatch.setenv("LLRM_C_FRONTEND", frontend)
    else:
        monkeypatch.delenv("LLRM_C_FRONTEND", raising=False)
    return dosbatch.c_compile(Path("a.c"), Path("a.obj"), target, "-O2", Path("inc"), ["-mabi=sysv"])


def test_llrm_c_stays_the_default_and_takes_the_target_as_a_flag(monkeypatch):
    words, env = command(monkeypatch, None)
    assert Path(words[0]).name == "llrm-c" and "-m16" in words and env == {}


def test_llrm_cc_takes_the_target_as_a_row_not_a_flag(monkeypatch):
    """Passed -m16, llrm-cc handed clang a flag it has no meaning for; the target is LLRM_CC_TARGET."""
    for target, row in (("dos16", "x86_16-wat"), ("dos32", "x86_32-wat")):
        words, env = command(monkeypatch, "llrm-cc", target)
        assert Path(words[0]).name == "llrm-cc" and "-m16" not in words and "-m32" not in words
        assert env == {"LLRM_CC_TARGET": row} and "-c" in words and words[-2:] == ["-o", "a.obj"]


def test_the_isel_switch_reaches_llrm_cc_too(monkeypatch):
    monkeypatch.setenv("LLRM_ISEL", "dag")
    assert dosbatch.with_isel(["/x/llrm-cc", "a.c"])[1] == "--isel=dag"


def test_a_target_without_a_row_is_a_build_error_not_a_key_error(monkeypatch):
    """x86-m64 has no Watcom row: the run died in a KeyError: 64 instead of skipping the program with a reason."""
    monkeypatch.setattr(dosbatch, "target_bits", lambda target: 64)
    monkeypatch.setenv("LLRM_C_FRONTEND", "llrm-cc")
    try:
        dosbatch.c_compile(Path("a.c"), Path("a.obj"), "x86-m64", "-O2", Path("inc"), [])
    except dosbatch.BuildError as why:
        assert "no row for x86-m64" in str(why)
    else:
        raise AssertionError("no error")


def test_a_c_program_itself_is_built_by_llrm_cc_where_the_run_names_it(monkeypatch, tmp_path):
    """Only the `link:` extras went through LLRM_C_FRONTEND: a pass of 463 programs 'on llrm-cc' ran llrm-c for all but 21 of them
    (wccq errors in its log), and passed with LLRM_CLANG_M16 unset."""
    import run_tests  # noqa: E402

    monkeypatch.setenv("LLRM_C_FRONTEND", "llrm-cc")
    monkeypatch.delenv("LLRM_ISEL", raising=False)
    source = tmp_path / "p.c"
    source.write_text("int main(void) { return 0; }\n")
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: tmp_path)
    program = run_tests.Program(source=source, flags=["-O2", "-m32", "-march=i486"], known=None)
    command = run_tests.compile_command(program, tmp_path / "p.obj")
    assert Path(command[0]).name == "llrm-cc" and "-c" in command and "-m32" not in command and "-march=i486" in command, command
    assert run_tests.compile_env(program) == {"LLRM_CC_TARGET": "x86_32-wat"}
    assert run_tests.compile_env(run_tests.Program(source=source, flags=[], known=None)) == {"LLRM_CC_TARGET": "x86_16-wat"}


def test_the_start_up_of_a_llrm_cc_c_program_enters_main_(monkeypatch):
    """The Watcom rows name the entry `main_`: with the start-up asking for `_main`, 420 of 463 programs failed to link
    ("E2028: _main is an undefined reference")."""
    monkeypatch.setattr(dosbatch, "os_layer", lambda target, field, language: "A=1 B=2")
    monkeypatch.setenv("LLRM_C_FRONTEND", "llrm-cc")
    assert dosbatch.os_defines("x86-m16", "c") == ("A=1", "B=2", "LL_MAIN=main_")
    assert dosbatch.os_defines("x86-m16", "nib") == ("A=1", "B=2")
    monkeypatch.delenv("LLRM_C_FRONTEND")
    assert dosbatch.os_defines("x86-m16", "c") == ("A=1", "B=2")


def test_a_sysv_or_cdecl_program_keeps_the_start_ups_default_entry(monkeypatch):
    """-mabi=sysv names `_main`, not Watcom's `main_`: 21 rows failed "main_ is an undefined reference"."""
    monkeypatch.setattr(dosbatch, "os_layer", lambda target, field, language: "A=1")
    monkeypatch.setenv("LLRM_C_FRONTEND", "llrm-cc")
    assert dosbatch.os_defines("x86-m32", "c", "sysv") == ("A=1",)
    assert dosbatch.os_defines("x86-m32", "c", "cdecl") == ("A=1",)
    assert dosbatch.os_defines("x86-m16", "c", "ia16") == ("A=1",)  # ia16_abi: "main_ is an undefined reference"
    assert dosbatch.os_defines("x86-m32", "c", None) == ("A=1", "LL_MAIN=main_")


def test_start_up_objects_built_with_different_entries_do_not_share_a_path(tmp_path):
    """Rows run at once in one work directory: the sysv rows' `_main` start-up and the Watcom rows' `main_` one were written to one
    file and replaced each other (a pass fell from 392 to 215, "_main is an undefined reference")."""
    a = dosbatch.runtime_path(tmp_path, "runtime/shared/dos/m32/start.asm", ("A=1",))
    b = dosbatch.runtime_path(tmp_path, "runtime/shared/dos/m32/start.asm", ("A=1", "LL_MAIN=main_"))
    assert a != b and a.parent == b.parent == tmp_path


def test_llrm_cc_reads_the_rows_own_os_header_not_llrm_cs(monkeypatch, tmp_path):
    """A -I of llrm-c's llrm_os.h came before the row's -isystem one: os_files, os_find, os_more... called the OS layer by llrm-c's
    pragmas and read -6 for 6 (7 programs of the DOSBox pass)."""
    import run_tests  # noqa: E402

    monkeypatch.setenv("LLRM_C_FRONTEND", "llrm-cc")
    monkeypatch.delenv("LLRM_ISEL", raising=False)
    source = tmp_path / "p.c"
    source.write_text("int main(void) { return 0; }\n")
    monkeypatch.setattr(dosbatch, "c_include", lambda target, work: Path("/llrm-c-header-dir"))
    command = run_tests.compile_command(run_tests.Program(source=source, flags=[], known=None), tmp_path / "p.obj")
    assert "/llrm-c-header-dir" not in command, command
    assert "/llrm-c-header-dir" not in dosbatch.c_compile(source, tmp_path / "p.obj", "x86-m16", "-O2", Path("/llrm-c-header-dir"), [])[0]

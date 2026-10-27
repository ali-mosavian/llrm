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

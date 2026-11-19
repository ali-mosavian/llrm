"""The harness builds a program with the flags its own `// flags:` header asks for.

wrapv_overflow_check (`// flags: -O2 -fwrapv | -Os -fwrapv`) was built without -fwrapv in every column: it returned 0 0 0 1 where the
test wants 1 1 0 1 (`ok: false` on main, tip and head), and its sizes in the m32 tables were another program's (O2 code 144 B against 194).
"""
import os
import re
from pathlib import Path

import pytest

import flagcheck
import headerflags

HERE = Path(__file__).parent
WRAPV = "// flags: -O2 -fwrapv | -Os -fwrapv\nint x;\n"


def test_a_wrapv_program_is_asked_for_fwrapv_at_every_level():
    for level in ("O1", "O2", "O3", "Os"):
        assert headerflags.column_flags("llrm", WRAPV, level) == ["-fwrapv"], level


def test_the_level_picks_its_own_configuration_and_other_targets_are_skipped():
    text = "// flags: -O2 -march=i486 -mabi=watcom -ffoo | -O2 -march=pentium -fbar | -O2 -march=i486 -m32 -fwrapv | -Os -fnosize\n"
    assert headerflags.program_tokens(text, "O2") == ["-fwrapv"]
    assert headerflags.program_tokens(text, "Os") == ["-fnosize"]
    assert headerflags.program_tokens(text, "O3") == ["-fwrapv"], "no -O3 configuration: the -O2 one"


def test_a_program_without_a_header_or_extra_flags_gets_none():
    assert headerflags.program_tokens("int x;\n", "O2") == []
    assert headerflags.program_tokens("// flags: -O2 -march=i486 | -O2 -march=i486 -m32\n", "O2") == []


def test_gcc_takes_its_own_spelling_and_a_flag_it_lacks_leaves_no_gcc_column():
    text = "// flags: -O2 -fno-inline-functions\n"
    assert headerflags.column_flags("gcc", text, "O2") == ["-fno-inline"]
    assert headerflags.column_flags("clang", text, "O2") == ["-fno-inline-functions"]
    with pytest.raises(ValueError):
        headerflags.column_flags("gcc", "// flags: -O2 -fno-inline=half\n", "O2")


def test_every_flagged_run_program_has_a_configuration_the_harness_can_read():
    run = HERE.parents[3] / "tests" / "run" / "c"
    flagged = [p for p in sorted(run.glob("*.c")) if "-fwrapv" in p.read_text(errors="replace").split("\n", 1)[0]]
    assert [p.name for p in flagged] == ["wrapv_overflow_check.c"], "premise: -fwrapv is in one program's header"
    assert headerflags.program_tokens(flagged[0].read_text(), "O2") == ["-fwrapv"]


def test_flagcheck_refuses_a_column_built_without_the_headers_flag(monkeypatch):
    import external_flags
    base = [*external_flags.flags("x86", ("-fno-pic",)), "-m32", "-Dfar=", "-march=i486"]
    source = HERE.parents[3] / "tests" / "run" / "c" / "wrapv_overflow_check.c"
    monkeypatch.setenv("VSGCC_SOURCE", str(source))
    assert flagcheck.check_external("gcc", "O2", [*base, "-fwrapv", "-mpreferred-stack-boundary=2", "-O2"]) == ""
    assert "-fwrapv" in flagcheck.check_external("gcc", "O2", [*base, "-mpreferred-stack-boundary=2", "-O2"])
    assert "-fwrapv" in flagcheck.check_external("clang", "O2", [*base, "-mstack-alignment=4", "-O2"])


def test_every_llrm_cc_line_of_build_sh_carries_the_header_flags():
    text = (HERE / "build.sh").read_text()
    lines = [line for line in text.splitlines() if "$LLRM --target=x86_32-wat" in line and "ld_probe" not in line]
    assert len(lines) == 4, lines
    assert all(re.search(r"\$\(HF llrm \$?\w+\)", line) for line in lines), lines
    assert text.count("$HFC") >= 3 and text.count("$HFG") >= 3

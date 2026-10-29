"""`#pragma aux NAME "LINK"` spells a symbol, function or object, in any calling convention.

A register-convention function's symbol was respelled `_name@3`, so the QB runtime's `B$LTRM` could not be defined in C without `__cdecl`.
"""

import os
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))

import llrmbin  # noqa: E402

# The pragma comes before the declaration it names, as Watcom's does and clang's reads it; llrm-c also took it after.
SOURCE = """\
#pragma aux ltrm "B$LTRM"
#pragma aux b_seg "b$seg"
unsigned b_seg;
int ltrm(int a, long b) { return a + (int)b + (int)b_seg; }
"""


ROWS = ("x86_16-wat", "x86_16-dos", "x86_16-dos-rp3")  # watcom (default), cdecl, regparm3


def compile_to_asm(source: Path, out: Path, row: str) -> subprocess.CompletedProcess:
    command = [str(llrmbin.bin_dir() / "llrm-cc"), f"--target={row}", "-O2", "-S", "-o", str(out), str(source)]
    return subprocess.run(command, capture_output=True, text=True, env={**os.environ, **llrmbin.cc_environment()})


@pytest.mark.parametrize("row", ROWS)
def test_the_pragma_name_is_the_symbol_in_every_abi(row, tmp_path):
    (tmp_path / "n.c").write_text(SOURCE)
    done = compile_to_asm(tmp_path / "n.c", tmp_path / "n.s", row)
    assert done.returncode == 0, done.stderr
    published = {line.split()[1] for line in (tmp_path / "n.s").read_text().splitlines() if line.startswith("public ")}
    assert published == {"B$LTRM", "b$seg"}


CALLER = """\
#pragma aux helper "B$HELPER"
extern void helper(unsigned);
void f(void) { helper(1); }
"""


def test_a_c_function_named_b_dollar_is_called_like_any_other(tmp_path):
    """A name must not change code generation: calling `B$HELPER` from C asked for a BASIC stack-cleanup
    contract and failed ("runtime call B$HELPER has no complete stack-cleanup contract")."""
    (tmp_path / "c.c").write_text(CALLER)
    done = compile_to_asm(tmp_path / "c.c", tmp_path / "c.s", "x86_16-wat")
    assert done.returncode == 0, done.stderr
    assert "far ptr B$HELPER" in (tmp_path / "c.s").read_text()  # a tail call at -O2

"""`#pragma aux NAME "LINK"` spells a symbol, function or object, in any calling convention.

llrm-c's default ABI passes arguments in registers, and the symbol of such a function was respelled
`_name@3`, so the QB runtime's `B$LTRM` could not be defined in C without `__cdecl`.
"""

import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))

import llrmbin  # noqa: E402

SOURCE = """\
unsigned b_seg;
int ltrm(int a, long b) { return a + (int)b + (int)b_seg; }
#pragma aux ltrm "B$LTRM"
#pragma aux b_seg "b$seg"
"""


@pytest.mark.parametrize("abi", [None, "cdecl", "regparm3"])
def test_the_pragma_name_is_the_symbol_in_every_abi(abi, tmp_path):
    (tmp_path / "n.c").write_text(SOURCE)
    command = [str(llrmbin.bin_dir() / "llrm-c"), str(tmp_path / "n.c"), "-m16", "-O2", "-S", "-o", str(tmp_path / "n.s")]
    subprocess.run(command + ([f"-mabi={abi}"] if abi else []), check=True, capture_output=True)
    published = {line.split()[1] for line in (tmp_path / "n.s").read_text().splitlines() if line.startswith("public ")}
    assert published == {"B$LTRM", "b$seg"}

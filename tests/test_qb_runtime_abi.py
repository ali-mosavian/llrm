"""How `llrm-qb -fqb-runtime=llrm` cleans the stack after a call that keeps BASIC's pushes.

COLOR, LOCATE, SCREEN, DIM and CLOSE push as many words as they have arguments and the callee pops them
all.  A call with more words than its declaration lists was taken for a variadic one whose extra words
the caller removes, so `COLOR 2, 1` left `add sp, 2` after a call that had already popped them: the stack
pointer moved up two bytes at every such statement, and a loop of them ended in a corrupted string space.
"""

import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))

import llrmbin  # noqa: E402


def listing(source: str, tmp_path: Path) -> list[str]:
    (tmp_path / "p.bas").write_text(source)
    command = [str(llrmbin.bin_dir() / "llrm-qb"), str(tmp_path / "p.bas"), "--dialect", "qb45", "-fqb-runtime=llrm", "-O0", "-S", "-o", str(tmp_path / "p.s")]
    subprocess.run(command, check=True, capture_output=True)
    return [line.strip() for line in (tmp_path / "p.s").read_text().splitlines()]


@pytest.mark.parametrize("statement", ["COLOR 2, 1", "COLOR , 3", "COLOR 1, 2, 3", "LOCATE 1, 2, 1, 3, 4", "COLOR a%, b%", "CLOSE #1, #2, #3"])
def test_the_caller_does_not_clean_what_a_block_call_pops(statement, tmp_path):
    lines = listing(f"a% = 1\nb% = 2\nSCREEN 0\n{statement}\nPRINT 1\n", tmp_path)
    for at, line in enumerate(lines):
        if line.startswith("call far ptr B$COLR") or line.startswith("call far ptr B$LOCT") or line.startswith("call far ptr B$CLOS"):
            assert not lines[at + 1].startswith("add sp"), f"{line} is followed by {lines[at + 1]}"

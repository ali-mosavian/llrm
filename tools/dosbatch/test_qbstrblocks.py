"""SPACE$, STRING$ and string comparison on the llrm runtime, m16 and m32.

They were C loops a byte at a time: SPACE$(54) cost 471 instructions and comparing two 54-byte strings 719, where BCOM45's
`rep stosw` and `repe cmpsb` take 157 and 72.  They are `rep stosd`/`repe cmpsd` kernels now (block.asm).  The limits are a
little over twice what the kernels take, so a byte loop fails them; a self-checking program covers the results at every
length and mismatch position."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbbuild  # noqa: E402

pytestmark = pytest.mark.slow_dos
N = 2000
HEAD = "DEFINT A-Z\nDIM s AS STRING, t AS STRING, u AS STRING\ns = SPACE$(768): t = SPACE$(54): u = SPACE$(54)\nFOR i = 1 TO %d\n" % N
TAIL = "\nNEXT\nEND\n"
# per iteration, over the empty loop
LIMITS = {"SPACE$(54)": ('t = SPACE$(54)', 300), "STRING$(300)": ('t = STRING$(300, 65)', 400), "compare 54 equal": ("IF t = u THEN j = j + 1", 160)}
CHECK = """DEFINT A-Z
DIM a AS STRING, b AS STRING, c AS STRING
lt = 0: gt = 0: eq = 0
FOR n = 0 TO 40
  a = STRING$(n, 97)
  IF a = STRING$(n, 97) THEN eq = eq + 1
  FOR p = 1 TO n
    b = a
    MID$(b, p, 1) = "b"
    IF a < b THEN lt = lt + 1
    IF b > a THEN gt = gt + 1
  NEXT
NEXT
hi = (CHR$(200) < CHR$(100)) + (CHR$(100) > CHR$(200)) + (CHR$(127) > CHR$(128))
sz = LEN(SPACE$(37)) + LEN(STRING$(5, 66))
PRINT lt; gt; eq; hi; sz; ASC(MID$(STRING$(41, 66), 41, 1)); ASC(RIGHT$(SPACE$(7), 1))
a = "abcdefghijklmnopqrst"
MID$(a, 2) = a
smear = 0
FOR p = 1 TO 20
  IF MID$(a, p, 1) = "a" THEN smear = smear + 1
NEXT
b = SPACE$(40)
c = "0123456789ABCDEFGHIJKLMNOP"
MID$(b, 7) = c
MID$(b, 36, 9) = c
ON ERROR GOTO oops
MID$(b, 0, 1) = "x"
%s
PRINT smear; ASC(MID$(b, 7, 1)); ASC(MID$(b, 32, 1)); ASC(MID$(b, 33, 1)); ASC(MID$(b, 40, 1)); errs
END
oops:
errs = errs + 1
RESUME NEXT
"""
# The flat runtime ends the program at a second error under ON ERROR (#1482): there only one is raised.
MORE_ERRORS = 'MID$(b, 41, 1) = "x"\nMID$(b, 1, -1) = "x"'
EXPECT = {qbbuild.REAL: [820, 820, 41, -3, 42, 66, 32, 20, 48, 80, 32, 52, 3], qbbuild.FLAT: [820, 820, 41, -3, 42, 66, 32, 20, 48, 80, 32, 52, 1]}


def instructions(where: Path) -> int:
    best = 0
    for line in (where / "events.txt").read_text(errors="replace").splitlines():
        if '"ev":"end"' in line:
            best = json.loads(line).get("instructions", 0)
    return best


@pytest.fixture(scope="module")
def runner(tmp_path_factory):
    if not dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    work = tmp_path_factory.mktemp("strblocks")
    objects = qbbuild.runtimes(work)

    def run(name: str, text: str):
        """`text` built for both targets and run on each: {target: (result, instructions)}."""
        src = work / f"{name}.bas"
        src.write_text(text)
        out = {}
        for target, built in qbbuild.build(src, work / f"b{name}", objects).items():
            where = work / f"run{name}{target}"
            result = dosbatch.run([dosbatch.Job("J", "exe", built.exe, files=built.files)], where, budget_ms=60000)["J"]
            assert result.status == "ok", f"{target}: {result.detail}"
            out[target] = (result, instructions(where))
        return out

    return run


@pytest.mark.parametrize("name", list(LIMITS))
def test_block_string_operations_cost_a_string_instruction_not_a_byte_loop(runner, name: str):
    body, limit = LIMITS[name]
    empty = runner("empty", HEAD + "j = j + 1" + TAIL)
    got = runner(name.replace("$", "S").replace("(", "").replace(")", "").replace(" ", ""), HEAD + body + TAIL)
    for target in qbbuild.TARGETS:
        assert (got[target][1] - empty[target][1]) / N <= limit, f"{target}"


def test_comparison_and_fill_results_at_every_length(runner):
    """The flat runtime ends the program at a second error under ON ERROR (#1482), so it raises one."""
    got = {}
    for target, text in ((qbbuild.REAL, CHECK % MORE_ERRORS), (qbbuild.FLAT, CHECK % "")):
        got[target] = runner(f"check{target}", text)[target][0]
    for target in qbbuild.TARGETS:
        assert [int(word) for word in got[target].text.split()] == EXPECT[target], f"{target}"

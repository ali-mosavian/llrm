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
import qb32  # noqa: E402
import qbruntime  # noqa: E402

pytestmark = pytest.mark.slow_dos
N = 2000
HEAD = "DEFINT A-Z\nDIM s AS STRING, t AS STRING, u AS STRING\ns = SPACE$(768): t = SPACE$(54): u = SPACE$(54)\nFOR i = 1 TO %d\n" % N
TAIL = "\nNEXT\nEND\n"
# per iteration, over the empty loop
LIMITS = {"SPACE$(54)": ('t = SPACE$(54)', 300), "STRING$(300)": ('t = STRING$(300, 65)', 400), "compare 54 equal": ("IF t = u THEN j = j + 1", 160)}
CHECK = """DEFINT A-Z
DIM a AS STRING, b AS STRING
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
hi = (CHR$(200) > CHR$(100)) + (CHR$(100) < CHR$(200)) + (CHR$(127) < CHR$(128))
sz = LEN(SPACE$(37)) + LEN(STRING$(5, 66))
PRINT lt; gt; eq; hi; sz; ASC(MID$(STRING$(41, 66), 41, 1)); ASC(RIGHT$(SPACE$(7), 1))
"""
EXPECT = [820, 820, 41, -3, 42, 66, 32]


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
    archive, _ = qbruntime.build(work / "archive")
    flat = qb32.build(work / "flat")
    seen = {}

    def run(target: str, name: str, text: str):
        src = work / f"{name}.bas"
        src.write_text(text)
        obj = work / f"{name}{target}.obj"
        flags = ["-O2", "-march=i486"] + ([dosbatch.m_flag(qb32.TARGET)] if target == "32" else [])
        done = dosbatch.subprocess.run([str(dosbatch.BIN / "llrm-qb"), str(src), "--dialect", "qb45", *flags, "-o", str(obj)], capture_output=True, text=True)
        assert done.returncode == 0, done.stderr[-300:]
        where = work / f"run{name}{target}"
        if target == "32":
            exe = work / f"{name}.exe"
            job = dosbatch.Job("J", "exe", exe, files=tuple(qb32.link(obj, flat, exe, work)))
            result = dosbatch.run([job], where, budget_ms=60000)["J"]
        else:
            job = dosbatch.Job("J", "obj", obj, runtime="llrmqb", runtime_file=archive)
            result = dosbatch.run([job], where, budget_ms=60000, tools=dosbatch.QB45_TOOLS)["J"]
        assert result.status == "ok", result.detail
        return result, instructions(where)

    return run


@pytest.mark.parametrize("target", ["16", "32"])
@pytest.mark.parametrize("name", list(LIMITS))
def test_block_string_operations_cost_a_string_instruction_not_a_byte_loop(runner, target: str, name: str):
    body, limit = LIMITS[name]
    _, empty = runner(target, "empty", HEAD + "j = j + 1" + TAIL)
    _, got = runner(target, name.replace("$", "S").replace("(", "").replace(")", "").replace(" ", ""), HEAD + body + TAIL)
    assert (got - empty) / N <= limit


@pytest.mark.parametrize("target", ["16", "32"])
def test_comparison_and_fill_results_at_every_length(runner, target: str):
    result, _ = runner(target, "check", CHECK)
    assert [int(word) for word in result.text.split()] == EXPECT

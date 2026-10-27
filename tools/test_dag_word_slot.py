"""`--isel=dag` stored a dword zero into a word spill slot (bench/grep, m16 -O2: `mov dword [bp-20],0` zeroed the loop bound spilled next to it; 2.39G instructions against the old selector's 245M, the DOSBox test stalled at its limit)."""
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402

FIXTURE = Path(__file__).resolve().parents[1] / "tests/fixtures/c/dag_zero_word_slot.c"
CELL = re.compile(r"^\s*mov\s+(.*?)\s*(?:,|$)", re.M)


def dword_zero_into_word_slot(asm: str) -> list[int]:
    """Frame offsets that get `mov dword [bp-N], imm` while every other access of N is a word (a long's slot is read as a dword)."""
    seen: dict[int, set[str]] = {}
    for line in asm.splitlines():
        for width, off in re.findall(r"(byte|word|dword) ptr \[bp-(\d+)\]", line):
            kind = "imm" if re.search(r"ptr \[bp-\d+\], -?\d+\s*$", line) else "reg"
            seen.setdefault(int(off), set()).add(f"{width}-{kind}")
    return sorted(off for off, uses in seen.items() if "dword-imm" in uses and "dword-reg" not in uses and any(use.startswith("word") for use in uses))


def test_a_dword_zero_into_a_word_slot_is_found_in_the_asm():
    assert dword_zero_into_word_slot("mov word ptr [bp-4], 1\nmov dword ptr [bp-4], 0\nmov eax, dword ptr [bp-8]\n") == [4]
    assert dword_zero_into_word_slot("mov dword ptr [bp-4], 0\nmov eax, dword ptr [bp-4]\nmov word ptr [bp-4], 1\n") == []


def test_dag_never_stores_a_dword_into_a_word_spill_slot():
    """The symptom: bench/grep's bound `n` at [bp-2] zeroed by the dword zero stored at [bp-4]."""
    binary = llrmbin.bin_dir() / "llrm-c"
    with tempfile.TemporaryDirectory() as work:
        out = Path(work) / "x.s"
        done = subprocess.run([str(binary), str(FIXTURE), "-m16", "-march=i386", "-O2", "--isel=dag", "-S", "-o", str(out)], capture_output=True, text=True)
        assert done.returncode == 0, done.stderr
        assert dword_zero_into_word_slot(out.read_text()) == []

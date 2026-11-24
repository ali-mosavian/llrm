"""tools/bench/modelclk.py: the model clocks of a host run are the 486 table's, and agree with an instruction-by-instruction emulation."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import modelclk as mc  # noqa: E402
from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_MODE_32, Uc  # noqa: E402

# mov ecx,100 ; loop: add eax,ecx ; dec ecx ; jnz loop ; ret
CODE = bytes.fromhex("b964000000" "01c8" "49" "75fb" "c3")
BASE = 0x1000
SEGMENTS = [(BASE, CODE, len(CODE))]
COUNTS = {BASE: 1, BASE + 5: 100, BASE + 7: 100, BASE + 8: 100, BASE + 10: 1}
TAKEN = {BASE + 8: 99}


def test_a_five_instruction_loop_is_the_clocks_added_by_hand():
    """486 table (machine/tune/x86/i486.toml): mov_ri 1, alu_rr 1 (add, dec), jcc 3 taken / 1 not taken, ret_near 5:
    1 + 100*1 + 100*1 + (99*3 + 1*1) + 5 = 504."""
    assert mc.priced(SEGMENTS, COUNTS, TAKEN) == (504, 302, 0)


def test_a_wrong_table_entry_changes_the_total():
    """The total is the table's: `alu_rr` priced 2 would make the loop 704, not 504."""
    clocks = mc.harness.clocks
    table = clocks._FLAT[1]
    kept = table["alu_rr"]
    table["alu_rr"] = 2
    try:
        assert mc.priced(SEGMENTS, COUNTS, TAKEN)[0] == 704
    finally:
        table["alu_rr"] = kept
    assert mc.priced(SEGMENTS, COUNTS, TAKEN)[0] == 504


def test_an_address_in_no_segment_is_unpriced_not_guessed():
    assert mc.priced(SEGMENTS, {0x5000: 7}, {}) == (0, 0, 7)


ASM = """
.globl _start
_start:
    movl $20000, %ecx
    xorl %eax, %eax
loop:
    addl %ecx, %eax
    movl %eax, %edx
    andl $255, %edx
    cmpl $100, %edx
    jb skip
    incl %eax
skip:
    decl %ecx
    jnz loop
done:
    movl $1, %eax
    xorl %ebx, %ebx
    int $0x80
"""


def emulated(exe: Path, low: int, high: int) -> tuple[int, int]:
    """(clocks, instructions) between `low` and `high` stepping every instruction in unicorn: a branch is taken where the next address is not the
    fall-through, as harness.run decides it."""
    segments = mc.harness.elf_segments(exe)
    uc = Uc(UC_ARCH_X86, UC_MODE_32)
    uc.mem_map(0x08000000, 0x1000000)
    for va, blob, _ in segments:
        uc.mem_write(va, blob)
    state = {"prev": None, "clocks": 0, "instructions": 0}

    def step(uc, address, size, _):
        prev = state["prev"]
        if prev is not None:
            ins, at = prev
            state["clocks"] += mc.harness.cost(ins, address != at + ins.len, True, None, mc.WORST_MULTIPLIER)
            state["instructions"] += 1
        state["prev"] = None
        if address >= high:
            uc.emu_stop()
        elif low <= address:
            state["prev"] = (mc.decoded(segments, address), address)

    uc.hook_add(UC_HOOK_CODE, step)
    entry = int.from_bytes(exe.read_bytes()[24:28], "little")
    uc.emu_start(entry, high)
    if state["prev"] is not None:  # unicorn stops at `high` before its hook sees it: the last instruction falls through
        ins, _ = state["prev"]
        state["clocks"] += mc.harness.cost(ins, False, True, None, mc.WORST_MULTIPLIER)
        state["instructions"] += 1
    return state["clocks"], state["instructions"]


def test_the_host_run_prices_as_the_emulation_does(tmp_path):
    """Callgrind's counts priced by `modelclk` against unicorn stepping the same ELF: the same clocks. A branch's taken share and every count
    must agree, or a kernel of 20000 iterations drifts by thousands of clocks."""
    source, exe = tmp_path / "k.s", tmp_path / "k"
    source.write_text(ASM)
    subprocess.run(["gcc", "-m32", "-static", "-nostdlib", "-o", str(exe), str(source)], check=True)
    symbols = subprocess.run(["nm", str(exe)], capture_output=True, text=True, check=True).stdout.split("\n")
    at = {name: int(addr, 16) for addr, _, name in (line.split() for line in symbols if line)}
    low, high = at["loop"], at["done"]
    assert mc.model_clocks(exe, region=(low, high)) == (*emulated(exe, low, high), 0)

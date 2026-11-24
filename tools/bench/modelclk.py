"""486 model clocks of a program run on the host: callgrind's executed-instruction counts, each instruction priced by the same
`harness.cost` the vsgcc comparison prices by (so one table for the compiler, the emulator and this).

MODEL, not time. It ignores: AGI stalls, Pentium pairing, cache and line-fill misses, branch mispredicts, misaligned operands, the
MUL early-out (priced at its worst case) and the iterations of a repeated string instruction (one execution each). An address outside
the binary (libc in a shared object) is unpriced and reported.

    uv run --project tools python tools/bench/modelclk.py EXE [ARGS...]
"""

from __future__ import annotations

import collections
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

VSGCC = Path(__file__).resolve().parents[2] / "crates/target/llrm-x86-m32/vsgcc"
sys.path.insert(0, str(VSGCC))
import harness  # noqa: E402
from iced_x86 import Decoder, Mnemonic  # noqa: E402

WORST_MULTIPLIER = 0xFFFFFFFF
IGNORES = "AGI, pairing, cache misses, mispredicts, misaligned operands, MUL early-out (worst case), repeated string iterations"


def decoded(segments, address):
    """The instruction at `address` in the ELF's `segments`, or None where no segment holds it."""
    for va, blob, _ in segments:
        if va <= address < va + len(blob):
            offset = address - va
            return Decoder(32, blob[offset:offset + 15], ip=address).decode()
    return None


def clocks_of(ins, count, taken):
    """`count` executions of `ins`, `taken` of them branching: harness.cost's price of each."""
    if ins.mnemonic == Mnemonic.INVALID:
        return None
    price = lambda branch: harness.cost(ins, branch, True, None, WORST_MULTIPLIER)  # noqa: E731
    if ins.is_jcc_short_or_near:
        return taken * price(True) + (count - taken) * price(False)
    return count * price(False)


def priced(segments, counts, taken, region=None):
    """(clocks, instructions, unpriced instructions) of the executed `counts` {address: n} and branches `taken` {address: n}; `region` (low, high)
    keeps only the addresses in it."""
    clocks = instructions = unpriced = 0
    for address, count in counts.items():
        if region and not region[0] <= address < region[1]:
            continue
        ins = decoded(segments, address)
        price = clocks_of(ins, count, taken.get(address, 0)) if ins is not None else None
        if price is None:
            unpriced += count
            continue
        clocks += price
        instructions += count
    return clocks, instructions, unpriced


def callgrind_counts(exe, args=()):
    """({address: executions}, {address: conditional branches taken}) of `exe` running `args`, from callgrind's per-instruction dump."""
    out = tempfile.mktemp(suffix=".cg")
    subprocess.run(
        ["valgrind", "--tool=callgrind", "--dump-instr=yes", "--collect-jumps=yes", f"--callgrind-out-file={out}", str(exe), *args],
        capture_output=True, check=True,
    )
    counts, taken = collections.Counter(), collections.Counter()
    last, skip = None, False
    for line in open(out):
        if line.startswith("calls="):
            skip = True  # the next line is the call's inclusive cost, not an instruction's
            continue
        if line.startswith("jcnd="):
            if last is not None:
                taken[last] += int(line[5:].split()[0].split("/")[0])
            continue
        if line.startswith("jump=") or line[0] not in "0+-*":
            continue
        fields = line.split()
        head = fields[0]
        if head.startswith("0x"):
            address = last = int(head, 16)
        elif head.startswith("+"):
            address = last = last + int(head[1:])
        elif head.startswith("-"):
            address = last = last - int(head[1:])
        else:
            address = last
        if skip:
            skip = False
            continue
        counts[address] += int(fields[-1])
    os.remove(out)
    return counts, taken


def model_clocks(exe, args=(), region=None):
    """(clocks, instructions, unpriced) of `exe` run on the host, priced as the 486 would."""
    counts, taken = callgrind_counts(exe, args)
    return priced(harness.elf_segments(exe), counts, taken, region)


if __name__ == "__main__":
    clocks, instructions, unpriced = model_clocks(Path(sys.argv[1]), sys.argv[2:])
    print(f"model clocks (486; ignores {IGNORES}): {clocks} over {instructions} instructions, {unpriced} unpriced")

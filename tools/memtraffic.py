"""
What BC leaves on the table: a load whose value something already has.

BC keeps almost everything in memory. A named variable is stored, then read
back a few instructions later; an array element is loaded twice with nothing
in between that could have changed it. Neither is a bug -- BC targets an 8086
and does not allocate registers -- and both are exactly what a real optimizer
removes. This counts them, so the size of that prize is a measurement rather
than an impression, and so the number moves visibly when the optimizer starts
taking it.

    uv run python tools/memtraffic.py fixtures/omf build/bench
    uv run python tools/memtraffic.py --frames-disjoint fixtures/omf

Two shapes are counted separately, because they need different machinery to
collect: a load of an address something *stored* earlier (store-to-load
forwarding -- the store may also turn out to be dead) and a load of an
address something already *loaded* (a redundant reload, where the value is
simply still sitting in a register). Both are per basic block, the same
scoping stack.py and registers.py already use and for the same reason: a
value's position is only knowable where nothing else could have run instead
of what did.

`--frames-disjoint` reports what one specific assumption would buy. BC's own
SS==DS means module.may_alias() cannot prove a frame slot and a DGROUP
segment address are different bytes, so every frame store conservatively
kills every tracked static. That is provable from a link map rather than from
the object -- LINK places the STACK class above every data segment -- but a
link map is not what this pass reads, so the assumption stays off by default
and this flag exists to price it.

Measured, it is worth nothing: 391/55 either way across all 112 objects. A
frame store does kill a tracked DGROUP static 9 times corpus-wide, but not
one of those statics is read again before something rewrites it, so no
opportunity is lost by refusing the assumption. The flag stays because that
is a fact about this corpus rather than a proof, and the next program to be
measured may not agree -- but nothing should be built on the link-map rule
until this number says it would pay.
"""

import sys
import argparse
from pathlib import Path
from dataclasses import dataclass

from iced_x86 import MemorySizeExt

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qbopt import omf
from qbopt import loops
from qbopt import blocks
from qbopt import module
from qbopt.declen import INFO
from qbopt.declen import Insn
from qbopt.module import Addr
from qbopt.blocks import Block
from qbopt.declen import READS
from qbopt.lift import operand
from qbopt.module import Space
from qbopt.declen import WRITES
from qbopt.lift import Resolver
from qbopt.flags import CLOBBERS

ROOT = Path(__file__).resolve().parents[1]

# The runtime routines whose own bodies are known not to touch caller memory
# -- the same four, from the same QuickBASIC 4.5 runtime source
# (runtime/rt/helpi4.asm), that stack.py already trusts to step over a nested
# call. Every other call is a barrier: it may read or write anything.
MEMORY_CLEAN = frozenset({"B$MUI4", "B$DVI4", "B$RMI4", "B$CPI4"})


@dataclass(frozen=True, slots=True)
class Access:
    """One instruction's own explicit memory operand."""

    addr: Addr
    width: int
    reads: bool
    writes: bool


def access_of(insn: Insn, resolve: Resolver) -> Access | None:
    """The memory this instruction names, or None if it names none this can read.

    `operand()` is lift.py's own resolution, reused rather than re-derived --
    it already refuses a segment override and a Space.GROUP address, and
    already carries an array element's index register along so two elements
    at one displacement stay distinct.
    """
    used = INFO.info(insn.insn).used_memory()
    if not used:
        return None
    addr = operand(insn, resolve)
    if addr is None:
        return None
    one = used[0]
    return Access(
        addr,
        MemorySizeExt.size(one.memory_size),
        one.access in READS,
        one.access in WRITES,
    )


def touches_memory(insn: Insn) -> bool:
    return bool(INFO.info(insn.insn).used_memory())


def counted(
    block: Block,
    resolve: Resolver,
    calls: dict[int, str],
    dgroup: frozenset[int],
    frames_disjoint: bool,
) -> tuple[int, int]:
    """(store-to-load, load-to-load) opportunities in one block.

    Tracked one byte at a time, not one access at a time, because the shape
    that dominates this corpus does not line up any other way: BC has no
    dword store, so it puts a LONG back as two word stores (`mov [x],ax`
    then `mov [x+2],dx`) and reads it again as a single dword push under
    /G3. Matching whole accesses by width sees two 2-byte stores and one
    4-byte load and calls them unrelated; matching bytes sees the load fully
    covered, which is what it is.
    """

    def aliases(one: Addr, access: Access) -> bool:
        if frames_disjoint and {one.space, access.addr.space} == {Space.FRAME, Space.SEGMENT}:
            return False
        return module.may_alias(one, access.addr, dgroup, 1, access.width)

    # every byte whose value something already has, and whether a store put
    # it there rather than a load
    held: dict[Addr, bool] = {}
    store_load = load_load = 0

    for insn in block.insns:
        if (name := calls.get(insn.at)) is not None:
            if name not in MEMORY_CLEAN:
                held = {}  # anything else may read or write whatever it likes
            continue
        if insn.flow in CLOBBERS:
            held = {}
            continue

        access = access_of(insn, resolve)
        if access is None:
            if touches_memory(insn):
                held = {}  # memory this cannot name is memory it cannot track
            continue

        wanted = [access.addr.plus(step) for step in range(access.width)]

        if access.reads and all(byte in held for byte in wanted):
            # a store anywhere under it makes the whole load a forward; only
            # a load-covered load is a plain reload
            if any(held[byte] for byte in wanted):
                store_load += 1
            else:
                load_load += 1

        if access.writes:
            held = {one: stored for one, stored in held.items() if not aliases(one, access)}
            for byte in wanted:
                held[byte] = True
        elif access.reads:
            for byte in wanted:
                held.setdefault(byte, False)

    return store_load, load_load


def measure(path: Path, frames_disjoint: bool) -> dict[int, tuple[int, int]] | None:
    """(store-to-load, load-to-load) per loop nesting depth, or None if unreadable.

    Split by depth because the totals alone mislead: an opportunity in
    straight-line setup code is worth taking once, and the same one inside
    two loops is worth taking every trip. loops.py's own caveat applies --
    the depth is within a body, so this ranks a kernel against its own
    neighbours and not against another procedure's.
    """
    found = module.of(omf.parse(path.read_bytes()))
    if found is None:
        return None
    mapped = blocks.code_map(found)
    if isinstance(mapped, str):
        return None
    partitioned = blocks.partition(found, mapped)
    nesting = loops.depth(partitioned)
    by_depth: dict[int, tuple[int, int]] = {}
    for block in partitioned:
        one, two = counted(block, found.resolve, found.calls, found.dgroup, frames_disjoint)
        was = by_depth.get(nesting.get(block.at, 0), (0, 0))
        by_depth[nesting.get(block.at, 0)] = (was[0] + one, was[1] + two)
    return by_depth


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="memtraffic")
    ap.add_argument("where", type=Path, nargs="+")
    ap.add_argument(
        "--frames-disjoint",
        action="store_true",
        help="assume a frame slot never aliases a static -- see the module docstring",
    )
    ap.add_argument("--per-object", action="store_true", help="one line per object, not just the total")
    args = ap.parse_args(argv)

    paths = [
        p for w in args.where for p in ([w] if w.is_file() else sorted(w.rglob("*.obj")) + sorted(w.rglob("*.OBJ")))
    ]

    totals: dict[int, tuple[int, int]] = {}
    skipped = 0
    for path in paths:
        found = measure(path, args.frames_disjoint)
        if found is None:
            skipped += 1
            continue
        for at_depth, (one, two) in found.items():
            was = totals.get(at_depth, (0, 0))
            totals[at_depth] = (was[0] + one, was[1] + two)
        if args.per_object and any(any(v) for v in found.values()):
            one = sum(v[0] for v in found.values())
            two = sum(v[1] for v in found.values())
            print(f"  {path.name:<28} store->load {one:>4}  load->load {two:>4}")

    scope = "frames disjoint from statics" if args.frames_disjoint else "SS==DS, conservative"
    print(f"\n{len(paths) - skipped} objects ({scope})")
    print(f"  {'loop depth':<12} {'store->load':>12} {'load->load':>12}")
    for at_depth in sorted(totals):
        one, two = totals[at_depth]
        print(f"  {at_depth:<12} {one:>12} {two:>12}")
    print(f"  {'total':<12} {sum(v[0] for v in totals.values()):>12} {sum(v[1] for v in totals.values()):>12}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

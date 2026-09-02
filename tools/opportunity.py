"""
What BC leaves on the table, counted rather than assumed.

This exists because the roadmap said for months that the classic passes
measure empty on BC's output, and that was the metric rather than BC. Every
measurement behind it asked a question shaped so that BC's own style could
not answer it: CSE over SSA values, when BC never recomputes and always
reloads; a redundant load defined as one into the register that already
holds the cell, when BC reloads into a different one.

So the questions here are deliberately crude and about *cells*, not values:
what is loaded that was just written, what is loaded twice, what is stored
twice with nothing reading it in between. A crude count that is right is
worth more than a precise one that is measuring the wrong thing.

Block-scoped, and that is a floor rather than an answer: BC's loop counter
round-trips through memory every iteration and the reload arrives over the
back-edge, which nothing here sees.
"""

import sys
import argparse
from pathlib import Path
from collections import Counter

import iced_x86

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from qbopt import mir
from qbopt import loops as loopy
from qbopt import omf
from qbopt import module
from qbopt import blocks as split
from qbopt.module import Space
from qbopt.blocks import code_map


def named(ref) -> bool:
    """A cell this can name. A stack slot is a depth from the top of its own
    block, and an address of None aliases everything."""
    return ref.addr is not None and ref.addr.space is not Space.STACK


def _through(block, module_, entering: dict[str, int], seen: set[str], found: Counter | None):
    """One block, forward, from what its predecessors agreed on.

    Returns what is known on the way out. `found` is None on the rounds that
    are only settling the fixed point -- counting them would count the same
    site once per round.
    """
    written = dict(entering)
    read = set(seen)
    for op in block.ops:
        # A call may write any cell, and a barrier's addresses are its own.
        if op.barrier or op.at in module_.calls:
            written.clear()
            read.clear()
            continue
        for ref in (one for one in op.loads if named(one)):
            cell = str(ref.addr)
            if found is not None:
                if cell in written:
                    found["load of a cell just written"] += 1
                elif cell in read:
                    found["load of a cell already loaded"] += 1
            read.add(cell)
        for ref in (one for one in op.stores if named(one)):
            cell = str(ref.addr)
            if found is not None and cell in written:
                found["store over a store nothing read"] += 1
            written[cell] = op.at
            read.discard(cell)
    return written, read


# What a 16-bit body has to hand: bp is the frame pointer, sp the stack, and
# the segment registers are not general. Six, and BC uses two.
GENERAL = ("AX", "BX", "CX", "DX", "SI", "DI")

# iced's Register is a bag of int constants rather than an enum, so the name
# of one has to be looked back up.
NAMED = {
    getattr(iced_x86.Register, one): one
    for one in dir(iced_x86.Register)
    if not one.startswith("_") and isinstance(getattr(iced_x86.Register, one), int)
}


# What one operation costs, in 386 cycles, near enough to rank by. A memory
# operand is counted on top of the instruction, which is the whole point:
# BC's per-statement code pays for one on almost every line.
CYCLES = {"imul": 22, "idiv": 43, "mul": 22, "div": 43, "shl": 3, "shr": 3, "sar": 3}
TOUCH = 4  # a memory operand, cached


def _cost(body, module_, found: Counter) -> None:
    """One number to minimise: cycles, weighted by how often a loop runs.

    Ten per level of nesting, which is a stand-in for a trip count nothing
    here knows. The point is not the absolute figure -- it is that the same
    program compiled two ways can be ranked, and that an `idiv` in an inner
    loop outranks a hundred straight-line moves, which is what BC's output
    actually costs.
    """
    depth = loopy.depth(list(body.blocks), body.entry)
    for block in body.blocks:
        weight = 10 ** min(depth.get(block.at, 0), 3)
        for op in block.ops:
            if op.at in module_.calls:
                found["cost"] += 20 * weight
                continue
            name = (op.name or "").lower()
            cycles = CYCLES.get(name, 2)
            cycles += TOUCH * len([one for one in (*op.loads, *op.stores) if named(one)])
            found["cost"] += cycles * weight


def _registers(body, module_, found: Counter) -> None:
    """How many variables a loop touches, against how many registers it uses.

    The question the roadmap never asked, and the one that says plainly
    there is no allocation here: BC emits a statement at a time, so a value
    lives in a register only for as long as one statement needs it. Every
    variable is re-read from memory in the next statement even when the loop
    touches six of them and the machine has six registers free.
    """
    inside = loopy.loops(list(body.blocks), body.entry)
    at_of = {block.at: block for block in body.blocks}
    for loop in inside:
        cells: set[str] = set()
        registers: set[str] = set()
        traffic = 0
        for at in loop.body:
            for op in at_of[at].ops:
                if op.at in module_.calls:
                    continue
                for ref in (one for one in (*op.loads, *op.stores) if named(one)):
                    cells.add(str(ref.addr))
                    traffic += 1
                for value in (*op.defines, *op.uses):
                    got = body.origin.get(value)
                    if got is None:
                        continue
                    name = NAMED.get(got, "").upper().removeprefix("E")
                    if name in GENERAL:
                        registers.add(name)
        if not cells:
            continue
        found["loops seen"] += 1
        found[f"  a loop touching {len(cells)} variables in {len(registers)} registers"] += 1
        if len(cells) <= len(GENERAL):
            found["  every variable in the loop would fit in registers"] += 1
            found["  memory accesses that would become none"] += traffic


def _invariant(body, module_, found: Counter) -> None:
    """Reads inside a loop of a cell the loop never writes.

    The LICM question, and the one a memory-redundancy count cannot ask: BC
    reloads a variable every iteration whether or not the loop touches it.
    `hotlop.bas` is the shape -- `mov ax,[n] / imul word [k]` runs on every
    pass through a loop that writes neither, and both are constants besides.

    Counted per site rather than per iteration, so it says how much code
    could move rather than how much time it costs.
    """
    inside = loopy.loops(list(body.blocks), body.entry)
    if not inside:
        return
    at_of = {block.at: block for block in body.blocks}
    for loop in inside:
        written: set[str] = set()
        opaque = False
        for at in loop.body:
            for op in at_of[at].ops:
                if op.barrier or op.at in module_.calls:
                    opaque = True
                written.update(str(one.addr) for one in op.stores if named(one))
        if opaque:
            continue  # a call in the loop may write anything
        for at in loop.body:
            for op in at_of[at].ops:
                for ref in (one for one in op.loads if named(one)):
                    if str(ref.addr) not in written:
                        found["read inside a loop of a cell the loop never writes"] += 1


def counted(paths: list[Path]) -> Counter:
    """Cross-block, because BC's redundancy is loop-carried.

    Block-scoped was the floor and it is the wrong floor: the loop counter
    round-trips through memory every iteration and the reload arrives over
    the back-edge, which a per-block walk cannot see at all.

    A forward fixed point, intersected at a join -- a cell counts as written
    only where *every* path into the block wrote it and none read it since.
    Starting from nothing and growing is the conservative direction: a cycle
    cannot talk itself into a fact, so this under-counts rather than over-.
    """
    found: Counter = Counter()
    for path in paths:
        module_ = module.of(omf.parse(path.read_bytes()))
        if module_ is None:
            continue
        mapped = code_map(module_)
        if isinstance(mapped, str):
            continue
        for _name, body in mir.bodies(module_, split.partition(module_, mapped)):
            blocks = {block.at: block for block in body.blocks}
            preds: dict[int, list[int]] = {at: [] for at in blocks}
            for block in body.blocks:
                for successor in block.succ:
                    if successor in preds:
                        preds[successor].append(block.at)
            exits: dict[int, tuple[dict[str, int], set[str]]] = {at: ({}, set()) for at in blocks}

            for _round in range(len(blocks) + 2):
                changing = False
                for at in sorted(blocks):
                    entering: dict[str, int] | None = None
                    seen: set[str] | None = None
                    for previous in preds[at]:
                        was, had = exits[previous]
                        if entering is None:
                            entering, seen = dict(was), set(had)
                        else:
                            entering = {c: v for c, v in entering.items() if c in was}
                            seen &= had
                    got = _through(blocks[at], module_, entering or {}, seen or set(), None)
                    if got != exits[at]:
                        exits[at] = got
                        changing = True
                if not changing:
                    break

            _invariant(body, module_, found)
            _registers(body, module_, found)
            _cost(body, module_, found)

            for at in sorted(blocks):
                entering, seen = None, None
                for previous in preds[at]:
                    was, had = exits[previous]
                    if entering is None:
                        entering, seen = dict(was), set(had)
                    else:
                        entering = {c: v for c, v in entering.items() if c in was}
                        seen &= had
                _through(blocks[at], module_, entering or {}, seen or set(), found)
    return found


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="opportunity")
    ap.add_argument("objects", nargs="*", type=Path)
    args = ap.parse_args(argv)
    paths = args.objects or sorted(Path("fixtures/omf").glob("*.obj"))
    found = counted(paths)
    print(f"  {found.pop('cost', 0):8d}  COST -- weighted cycles, the number to minimise")
    for name, count in sorted(found.items(), key=lambda one: -one[1]):
        print(f"  {count:8d}  {name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

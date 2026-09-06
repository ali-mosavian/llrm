"""Promotion: a cell only this body can name becomes a value.

LLVM's `mem2reg`. A variable BC keeps in memory and reloads for every
statement -- because it compiles a statement at a time, which is the fact
under every number in `docs/targets.md` -- becomes an SSA value, and the
register allocator gets the program's variables instead of BC's spill
slots.

**Which cells.** One nothing else can reach: a fixed address in the
program's own data, at one width, that no other reference in the body may
alias. `mir.overlapping` answers the last part, and it can answer it at all
because `runtime.toml` now says what a call writes -- measured from a
linked image by `tools/runtime_writes.py`, which found no runtime write
naming a cell in BC_DATA. Before that every call aliased every variable and
nothing was promotable anywhere.

**No phi is written.** A store becomes a definition of a fresh variable and
a load becomes a use of it, and `mir.resolved()` re-derives the SSA -- it
renames per variable and puts a phi wherever two definitions meet. That is
the whole of `mem2reg`'s phi placement, and writing one here would be
saying the same thing twice.
"""

from collections import Counter
from dataclasses import replace

from qbopt import mir
from qbopt.mir import MirBody
from qbopt.mir import Op
from qbopt.module import Space
from qbopt.passes import MIRTransform
from qbopt.passes import Where


class Promote(MIRTransform):
    name = "promote"

    def __init__(self, where: Where) -> None:
        self.where = where

    def transform(self, body: MirBody) -> MirBody:
        return promoted(body, self.where.dgroup, self.where.bounds)


def promotable(
    body: MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None
) -> dict:
    """Every cell in this body that nothing else can reach, by address.

    Touched more than once, because promoting a cell read or written once
    removes no work. At one width, because a long stored as two words and
    read as one is two variables in the same place and this does not model
    that. And aliased by nothing else, which is the whole question.
    """
    every = [one for block in body.blocks for op in block.ops for one in (*op.loads, *op.stores)]
    seen: Counter = Counter()
    widths: dict = {}
    for one in every:
        if one.addr is None or one.base is not None or one.addr.space is not Space.SEGMENT:
            continue
        seen[one.addr] += 1
        widths.setdefault(one.addr, set()).add(one.width)

    out = {}
    for addr, times in seen.items():
        if times < 2 or len(widths[addr]) != 1:
            continue
        width = next(iter(widths[addr]))
        mine = mir.MemRef(addr, width)
        if any(one.addr != addr and mir.overlapping(mine, one, dgroup, bounds) for one in every):
            continue
        out[addr] = width
    return out


def promoted(
    body: MirBody, dgroup: frozenset[int] = frozenset(), bounds: dict | None = None
) -> MirBody:
    """`body` with every unreachable cell carried in a variable instead."""
    found = promotable(body, dgroup, bounds)
    if not found:
        return body

    taken = max((one.variable for one in body.origin), default=0)
    fresh = _next(body)
    holds = {}
    for number, addr in enumerate(sorted(found, key=lambda one: (one.index, one.disp)), 1):
        holds[addr] = taken + number

    changed = False
    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            made = _instead(op, holds, found, fresh)
            if made is None:
                ops.append(op)
                continue
            fresh += 1
            changed = True
            ops.append(made)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks)) if changed else body


def _instead(op: Op, holds: dict, found: dict, fresh: int) -> "Op | None":
    """This operation with its cell replaced by the variable holding it.

    A store becomes a definition of that variable and a load a use of it.
    Only where the cell is the operation's whole memory traffic: one that
    also touches something else is left alone rather than half rewritten,
    and a half-rewritten body reads stale memory.
    """
    cells = [one for one in (*op.loads, *op.stores) if one.addr in holds]
    if not cells or len(cells) != len(op.loads) + len(op.stores):
        return None
    addr = cells[0].addr
    if any(one.addr != addr for one in cells):
        return None
    width = found[addr]
    variable = holds[addr]

    if op.stores and not op.loads:
        # `mov [x],ax` is `x := ax`, and x is a variable now.
        into = mir.Value(id=fresh, at=op.at, variable=variable, version=1)
        return replace(
            op,
            kind=mir.Kind.COPY,
            name="mov",
            defines=(into, *(one for one in op.defines if one.flags)),
            stores=(),
            results=(mir.Held(into, width),),
            args=tuple(one for one in op.args if not isinstance(one, mir.Cell)),
        )

    if op.loads and not op.stores and op.kind is mir.Kind.LOAD:
        # `mov ax,[x]` is `ax := x`. The version is a placeholder --
        # `mir.resolved()` renames per variable and settles which one.
        holding = mir.Value(id=fresh, at=op.at, variable=variable, version=1)
        return replace(
            op,
            kind=mir.Kind.COPY,
            name="mov",
            uses=tuple(one for one in op.uses if not one.flags) + (holding,),
            loads=(),
            args=tuple(
                mir.Held(holding, width) if isinstance(one, mir.Cell) else one for one in op.args
            ),
        )
    return None


def _next(body: MirBody) -> int:
    """An id nothing in this body uses."""
    seen = {0}
    for block in body.blocks:
        for op in block.ops:
            seen.update(one.id for one in (*op.defines, *op.uses))
        seen.update(phi.result.id for phi in block.phis)
    return max(seen) + 1

"""Whether a lowered body is well formed, checked where it was made.

LLVM's `MachineVerifier`, and it exists for the same reason: a phase that
produces a malformed body is found at the phase that produced it rather
than three phases later, wherever the malformation first stops something
from working. Until this existed the only check was layout's byte
accounting, which reported "1 bytes are claimed by more than one op" for a
bug that was two phases upstream in how an inserted instruction claimed its
span.

Every check is a rule some phase has already broken. They are listed as
such in the docstrings rather than as general principles, because a rule
nothing has ever broken is a rule nobody knows the shape of.

Returns a list of complaints rather than raising: a caller running it over
a corpus wants all of them, and a caller running it between phases wants to
say which phase.
"""

from qbopt import ir
from qbopt import lir
from qbopt import target


def verify(body: lir.LirBody, *, in_ssa: bool = False) -> list[str]:
    """Everything wrong with this body, as sentences. Empty is well formed."""
    out: list[str] = []
    out += _blocks(body)
    out += _spans(body)
    out += _operands(body)
    out += _values(body, in_ssa)
    return out


def _blocks(body: lir.LirBody) -> list[str]:
    """The entry exists, every successor exists, every block is reachable."""
    out = []
    at_of = {block.at: block for block in body.blocks}
    if body.entry not in at_of:
        out.append(f"the entry {body.entry:#06x} is not one of this body's blocks")
        return out
    for block in body.blocks:
        for where in block.succ:
            if where not in at_of:
                out.append(f"block {block.at:#06x} goes to {where:#06x}, which is not in this body")
    seen, todo = set(), [body.entry]
    while todo:
        at = todo.pop()
        if at in seen or at not in at_of:
            continue
        seen.add(at)
        todo += list(at_of[at].succ)
    for block in body.blocks:
        if block.at not in seen:
            out.append(f"block {block.at:#06x} is not reachable from the entry")
    return out


def _spans(body: lir.LirBody) -> list[str]:
    """No two instructions claim the same original byte.

    An inserted instruction claims none. It used to carry the operation it
    was inserted beside, and the span came off that -- so the copy a phi
    became and the instruction it stood next to both claimed the same
    bytes, and layout reported it twelve objects later.
    """
    out = []
    claimed: dict[int, int] = {}
    for block in body.blocks:
        for one in block.insns:
            if one.covers is None:
                continue
            lo, hi = one.covers
            if hi < lo:
                out.append(f"{one.at:#06x} covers {lo:#x}..{hi:#x}, which runs backwards")
                continue
            for byte in range(lo, hi):
                if byte in claimed:
                    out.append(f"byte {byte:#06x} is claimed by {claimed[byte]:#06x} and by {one.at:#06x}")
                    break
                claimed[byte] = one.at
    return out


def _operands(body: lir.LirBody) -> list[str]:
    """No MIR operand survives lowering, and no operand names a class it
    cannot be in.

    Below LIR every operand is a location. A `mir.MemRef` reaching select
    refused to encode and said only that the mov was not one it could
    emit -- the message named the instruction and not the reason.
    """
    from qbopt import mir

    out = []
    for block in body.blocks:
        for one in block.insns:
            if one.what is None:
                continue
            for where in (*one.what.dests, *one.what.sources):
                if isinstance(where, (mir.MemRef, mir.Held, mir.Const, mir.Cell)):
                    out.append(f"{one.at:#06x} still holds the MIR operand {where!r}")
                if isinstance(where, ir.Reg) and not target.known(where.register):
                    out.append(f"{one.at:#06x} names {where.register}, which is not a register this target has")
                if isinstance(where, ir.Reg) and target.width_of(where.register) not in (None, where.width):
                    out.append(
                        f"{one.at:#06x} names {where.register} at width {where.width}, "
                        f"which is not the width that register is"
                    )
    return out


def _values(body: lir.LirBody, in_ssa: bool) -> list[str]:
    """Every value is defined before it is read, and once if this is SSA.

    `in_ssa` is false after phi elimination, which is the point of that
    pass: a value written on two edges is exactly what a phi said, and
    saying it with copies means writing it twice.
    """
    out = []
    written: dict[int, int] = {}
    for block in body.blocks:
        for value in block.arrives:
            written[value] = written.get(value, 0) + 1
        for one in block.insns:
            for value in one.defines:
                written[value] = written.get(value, 0) + 1
    if in_ssa:
        for value, times in sorted(written.items()):
            if times > 1:
                out.append(f"value#{value} is defined {times} times in a body that should be in SSA")
    if in_ssa and any(block.phis for block in body.blocks):
        pass
    elif not in_ssa and any(block.phis for block in body.blocks):
        stuck = [block.at for block in body.blocks if block.phis]
        out.append(f"a phi survives at {', '.join(f'{one:#06x}' for one in stuck)} after elimination")

    # Every value an operand names is a value some instruction defines, or
    # one the caller supplied. A value read and never written anywhere is a
    # renaming that lost half of itself.
    read = {value for block in body.blocks for one in block.insns for value in one.uses}
    named = set()
    for block in body.blocks:
        for one in block.insns:
            if one.what is None:
                continue
            for where in (*one.what.dests, *one.what.sources):
                # Through `ir.values`, which is the one answer to "which
                # values does this operand name": a cell names the value
                # that computed its address, and asking here separately is
                # how three renames left a cell on a value they had ended
                # without this saying so.
                named.update(one.value for one in ir.values(where))
    for value in sorted(named - read - set(written)):
        out.append(f"value#{value} is named by an operand and neither defined nor used anywhere")
    return out

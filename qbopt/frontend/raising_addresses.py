"""Expose loaded address-space identities and their memory dependencies."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.model import mir
from qbopt.objectfile.module import Space


def loaded(body: mir.MirBody, contracts: "dict | None" = None) -> mir.MirBody:
    """Give the segment register a value, so that reloading it is redundant.

    One mechanism, not two. This used to run a block-local version -- a fresh
    variable per load, dropped at every block boundary and every write -- and
    reach for real SSA only where an `les` had already forced the question.
    Two loads of one descriptor could then never be the same value, which is
    the whole of what makes a reload removable, so a separate pass removed
    them against the machine instead.
    """
    from qbopt.frontend import raising_address_state

    return _allocated(raising_address_state.raised(body, _selector, contracts))


def _allocated(body: mir.MirBody) -> mir.MirBody:
    """Attach a FAR access to the descriptor its selector was loaded from.

    Bounds and object identity are separate facts.  An unchecked subscript
    may be outside the allocation, but loading ES from descriptor D still
    puts the access in D's heap segment; it cannot thereby become a write to
    the caller's stack frame or to another array's descriptor.  The earlier
    array raise already identified allocation requests, and this is the
    first point where selector loads and their SSA values both exist.
    """
    selectors = {
        (
            request.descriptor.space,
            request.descriptor.index,
            request.descriptor.offset + request.descriptor.addend + 2,
        ): request.descriptor
        for block in body.blocks
        for op in block.ops
        if (request := op.array) is not None
    }
    if not selectors:
        return body

    owners: dict[mir.Value, mir.Symbol] = {}
    for block in body.blocks:
        for op in block.ops:
            if op.kind is not mir.Kind.LOAD or len(op.loads) != 1:
                continue
            ref = mir._symbolic_ref(op.loads[0])
            if ref.addr is None or ref.base is not None or ref.segment is not None:
                continue
            owner = selectors.get((ref.addr.space, ref.addr.index, ref.addr.disp))
            if owner is None:
                continue
            owners.update(
                (result.value, owner) for result in op.results if isinstance(result, mir.Held) and result.width == 2
            )
    if not owners:
        return body

    def allocated(ref: mir.MemRef) -> mir.MemRef:
        owner = owners.get(ref.segment)
        return (
            replace(ref, allocation=owner)
            if owner is not None and ref.addr is not None and ref.addr.space is Space.FAR
            else ref
        )

    def argument(arg: mir.Arg) -> mir.Arg:
        return mir.Cell(allocated(arg.ref)) if isinstance(arg, mir.Cell) else arg

    return replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(
                    replace(
                        op,
                        loads=tuple(map(allocated, op.loads)),
                        stores=tuple(map(allocated, op.stores)),
                        args=tuple(map(argument, op.args)),
                        results=tuple(map(argument, op.results)),
                    )
                    for op in block.ops
                ),
            )
            for block in body.blocks
        ),
    )


def _selector(op: mir.Op) -> bool:
    if op.kind is not mir.Kind.LOAD or op.barrier or op.defines or op.stores:
        return False
    match op.args, op.results:
        case (mir.Cell(ref=ref),), (mir.Opaque(name="es"),):
            return ref.width == 2 and op.loads == (ref,) and ref.addr is not None and ref.addr.segment != Register.ES
    return False

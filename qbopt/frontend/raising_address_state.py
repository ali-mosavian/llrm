from dataclasses import replace
from collections.abc import Callable

from iced_x86 import Register

from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.analysis import ssa


def raised(
    body: mir.MirBody, selector: Callable[[mir.Op], bool], contracts: "dict[int, runtime.Contract] | None" = None
) -> mir.MirBody:
    if not _selects(body, selector):
        return body
    values = tuple(ssa.values(body))
    serial = max((value.id for value in values), default=0) + 1
    variable = max((value.variable for value in values), default=0) + 1
    incoming = mir.Value(serial, body.entry, variable=variable, version=0)
    blocks = []
    for block in body.blocks:
        current = incoming
        operations = []
        for op in block.ops:

            def reference(ref: mir.MemRef, state: mir.Value = current) -> mir.MemRef:
                if ref.addr is not None and ref.addr.segment == Register.ES:
                    return replace(ref, segment=state)
                return ref

            def argument(arg: mir.Arg) -> mir.Arg:
                return mir.Cell(reference(arg.ref)) if isinstance(arg, mir.Cell) else arg

            loads, stores = tuple(map(reference, op.loads)), tuple(map(reference, op.stores))
            effect = getattr(getattr(op, "node", None), "effects", None)
            contract = (contracts or {}).get(op.at) if op.kind is mir.Kind.CALL else None
            if contract is not None and contract.established:
                # A call says what it touches where anything does. Without
                # asking, every call minted a selector nothing wrote, and a
                # routine established to leave ES alone still ended its
                # caller's descriptor.
                reads = contract.inputs is None or runtime.Reg.ES in contract.inputs
                writes = runtime.Reg.ES in contract.clobbers
            else:
                unknown = op.barrier or op.kind is mir.Kind.CALL
                reads = unknown if effect is None else effect.uses is None or Register.ES in effect.uses
                writes = unknown if effect is None else effect.defs is None or Register.ES in effect.defs
            reads |= any(ref.segment == current for ref in (*loads, *stores))
            selected = selector(op)
            op = replace(
                op,
                loads=loads,
                stores=stores,
                args=tuple(map(argument, op.args)),
                results=tuple(map(argument, op.results)),
                uses=tuple(dict.fromkeys((*op.uses, current))) if reads else op.uses,
            )
            if selected or writes:
                serial += 1
                current = mir.Value(serial, op.at, variable=variable, version=1)
                op = replace(
                    op,
                    defines=(*op.defines, current),
                    results=tuple(
                        mir.Held(current, 2) if isinstance(result, mir.Opaque) and result.name == "es" else result
                        for result in op.results
                    ),
                )
                if selected:
                    op = replace(op, results=(mir.Held(current, 2),), merges={})
            operations.append(op)
        blocks.append(replace(block, ops=tuple(operations)))
    built = _undefined(ssa.constructed(replace(body, blocks=tuple(blocks)), frozenset({variable})), variable)
    result = ssa.renumbered(built, variable)
    origin = {**body.origin, **{value: Register.ES for value in ssa.values(result) if value.variable == variable}}
    return replace(result, origin=origin)


def _selects(body: mir.MirBody, selector: Callable[[mir.Op], bool]) -> bool:
    """Whether anything here goes through a selector at all.

    A body with no far access has none to name, and minting one anyway makes
    every call define a value nothing reads -- which reads, correctly, as the
    call disturbing one more thing than it does.
    """
    return any(
        selector(op) or any(ref.addr is not None and ref.addr.segment == Register.ES for ref in (*op.loads, *op.stores))
        for block in body.blocks
        for op in block.ops
    )


def _undefined(body: mir.MirBody, variable: int) -> mir.MirBody:
    """Forget the selector where no definition of it reaches.

    Construction starts every block from one placeholder standing for "the
    reaching definition here", and resolves it. Where no definition reaches
    -- a body entered with ES already loaded, a resume entry -- what is left
    names no definition: the reference has no selector, exactly as it had
    none before there was a value at all. Left in, it is a value the
    allocator must find a register for and the spiller a slot, though no
    instruction anywhere writes it.

    Asked of the body rather than of a version number, because construction
    numbers a value it did not see defined the same as one it did.
    """
    defined = {
        value
        for block in body.blocks
        for value in (*(phi.result for phi in block.phis), *(one for op in block.ops for one in op.defines))
    }

    def _entry(value) -> bool:
        return isinstance(value, mir.Value) and value.variable == variable and value not in defined

    def forget(ref: mir.MemRef) -> mir.MemRef:
        return replace(ref, segment=None) if _entry(ref.segment) else ref

    def argument(arg: mir.Arg) -> mir.Arg:
        return mir.Cell(forget(arg.ref)) if isinstance(arg, mir.Cell) else arg

    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            ops.append(
                replace(
                    op,
                    loads=tuple(map(forget, op.loads)),
                    stores=tuple(map(forget, op.stores)),
                    args=tuple(map(argument, op.args)),
                    results=tuple(map(argument, op.results)),
                    uses=tuple(one for one in op.uses if not _entry(one)),
                )
            )
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))

from typing import TYPE_CHECKING

from qbopt.abi import runtime

if TYPE_CHECKING:
    from qbopt.model import mir

type Reach = tuple[int, frozenset[tuple[int, int]]]

# Where a routine leaves control is a different question from what it can
# write, and the bound returned here answers only the second. INLINE_TABLE
# resumes at one of the table's own targets, which the raise already models
# as the dispatch block's successors -- so the data bound holds across it
# exactly as it holds across a return.
#
# Excluding it cost the `jumps` target: B$OGTA writes at the same Memory
# level as every B$P* output routine beside it in that loop, and those are
# bounded. Unbounded, its store aliased every cell in the program once per
# iteration, so nothing was promotable, so `induction.basics` saw no
# counter, so the trip count was unknown and neither expansion would run.
_BOUNDED_CONTROL = (runtime.Control.RETURNS, runtime.Control.NEVER, runtime.Control.INLINE_TABLE)


def reachable(
    contract: runtime.Contract | None,
    access: runtime.Memory,
    escaped: Reach | None,
    handles_errors: bool = True,
) -> Reach | None:
    if (
        contract is None
        or escaped is None
        or runtime.barrier(contract)
        or (contract.raises_error and handles_errors)
        or contract.control not in _BOUNDED_CONTROL
        or access is runtime.Memory.ANY
    ):
        return None
    # NONE excludes caller data, not runtime scratch: B$FCMP writes DGROUP.
    return (escaped[0], frozenset()) if access <= runtime.Memory.ARGUMENTS else escaped


def _direct_reach(contract: runtime.Contract | None, access: runtime.Memory, escaped: Reach | None) -> Reach | None:
    """A callee's own footprint, independent of where control goes next.

    `reachable` is the normal-return answer and therefore rejects callbacks
    and error transfers.  An error-handler summary instead stops at the
    transfer back to the interrupted body, so it needs the runtime routine's
    direct footprint without conflating that later user code with the call.
    """
    if contract is None or escaped is None or not contract.established or access is runtime.Memory.ANY:
        return None
    return (escaped[0], frozenset()) if access <= runtime.Memory.ARGUMENTS else escaped


def handler_effects(
    body: "mir.MirBody",
    calls: dict[int, str],
    contracts: dict[int, runtime.Contract],
    escaped: Reach | None,
) -> tuple[tuple["mir.MemRef", ...], tuple["mir.MemRef", ...]] | None:
    """Complete caller-memory effects before an error handler resumes.

    The resumed program is represented by its own MIR and is not part of the
    handler's footprint.  Unknown user calls still refuse the summary.  This
    is a mod/ref summary, not an attempt to inline the handler's control flow.
    """
    from qbopt.analysis import effects
    from qbopt.model import mir
    from qbopt.objectfile.module import Space

    reads, writes = [], []
    for block in body.blocks:
        for op in block.ops:
            if op.kind is mir.Kind.CALL and op.at in calls:
                contract = contracts.get(op.at)
                direct_reads = contract.direct_reads if contract and contract.direct_reads is not None else (
                    contract.reads if contract else runtime.Memory.ANY
                )
                direct_writes = contract.direct_writes if contract and contract.direct_writes is not None else (
                    contract.writes if contract else runtime.Memory.ANY
                )
                read_reach = _direct_reach(contract, direct_reads, escaped)
                write_reach = _direct_reach(contract, direct_writes, escaped)
                if read_reach is None or write_reach is None:
                    return None
                if direct_reads > runtime.Memory.ARGUMENTS:
                    reads.append(mir.MemRef(None, 0, beyond=read_reach))
                if direct_writes > runtime.Memory.ARGUMENTS:
                    writes.append(mir.MemRef(None, 0, beyond=write_reach))
                if contract.error_handling and contract.control is runtime.Control.NEVER:
                    break
                continue
            if effects.unmodeled_read(op) or effects.unmodeled_write(op):
                return None
            reads.extend(ref for ref in op.loads if ref.space is not Space.STACK)
            writes.extend(ref for ref in op.stores if ref.space is not Space.STACK)
    return tuple(reads), tuple(writes)


def with_handler_effects(
    body: "mir.MirBody",
    summary: tuple[tuple["mir.MemRef", ...], tuple["mir.MemRef", ...]] | None,
    calls: dict[int, str],
    contracts: dict[int, runtime.Contract],
    escaped: Reach | None,
) -> "mir.MirBody":
    """Join a precise handler mod/ref summary into each error-capable call."""
    from dataclasses import replace
    from qbopt.model import mir
    from qbopt.objectfile.module import Space

    if summary is None:
        return body
    handler_reads, handler_writes = summary

    def bounded(ref: "mir.MemRef", reach: Reach) -> "mir.MemRef":
        return ref if ref.space is Space.STACK else replace(ref, beyond=reach)

    blocks = []
    for block in body.blocks:
        ops = []
        for op in block.ops:
            contract = contracts.get(op.at) if op.kind is mir.Kind.CALL and op.at in calls else None
            if contract is not None and contract.raises_error and not contract.enters_user_code:
                read_reach = reachable(contract, contract.reads, escaped, handles_errors=False)
                write_reach = reachable(contract, contract.writes, escaped, handles_errors=False)
                if read_reach is not None and write_reach is not None:
                    op = replace(
                        op,
                        loads=tuple(bounded(ref, read_reach) for ref in op.loads) + handler_reads,
                        stores=tuple(bounded(ref, write_reach) for ref in op.stores) + handler_writes,
                        memory_complete=True,
                    )
            ops.append(op)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


# An exclusion covering every displacement of one extern.
_WHOLE_SYMBOL = (-(1 << 15), 1 << 16)


def spared(found, decoded, contracts: dict[int, runtime.Contract]) -> dict[int, tuple]:
    """Per call site, the runtime cells its callee is known not to write.

    `runtime.WRITERS` says which routines write a cell. A call writes it when
    it is to one of them, or to the program's own procedure that stores it or
    calls one, or to anything that can run the program's code, or to what
    nobody can name. A cell whose address this module hands out is written
    by whoever holds the address, and is spared nowhere.

    The answer is an exclusion on the callee's write, so every alias query
    reads it -- memory SSA, availability and constant cells alike.
    """
    from qbopt.model import ir
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    family = module.family(found.records)
    names = omf.externals(found.records)
    cells = {
        index: runtime.WRITERS[(name, family)] for index, name in enumerate(names) if (name, family) in runtime.WRITERS
    }
    if not cells or isinstance(decoded, str):
        return {}

    nodes = [node for body in decoded for node in body.nodes]
    for fixup in omf.fixups(found.records):
        if fixup.target == "external" and fixup.index in cells and fixup.seg != found.seg:
            del cells[fixup.index]
    for node in nodes:
        semantics = getattr(node, "semantics", None)
        operands = (*semantics.dests, *semantics.sources) if semantics is not None else ()
        for one in operands:
            addr = one.address if isinstance(one, ir.Imm) else one.addr if isinstance(one, ir.Address) else None
            if addr is not None and addr.space is Space.EXTERNAL:
                cells.pop(addr.index, None)
    if not cells:
        return {}

    defined = module.defines(found.records, found.seg)
    handles = runtime.handles_errors(contracts.values())
    procedures = {body.body.name: body for body in decoded if body.body.name}

    def calling(at: int, index: int, writes: dict[str, set[int]]) -> bool:
        name = found.calls.get(at)
        if name is None:
            return True
        if name in defined:
            return name not in procedures or index in writes[name]
        if name in cells[index]:
            return True
        contract = contracts.get(at)
        return (
            contract is None
            or contract.enters_user_code
            or contract.error_handling
            or (contract.raises_error and handles)
        )

    def storing(node, index: int) -> bool:
        semantics = getattr(node, "semantics", None)
        effects = getattr(node, "effects", None)
        if (
            semantics is None
            or effects is None
            or (semantics.op is ir.Operation.BARRIER and not effects.memory_complete)
        ):
            return True
        if semantics.op is ir.Operation.PUSH:
            return False
        return any(
            cell.addr is None
            or cell.addr.space in (Space.FAR, Space.LITERAL, Space.GROUP)
            or (cell.addr.space is Space.EXTERNAL and cell.addr.index == index)
            for cell in effects.stores
        )

    writes: dict[str, set[int]] = {name: set() for name in procedures}
    changing = True
    while changing:
        changing = False
        for name, body in procedures.items():
            for node in body.nodes:
                at = ir.span(node)[0]
                for index in cells:
                    if index in writes[name]:
                        continue
                    call = getattr(node, "semantics", None) is not None and node.semantics.op is ir.Operation.CALL
                    if calling(at, index, writes) if call else storing(node, index):
                        writes[name].add(index)
                        changing = True

    out = {}
    for at in found.calls:
        kept = tuple(
            (Addr(Space.EXTERNAL, _WHOLE_SYMBOL[0], index), _WHOLE_SYMBOL[1])
            for index in cells
            if not calling(at, index, writes)
        )
        if kept:
            out[at] = kept
    return out

from dataclasses import replace
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


def indirect_results(body: "mir.MirBody", found) -> "mir.MirBody":
    """Bind a typed BASIC function's indirect result to its actual cell.

    SINGLE and DOUBLE functions receive a final hidden near pointer where the
    callee stores its answer.  The decoded call knew the result width but not
    that pointer, so its anonymous write killed every caller-frame value.
    CodeView establishes the non-INTEGER return class and the ordinary ARG
    contract identifies the exact hidden argument.  Objects without both
    facts retain the conservative call effect.
    """
    from qbopt.model import mir
    from qbopt.objectfile import cvinfo
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    signatures = {
        procedure.name.upper(): cvinfo.type_name(procedure.signature.return_type, procedure.types)
        for procedure in cvinfo.parse(found.records).procedures
        if procedure.signature is not None
    }
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}

    def frame(arg) -> int | None:
        seen = set()
        while isinstance(arg, mir.Held) and arg.value not in seen:
            seen.add(arg.value)
            op = definitions.get(arg.value)
            if op is None or op.loads or op.stores or op.barrier or len(op.args) != 1:
                return None
            if op.kind is mir.Kind.ADDRESS and isinstance(op.args[0], mir.FrameAddress):
                return op.args[0].offset
            if op.kind is not mir.Kind.COPY:
                return None
            arg = op.args[0]
        return None

    widths = {"SINGLE": 4, "DOUBLE": 8}
    blocks = []
    for block in body.blocks:
        ops = list(block.ops)
        for index, call in enumerate(ops):
            name = found.calls.get(call.at)
            width = widths.get(signatures.get(name.upper(), "")) if name is not None else None
            if call.kind is not mir.Kind.CALL or width is None:
                continue
            argument = None
            for prior in reversed(ops[:index]):
                if prior.kind is mir.Kind.CALL:
                    break
                if prior.kind is mir.Kind.ARG and len(prior.args) == 1:
                    argument = prior.args[0]
                    break
            destination = frame(argument)
            unknown = tuple(
                ref
                for ref in call.stores
                if ref.space is not Space.STACK and ref.addr is None and ref.provenance is None
            )
            if destination is None or len(unknown) != 1 or unknown[0].width != width:
                continue
            stores = tuple(
                mir.MemRef(Addr(Space.FRAME, destination), width, space=Space.FRAME) if ref is unknown[0] else ref
                for ref in call.stores
            )
            ops[index] = replace(call, stores=stores)
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


def result_only_functions(bodies: list[tuple[str, "mir.MirBody"]], found) -> frozenset[str]:
    """Prove BASIC functions whose only write is their hidden result.

    CodeView supplies the BYREF parameter slots that the machine frontend
    could not otherwise distinguish from integers.  Those slots become the
    same PARAMETER objects used by the C frontend's whole-module mod/ref
    analysis.  A floating function is result-only only when the fixed-point
    summary contains no unknown write and every written slice belongs to its
    final hidden-result parameter.
    """
    from qbopt.model import mir
    from qbopt.model import memory
    from qbopt.analysis import alias
    from qbopt.objectfile import cvinfo
    from qbopt.objectfile.module import Space

    procedures = {procedure.offset: procedure for procedure in cvinfo.parse(found.records).procedures}
    candidates: dict[str, tuple[alias.Procedure, int]] = {}
    for _label, body in bodies:
        procedure = procedures.get(body.entry)
        if procedure is None or procedure.signature is None:
            continue
        returned = cvinfo.type_name(procedure.signature.return_type, procedure.types)
        if returned not in {"SINGLE", "DOUBLE"}:
            continue
        params = sorted(procedure.params, key=lambda parameter: parameter.bp_offset, reverse=True)
        hidden = len(params)
        offsets = {parameter.bp_offset: index for index, parameter in enumerate(params)}
        offsets[min(offsets, default=8) - 2] = hidden
        seeds = dict(body.pointer_seeds)
        for block in body.blocks:
            for op in block.ops:
                if (
                    op.kind is mir.Kind.LOAD
                    and len(op.results) == 1
                    and isinstance(op.results[0], mir.Held)
                    and len(op.loads) == 1
                    and (ref := op.loads[0]).addr is not None
                    and ref.addr.space is Space.FRAME
                    and ref.base is None
                    and ref.segment is None
                    and ref.addr.disp in offsets
                ):
                    index = offsets[ref.addr.disp]
                    seeds[op.results[0].value] = memory.Provenance.one(memory.Object(memory.Kind.PARAMETER, index))
        seeded = alias.annotated(replace(body, pointer_seeds=seeds, pointer_values=body.pointer_values | seeds.keys()))
        calls = {
            op.at: found.calls[op.at]
            for block in seeded.blocks
            for op in block.ops
            if op.kind is mir.Kind.CALL and op.at in found.calls
        }
        candidates[procedure.name.upper()] = (alias.Procedure(seeded, calls, {}), hidden)

    # The BASIC frame helpers implement this body's activation and carry no
    # source-language caller-memory effect on the ordinary edge.
    empty = alias.Summary()
    summaries = alias.summaries(
        {name: procedure for name, (procedure, _hidden) in candidates.items()},
        {"B$ENRA": empty, "B$EXSA": empty},
    )
    return frozenset(
        name
        for name, (_procedure, hidden) in candidates.items()
        if not summaries[name].unknown_write
        and summaries[name].writes
        and all(
            one.object.kind is memory.Kind.PARAMETER and one.object.identity == hidden for one in summaries[name].writes
        )
    )


def complete_result_calls(body: "mir.MirBody", calls: dict[int, str], result_only: frozenset[str]) -> "mir.MirBody":
    """Record the completed mod/ref proof on direct result-only calls."""
    from qbopt.model import mir

    blocks = tuple(
        replace(
            block,
            ops=tuple(
                replace(op, memory_complete=True)
                if op.kind is mir.Kind.CALL and calls.get(op.at, "").upper() in result_only
                else op
                for op in block.ops
            ),
        )
        for block in body.blocks
    )
    return replace(body, blocks=blocks)


def fixed_assignments(body: "mir.MirBody", found) -> "mir.MirBody":
    """Give fixed-length B$ASSN calls their actual caller-memory ranges.

    B$ASSN is the runtime spelling of both string assignment and a fixed UDT
    copy.  With nonzero equal source/destination lengths it reads exactly the
    source byte range and writes exactly the destination byte range; the six
    words carrying those facts are explicit ARG operations.  Keep the call
    itself as language-runtime scaffolding, but do not let its conservative
    error paths alias unrelated frame objects.

    Only frame addresses and FAR dynamic-array operands are admitted.  A
    descriptor string, unequal padding/truncation, unknown segment, or
    unresolved size retains the original conservative effect.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.analysis import consts
    from qbopt.objectfile import cvinfo
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    facts = consts.known(body)
    definitions = {value: op for block in body.blocks for op in block.ops for value in op.defines}
    procedure = next(
        (procedure for procedure in cvinfo.parse(found.records).procedures if procedure.offset == body.entry),
        None,
    )
    array_parameters = (
        frozenset(
            parameter.bp_offset
            for parameter in procedure.params
            if (parameter.type_name or "").startswith("BYREF ARRAY OF ")
        )
        if procedure is not None
        else frozenset()
    )

    def number(arg) -> int | None:
        if isinstance(arg, mir.Const):
            return arg.n
        if isinstance(arg, mir.Held) and (fact := facts.get(arg.value)) is not None:
            return consts.masked(fact.n, arg.width)
        return None

    def frame(arg) -> int | None:
        seen = set()
        while isinstance(arg, mir.Held) and arg.value not in seen:
            seen.add(arg.value)
            op = definitions.get(arg.value)
            if op is None or op.loads or op.stores or op.barrier or len(op.args) != 1:
                return None
            if op.kind is mir.Kind.ADDRESS and isinstance(op.args[0], mir.FrameAddress):
                return op.args[0].offset
            if op.kind is not mir.Kind.COPY:
                return None
            arg = op.args[0]
        return None

    def array_descriptor(value: mir.Value) -> bool:
        op = definitions.get(value)
        return bool(
            op is not None
            and op.kind is mir.Kind.LOAD
            and len(op.loads) == 1
            and (ref := op.loads[0]).addr is not None
            and ref.addr.space is Space.FRAME
            and ref.base is None
            and ref.segment is None
            and ref.addr.disp in array_parameters
        )

    def dynamic_array_pointer(arg) -> bool:
        if not isinstance(arg, mir.Held):
            return False
        op = definitions.get(arg.value)
        if op is None or op.kind not in (mir.Kind.ADD, mir.Kind.COPY):
            return False
        return any(
            isinstance(source, mir.Cell)
            and source.ref.addr is not None
            and source.ref.addr.disp == 10
            and source.ref.base is not None
            and array_descriptor(source.ref.base)
            for source in op.args
        )

    def reference(segment, pointer, width: int) -> mir.MemRef | None:
        if not isinstance(segment, mir.Opaque) or not isinstance(segment.what, ir.Reg):
            return None
        if segment.what.register == Register.DS and (offset := frame(pointer)) is not None:
            return mir.MemRef(Addr(Space.FRAME, offset), width, space=Space.FRAME)
        if segment.what.register == Register.ES and isinstance(pointer, mir.Held):
            # The selector and offset came from a dynamic-array descriptor.
            # Its allocation predates the current activation and therefore
            # cannot be one of this activation's frame objects.  FAR alone is
            # not enough: an arbitrary far pointer may use SS.  State the
            # object-lifetime proof explicitly, while not retaining the
            # historical offset as a live call operand after ARG pushed it.
            excludes = (mir.WHOLE_FRAME,) if dynamic_array_pointer(pointer) else ()
            return mir.MemRef(
                None,
                width,
                space=Space.FAR,
                base_width=2,
                excludes=excludes,
            )
        return None

    blocks = []
    for block in body.blocks:
        ops = list(block.ops)
        for index, call in enumerate(ops):
            if call.kind is not mir.Kind.CALL or found.calls.get(call.at) != "B$ASSN":
                continue
            arguments = []
            for prior in reversed(ops[:index]):
                if prior.kind is mir.Kind.CALL:
                    break
                if prior.kind is mir.Kind.ARG and len(prior.args) == 1:
                    arguments.append(prior.args[0])
                    if len(arguments) == 6:
                        break
            arguments.reverse()
            if len(arguments) != 6:
                continue
            source_segment, source_pointer, source_count, dest_segment, dest_pointer, dest_count = arguments
            source_width, dest_width = number(source_count), number(dest_count)
            if source_width is None or source_width <= 0 or source_width != dest_width:
                continue
            source = reference(source_segment, source_pointer, source_width)
            destination = reference(dest_segment, dest_pointer, dest_width)
            if source is None or destination is None:
                continue
            stack_writes = tuple(ref for ref in call.stores if ref.space is Space.STACK)
            ops[index] = replace(
                call,
                loads=(source,),
                stores=(*stack_writes, destination),
                memory_complete=True,
            )
        blocks.append(replace(block, ops=tuple(ops)))
    return replace(body, blocks=tuple(blocks))


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
    from qbopt.model import mir
    from qbopt.analysis import effects
    from qbopt.objectfile.module import Space

    reads, writes = [], []
    for block in body.blocks:
        for op in block.ops:
            if op.kind is mir.Kind.CALL and op.at in calls:
                contract = contracts.get(op.at)
                direct_reads = (
                    contract.direct_reads
                    if contract and contract.direct_reads is not None
                    else (contract.reads if contract else runtime.Memory.ANY)
                )
                direct_writes = (
                    contract.direct_writes
                    if contract and contract.direct_writes is not None
                    else (contract.writes if contract else runtime.Memory.ANY)
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

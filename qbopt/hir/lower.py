"""Lower verified common HIR into the existing MIR, without a p-code tier."""

from dataclasses import replace
from dataclasses import dataclass

from qbopt.abi import ports
from qbopt.model import ir
from qbopt.hir import model
from qbopt.model import mir
from qbopt.hir import escape
from qbopt.model import memory
from qbopt.model import floating
from qbopt.hir.verify import verify
from qbopt.hir.verify import InvalidHIR
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space

_KINDS = {one.value: one for one in mir.Kind}
_FORMATS = {
    model.FloatEvaluation.BINARY32: floating.Format.BINARY32,
    model.FloatEvaluation.BINARY64: floating.Format.BINARY64,
    model.FloatEvaluation.EXTENDED80: floating.Format.EXTENDED80,
}
_BINARY_FLOAT = {model.Op.FADD, model.Op.FSUB, model.Op.FMUL, model.Op.FDIV}
_UNARY_FLOAT = {
    model.Op.FNEG,
    model.Op.FABS,
    model.Op.FSQRT,
    model.Op.FSIN,
    model.Op.FCOS,
    model.Op.FATAN,
    model.Op.FLOG2,
    model.Op.FEXP2,
}
_X87_INTRINSICS = {
    model.Op.FSIN: "fsin",
    model.Op.FCOS: "fcos",
    model.Op.FATAN: "fatan",
    model.Op.FLOG2: "flog2",
    model.Op.FEXP2: "fexp2",
}


def _stored_format(type_: model.Type) -> floating.Format:
    if type_.kind is model.TypeKind.FLOAT:
        return floating.Format.BINARY32 if type_.width == 4 else floating.Format.BINARY64
    if type_.kind in (model.TypeKind.INTEGER, model.TypeKind.BOOLEAN):
        stored = {2: floating.Format.SIGNED16, 4: floating.Format.SIGNED32}.get(type_.width)
        if stored is not None:
            return stored
    raise InvalidHIR(f"{type_.name}: no floating storage format")


@dataclass(frozen=True, slots=True)
class Lowered:
    name: str
    body: mir.MirBody
    values: dict[int, mir.Value]
    externals: dict[int, str] | None = None
    # Stable handoff from a source HIR instruction to the unique MIR address
    # of the operation it became. HIR ids are per-function source identities;
    # MIR operation ids also include terminators and can numerically collide.
    source_instructions: dict[int, int] | None = None


_Pieces = dict[int, tuple[tuple[int, int, tuple[object, ...]], ...]]


def _space(place: model.Place) -> Space:
    if place.storage in (model.Storage.LOCAL, model.Storage.PARAMETER):
        return Space.FRAME
    if place.storage is model.Storage.EXTERNAL:
        return Space.EXTERNAL
    return Space.SEGMENT


def _provenance(place: model.Place, width: int, escaped: frozenset[int], pieces: _Pieces) -> memory.Provenance:
    """`width` bytes from `place`'s start, in the objects holding them."""
    if _space(place) is Space.FRAME:
        start, end = place.offset, place.offset + width
        return memory.Provenance(
            frozenset(
                memory.Slice(
                    memory.Object(memory.Kind.FRAME, identity, extent=high - low),
                    max(start, low) - low,
                    min(end, high) - low,
                    1,
                    1,
                )
                for low, high, identity in pieces[place.id]
                if low < end and start < high
            )
        )
    private = place.storage in (model.Storage.STATIC, model.Storage.MODULE) and place.symbol not in escaped
    object_ = memory.Object(
        memory.Kind.GLOBAL,
        (place.storage, place.id),
        extent=place.extent,
        addressed=not private,
        captured=not private,
    )
    return memory.Provenance.one(object_, 0, width)


def _ref(place: model.Place, type_: model.Type, escaped: frozenset[int], pieces: _Pieces) -> mir.MemRef:
    space = _space(place)
    index = 0 if space is Space.FRAME else place.symbol
    provenance = _provenance(place, type_.width, escaped, pieces)
    return mir.MemRef(
        Addr(space, place.offset, index),
        type_.width,
        space=space,
        provenance=provenance,
        volatile=place.volatile,
    )


def lower(program: model.Program) -> tuple[Lowered, ...]:
    verify(program)
    out: list[Lowered] = []
    for module in program.modules:
        types = {one.id: one for one in module.types}
        externals = {one.id: one.name for one in module.data if one.linkage is model.DataLinkage.EXTERNAL}
        escaped = escape.escaped(module)
        for function in module.functions:
            out.append(
                _function(
                    module.name,
                    _materialized_booleans(function, types),
                    types,
                    externals,
                    program.array_order,
                    escaped,
                )
            )
    return tuple(out)


_COMPARISONS = {
    model.Op.EQ,
    model.Op.NE,
    model.Op.LT,
    model.Op.LE,
    model.Op.GT,
    model.Op.GE,
    model.Op.BELOW,
    model.Op.BELOW_EQ,
    model.Op.ABOVE,
    model.Op.ABOVE_EQ,
    model.Op.STRING_EQ,
    model.Op.STRING_NE,
    model.Op.STRING_LT,
    model.Op.STRING_LE,
    model.Op.STRING_GT,
    model.Op.STRING_GE,
}
_STRING_COMPARISONS = {
    model.Op.STRING_EQ,
    model.Op.STRING_NE,
    model.Op.STRING_LT,
    model.Op.STRING_LE,
    model.Op.STRING_GT,
    model.Op.STRING_GE,
}


def _materialized_booleans(function: model.Function, types: dict[int, model.Type]) -> model.Function:
    """Turn non-branch comparison values into ordinary -1/0 frame values.

    MIR represents comparisons as flags. A comparison consumed directly by
    one branch can remain flags; every other QB boolean is a language value
    and must be materialized without inventing a machine-level SETcc tier.
    Splitting HIR here preserves evaluation order and lets ordinary SSA and
    CFG cleanup remove the temporary later.
    """

    def referenced(operand: model.Operand) -> tuple[int, ...]:
        if isinstance(operand, model.ValueRef):
            return (operand.value,)
        if isinstance(operand, (model.ArrayElement, model.ProjectedPlace)):
            return tuple(value for index in operand.indices for value in referenced(index))
        if isinstance(operand, model.IndirectPlace):
            return (operand.base,)
        if isinstance(operand, model.DescriptorPlace):
            return (operand.base,)
        return ()

    values = list(function.values)
    places = list(function.places)
    blocks = list(function.blocks)
    next_value = max((one.id for one in values), default=0) + 1
    next_place = max((one.id for one in places), default=0) + 1
    next_block = max((one.id for one in blocks), default=0) + 1
    next_instruction = max((one.id for block in blocks for one in block.instructions), default=0) + 1

    while True:
        uses: dict[int, int] = {}
        for block in blocks:
            for operand in (
                *(one for instruction in block.instructions for one in instruction.operands),
                *block.terminator.operands,
            ):
                for value in referenced(operand):
                    uses[value] = uses.get(value, 0) + 1
        found = None
        for block_index, block in enumerate(blocks):
            for instruction_index, instruction in enumerate(block.instructions):
                if instruction.op not in _COMPARISONS or len(instruction.results) != 1:
                    continue
                result = instruction.results[0]
                direct = (
                    uses.get(result) == 1
                    and block.terminator.kind is model.TerminatorKind.BRANCH
                    and block.terminator.operands == (model.ValueRef(result),)
                )
                if not direct:
                    found = block_index, instruction_index
                    break
            if found is not None:
                break
        if found is None:
            return replace(function, values=tuple(values), places=tuple(places), blocks=tuple(blocks))

        block_index, instruction_index = found
        block = blocks[block_index]
        comparison = block.instructions[instruction_index]
        result = comparison.results[0]
        result_type = next(one.type for one in values if one.id == result)
        width = types[result_type].width
        frame_low = min(
            (one.offset for one in places if one.storage in (model.Storage.LOCAL, model.Storage.PARAMETER)),
            default=0,
        )
        place = model.Place(
            next_place,
            f"$bool{next_place}",
            result_type,
            model.Storage.LOCAL,
            frame_low - width,
            extent=width,
        )
        next_place += 1
        places.append(place)
        condition = next_value
        next_value += 1
        values.append(model.Value(condition, result_type))
        comparison = replace(comparison, results=(condition,))
        true_block, false_block, join_block = next_block, next_block + 1, next_block + 2
        next_block += 3
        prefix = model.Block(
            block.id,
            (*block.instructions[:instruction_index], comparison),
            model.Terminator(
                model.TerminatorKind.BRANCH,
                (model.ValueRef(condition),),
                (true_block, false_block),
            ),
        )
        true_store = model.Instruction(
            next_instruction,
            model.Op.STORE,
            operands=(model.PlaceRef(place.id), model.Constant(result_type, -1)),
        )
        next_instruction += 1
        false_store = model.Instruction(
            next_instruction,
            model.Op.STORE,
            operands=(model.PlaceRef(place.id), model.Constant(result_type, 0)),
        )
        next_instruction += 1
        load = model.Instruction(
            next_instruction,
            model.Op.LOAD,
            results=(result,),
            operands=(model.PlaceRef(place.id),),
        )
        next_instruction += 1
        made = (
            prefix,
            model.Block(
                true_block,
                (true_store,),
                model.Terminator(model.TerminatorKind.JUMP, targets=(join_block,)),
            ),
            model.Block(
                false_block,
                (false_store,),
                model.Terminator(model.TerminatorKind.JUMP, targets=(join_block,)),
            ),
            model.Block(
                join_block,
                (load, *block.instructions[instruction_index + 1 :]),
                block.terminator,
            ),
        )
        blocks[block_index : block_index + 1] = made


def _function(
    module: str,
    function: model.Function,
    types: dict[int, model.Type],
    externals: dict[int, str],
    array_order: model.ArrayOrder,
    escaped: frozenset[int],
) -> Lowered:
    values = {one.id: mir.Value(one.id, one.id, variable=one.id, version=1) for one in function.values}
    value_types = {one.id: types[one.type] for one in function.values}
    integer_ranges: dict[mir.Value, mir.IntegerRange] = {}

    def value_width(type_: model.Type) -> int:
        return 10 if type_.kind is model.TypeKind.FLOAT else type_.width

    places = {one.id: one for one in function.places}
    pieces = model.frame_pieces(function.places)
    parameter_numbers = {value: number for number, value in enumerate(function.parameters)}
    next_frame_offset = min(
        (one.offset for one in function.places if one.storage in (model.Storage.LOCAL, model.Storage.PARAMETER)),
        default=0,
    )
    at = 0
    next_value = max(values, default=0) + 1
    definitions = {
        result: instruction
        for block in function.blocks
        for instruction in block.instructions
        for result in instruction.results
    }

    def referenced(one: model.Operand) -> tuple[int, ...]:
        if isinstance(one, model.ValueRef):
            return (one.value,)
        if isinstance(one, (model.ArrayElement, model.ProjectedPlace)):
            return tuple(value for index in one.indices for value in referenced(index))
        if isinstance(one, model.IndirectPlace):
            return (one.base,)
        if isinstance(one, model.DescriptorPlace):
            return (one.base,)
        return ()

    use_counts: dict[int, int] = {}
    for block in function.blocks:
        operands = (
            *(operand for instruction in block.instructions for operand in instruction.operands),
            *block.terminator.operands,
        )
        for operand_ in operands:
            for value in referenced(operand_):
                use_counts[value] = use_counts.get(value, 0) + 1

    def fresh(type_: model.Type) -> mir.Held:
        nonlocal next_value
        value = mir.Value(next_value, at + 1, variable=next_value, version=1)
        values[next_value] = value
        value_types[next_value] = type_
        next_value += 1
        return mir.Held(value, value_width(type_))

    def arithmetic(kind: mir.Kind, left: mir.Arg, right: mir.Arg, type_: model.Type, before: list[mir.Op]) -> mir.Held:
        nonlocal at
        result = fresh(type_)
        uses = tuple(one.value for one in (left, right) if isinstance(one, mir.Held))
        at += 1
        machine = ir.Operation.BINARY if kind is mir.Kind.PTR_OFFSET else ir.Operation.NOTHING
        name = "ptr_offset" if kind is mir.Kind.PTR_OFFSET else ""
        before.append(
            mir.Op(
                at,
                machine,
                name,
                (result.value,),
                uses,
                kind=kind,
                args=(left, right),
                results=(result,),
                id=at,
                reads_complete=True,
                memory_complete=True,
            )
        )
        return result

    def operand(one: model.Operand, before: list[mir.Op]) -> mir.Arg:
        nonlocal at, next_value
        match one:
            case model.ValueRef(value):
                return mir.Held(values[value], value_width(value_types[value]))
            case model.Constant(type_, value) if isinstance(value, int):
                return mir.Const(value, types[type_].width)
            case model.Constant():
                raise InvalidHIR(f"{module}.{function.name}: floating constants require a constant-pool place")
            case model.PlaceRef(place):
                type_ = types[places[place].type]
                return mir.Cell(_ref(places[place], type_, escaped, pieces))
            case model.ArrayElement(place_id, indices):
                place = places[place_id]
                array = types[place.type]
                assert array.element is not None
                element = types[array.element]
                offset = None
                dimensions = list(zip(indices, array.bounds, strict=True))
                if array_order is model.ArrayOrder.COLUMN_MAJOR:
                    dimensions.reverse()
                for index, (lower, upper) in dimensions:
                    got = operand(index, before)
                    index_type = types[index.type] if isinstance(index, model.Constant) else value_types[index.value]
                    adjusted = arithmetic(mir.Kind.SUB, got, mir.Const(lower, index_type.width), index_type, before)
                    if offset is not None:
                        count = upper - lower + 1
                        offset = arithmetic(
                            mir.Kind.MUL, offset, mir.Const(count, index_type.width), index_type, before
                        )
                        offset = arithmetic(mir.Kind.ADD, offset, adjusted, index_type, before)
                    else:
                        offset = adjusted
                assert offset is not None
                offset = arithmetic(
                    mir.Kind.MUL,
                    offset,
                    mir.Const(element.width, offset.width),
                    value_types[offset.value.id],
                    before,
                )
                space = _space(place)
                index = 0 if space is Space.FRAME else place.symbol
                provenance = _provenance(place, place.extent or array.width, escaped, pieces)
                ref = mir.MemRef(
                    Addr(space, place.offset, index),
                    element.width,
                    base=offset.value,
                    space=space,
                    base_width=offset.width,
                    provenance=provenance,
                    volatile=place.volatile,
                    inbounds=True,
                )
                return mir.Cell(ref)
            case model.ProjectedPlace(place_id, indices, field_offset, type_id):
                place = places[place_id]
                root = types[place.type]
                field_type = types[type_id]
                space = _space(place)
                segment = 0 if space is Space.FRAME else place.symbol
                provenance = _provenance(place, place.extent or root.width, escaped, pieces)
                if not indices:
                    return mir.Cell(
                        mir.MemRef(
                            Addr(space, place.offset + field_offset, segment),
                            field_type.width,
                            space=space,
                            provenance=provenance,
                            volatile=place.volatile,
                        )
                    )
                assert root.element is not None
                element = types[root.element]
                offset = None
                dimensions = list(zip(indices, root.bounds, strict=True))
                if array_order is model.ArrayOrder.COLUMN_MAJOR:
                    dimensions.reverse()
                for index, (lower, upper) in dimensions:
                    got = operand(index, before)
                    index_type = types[index.type] if isinstance(index, model.Constant) else value_types[index.value]
                    adjusted = arithmetic(mir.Kind.SUB, got, mir.Const(lower, index_type.width), index_type, before)
                    if offset is not None:
                        count = upper - lower + 1
                        offset = arithmetic(
                            mir.Kind.MUL, offset, mir.Const(count, index_type.width), index_type, before
                        )
                        offset = arithmetic(mir.Kind.ADD, offset, adjusted, index_type, before)
                    else:
                        offset = adjusted
                assert offset is not None
                offset = arithmetic(
                    mir.Kind.MUL,
                    offset,
                    mir.Const(element.width, offset.width),
                    value_types[offset.value.id],
                    before,
                )
                if field_offset:
                    offset = arithmetic(
                        mir.Kind.ADD,
                        offset,
                        mir.Const(field_offset, offset.width),
                        value_types[offset.value.id],
                        before,
                    )
                return mir.Cell(
                    mir.MemRef(
                        Addr(space, place.offset, segment),
                        field_type.width,
                        base=offset.value,
                        space=space,
                        base_width=offset.width,
                        provenance=provenance,
                        volatile=place.volatile,
                        inbounds=True,
                    )
                )
            case model.IndirectPlace(base, offset, type_id, volatile, inbounds):
                type_ = types[type_id]
                pointer_type = value_types[base]
                parameter = parameter_numbers.get(base)
                provenance = (
                    memory.Provenance.one(memory.Object(memory.Kind.PARAMETER, parameter))
                    if parameter is not None
                    else None
                )
                if pointer_type.address is model.AddressKind.NEAR:
                    return mir.Cell(
                        mir.MemRef(
                            Addr(Space.LITERAL, offset),
                            type_.width,
                            base=values[base],
                            space=Space.LITERAL,
                            base_width=pointer_type.width,
                            provenance=provenance,
                            inbounds=inbounds,
                            volatile=volatile,
                        )
                    )
                pointer = values[base]
                if pointer_type.address is model.AddressKind.FAR:
                    # A far pointer's selector and offset are independent
                    # address components.  Constant field selection advances
                    # only the 16-bit offset; unlike a huge pointer it must not
                    # normalize carry into the selector.  Preserve that form
                    # in MIR so every consumer, integral or floating, reaches
                    # lowering as one segmented memory reference.
                    halves = []
                    for bit in (0, 16):
                        half = mir.Value(next_value, at + 1, variable=next_value, version=1)
                        next_value += 1
                        at += 1
                        before.append(
                            mir.Op(
                                at,
                                ir.Operation.MOVE,
                                "extract",
                                (half,),
                                (pointer,),
                                kind=mir.Kind.EXTRACT,
                                args=(mir.Held(pointer, 4), mir.Const(bit, 1)),
                                results=(mir.Held(half, 2),),
                                id=at,
                                reads_complete=True,
                                memory_complete=True,
                            )
                        )
                        halves.append(half)
                    offset_value, segment_value = halves
                    if offset:
                        adjusted = mir.Value(next_value, at + 1, variable=next_value, version=1)
                        next_value += 1
                        at += 1
                        before.append(
                            mir.Op(
                                at,
                                ir.Operation.NOTHING,
                                "",
                                (adjusted,),
                                (offset_value,),
                                kind=mir.Kind.ADD,
                                args=(mir.Held(offset_value, 2), mir.Const(offset, 2)),
                                results=(mir.Held(adjusted, 2),),
                                id=at,
                                reads_complete=True,
                                memory_complete=True,
                            )
                        )
                        offset_value = adjusted
                    return mir.Cell(
                        mir.MemRef(
                            Addr(Space.FAR, 0),
                            type_.width,
                            base=offset_value,
                            segment=segment_value,
                            space=Space.FAR,
                            base_width=2,
                            provenance=provenance,
                            inbounds=inbounds,
                            volatile=volatile,
                        )
                    )
                if offset:
                    adjusted = arithmetic(
                        mir.Kind.PTR_OFFSET,
                        mir.Held(pointer, pointer_type.width),
                        mir.Const(offset, 4),
                        pointer_type,
                        before,
                    )
                    pointer = adjusted.value
                if type_.kind is model.TypeKind.FLOAT:
                    halves = []
                    for bit in (0, 16):
                        half = mir.Value(next_value, at + 1, variable=next_value, version=1)
                        next_value += 1
                        at += 1
                        before.append(
                            mir.Op(
                                at,
                                ir.Operation.MOVE,
                                "extract",
                                (half,),
                                (pointer,),
                                kind=mir.Kind.EXTRACT,
                                args=(mir.Held(pointer, 4), mir.Const(bit, 1)),
                                results=(mir.Held(half, 2),),
                                id=at,
                                reads_complete=True,
                                memory_complete=True,
                            )
                        )
                        halves.append(half)
                    offset_value, segment_value = halves
                    return mir.Cell(
                        mir.MemRef(
                            Addr(Space.FAR, 0),
                            type_.width,
                            base=offset_value,
                            segment=segment_value,
                            space=Space.FAR,
                            base_width=2,
                            provenance=provenance,
                            inbounds=inbounds,
                        )
                    )
                return mir.Cell(
                    mir.MemRef(
                        None,
                        type_.width,
                        base=pointer,
                        base_width=pointer_type.width,
                        pointer=True,
                        provenance=provenance,
                        inbounds=inbounds,
                        volatile=volatile,
                    )
                )
            case model.DescriptorPlace(base, field, type_id):
                pointer_type = value_types[base]
                pointee = types[pointer_type.element] if pointer_type.element is not None else None
                scoped_view = (
                    pointee is not None and pointee.kind is model.TypeKind.OPAQUE and pointee.name.startswith("$slice[")
                )
                if scoped_view:
                    offset = 0 if field is model.DescriptorField.LENGTH else 2
                else:
                    offset = -4 if field is model.DescriptorField.LENGTH else -2
                return operand(model.IndirectPlace(base, offset, type_id), before)

    def operation(instruction: model.Instruction) -> tuple[mir.Op, ...]:
        nonlocal at, next_value, next_frame_offset
        before: list[mir.Op] = []
        args = tuple(operand(one, before) for one in instruction.operands)
        if instruction.op is model.Op.ADDRESS:
            if len(args) != 1 or not isinstance(args[0], mir.Cell):
                raise InvalidHIR(f"{module}.{function.name}: address needs one place")
            # ADDRESS observes no payload bytes. A source object's extent may
            # be many kilobytes, but the real-mode LEA operand is a word-sized
            # effective address. Carrying the object's width here eventually
            # asks the encoder for an impossible 5279-byte memory operand.
            args = (mir.Cell(replace(args[0].ref, width=2)),)
        if (
            instruction.op is model.Op.ADDRESS
            and value_types[instruction.results[0]].address is model.AddressKind.NEAR
            and isinstance(args[0], mir.Cell)
            and args[0].ref.base is not None
        ):
            # A 16-bit frame address cannot use every allocator register as
            # BP's index (`[bp+bx]` has no encoding). Materialize the fixed
            # base and add the dynamic byte offset as ordinary integer MIR;
            # the latter may then live in any general register.
            reference = args[0].ref
            if reference.addr is not None and reference.addr.space is Space.LITERAL:
                at += 1
                result = values[instruction.results[0]]
                displacement = reference.addr.disp
                direct = mir.Op(
                    at,
                    ir.Operation.MOVE if displacement == 0 else ir.Operation.BINARY,
                    "mov" if displacement == 0 else "add",
                    (result,),
                    (reference.base,),
                    kind=mir.Kind.COPY if displacement == 0 else mir.Kind.ADD,
                    args=(mir.Held(reference.base, reference.base_width),)
                    if displacement == 0
                    else (
                        mir.Held(reference.base, reference.base_width),
                        mir.Const(displacement, reference.base_width),
                    ),
                    results=(mir.Held(result, 2),),
                    id=instruction.id,
                    reads_complete=True,
                    memory_complete=True,
                )
                return (*before, direct)
            base = mir.Value(next_value, at + 1, variable=next_value, version=1)
            next_value += 1
            at += 1
            base_address = mir.Op(
                at,
                ir.Operation.ADDRESS,
                "lea",
                (base,),
                (),
                kind=mir.Kind.ADDRESS,
                args=(mir.Cell(replace(reference, base=None, width=2)),),
                results=(mir.Held(base, 2),),
                id=at,
                reads_complete=True,
                memory_complete=True,
            )
            at += 1
            result = values[instruction.results[0]]
            added = mir.Op(
                at,
                ir.Operation.BINARY,
                "add",
                (result,),
                (base, reference.base),
                kind=mir.Kind.ADD,
                args=(mir.Held(base, 2), mir.Held(reference.base, reference.base_width)),
                results=(mir.Held(result, 2),),
                id=instruction.id,
                reads_complete=True,
                memory_complete=True,
            )
            return (*before, base_address, added)
        if instruction.op is model.Op.ADDRESS and value_types[instruction.results[0]].address in (
            model.AddressKind.FAR,
            model.AddressKind.HUGE,
        ):
            if len(args) != 1 or not isinstance(args[0], mir.Cell):
                raise InvalidHIR(f"{module}.{function.name}: whole address needs one place")
            reference = args[0].ref
            offset = mir.Value(next_value, at + 1, variable=next_value, version=1)
            next_value += 1
            at += 1
            if reference.addr is not None and reference.addr.space is Space.LITERAL and reference.base is not None:
                # An IndirectPlace is already relative to a pointer value.
                # Its literal displacement is not an absolute symbol that can
                # be addressed independently: the offset half is base+disp.
                displacement = reference.addr.disp
                offset_ops = [
                    mir.Op(
                        at,
                        ir.Operation.MOVE if displacement == 0 else ir.Operation.BINARY,
                        "mov" if displacement == 0 else "add",
                        (offset,),
                        (reference.base,),
                        kind=mir.Kind.COPY if displacement == 0 else mir.Kind.ADD,
                        args=(mir.Held(reference.base, reference.base_width),)
                        if displacement == 0
                        else (
                            mir.Held(reference.base, reference.base_width),
                            mir.Const(displacement, reference.base_width),
                        ),
                        results=(mir.Held(offset, 2),),
                        id=at,
                        reads_complete=True,
                        memory_complete=True,
                    )
                ]
            else:
                address = mir.Op(
                    at,
                    ir.Operation.ADDRESS,
                    "lea",
                    (offset,),
                    (),
                    kind=mir.Kind.ADDRESS,
                    args=(mir.Cell(replace(reference, base=None, width=2)),),
                    results=(mir.Held(offset, 2),),
                    id=at,
                    reads_complete=True,
                    memory_complete=True,
                )
                offset_ops = [address]
            if reference.base is not None and not (
                reference.addr is not None and reference.addr.space is Space.LITERAL
            ):
                # A whole pointer still contains a 16-bit offset. Keep its
                # segment construction below, but form that offset with the
                # same unrestricted integer add used by a near pointer. This
                # prevents allocation from turning a local array element into
                # an unencodable 16-bit address such as [bp+bx].
                added = mir.Value(next_value, at + 1, variable=next_value, version=1)
                next_value += 1
                at += 1
                offset_ops.append(
                    mir.Op(
                        at,
                        ir.Operation.BINARY,
                        "add",
                        (added,),
                        (offset, reference.base),
                        kind=mir.Kind.ADD,
                        args=(mir.Held(offset, 2), mir.Held(reference.base, reference.base_width)),
                        results=(mir.Held(added, 2),),
                        id=at,
                        reads_complete=True,
                        memory_complete=True,
                    )
                )
                offset = added
            segment = mir.Value(next_value, at + 1, variable=next_value, version=1)
            next_value += 1
            at += 1
            selector_source = (
                mir.FrameSelector() if reference.space is Space.FRAME else mir.Symbol(Space.GROUP, 0, 0, 2)
            )
            selector = mir.Op(
                at,
                ir.Operation.MOVE,
                "mov",
                (segment,),
                (),
                kind=mir.Kind.COPY,
                args=(selector_source,),
                results=(mir.Held(segment, 2),),
                id=at,
                reads_complete=True,
                memory_complete=True,
            )
            at += 1
            result = values[instruction.results[0]]
            joined = mir.Op(
                at,
                ir.Operation.MOVE,
                "",
                (result,),
                (segment, offset),
                kind=mir.Kind.CONCAT,
                args=(mir.Held(segment, 2), mir.Held(offset, 2)),
                results=(mir.Held(result, 4),),
                id=instruction.id,
                reads_complete=True,
                memory_complete=True,
            )
            return (*before, *offset_ops, selector, joined)
        if (
            instruction.op is model.Op.PTR_OFFSET
            and value_types[instruction.results[0]].address is model.AddressKind.FAR
        ):
            if (
                len(args) != 2
                or not isinstance(args[0], mir.Held)
                or args[0].width != 4
                or not isinstance(args[1], (mir.Held, mir.Const))
                or args[1].width not in (1, 2, 4)
            ):
                raise InvalidHIR(
                    f"{module}.{function.name}: far pointer offset needs a pointer and integer displacement"
                )
            pointer, displacement = args
            if displacement.width != 2:
                displacement = (
                    mir.Held(displacement.value, 2)
                    if isinstance(displacement, mir.Held)
                    else mir.Const(displacement.n, 2)
                )
            halves = []
            for bit in (0, 16):
                half = mir.Value(next_value, at + 1, variable=next_value, version=1)
                next_value += 1
                at += 1
                before.append(
                    mir.Op(
                        at,
                        ir.Operation.MOVE,
                        "extract",
                        (half,),
                        (pointer.value,),
                        kind=mir.Kind.EXTRACT,
                        args=(pointer, mir.Const(bit, 1)),
                        results=(mir.Held(half, 2),),
                        id=at,
                        reads_complete=True,
                        memory_complete=True,
                    )
                )
                halves.append(half)
            offset, segment = halves
            adjusted = mir.Value(next_value, at + 1, variable=next_value, version=1)
            next_value += 1
            at += 1
            before.append(
                mir.Op(
                    at,
                    ir.Operation.BINARY,
                    "add",
                    (adjusted,),
                    tuple(one.value for one in (mir.Held(offset, 2), displacement) if isinstance(one, mir.Held)),
                    kind=mir.Kind.ADD,
                    args=(mir.Held(offset, 2), displacement),
                    results=(mir.Held(adjusted, 2),),
                    id=at,
                    reads_complete=True,
                    memory_complete=True,
                )
            )
            at += 1
            result = values[instruction.results[0]]
            before.append(
                mir.Op(
                    at,
                    ir.Operation.MOVE,
                    "",
                    (result,),
                    (segment, adjusted),
                    kind=mir.Kind.CONCAT,
                    args=(mir.Held(segment, 2), mir.Held(adjusted, 2)),
                    results=(mir.Held(result, 4),),
                    id=instruction.id,
                    reads_complete=True,
                    memory_complete=True,
                )
            )
            return tuple(before)
        if instruction.op in (model.Op.POINTER_SEGMENT, model.Op.POINTER_OFFSET):
            if len(args) != 1 or not isinstance(args[0], mir.Held):
                raise InvalidHIR(f"{module}.{function.name}: pointer projection needs one pointer")
            if instruction.op is model.Op.POINTER_SEGMENT and args[0].width != 4:
                raise InvalidHIR(f"{module}.{function.name}: segment projection needs one far pointer")
            if instruction.op is model.Op.POINTER_OFFSET and args[0].width not in (2, 4):
                raise InvalidHIR(f"{module}.{function.name}: offset projection needs a near or far pointer")
            at += 1
            result = values[instruction.results[0]]
            if instruction.op is model.Op.POINTER_SEGMENT:
                projected = mir.Op(
                    at,
                    ir.Operation.NOTHING,
                    "",
                    (result,),
                    (args[0].value,),
                    kind=mir.Kind.SHR,
                    args=(args[0], mir.Const(16, 1)),
                    results=(mir.Held(result, 4),),
                    id=instruction.id,
                    reads_complete=True,
                    memory_complete=True,
                )
            else:
                projected = mir.Op(
                    at,
                    ir.Operation.MOVE,
                    "mov",
                    (result,),
                    (args[0].value,),
                    kind=mir.Kind.COPY,
                    args=(mir.Held(args[0].value, 2),),
                    results=(mir.Held(result, 2),),
                    id=instruction.id,
                    reads_complete=True,
                    memory_complete=True,
                )
            return (*before, projected)
        if instruction.op is model.Op.CONVERT and len(instruction.operands) == len(instruction.results) == 1:
            source_type = (
                value_types[instruction.operands[0].value]
                if isinstance(instruction.operands[0], model.ValueRef)
                else types[places[instruction.operands[0].place].type]
                if isinstance(instruction.operands[0], model.PlaceRef)
                else types[instruction.operands[0].type]
            )
            target_type = value_types[instruction.results[0]]
            if (
                source_type.kind is model.TypeKind.FLOAT
                and target_type.kind is model.TypeKind.FLOAT
                and target_type.width >= source_type.width
                and isinstance(args[0], mir.Held)
            ):
                # Both QB formats evaluate in extended80. Widening changes
                # the source type of later storage and calls, but creates no
                # new machine value and performs no rounding.
                values[instruction.results[0]] = args[0].value
                return tuple(before)
        at += 1
        made = tuple(values[one] for one in instruction.results)
        results = tuple(
            mir.Held(one, value_width(value_types[source]))
            for one, source in zip(made, instruction.results, strict=True)
        )
        if (
            instruction.op is model.Op.LOAD
            and len(instruction.operands) == len(made) == 1
            and isinstance(instruction.operands[0], model.DescriptorPlace)
        ):
            descriptor = instruction.operands[0]
            pointer = value_types[descriptor.base]
            assert pointer.element is not None
            element = types[pointer.element]
            if element.kind is model.TypeKind.OPAQUE and element.name.startswith("$slice["):
                assert element.element is not None
                element = types[element.element]
            # Translate the target ABI rule here, at the HIR boundary.  A
            # descriptor-backed slice fits in one pointer-offset domain, so
            # its element count cannot exceed that domain divided by the
            # element width.  MIR receives only the resulting integer fact.
            offset_width = pointer.width if pointer.address is model.AddressKind.NEAR else pointer.width // 2
            field_width = value_types[instruction.results[0]].width
            maximum = min((1 << (field_width * 8)) - 1, (1 << (offset_width * 8)) // element.width)
            integer_ranges[made[0]] = mir.IntegerRange(0, maximum, field_width)
        # Shared MIR deliberately has no target-instruction catalogue. Keep
        # source intrinsics as unary floating computation, never CALL: FSQRT
        # is the existing unary-float carrier and ``name`` retains the exact
        # operation for the QB-owned physical boundary. No sqrt facts are
        # claimed because these operations have no ``floating`` rule below.
        kind = (
            mir.Kind.CALL
            if instruction.op in _STRING_COMPARISONS
            else mir.Kind.FSQRT
            if instruction.op in _X87_INTRINSICS
            else mir.Kind.UDIVMOD
            if instruction.op in (model.Op.UDIV, model.Op.UREM, model.Op.UDIVMOD)
            else mir.Kind.FSTORE
            if instruction.op is model.Op.TRUNCATE
            else _KINDS[instruction.op.value]
        )
        if instruction.op in (model.Op.DIV, model.Op.REM, model.Op.UDIV, model.Op.UREM):
            source = instruction.results[0]
            extra = mir.Value(next_value, at, variable=next_value, version=1)
            value_types[next_value] = value_types[source]
            next_value += 1
            made = (made[0], extra) if instruction.op in (model.Op.DIV, model.Op.UDIV) else (extra, made[0])
            results = tuple(mir.Held(one, value_types[source].width) for one in made)
            kind = mir.Kind.UDIVMOD if instruction.op in (model.Op.UDIV, model.Op.UREM) else mir.Kind.DIVMOD
        cells = tuple(one.ref for one in args if isinstance(one, mir.Cell))
        semantics = None
        if instruction.op in _BINARY_FLOAT | _UNARY_FLOAT:
            formats = tuple(
                _FORMATS[value_types[one.value].evaluation]
                for one in instruction.operands
                if isinstance(one, model.ValueRef)
            )
            result = _FORMATS[value_types[instruction.results[0]].evaluation]
            semantics = floating.Semantics(formats, result, floating.Precision.DYNAMIC, floating.Rounding.DYNAMIC)
            if instruction.op in (model.Op.FNEG, model.Op.FABS):
                semantics = replace(
                    semantics,
                    precision=floating.Precision.EXACT,
                    rounding=floating.Rounding.NONE,
                )
            elif instruction.op in _X87_INTRINSICS:
                # The strict-float fact model knows FSQRT but not
                # transcendental functions. Omitting a false rule keeps the
                # optimizer conservative without hiding work in a pseudo-call.
                semantics = None
        if instruction.op is model.Op.LOAD and value_types[instruction.results[0]].kind is model.TypeKind.FLOAT:
            stored = value_types[instruction.results[0]]
            kind = mir.Kind.FLOAD
            semantics = floating.Semantics(
                (_stored_format(stored),),
                _FORMATS[value_types[instruction.results[0]].evaluation],
                floating.Precision.EXACT,
                floating.Rounding.NONE,
            )
        if (
            instruction.op is model.Op.STORE
            and isinstance(instruction.operands[1], model.ValueRef)
            and value_types[instruction.operands[1].value].kind is model.TypeKind.FLOAT
        ):
            stored = value_types[instruction.operands[1].value]
            source = value_types[instruction.operands[1].value]
            kind = mir.Kind.FSTORE
            semantics = floating.Semantics(
                (_FORMATS[source.evaluation],),
                _stored_format(stored),
                floating.Precision.DESTINATION,
                floating.Rounding.DYNAMIC,
            )
        uses = tuple(
            dict.fromkeys(
                value
                for one in args
                for value in (
                    (one.value,)
                    if isinstance(one, mir.Held)
                    else tuple(candidate for candidate in (one.ref.base, one.ref.segment) if candidate is not None)
                    if isinstance(one, mir.Cell)
                    else ()
                )
            )
        )
        operation_kind = (
            ir.Operation.FLOAT_UNARY
            if instruction.op in _X87_INTRINSICS
            else ir.Operation.CALL
            if kind is mir.Kind.CALL
            else ir.Operation.NOTHING
        )
        operation_name = _X87_INTRINSICS.get(instruction.op, instruction.callee or "")
        if instruction.op is model.Op.PTR_OFFSET:
            target_pointer = value_types[instruction.results[0]]
            if target_pointer.width == 4:
                operation_kind, operation_name = ir.Operation.BINARY, "ptr_offset"
            else:
                kind = mir.Kind.ADD
        if instruction.op in (model.Op.CONVERT, model.Op.TRUNCATE):
            source_id = (
                value_types[instruction.operands[0].value]
                if isinstance(instruction.operands[0], model.ValueRef)
                else types[places[instruction.operands[0].place].type]
                if isinstance(instruction.operands[0], model.PlaceRef)
                else types[instruction.operands[0].type]
            )
            target_type = value_types[instruction.results[0]]
            if source_id.kind in (model.TypeKind.INTEGER, model.TypeKind.BOOLEAN) and target_type.kind in (
                model.TypeKind.INTEGER,
                model.TypeKind.BOOLEAN,
            ):
                if isinstance(args[0], mir.Const):
                    args = (mir.Const(args[0].n, target_type.width),)
                    operation_kind, operation_name, kind = ir.Operation.MOVE, "mov", mir.Kind.COPY
                elif target_type.width > source_id.width:
                    operation_kind = ir.Operation.EXTEND
                    operation_name = "movsx" if source_id.signed is not False else "movzx"
                    kind = mir.Kind.SIGN_EXTEND if source_id.signed is not False else mir.Kind.ZERO_EXTEND
                else:
                    operation_kind, operation_name, kind = ir.Operation.MOVE, "mov", mir.Kind.COPY
                    if isinstance(args[0], mir.Held):
                        args = (mir.Held(args[0].value, target_type.width),)
            elif (
                source_id.kind in (model.TypeKind.INTEGER, model.TypeKind.BOOLEAN)
                and target_type.kind is model.TypeKind.FLOAT
            ):
                if not isinstance(args[0], mir.Cell):
                    raise InvalidHIR(f"{module}.{function.name}: integer-to-float conversion needs a place")
                kind = mir.Kind.FLOAD
                operation_kind, operation_name = ir.Operation.FLOAT_LOAD, "fild"
                semantics = floating.Semantics(
                    (_stored_format(source_id),),
                    _FORMATS[target_type.evaluation],
                    floating.Precision.EXACT,
                    floating.Rounding.NONE,
                )
            elif source_id.kind is model.TypeKind.FLOAT and target_type.kind in (
                model.TypeKind.INTEGER,
                model.TypeKind.BOOLEAN,
            ):
                kind = mir.Kind.FSTORE
                operation_kind, operation_name = ir.Operation.FLOAT_STORE, "fistp"
                semantics = floating.Semantics(
                    (floating.Format.EXTENDED80,),
                    _stored_format(target_type),
                    floating.Precision.DESTINATION,
                    floating.Rounding.TOWARD_ZERO if instruction.op is model.Op.TRUNCATE else floating.Rounding.DYNAMIC,
                )
            elif source_id.kind is model.TypeKind.FLOAT and target_type.kind is model.TypeKind.FLOAT:
                if target_type.width >= source_id.width:
                    kind = mir.Kind.COPY
                    operation_kind, operation_name = ir.Operation.MOVE, "mov"
                else:
                    next_frame_offset -= target_type.width
                    object_ = memory.Object(
                        memory.Kind.FRAME,
                        ("float-convert", instruction.id),
                        extent=target_type.width,
                    )
                    reference = mir.MemRef(
                        Addr(Space.FRAME, next_frame_offset),
                        target_type.width,
                        space=Space.FRAME,
                        provenance=memory.Provenance.one(object_, 0, target_type.width),
                    )
                    stored = floating.Semantics(
                        (floating.Format.EXTENDED80,),
                        _stored_format(target_type),
                        floating.Precision.DESTINATION,
                        floating.Rounding.DYNAMIC,
                    )
                    store = mir.Op(
                        at,
                        ir.Operation.FLOAT_STORE,
                        "fstp",
                        (),
                        tuple(one.value for one in args if isinstance(one, mir.Held)),
                        floating=stored,
                        stores=(reference,),
                        kind=mir.Kind.FSTORE,
                        args=args,
                        results=(mir.Cell(reference),),
                        id=at,
                        reads_complete=True,
                        memory_complete=True,
                    )
                    at += 1
                    loaded = floating.Semantics(
                        (_stored_format(target_type),),
                        floating.Format.EXTENDED80,
                        floating.Precision.EXACT,
                        floating.Rounding.NONE,
                    )
                    load = mir.Op(
                        at,
                        ir.Operation.FLOAT_LOAD,
                        "fld",
                        made,
                        (),
                        floating=loaded,
                        loads=(reference,),
                        kind=mir.Kind.FLOAD,
                        args=(mir.Cell(reference),),
                        results=results,
                        id=instruction.id,
                        reads_complete=True,
                        memory_complete=True,
                    )
                    return (*before, store, load)
        loads = cells if kind in (mir.Kind.LOAD, mir.Kind.FLOAD) else ()
        stores = cells if kind in (mir.Kind.STORE, mir.Kind.FSTORE) else ()
        if instruction.op is model.Op.CALL:
            # BASIC's default argument convention is BYREF. A pointer actual
            # therefore publishes its pointee to the callee for both reading
            # and writing; the stack word alone is not the call's complete
            # memory effect. Without these witnesses, promotion deleted the
            # stores of 100000 and 23 before Q45P04's addLong call and passed
            # two uninitialized frame slots instead.
            pointees = []
            for argument in args:
                if not isinstance(argument, mir.Held):
                    continue
                type_ = value_types.get(argument.value.id)
                if type_ is None or type_.kind is not model.TypeKind.POINTER or type_.element is None:
                    continue
                element = types[type_.element]
                pointees.append(
                    mir.MemRef(
                        None,
                        element.width,
                        base=argument.value,
                        space=Space.LITERAL,
                        base_width=type_.width,
                        pointer=True,
                    )
                )
            loads = tuple(pointees)
            stores = tuple(pointees)
        if instruction.op is model.Op.STORE:
            if len(args) != 2 or not isinstance(args[0], mir.Cell):
                raise InvalidHIR(f"{module}.{function.name}: store has no destination cell")
            address_uses = tuple(one for one in (args[0].ref.base, args[0].ref.segment) if one is not None)
            results = (args[0],)
            args = (args[1],)
            uses = tuple(dict.fromkeys((*address_uses, *(one.value for one in args if isinstance(one, mir.Held)))))
        port = instruction.op in (model.Op.PORT_IN, model.Op.PORT_OUT)
        # A device with no path to memory leaves every cell alone; any other
        # port may start a transfer, so its memory effect stays unknown.
        silent_port = port and isinstance(args[0], mir.Const) and ports.silent(args[0].n)
        final = mir.Op(
            at,
            operation_kind,
            operation_name,
            made,
            uses,
            floating=semantics,
            loads=loads,
            stores=stores,
            kind=kind,
            args=args,
            results=results,
            id=instruction.id,
            args_known=True,
            memory_complete=(
                instruction.op is not model.Op.CALL and instruction.op not in _STRING_COMPARISONS and not port
            )
            or silent_port,
            reads_complete=instruction.op is not model.Op.CALL and instruction.op not in _STRING_COMPARISONS,
            volatile=port or any(reference.volatile for reference in (*loads, *stores)),
        )
        return (*before, final)

    # HIR block ids are stable source identities. Preserve them through MIR:
    # preprocessing can insert comparison-materialization blocks, so mapping
    # by the post-expansion list position silently retargets independently
    # recorded entries such as ON ERROR handlers and RESUME statement rows.
    block_at = {one.id: one.id for one in function.blocks}
    blocks = []
    source_instructions: dict[int, int] = {}
    for source in function.blocks:
        ops = []
        pending_source: list[int] = []
        for instruction in source.instructions:
            made = operation(instruction)
            if not made:
                # A semantic no-op (for example an identity conversion) owns
                # no machine address. Its statement begins at the next real
                # operation, or at this block's terminator if none follows.
                pending_source.append(instruction.id)
                continue
            # The first emitted operation includes any required operand
            # materialization and is the statement's actual machine entry.
            # MIR addresses are globally unique and survive ordinary
            # optimizer rewrites as Op.at.
            for source_instruction in (*pending_source, instruction.id):
                source_instructions[source_instruction] = made[0].at
            pending_source.clear()
            ops.extend(made)
        term = source.terminator
        before: list[mir.Op] = []
        args = tuple(operand(one, before) for one in term.operands)
        if term.kind is model.TerminatorKind.RETURN:
            materialized = []
            for source_operand, argument in zip(term.operands, args, strict=True):
                if isinstance(argument, mir.Const):
                    type_id = source_operand.type if isinstance(source_operand, model.Constant) else None
                    if type_id is None:
                        raise InvalidHIR(f"{module}.{function.name}: return constant lost its type")
                    held = fresh(types[type_id])
                    at += 1
                    before.append(
                        mir.Op(
                            at,
                            ir.Operation.MOVE,
                            "mov",
                            (held.value,),
                            (),
                            kind=mir.Kind.COPY,
                            args=(argument,),
                            results=(held,),
                            id=at,
                            reads_complete=True,
                            memory_complete=True,
                        )
                    )
                    argument = held
                materialized.append(argument)
            args = tuple(materialized)
        ops.extend(before)
        uses = tuple(one.value for one in args if isinstance(one, mir.Held))
        target = block_at[term.targets[0]] if term.targets else None
        kind = {
            model.TerminatorKind.JUMP: mir.Kind.JUMP,
            model.TerminatorKind.BRANCH: mir.Kind.BRANCH,
            model.TerminatorKind.SWITCH: mir.Kind.SWITCH,
            model.TerminatorKind.RETURN: mir.Kind.RETURN,
            model.TerminatorKind.UNREACHABLE: mir.Kind.ESCAPE,
        }[term.kind]
        test = None
        if term.kind is model.TerminatorKind.BRANCH:
            condition = args[0]
            if not isinstance(condition, mir.Held):
                raise InvalidHIR(f"{module}.{function.name}: branch condition must be a value")
            source_value = term.operands[0]
            comparison = definitions.get(source_value.value) if isinstance(source_value, model.ValueRef) else None
            comparisons = {
                model.Op.EQ: mir.Kind.EQ,
                model.Op.NE: mir.Kind.NE,
                model.Op.LT: mir.Kind.LT,
                model.Op.LE: mir.Kind.LE,
                model.Op.GT: mir.Kind.GT,
                model.Op.GE: mir.Kind.GE,
                model.Op.BELOW: mir.Kind.BELOW,
                model.Op.BELOW_EQ: mir.Kind.BELOW_EQ,
                model.Op.ABOVE: mir.Kind.ABOVE,
                model.Op.ABOVE_EQ: mir.Kind.ABOVE_EQ,
                model.Op.STRING_EQ: mir.Kind.EQ,
                model.Op.STRING_NE: mir.Kind.NE,
                model.Op.STRING_LT: mir.Kind.LT,
                model.Op.STRING_LE: mir.Kind.LE,
                model.Op.STRING_GT: mir.Kind.GT,
                model.Op.STRING_GE: mir.Kind.GE,
            }
            if comparison is not None and comparison.op in comparisons and use_counts.get(source_value.value) == 1:
                position = next(
                    (
                        index
                        for index, operation_ in enumerate(ops)
                        if any(
                            isinstance(result, mir.Held) and result.value == condition.value
                            for result in operation_.results
                        )
                    ),
                    None,
                )
                if position is None:
                    raise InvalidHIR(f"{module}.{function.name}: comparison crosses a block")
                compared = ops[position]
                flags = mir.Value(next_value, compared.at, flags=True, variable=next_value, version=1)
                next_value += 1
                floating_compare = any(
                    value_types[operand_.value].kind is model.TypeKind.FLOAT
                    for operand_ in comparison.operands
                    if isinstance(operand_, model.ValueRef)
                )
                if comparison.op in _STRING_COMPARISONS:
                    ops[position] = replace(compared, defines=(flags,), results=())
                else:
                    ops[position] = replace(
                        compared,
                        op=ir.Operation.COMPARE,
                        name="",
                        defines=(flags,),
                        uses=tuple(argument.value for argument in compared.args if isinstance(argument, mir.Held)),
                        kind=mir.Kind.FCOMPARE if floating_compare else mir.Kind.SUB,
                        results=(),
                    )
                test = comparisons[comparison.op]
            else:
                flags = mir.Value(next_value, at + 1, flags=True, variable=next_value, version=1)
                next_value += 1
                at += 1
                ops.append(
                    mir.Op(
                        at,
                        ir.Operation.COMPARE,
                        "",
                        (flags,),
                        (condition.value,),
                        kind=mir.Kind.SUB,
                        args=(condition, mir.Const(0, condition.width)),
                        id=at,
                        reads_complete=True,
                    )
                )
                test = mir.Kind.NE
            args = ()
            uses = (flags,)
        at += 1
        machine = {
            model.TerminatorKind.JUMP: ir.Operation.JUMP,
            model.TerminatorKind.BRANCH: ir.Operation.BRANCH,
            model.TerminatorKind.SWITCH: ir.Operation.JUMP,
            model.TerminatorKind.RETURN: ir.Operation.RETURN,
            model.TerminatorKind.UNREACHABLE: ir.Operation.ESCAPE,
        }[term.kind]
        terminal = mir.Op(
            at,
            machine,
            "",
            (),
            uses,
            kind=kind,
            args=args,
            test=test,
            target=target,
            cases=tuple((value, block_at[label]) for value, label in term.cases),
            id=at,
            reads_complete=True,
        )
        if pending_source:
            marker = before[0].at if before else terminal.at
            for source_instruction in pending_source:
                source_instructions[source_instruction] = marker
        ops.append(terminal)
        succ = tuple(block_at[one] for one in dict.fromkeys((*term.targets, *(target for _, target in term.cases))))
        blocks.append(mir.MirBlock(block_at[source.id], (), tuple(ops), succ, source.cold))
    pointer_values = frozenset(
        values[one.id] for one in function.values if types[one.type].kind is model.TypeKind.POINTER
    )
    pointer_seeds = {
        values[value]: memory.Provenance.one(memory.Object(memory.Kind.PARAMETER, number))
        for value, number in parameter_numbers.items()
        if value_types[value].kind is model.TypeKind.POINTER
    }
    body = mir.MirBody(
        block_at[function.entry],
        tuple(blocks),
        sealed=True,
        pointer_values=pointer_values,
        pointer_seeds=pointer_seeds,
        integer_ranges=integer_ranges,
    )
    checked = body
    external = tuple(
        dict.fromkeys(
            (
                body.entry,
                *(block_at[one] for one in function.external_entries),
                *(() if function.error_handler is None else (block_at[function.error_handler],)),
            )
        )
    )
    if len(external) > 1:
        # Runtime-dispatched statement and ON ERROR entries are independent
        # roots. Verification gets an empty synthetic super-root; returning it
        # would falsify source control flow, so the lowered body remains exact.
        root = max(block.at for block in body.blocks) + 1
        checked = replace(
            body,
            entry=root,
            blocks=(mir.MirBlock(root, (), (), external), *body.blocks),
        )
    problems = mir.verify(checked)
    if problems:
        raise InvalidHIR(f"{module}.{function.name}: invalid lowered MIR: {problems[:3]}")
    return Lowered(
        f"{module}.{function.name}",
        body,
        values,
        externals,
        source_instructions,
    )

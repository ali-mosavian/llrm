"""Structural and semantic checks at the source/frontend boundary."""

from qbopt.hir import model


class InvalidHIR(ValueError):
    """HIR cannot be represented faithfully by the current MIR."""


_RESULTS = {
    model.Op.STORE: 0,
    model.Op.CALL: None,
    model.Op.DIVMOD: 2,
    model.Op.UDIVMOD: 2,
}
_PLACES = {model.Op.LOAD, model.Op.STORE, model.Op.ADDRESS}
_FLOAT = {
    model.Op.FADD,
    model.Op.FSUB,
    model.Op.FMUL,
    model.Op.FDIV,
    model.Op.FNEG,
    model.Op.FABS,
    model.Op.FSQRT,
    model.Op.FSIN,
    model.Op.FCOS,
    model.Op.FATAN,
    model.Op.FLOG2,
    model.Op.FEXP2,
}
_INTEGER = {
    model.Op.ADD,
    model.Op.SUB,
    model.Op.MUL,
    model.Op.FIXED_MUL,
    model.Op.FIXED_DIV,
    model.Op.DIV,
    model.Op.REM,
    model.Op.DIVMOD,
    model.Op.UDIV,
    model.Op.UREM,
    model.Op.UDIVMOD,
    model.Op.AND,
    model.Op.OR,
    model.Op.XOR,
    model.Op.SHL,
    model.Op.SHR,
    model.Op.SAR,
    model.Op.NEG,
    model.Op.NOT,
}
_COMPARE = {
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
_STRING_COMPARE = {
    model.Op.STRING_EQ,
    model.Op.STRING_NE,
    model.Op.STRING_LT,
    model.Op.STRING_LE,
    model.Op.STRING_GT,
    model.Op.STRING_GE,
}
_UNSIGNED = {
    model.Op.UDIV,
    model.Op.UREM,
    model.Op.UDIVMOD,
    model.Op.BELOW,
    model.Op.BELOW_EQ,
    model.Op.ABOVE,
    model.Op.ABOVE_EQ,
}
_POINTER_PART = {model.Op.POINTER_SEGMENT, model.Op.POINTER_OFFSET}


def _operand_type(operand: model.Operand, values: dict[int, model.Value], places: dict[int, model.Place]) -> int:
    match operand:
        case model.ValueRef(value):
            if value not in values:
                raise InvalidHIR(f"unknown value {value}")
            return values[value].type
        case model.Constant(type_, _):
            return type_
        case model.PlaceRef(place):
            if place not in places:
                raise InvalidHIR(f"unknown place {place}")
            return places[place].type
        case model.ArrayElement(place, _):
            if place not in places:
                raise InvalidHIR(f"unknown place {place}")
            return places[place].type
        case model.ProjectedPlace(place, _, _, type_):
            if place not in places:
                raise InvalidHIR(f"unknown place {place}")
            return type_
        case model.IndirectPlace(base, _, type_, _):
            if base not in values:
                raise InvalidHIR(f"unknown pointer value {base}")
            return type_


def verify(program: model.Program) -> None:
    if program.schema != model.SCHEMA_VERSION:
        raise InvalidHIR(f"unsupported HIR schema {program.schema}")
    if program.target is not model.TargetProfile.I386_REAL_MODE:
        raise InvalidHIR(f"unsupported target {program.target!r}")
    module_ids: set[int] = set()
    for module in program.modules:
        if module.id in module_ids:
            raise InvalidHIR(f"duplicate module {module.id}")
        module_ids.add(module.id)
        data = {one.id: one for one in module.data}
        if len(data) != len(module.data):
            raise InvalidHIR(f"{module.name}: duplicate data object id")
        for object_ in module.data:
            if object_.linkage is model.DataLinkage.EXTERNAL and (object_.bytes or object_.relocations):
                raise InvalidHIR(f"{module.name}: external {object_.name} has an initializer")
            if any(not 0 <= byte <= 255 for byte in object_.bytes):
                raise InvalidHIR(f"{module.name}: {object_.name} has a non-byte initializer")
            for relocation in object_.relocations:
                if relocation.target not in data:
                    raise InvalidHIR(f"{module.name}: {object_.name} relocates to unknown data")
                width = 4 if relocation.address in (model.AddressKind.FAR, model.AddressKind.HUGE) else 2
                if relocation.at < 0 or relocation.at + width > len(object_.bytes):
                    raise InvalidHIR(f"{module.name}: {object_.name} relocation exceeds initializer")
        types = {one.id: one for one in module.types}
        if len(types) != len(module.types):
            raise InvalidHIR(f"{module.name}: duplicate type id")
        for type_ in module.types:
            if type_.width < 0:
                raise InvalidHIR(f"{module.name}: {type_.name} has negative width")
            if type_.kind is model.TypeKind.FLOAT and type_.evaluation is model.FloatEvaluation.NONE:
                raise InvalidHIR(f"{module.name}: float {type_.name} has no evaluation format")
            if type_.kind is model.TypeKind.ARRAY and (
                type_.element not in types
                or type_.rank < 1
                or len(type_.bounds) != type_.rank
                or any(upper < lower for lower, upper in type_.bounds)
            ):
                raise InvalidHIR(f"{module.name}: incomplete array type {type_.name}")
        callables = {one.id: one for one in module.callables}
        if len(callables) != len(module.callables):
            raise InvalidHIR(f"{module.name}: duplicate callable id")
        for callable_ in module.callables:
            if callable_.result_type is not None and callable_.result_type not in types:
                raise InvalidHIR(f"{module.name}: {callable_.name} has an unknown result type")
            count = len(callable_.parameter_types)
            if any(one not in types for one in callable_.parameter_types) or not all(
                len(one) == count for one in (callable_.by_value, callable_.segmented, callable_.arrays)
            ):
                raise InvalidHIR(f"{module.name}: {callable_.name} has an incomplete signature")
        function_ids: set[int] = set()
        for function in module.functions:
            if function.id in function_ids:
                raise InvalidHIR(f"{module.name}: duplicate function {function.id}")
            function_ids.add(function.id)
            _function(module, function, types)


def _function(module: model.Module, function: model.Function, types: dict[int, model.Type]) -> None:
    prefix = f"{module.name}.{function.name}"
    if function.result_type not in types:
        raise InvalidHIR(f"{prefix}: unknown result type {function.result_type}")
    values = {one.id: one for one in function.values}
    places = {one.id: one for one in function.places}
    data = {one.id: one for one in module.data}
    blocks = {one.id: one for one in function.blocks}
    instructions = {one.id: one for block in function.blocks for one in block.instructions}
    if len(values) != len(function.values):
        raise InvalidHIR(f"{prefix}: duplicate value id")
    if len(places) != len(function.places):
        raise InvalidHIR(f"{prefix}: duplicate place id")
    if len(blocks) != len(function.blocks):
        raise InvalidHIR(f"{prefix}: duplicate block id")
    if function.entry not in blocks:
        raise InvalidHIR(f"{prefix}: unknown entry block {function.entry}")
    if function.error_handler is not None and function.error_handler not in blocks:
        raise InvalidHIR(f"{prefix}: unknown error-handler block {function.error_handler}")
    if function.error_handler_local and function.error_handler is None:
        raise InvalidHIR(f"{prefix}: local error-handler flag without a handler")
    if any(one not in blocks for one in function.external_entries):
        raise InvalidHIR(f"{prefix}: unknown external-entry block")
    if len(set(function.external_entries)) != len(function.external_entries):
        raise InvalidHIR(f"{prefix}: duplicate external-entry block")
    if function.abi is not None and function.abi.parameter_bytes < 0:
        raise InvalidHIR(f"{prefix}: negative ABI parameter size")
    call_sites = {one.instruction: one for one in function.calls}
    if len(call_sites) != len(function.calls):
        raise InvalidHIR(f"{prefix}: duplicate call ABI")
    for site in function.calls:
        instruction = instructions.get(site.instruction)
        if instruction is None or instruction.op not in ({model.Op.CALL} | _STRING_COMPARE):
            raise InvalidHIR(f"{prefix}: ABI site {site.instruction} is not a call")
        if sorted(site.order) != list(range(len(instruction.operands))):
            raise InvalidHIR(f"{prefix}: call {site.instruction} has invalid argument order")
        if site.callee is not None and site.callee not in {one.id for one in module.callables}:
            raise InvalidHIR(f"{prefix}: call {site.instruction} names an unknown callable")
    for value in values.values():
        if value.type not in types:
            raise InvalidHIR(f"{prefix}: value {value.id} has unknown type {value.type}")
    for place in places.values():
        if place.type not in types:
            raise InvalidHIR(f"{prefix}: place {place.id} has unknown type {place.type}")
        if place.extent is None or place.extent < types[place.type].width:
            raise InvalidHIR(f"{prefix}: place {place.name} has incomplete extent")
        if place.storage not in (model.Storage.LOCAL, model.Storage.PARAMETER):
            object_ = data.get(place.symbol)
            if object_ is None:
                raise InvalidHIR(f"{prefix}: place {place.name} has no data object")
            if place.storage is model.Storage.EXTERNAL and object_.linkage is not model.DataLinkage.EXTERNAL:
                raise InvalidHIR(f"{prefix}: external place {place.name} names a definition")
            if place.storage is not model.Storage.EXTERNAL and object_.linkage is model.DataLinkage.EXTERNAL:
                raise InvalidHIR(f"{prefix}: defined place {place.name} names an external declaration")
            if object_.linkage is not model.DataLinkage.EXTERNAL and (
                place.offset < 0 or place.offset + place.extent > len(object_.bytes)
            ):
                raise InvalidHIR(f"{prefix}: place {place.name} exceeds its data object")
    defined: set[int] = set()
    for block in function.blocks:
        for instruction in block.instructions:
            expected = _RESULTS.get(instruction.op, 1)
            if expected is not None and len(instruction.results) != expected:
                raise InvalidHIR(
                    f"{prefix}: {instruction.op} has {len(instruction.results)} results, expected {expected}"
                )
            if instruction.op is model.Op.CALL and not instruction.callee:
                raise InvalidHIR(f"{prefix}: call {instruction.id} has no callee")
            if instruction.op in ({model.Op.CALL} | _STRING_COMPARE) and instruction.id not in call_sites:
                raise InvalidHIR(f"{prefix}: call {instruction.id} has no ABI site")
            if instruction.op in _STRING_COMPARE and instruction.callee != "B$SCMP":
                raise InvalidHIR(f"{prefix}: string comparison is not B$SCMP")
            if instruction.op in _PLACES and not instruction.operands:
                raise InvalidHIR(f"{prefix}: {instruction.op} {instruction.id} has no place")
            for result in instruction.results:
                if result not in values:
                    raise InvalidHIR(f"{prefix}: instruction {instruction.id} defines unknown value {result}")
                if result in defined:
                    raise InvalidHIR(f"{prefix}: value {result} is defined twice")
                defined.add(result)
            operand_types = [_operand_type(one, values, places) for one in instruction.operands]
            if any(one not in types for one in operand_types):
                raise InvalidHIR(f"{prefix}: instruction {instruction.id} has an unknown operand type")
            for index, (operand, type_id) in enumerate(zip(instruction.operands, operand_types, strict=True)):
                if isinstance(operand, model.ArrayElement):
                    element = types[type_id].element
                    if element is None:
                        raise InvalidHIR(f"{prefix}: array element has no element type")
                    operand_types[index] = element
            result_types = [values[one].type for one in instruction.results]
            if instruction.op is model.Op.COPY and (len(operand_types) != 1 or result_types != operand_types):
                raise InvalidHIR(f"{prefix}: copy changes type without a conversion")
            if instruction.op is model.Op.LOAD:
                if len(operand_types) != 1 or result_types != operand_types:
                    raise InvalidHIR(f"{prefix}: load result type does not match its place")
                if not isinstance(
                    instruction.operands[0],
                    (model.PlaceRef, model.ArrayElement, model.ProjectedPlace, model.IndirectPlace),
                ):
                    raise InvalidHIR(f"{prefix}: load operand is not a place")
            if instruction.op is model.Op.STORE:
                if len(operand_types) != 2 or operand_types[0] != operand_types[1]:
                    raise InvalidHIR(f"{prefix}: store value type does not match its place")
                if not isinstance(
                    instruction.operands[0],
                    (model.PlaceRef, model.ArrayElement, model.ProjectedPlace, model.IndirectPlace),
                ):
                    raise InvalidHIR(f"{prefix}: store destination is not a place")
            if instruction.op in _FLOAT:
                involved = [*(values[one].type for one in instruction.results), *operand_types]
                if any(types[one].kind is not model.TypeKind.FLOAT for one in involved):
                    raise InvalidHIR(f"{prefix}: {instruction.op} has a non-floating operand")
            if instruction.op in _INTEGER:
                involved = [*(values[one].type for one in instruction.results), *operand_types]
                if any(types[one].kind not in (model.TypeKind.INTEGER, model.TypeKind.BOOLEAN) for one in involved):
                    raise InvalidHIR(f"{prefix}: {instruction.op} has a non-integer operand")
            if instruction.op in (model.Op.FIXED_MUL, model.Op.FIXED_DIV):
                if len(result_types) != 1 or len(operand_types) != 3:
                    raise InvalidHIR(f"{prefix}: {instruction.op} has the wrong arity")
                value_type = types[result_types[0]]
                left_type, right_type, fraction_type = (types[one] for one in operand_types)
                fraction = instruction.operands[2]
                if (
                    value_type.kind is not model.TypeKind.INTEGER
                    or value_type.width != 4
                    or value_type.signed is not True
                    or left_type != value_type
                    or right_type != value_type
                    or fraction_type.kind is not model.TypeKind.INTEGER
                    or fraction_type.width != 1
                    or not isinstance(fraction, model.Constant)
                    or not 1 <= fraction.value < 32
                ):
                    raise InvalidHIR(f"{prefix}: {instruction.op} is not fixed i32 arithmetic")
            if instruction.op in _COMPARE:
                if len(result_types) != 1 or types[result_types[0]].kind is not model.TypeKind.BOOLEAN:
                    raise InvalidHIR(f"{prefix}: comparison does not produce a boolean")
                if len(operand_types) != 2:
                    raise InvalidHIR(f"{prefix}: comparison does not have two operands")
                if instruction.op in _STRING_COMPARE:
                    # A stored dynamic-string array element is reached through
                    # a whole array pointer, but QB's string arena is near and
                    # its runtimes receive the extracted 16-bit descriptor
                    # offset. A normal scalar supplies a typed near pointer.
                    # Both are the same runtime address form; wider operands
                    # would lose the established real-mode contract.
                    if any(types[one].width != 2 for one in operand_types):
                        raise InvalidHIR(f"{prefix}: string comparison operands are not near addresses")
                elif operand_types[0] != operand_types[1]:
                    raise InvalidHIR(f"{prefix}: comparison operand types do not agree")
            if instruction.op in _UNSIGNED:
                involved = operand_types if instruction.op in _COMPARE else [*result_types, *operand_types]
                unsigned = all(
                    types[one].kind is model.TypeKind.INTEGER and types[one].signed is False for one in involved
                )
                if not unsigned:
                    raise InvalidHIR(f"{prefix}: {instruction.op} requires unsigned integer operands")
            if instruction.op in _POINTER_PART:
                if len(operand_types) != 1 or len(result_types) != 1:
                    raise InvalidHIR(f"{prefix}: pointer projection has the wrong arity")
                pointer = types[operand_types[0]]
                result = types[result_types[0]]
                if (
                    pointer.kind is not model.TypeKind.POINTER
                    or (instruction.op is model.Op.POINTER_SEGMENT and pointer.width != 4)
                    or (instruction.op is model.Op.POINTER_OFFSET and pointer.width not in (2, 4))
                    or result.kind is not model.TypeKind.INTEGER
                    or result.width != 2
                ):
                    raise InvalidHIR(f"{prefix}: pointer projection cannot produce INTEGER")
            if instruction.op is model.Op.CONCAT:
                if len(operand_types) != 2 or len(result_types) != 1:
                    raise InvalidHIR(f"{prefix}: pointer concat has the wrong arity")
                high, low = (types[one] for one in operand_types)
                result = types[result_types[0]]
                if (
                    high.kind is not model.TypeKind.INTEGER
                    or low.kind is not model.TypeKind.INTEGER
                    or high.width != 2
                    or low.width != 2
                    or result.kind is not model.TypeKind.POINTER
                    or result.width != 4
                ):
                    raise InvalidHIR(f"{prefix}: pointer concat is not INTEGER:INTEGER to 16:16")
            for operand in instruction.operands:
                if isinstance(operand, model.ArrayElement):
                    array = types[places[operand.place].type]
                    if array.kind is not model.TypeKind.ARRAY or len(operand.indices) != array.rank:
                        raise InvalidHIR(f"{prefix}: invalid array element for place {operand.place}")
                    for index in operand.indices:
                        index_type = types[_operand_type(index, values, places)]
                        if index_type.kind is not model.TypeKind.INTEGER:
                            raise InvalidHIR(f"{prefix}: array index is not an integer")
                if isinstance(operand, model.ProjectedPlace):
                    root = types[places[operand.place].type]
                    container = types[root.element] if root.kind is model.TypeKind.ARRAY else root
                    if operand.type not in types or operand.offset < 0:
                        raise InvalidHIR(f"{prefix}: invalid projection for place {operand.place}")
                    if operand.offset + types[operand.type].width > container.width:
                        raise InvalidHIR(f"{prefix}: projection exceeds place {operand.place}")
                    expected_rank = root.rank if root.kind is model.TypeKind.ARRAY else 0
                    if len(operand.indices) != expected_rank:
                        raise InvalidHIR(f"{prefix}: invalid projection rank for place {operand.place}")
                    for index in operand.indices:
                        index_type = types[_operand_type(index, values, places)]
                        if index_type.kind is not model.TypeKind.INTEGER:
                            raise InvalidHIR(f"{prefix}: projection index is not an integer")
                if isinstance(operand, model.IndirectPlace):
                    if operand.type not in types or operand.offset < 0:
                        raise InvalidHIR(f"{prefix}: invalid indirect place")
                    pointer = types[values[operand.base].type]
                    if pointer.kind is not model.TypeKind.POINTER or pointer.element not in types:
                        raise InvalidHIR(f"{prefix}: indirect place disagrees with pointer type")
                    if operand.offset + types[operand.type].width > types[pointer.element].width:
                        raise InvalidHIR(f"{prefix}: indirect place exceeds its pointee")
        term = block.terminator
        if any(target not in blocks for target in (*term.targets, *(target for _, target in term.cases))):
            raise InvalidHIR(f"{prefix}: block {block.id} has an unknown target")
        result = types[function.result_type]
        operand_count = {
            model.TerminatorKind.JUMP: 0,
            model.TerminatorKind.BRANCH: 1,
            model.TerminatorKind.SWITCH: 1,
            model.TerminatorKind.RETURN: 0 if result.kind is model.TypeKind.VOID else 1,
            model.TerminatorKind.UNREACHABLE: 0,
        }[term.kind]
        target_count = {
            model.TerminatorKind.JUMP: 1,
            model.TerminatorKind.BRANCH: 2,
            model.TerminatorKind.SWITCH: 1,
            model.TerminatorKind.RETURN: 0,
            model.TerminatorKind.UNREACHABLE: 0,
        }[term.kind]
        if len(term.operands) != operand_count or len(term.targets) != target_count:
            raise InvalidHIR(f"{prefix}: malformed {term.kind} terminator in block {block.id}")
        for operand in term.operands:
            type_id = _operand_type(operand, values, places)
            if type_id not in types:
                raise InvalidHIR(f"{prefix}: terminator in block {block.id} has an unknown operand type")
        if (
            term.kind is model.TerminatorKind.RETURN
            and term.operands
            and _operand_type(term.operands[0], values, places) != function.result_type
        ):
            raise InvalidHIR(f"{prefix}: return value type does not match the function")
        if term.kind in (model.TerminatorKind.BRANCH, model.TerminatorKind.SWITCH):
            condition = types[_operand_type(term.operands[0], values, places)]
            if condition.kind not in (model.TypeKind.BOOLEAN, model.TypeKind.INTEGER):
                raise InvalidHIR(f"{prefix}: {term.kind} condition is not integral")
    invalid_parameters = len(set(function.parameters)) != len(function.parameters) or any(
        one not in values for one in function.parameters
    )
    if invalid_parameters:
        raise InvalidHIR(f"{prefix}: invalid parameter values")
    undefined = set(values) - defined
    if undefined != set(function.parameters):
        raise InvalidHIR(f"{prefix}: undefined values {sorted(undefined - set(function.parameters))}")

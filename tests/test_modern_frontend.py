import re
import json
from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import hir
from qbopt.model import mir
from qbopt.hir import execute
from qbopt.backend import masm
from qbopt.analysis import loops
from qbopt.analysis import induction
from qbopt.backend import lower_int64
from qbopt.model.passes import LEVELS
from qbopt.model.passes import Options
from qbopt.backend import cpu as targets
from qbopt.frontend.modern import driver
from qbopt.frontend.qb import physicalize
from qbopt.frontend.modern import compile as modern_compile

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "frontends" / "modern" / "fixtures" / "control.mod"
PRIMITIVES = ROOT / "frontends" / "modern" / "fixtures" / "primitives.mod"
NBODY = ROOT / "frontends" / "modern" / "fixtures" / "nbody.mod"
SUM = ROOT / "frontends" / "modern" / "fixtures" / "sum.mod"
SUM_THREE = ROOT / "frontends" / "modern" / "fixtures" / "sum_three.mod"
FIXED = ROOT / "frontends" / "modern" / "fixtures" / "fixed.mod"
STARTUP = ROOT / "runtime" / "modern" / "start.asm"
RUNTIME = ROOT / "runtime" / "modern" / "rt.c"


@pytest.fixture(scope="module")
def program() -> hir.Program:
    return driver.parsed(FIXTURE)


def test_frontend_document_crosses_the_strict_common_hir_boundary(program: hir.Program) -> None:
    assert program.dialect is hir.Dialect.MODERN
    assert program.runtime is hir.RuntimeProfile.FREESTANDING
    assert [function.name for function in program.modules[0].functions] == ["step", "count"]
    assert hir.decode(hir.encode(program)) == program


def test_dos_bootstrap_enters_the_runtime_before_language_main() -> None:
    """The assembly entry must not bypass runtime initialization policy."""
    startup = STARTUP.read_text()
    runtime = RUNTIME.read_text()
    assert "extrn _start:far" in startup
    assert "call far ptr _start" in startup
    assert "_main" not in startup
    assert "int start(void)" in runtime
    assert "return main();" in runtime


def test_frontend_lowers_control_flow_and_calls_to_existing_mir(program: hir.Program) -> None:
    lowered = {function.name: function.body for function in hir.lower(program)}
    count = lowered["control.count"]
    count_kinds = {operation.kind.value for block in count.blocks for operation in block.ops}
    step_kinds = {operation.kind.value for block in lowered["control.step"].blocks for operation in block.ops}
    assert {"call", "branch", "load", "store"} <= count_kinds
    assert "add" in step_kinds


def test_frontend_json_is_deterministic_and_replayable(tmp_path: Path) -> None:
    first = tmp_path / "first.json"
    second = tmp_path / "second.json"
    driver.parsed(FIXTURE, dump=first)
    driver.parsed(FIXTURE, dump=second)
    assert first.read_bytes() == second.read_bytes()
    assert json.loads(first.read_text())["schema"] == 1


def test_type_error_is_reported_above_hir(tmp_path: Path) -> None:
    source = tmp_path / "wrong.mod"
    source.write_text("fn wrong(value: i16) -> i16:\n    if value:\n        return 1\n    return 0\n")
    with pytest.raises(driver.FrontendError, match="expected bool"):
        driver.parsed(source)


def test_all_primitive_types_cross_hir_with_their_exact_representation() -> None:
    program = driver.parsed(PRIMITIVES)
    types = {one.name: one for one in program.modules[0].types}
    assert set(types) == {
        "void",
        "bool",
        "char",
        "i8",
        "u8",
        "i16",
        "u16",
        "i32",
        "u32",
        "f32",
        "f64",
        "string",
        "addr",
    }
    integral_names = ("char", "i8", "u8", "i16", "u16", "i32", "u32")
    integral = {name: (types[name].width, types[name].signed) for name in integral_names}
    assert integral == {
        "char": (1, False),
        "i8": (1, True),
        "u8": (1, False),
        "i16": (2, True),
        "u16": (2, False),
        "i32": (4, True),
        "u32": (4, False),
    }
    assert (types["bool"].width, types["void"].width) == (1, 0)
    assert types["f32"].evaluation is hir.FloatEvaluation.BINARY32
    assert types["f64"].evaluation is hir.FloatEvaluation.BINARY64
    assert [len(one.bytes) for one in program.modules[0].data] == [4, 8]


def test_unsigned_and_floating_operations_keep_their_semantics_in_mir() -> None:
    lowered = {one.name: one.body for one in hir.lower(driver.parsed(PRIMITIVES))}
    assert all(not mir.verify(body) for body in lowered.values())

    def kinds(name: str) -> set[mir.Kind]:
        return {operation.kind for block in lowered[f"primitives.{name}"].blocks for operation in block.ops}

    assert mir.Kind.UDIVMOD in kinds("unsigned_divide")
    assert mir.Kind.DIVMOD not in kinds("unsigned_divide")
    assert mir.Kind.UDIVMOD in kinds("unsigned_remainder")
    assert mir.Kind.FMUL in kinds("float_product")

    branch = next(
        operation
        for block in lowered["primitives.unsigned_less"].blocks
        for operation in block.ops
        if operation.kind is mir.Kind.BRANCH
    )
    assert branch.test is mir.Kind.BELOW


def test_fixed_point_types_scale_literals_and_keep_storage_width_in_mir() -> None:
    program = driver.parsed(FIXED)
    module = program.modules[0]
    types = {one.name: one for one in module.types}
    assert (types["fixed8"].width, types["fixed8"].signed) == (2, True)
    assert (types["fixed16"].width, types["fixed16"].signed) == (4, True)
    assert (types["$i64"].width, types["$i64"].signed) == (8, True)

    fixed_literals = next(one for one in module.functions if one.name == "fixed_literals")
    constants = [
        operand
        for block in fixed_literals.blocks
        for instruction in block.instructions
        for operand in instruction.operands
        if isinstance(operand, hir.Constant)
    ]
    assert hir.Constant(types["fixed16"].id, 98_304) in constants
    assert hir.Constant(types["fixed16"].id, 147_456) in constants

    decimal_prints = [
        instruction
        for block in fixed_literals.blocks
        for instruction in block.instructions
        if instruction.callee == "_pf4"
    ]
    assert len(decimal_prints) == 2
    value_types = {one.id: one.type for one in fixed_literals.values}
    for decimal_print in decimal_prints:
        raw, fraction = decimal_print.operands
        assert isinstance(raw, hir.ValueRef)
        assert value_types[raw.value] == types["i32"].id
        assert fraction == hir.Constant(types["u8"].id, 16)

    lowered = {one.name: one.body for one in hir.lower(program)}
    assert all(not mir.verify(body) for body in lowered.values())
    product_kinds = {operation.kind for block in lowered["fixed.product"].blocks for operation in block.ops}
    quotient_kinds = {operation.kind for block in lowered["fixed.quotient"].blocks for operation in block.ops}
    # fixed8 uses a 32-bit intermediate; fixed16 is already based on i32 and
    # stays one semantic operation until target lowering selects EDX:EAX.
    assert {mir.Kind.SIGN_EXTEND, mir.Kind.MUL, mir.Kind.SAR} <= product_kinds
    assert mir.Kind.FIXED_DIV in quotient_kinds
    assert not {mir.Kind.SIGN_EXTEND, mir.Kind.SHL, mir.Kind.DIVMOD} & quotient_kinds


def test_fixed_i32_product_stays_a_storage_width_operation_through_physicalization() -> None:
    """Native nbody used to route every Q23.9 product through generic i64 MIR."""
    program = driver.parsed(NBODY)
    function = next(one for one in program.modules[0].functions if one.name == "nbody")
    lowered = next(one for one in hir.lower(program) if one.name == "nbody.nbody")
    physical = physicalize(program, function, lowered)
    fixed = [
        operation
        for block in physical.lowered.body.blocks
        for operation in block.ops
        if operation.kind in (mir.Kind.FIXED_MUL, mir.Kind.FIXED_DIV)
    ]

    assert fixed
    assert all(operation.results[0].width == 4 for operation in fixed)
    assert all(all(argument.width <= 4 for argument in operation.args) for operation in fixed)


def test_fixed_i32_arithmetic_never_enters_generic_int64_legalization() -> None:
    """nbody's Q23.9 inner loop expanded one division to 311 inline bytes.

    A fixed i32 product needs the 386's native 32x32->64 IMUL result, and a
    scaled dividend already fits IDIV's EDX:EAX input.  Neither operation is
    an arbitrary i64 operation and neither may acquire an int64 helper blob.
    """
    program = driver.parsed(NBODY)
    function = next(one for one in program.modules[0].functions if one.name == "nbody")
    lowered = next(one for one in hir.lower(program) if one.name == "nbody.nbody")
    physical = physicalize(program, function, lowered)
    legalized = lower_int64.expanded(
        physical.lowered.body,
        physical.calls,
        physical.contracts,
        physical.hints,
    )

    assert legalized.inline == {}


def test_nbody_arrays_strings_and_print_cross_hir_and_verify_in_mir() -> None:
    program = driver.parsed(NBODY)
    module = program.modules[0]
    types = {one.name: one for one in module.types}
    scalar = types["scalar"]
    assert (scalar.kind, scalar.width, scalar.signed) == (hir.TypeKind.INTEGER, 4, True)
    vec2i = types["vec2i"]
    assert (vec2i.kind, vec2i.width) == (hir.TypeKind.OPAQUE, 8)
    body = types["body"]
    assert (body.kind, body.width) == (hir.TypeKind.OPAQUE, 16)
    array = types["[body; 6]"]
    assert (array.element, array.rank, array.bounds, array.width) == (body.id, 1, ((0, 5),), 96)

    string = types["string"]
    assert (string.element, string.width, string.address) == (
        types["char"].id,
        2,
        hir.AddressKind.NEAR,
    )
    for literal in module.data:
        length = literal.bytes[0] | literal.bytes[1] << 8
        capacity = literal.bytes[2] | literal.bytes[3] << 8
        assert length == capacity == len(literal.bytes) - 5
        assert literal.bytes[-1] == 0

    callables = {one.name: one for one in module.callables}
    assert callables["_pt"].defined is False
    assert callables["_pf4"].defined is False
    assert callables["_pf4"].parameter_types == (types["i32"].id, types["u8"].id)
    assert callables["_pn"].defined is False
    assert all(call.distance is hir.CallDistance.FAR for call in module.functions[0].calls)
    fixed_id = callables["_pf4"].id
    assert all(call.order == (1, 0) for call in module.functions[0].calls if call.callee == fixed_id)

    lowered = next(one for one in hir.lower(program) if one.name == "nbody.nbody")
    assert not mir.verify(lowered.body)
    kinds = {operation.kind for block in lowered.body.blocks for operation in block.ops}
    assert {
        mir.Kind.ADDRESS,
        mir.Kind.BRANCH,
        mir.Kind.CALL,
        mir.Kind.FIXED_DIV,
        mir.Kind.FIXED_MUL,
        mir.Kind.LOAD,
        mir.Kind.MUL,
        mir.Kind.STORE,
    } <= kinds

    operands = [
        operand
        for block in module.functions[0].blocks
        for instruction in block.instructions
        for operand in instruction.operands
    ]
    projections = [operand for operand in operands if isinstance(operand, hir.ProjectedPlace)]
    assert projections
    assert {one.offset for one in projections} == {0, 4, 8, 12}
    assert any(
        instruction.op is hir.Op.NE for block in module.functions[0].blocks for instruction in block.instructions
    )
    range_counter = next(place for place in module.functions[0].places if place.name == "$range_step_no")
    assert range_counter.type == types["i32"].id


def test_nbody_string_places_point_after_the_descriptor() -> None:
    function = driver.parsed(NBODY).modules[0].functions[0]
    strings = [place for place in function.places if place.name.startswith("$string")]
    assert strings
    assert all(place.offset == 4 for place in strings)


def test_nbody_native_loops_eliminate_redundant_index_arithmetic() -> None:
    """Modern nbody emitted 52 `sub index,0; shl index,4` address chains."""
    assembly = masm.text(modern_compile.assembled(driver.parsed(NBODY), entry="main"))

    assert "sub si, 0" not in assembly
    assert "sub di, 0" not in assembly
    scaled_indices = assembly.count("shl si, 4") + assembly.count("shl di, 4")
    assert scaled_indices <= 2


def test_nbody_position_loop_uses_one_end_relative_byte_offset() -> None:
    """nbody updated six bodies with an index, `index << 4`, two address
    temporaries, and a separate compare.  One -96,+16 byte recurrence can
    address the fields and terminate on the step's own zero flag.
    """
    # -O2 unrolls the loop away.
    assembly = masm.text(modern_compile.assembled(driver.parsed(NBODY), entry="main", options=LEVELS["Os"]))
    loop = assembly.split("L0_18:\n", 1)[1].split("    jne L0_18\n", 1)[0]

    assert "mov si, ax" not in loop
    assert "shl si, 4" not in loop
    assert "lea di" not in loop
    assert "cmp ax, 6" not in loop
    assert "mov si, 65440\nL0_18:" in assembly  # -96 in a word
    assert re.search(
        r"    mov (?P<x>e(?:ax|bx|cx|dx|si|di)), dword ptr \[bp\+si\+8\]\n"
        r"    add dword ptr \[bp\+si\], (?P=x)\n",
        loop,
    )
    assert re.search(
        r"    mov (?P<y>e(?:ax|bx|cx|dx|si|di)), dword ptr \[bp\+si\+12\]\n"
        r"    add dword ptr \[bp\+si\+4\], (?P=y)\n",
        loop,
    )
    assert "add si, 16\nL0_17:\n" in loop
    assert "or si, si" not in loop
    assert "cmp si" not in loop


def test_nbody_identity_uses_the_paired_byte_recurrences() -> None:
    """nbody carried scalar current/other indices beside two `index * 16`
    address chains because their identity comparison hid that both byte
    offsets are the same injective encoding of those indices.
    """
    # -O2 unrolls the loop away.
    assembly = masm.text(modern_compile.assembled(driver.parsed(NBODY), entry="main", options=LEVELS["Os"]))
    interaction = assembly.split("L0_7:\n", 1)[1].split("L0_2:\n", 1)[0]
    force_loops = assembly.split("L0_3:\n", 1)[1].split("L0_9:\n", 1)[0]

    assert re.search(r"    shl (?:[sd]i|word ptr \[[^]]+\]), 4\n", interaction) is None
    assert ", 96\n" not in interaction
    recurrences = re.findall(
        r"    add (?:[sd]i|word ptr \[[^]]+\]), 16\nL\d+_\d+:\n    jne L\d+_\d+\n",
        force_loops,
    )
    assert len(recurrences) == 2
    assert len(re.findall(r"    mov (?:[sd]i|word ptr \[[^]]+\]), 65440\n", force_loops)) == 2


def test_nbody_velocity_fields_are_stored_once_per_update() -> None:
    """nbody stored vel.x after `+= acc.x`, stored vel.y, then reloaded vel.x for `-= vel.x / 16`.

    Both fields are displacements off one base, so the store at +12 cannot
    reach +8: the first store is dead and the value stays in a register.
    """
    assembly = masm.text(modern_compile.assembled(driver.parsed(NBODY), entry="main"))
    interaction = assembly.split("L0_7:\n", 1)[1].split("L0_2:\n", 1)[0]

    for field in (8, 12):
        assert len(re.findall(rf"dword ptr \[bp\+[sd]i\+{field}\], e(?:ax|bx|cx|dx|si|di)\n", interaction)) == 1
        assert len(re.findall(rf"e(?:ax|bx|cx|dx|si|di), dword ptr \[bp\+[sd]i\+{field}\]\n", interaction)) == 1


def test_counted_struct_loop_uses_its_record_width_as_the_byte_stride(tmp_path: Path) -> None:
    """The end-relative recurrence is an affine-loop rule, not a body/16 rule."""
    source = tmp_path / "stride.mod"
    source.write_text(
        """\
struct sample:
    tag: i16
    value: i32
    delta: i32

fn update() -> i32:
    var samples: [sample; 5] = [
        sample { tag: 0, value: 1, delta: 2 },
        sample { tag: 0, value: 2, delta: 3 },
        sample { tag: 0, value: 3, delta: 4 },
        sample { tag: 0, value: 4, delta: 5 },
        sample { tag: 0, value: 5, delta: 6 },
    ]
    for current in &mut samples:
        current.value += current.delta
    return samples[0].value + samples[4].value

fn main() -> i16:
    update()
    return 0
"""
    )

    # -O2 unrolls the loop away.
    assembly = masm.text(modern_compile.assembled(driver.parsed(source), entry="main", options=LEVELS["Os"]))

    loop = re.search(
        r"    mov (?P<offset>[sd]i), 65486\n"
        r"(?P<label>L\d+_\d+):\n"
        r"(?P<body>(?:    .*\n)+?)"
        r"    add (?P=offset), 10\n"
        r"L\d+_\d+:\n"
        r"    jne (?P=label)\n",
        assembly,
    )
    assert loop is not None  # -5 * sizeof(sample), with sizeof(sample) == 10
    offset = loop.group("offset")
    assert f"dword ptr [bp+{offset}+2]" in loop.group("body")
    assert f"dword ptr [bp+{offset}+6]" in loop.group("body")
    assert f"shl {offset}" not in assembly


def test_os_copies_no_loop_into_larger_code(tmp_path: Path) -> None:
    """-O2 unrolls the five-record update from 54 instructions to 81.

    -Os is GCC's UL_NO_GROWTH: never larger than not copying at all.
    """
    source = tmp_path / "stride.mod"
    source.write_text(
        """\
struct sample:
    tag: i16
    value: i32
    delta: i32

fn update() -> i32:
    var samples: [sample; 5] = [
        sample { tag: 0, value: 1, delta: 2 },
        sample { tag: 0, value: 2, delta: 3 },
        sample { tag: 0, value: 3, delta: 4 },
        sample { tag: 0, value: 4, delta: 5 },
        sample { tag: 0, value: 5, delta: 6 },
    ]
    for current in &mut samples:
        current.value += current.delta
    return samples[0].value + samples[4].value

fn main() -> i16:
    update()
    return 0
"""
    )

    def size(options: Options) -> int:
        text = masm.text(modern_compile.assembled(driver.parsed(source), entry="main", options=options))
        return sum(line.startswith("    ") for line in text.splitlines())

    uncopied = size(Options(unroll=False, peel=False))
    assert size(LEVELS["O2"]) > uncopied
    assert size(LEVELS["Os"]) <= uncopied


def test_fixed_array_storage_has_a_prefix_descriptor(tmp_path: Path) -> None:
    """A local fixed array must reserve and initialize length/capacity/stride before its payload."""
    source = tmp_path / "array_descriptor.mod"
    source.write_text(
        "fn main() -> i16:\n    var values: [i16; 3] = [10, 20, 30]\n    print(values.len())\n    return values[0]\n"
    )

    function = driver.parsed(source).modules[0].functions[0]
    values = next(place for place in function.places if place.name == "values")
    descriptor = {place.name: place for place in function.places if place.name.startswith("$values.")}
    assert values.offset == -6
    assert values.extent == 6
    assert descriptor["$values.length"].offset == values.offset - 6
    assert descriptor["$values.capacity"].offset == values.offset - 4
    assert descriptor["$values.stride0"].offset == values.offset - 2

    assembly = masm.text(modern_compile.assembled(driver.parsed(source), entry="main"))
    assert "mov word ptr [bp-12], 3" in assembly
    assert "mov word ptr [bp-10], 3" in assembly
    assert "mov word ptr [bp-8], 1" in assembly


def test_borrowed_array_call_builds_one_view_from_the_direct_payload(tmp_path: Path) -> None:
    """A slice view carries one direct payload pointer without a hidden length argument."""
    source = tmp_path / "array_borrow.mod"
    source.write_text(
        "fn bump(values: &mut [u16]) -> void:\n"
        "    values[1] += 3\n"
        "fn main() -> i16:\n"
        "    var values: [u16; 3] = [10, 20, 30]\n"
        "    bump(&mut values)\n"
        "    return 0\n"
    )

    program = driver.parsed(source)
    caller = next(function for function in program.modules[0].functions if function.name == "main")
    values = next(place for place in caller.places if place.name == "values")
    addresses = [
        instruction for block in caller.blocks for instruction in block.instructions if instruction.op is hir.Op.ADDRESS
    ]
    payload_address = next(one for one in addresses if one.operands == (hir.PlaceRef(values.id),))
    view = next(place for place in caller.places if place.name == "$slice_values")
    view_address = next(one for one in addresses if one.operands == (hir.PlaceRef(view.id),))
    call = next(
        instruction
        for block in caller.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL and instruction.callee == "bump"
    )
    assert call.operands == (hir.ValueRef(view_address.results[0]),)
    pointer = next(value for value in caller.values if value.id == payload_address.results[0])
    pointer_type = next(type_ for type_ in program.modules[0].types if type_.id == pointer.type)
    assert pointer_type.width == 4
    assert pointer_type.address is hir.AddressKind.FAR

    assembly = masm.text(modern_compile.assembled(program, entry="main"))
    bump = assembly.split("_bump proc far", 1)[1].split("_bump endp", 1)[0]
    main = assembly.split("_main proc far", 1)[1].split("_main endp", 1)[0]
    assert re.search(r"    lea [a-z]+, (?:word ptr )?\[bp-6\]\n", main)
    assert re.search(r"    mov [a-z]+, ss\n", main)
    assert "call far ptr _bump" in main
    assert "add sp, 4" in main
    assert "es:[" in bump
    assert modern_compile.written(program, entry="main", source=source)


def test_borrow_rules_reject_shared_mutation_and_aliasing_mutable_arguments(tmp_path: Path) -> None:
    shared = tmp_path / "shared.mod"
    shared.write_text("fn bad(values: &[u16]) -> void:\n    values[0] = 2\n")
    with pytest.raises(driver.FrontendError, match="immutable"):
        driver.parsed(shared)

    aliased = tmp_path / "aliased.mod"
    aliased.write_text(
        "fn use(left: &mut [u16], right: &[u16]) -> void:\n"
        "    left[0] += right[0]\n"
        "fn bad() -> void:\n"
        "    var values: [u16; 1] = [1]\n"
        "    use(&mut values, &values)\n"
    )
    with pytest.raises(driver.FrontendError, match="aliases a mutable argument"):
        driver.parsed(aliased)


def test_readonly_array_borrow_keeps_payload_initialization_visible_to_callee() -> None:
    """sum returned stack garbage after DSE erased every payload store before its read-only call."""
    assembly = masm.text(modern_compile.assembled(driver.parsed(SUM), entry="main"))
    main = assembly.split("_main proc far", 1)[1].split("_main endp", 1)[0]

    assert all(f", {value}" in main for value in range(1, 7))


def test_array_parameter_is_one_unsized_view_pointer() -> None:
    """A slice passes one descriptor pointer, never a pointer-plus-length pair."""
    program = driver.parsed(SUM)
    module = program.modules[0]
    function = next(one for one in module.functions if one.name == "sum")
    pointer = next(one for one in module.types if one.id == function.values[0].type)
    descriptor = next(one for one in module.types if one.id == pointer.element)
    element = next(one for one in module.types if one.id == descriptor.element)
    metadata = next(one for one in module.types if one.name == "u16")
    descriptor_loads = [
        instruction.operands[0]
        for block in function.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.LOAD and isinstance(instruction.operands[0], hir.DescriptorPlace)
    ]

    assert len(function.parameters) == 1
    assert pointer.kind is hir.TypeKind.POINTER
    assert pointer.rank == 1
    assert (descriptor.kind, descriptor.width) == (hir.TypeKind.OPAQUE, 10)
    assert element.name == "i16"
    assert descriptor_loads == [hir.DescriptorPlace(function.parameters[0], hir.DescriptorField.LENGTH, metadata.id)]


def test_runtime_bounded_array_loop_advances_its_payload_address() -> None:
    """sum rebuilt ``payload + index * stride * 2`` on every trip; the byte stride is scaled once."""
    assembly = masm.text(modern_compile.assembled(driver.parsed(SUM), entry="main"))
    function = assembly.split("_sum proc far", 1)[1].split("_sum endp", 1)[0]
    hot = function.split("L0_3:", 1)[1].split("L0_5:", 1)[0]

    assert not re.search(r"\b(?:imul|shl|lea)\b", hot)
    assert re.search(r"\badd\s+(?:si|di|bx),\s*(?:ax|bx|cx|dx|si|di)\b", hot)


def test_three_array_initializer_keeps_the_fixed_frame_address_component() -> None:
    """sum_three wrote locals through EAX+SI after a secondary-base rewrite lost BP."""
    assembly = masm.text(modern_compile.assembled(driver.parsed(SUM_THREE), entry="main"))
    main = assembly.split("_main proc far", 1)[1].split("call far ptr _sum_three", 1)[0]
    initializers = [
        line.strip() for line in main.splitlines() if re.search(r"mov word ptr \[[^]]+-(?:8|22|36)\],", line)
    ]

    assert len(initializers) == 12
    assert all("bp" in line for line in initializers)


def test_runtime_bounded_array_loop_has_a_symbolic_count_proof() -> None:
    """A runtime descriptor extent is an exact trip count, not an unknown loop."""
    program = driver.parsed(SUM)
    function = next(one for one in program.modules[0].functions if one.name == "sum")
    semantic = next(one for one in modern_compile.semantic_lowered(program) if one.name == "sum.sum")
    length = next(
        value
        for block in semantic.body.blocks
        for op in block.ops
        for value in op.defines
        if value in semantic.body.integer_ranges
    )
    assert semantic.body.integer_ranges[length] == mir.IntegerRange(0, 32768, 2)

    target = targets.profile("386")
    optimized = modern_compile.optimized(program, function, semantic, target)
    physical = physicalize(program, function, optimized)
    body = modern_compile.optimized(program, function, physical.lowered, target, physical.calls).body
    (loop,) = loops.loops(body.blocks, body.entry)
    steps = [one.step for one in induction.basics(body, loop).values()]
    assert mir.Const(1, 2) in steps
    assert any(isinstance(one, mir.Held) for one in steps)  # the offset steps by the view's byte stride

    predecessors = loops.predecessors(body.blocks)
    assert all(set(phi.incoming) == set(predecessors[block.at]) for block in body.blocks for phi in block.phis)


def test_a_view_offset_never_replaces_the_loop_counter() -> None:
    """A view's stride is read at run time and may be 0, so its offset cannot count the loop,
    however short the loop's proven bound; the stride-two offset once did."""
    program = driver.parsed(SUM)
    function = next(one for one in program.modules[0].functions if one.name == "sum")
    semantic = next(one for one in modern_compile.semantic_lowered(program) if one.name == "sum.sum")
    (length,) = semantic.body.integer_ranges
    unsafe = replace(
        semantic,
        body=replace(semantic.body, integer_ranges={length: mir.IntegerRange(0, 4, 2)}),
    )
    target = targets.profile("386")
    optimized = modern_compile.optimized(program, function, unsafe, target)
    physical = physicalize(program, function, optimized)
    body = modern_compile.optimized(program, function, physical.lowered, target, physical.calls).body
    (loop,) = loops.loops(body.blocks, body.entry)

    steps = sorted(one.step.n for one in induction.basics(body, loop).values() if isinstance(one.step, mir.Const))
    assert steps == [1]


def test_borrowed_array_parameter_rejects_a_repeated_fixed_length(tmp_path: Path) -> None:
    source = tmp_path / "sized_parameter.mod"
    source.write_text("fn old(values: &[u16; 3]) -> void:\n    return\n")

    with pytest.raises(driver.FrontendError, match="omit the length"):
        driver.parsed(source)


def test_scoped_array_range_is_one_descriptor_pointer_and_executes(tmp_path: Path) -> None:
    """An interior range cannot borrow the owner's prefix as its own descriptor."""
    source = tmp_path / "slice.mod"
    source.write_text(
        "fn sum(values: &[i16]) -> i16:\n"
        "    var total: i16 = 0\n"
        "    for value in &values:\n"
        "        total += value\n"
        "    return total\n"
        "fn main() -> i16:\n"
        "    let values: [i16; 4] = [1, 2, 3, 4]\n"
        "    return sum(&values[1:3])\n"
    )

    program = driver.parsed(source)
    function = next(one for one in program.modules[0].functions if one.name == "sum")
    pointer = next(one for one in program.modules[0].types if one.id == function.values[0].type)

    assert len(function.parameters) == 1
    assert pointer.kind is hir.TypeKind.POINTER
    assert pointer.width == 4
    assert pointer.rank == 1
    assert execute.run(program, "main").value == 5


def test_scoped_range_iteration_uses_only_the_selected_elements(tmp_path: Path) -> None:
    source = tmp_path / "slice_loop.mod"
    source.write_text(
        "fn main() -> i16:\n"
        "    let values: [i16; 5] = [1, 2, 4, 8, 16]\n"
        "    var total: i16 = 0\n"
        "    for value in &values[1:4]:\n"
        "        total += value\n"
        "    return total\n"
    )

    assert execute.run(driver.parsed(source), "main").value == 14


@pytest.mark.parametrize(("chosen", "total"), [("1:3", 5), ("1:", 9), (":3", 6), (":", 10)])
def test_a_slice_is_spelled_as_in_python(tmp_path: Path, chosen: str, total: int) -> None:
    """Slices were `a..b`; the language spells them `a:b`, `a:`, `:b` and `:`."""
    source = tmp_path / "python_slice.mod"
    source.write_text(
        "fn sum(values: &[i16]) -> i16:\n"
        "    var total: i16 = 0\n"
        "    for value in &values:\n"
        "        total += value\n"
        "    return total\n"
        "fn main() -> i16:\n"
        "    let values: [i16; 4] = [1, 2, 3, 4]\n"
        f"    return sum(&values[{chosen}])\n"
    )

    assert execute.run(driver.parsed(source), "main").value == total


def test_a_range_is_not_a_slice(tmp_path: Path) -> None:
    source = tmp_path / "range_slice.mod"
    source.write_text(
        "fn sum(values: &[i16]) -> i16:\n"
        "    return values[0]\n"
        "fn main() -> i16:\n"
        "    let values: [i16; 4] = [1, 2, 3, 4]\n"
        "    return sum(&values[1..3])\n"
    )

    with pytest.raises(driver.FrontendError):
        driver.parsed(source)


def test_data_is_an_explicit_pointer_escape_hatch(tmp_path: Path) -> None:
    source = tmp_path / "data.mod"
    source.write_text(
        "fn data(values: &[i16]) -> addr:\n"
        "    return values.data()\n"
        "fn main() -> i16:\n"
        "    let values: [i16; 2] = [4, 9]\n"
        "    data(&values)\n"
        "    return 0\n"
    )

    program = driver.parsed(source)
    types = {one.name: one for one in program.modules[0].types}
    assert types["addr"].kind is hir.TypeKind.POINTER
    assert (types["addr"].width, types["addr"].address) == (4, hir.AddressKind.FAR)
    assert modern_compile.written(program, entry="main", source=source)


def test_string_descriptor_methods_and_value_iteration_need_no_runtime(tmp_path: Path) -> None:
    source = tmp_path / "string_view.mod"
    source.write_text(
        "fn first(text: string) -> char:\n"
        "    for byte in text:\n"
        "        return byte\n"
        "    return '\\0'\n"
        "fn size(text: string) -> u16:\n"
        "    return text.len() + text.capacity()\n"
        "fn main() -> u16:\n"
        '    let text: string = "abc"\n'
        "    if first(text) == 'a':\n"
        "        return size(text)\n"
        "    return 0\n"
    )

    program = driver.parsed(source)
    assert execute.run(program, "main").value == 6
    assert all(callable_.name not in {"len", "capacity", "iter", "next"} for callable_ in program.modules[0].callables)


def test_return_inside_sequence_iteration_reaches_object_generation(tmp_path: Path) -> None:
    """`first` once left a dead increment block with non-dominating SSA values."""
    source = tmp_path / "first.mod"
    source.write_text(
        "fn first(text: string) -> char:\n"
        "    for byte in text:\n"
        "        return byte\n"
        "    return '\\0'\n"
        "fn main() -> i16:\n"
        "    if first(\"metal\") == 'm':\n"
        '        print("ok")\n'
        "        return 0\n"
        "    return 1\n"
    )

    program = driver.parsed(source)
    assert execute.run(program, "main").output == "ok\n"
    assert modern_compile.written(program, entry="main", source=source)


def test_bounded_comprehension_materializes_and_generator_fuses(tmp_path: Path) -> None:
    source = tmp_path / "comprehension.mod"
    source.write_text(
        "fn main() -> i16:\n"
        "    let values: [i16; 4] = [1, 2, 3, 4]\n"
        "    let doubled = [value * 2 for value in values]\n"
        "    var total: i16 = 0\n"
        "    for value in (item + 1 for item in doubled):\n"
        "        total += value\n"
        "    return total\n"
    )

    program = driver.parsed(source)
    assert execute.run(program, "main").value == 24
    call_names = {one.name for one in program.modules[0].callables}
    assert not {"iter", "next", "collect", "append"} & call_names
    assert modern_compile.written(program, entry="main", source=source)


def test_dictionary_comprehension_deduplicates_and_has_explicit_lookup(tmp_path: Path) -> None:
    source = tmp_path / "dictionary.mod"
    source.write_text(
        "fn main() -> i16:\n"
        "    let values: [i16; 4] = [1, 2, 1, 3]\n"
        "    let table = {item: item * 10 for item in values}\n"
        "    return table.get(1, 0) + table.get(3, 0) + table.get(9, 5)\n"
        "fn count() -> u16:\n"
        "    let values: [i16; 4] = [1, 2, 1, 3]\n"
        "    let table = {item: item * 10 for item in values}\n"
        "    return table.len()\n"
    )

    program = driver.parsed(source)
    assert execute.run(program, "main").value == 45
    assert execute.run(program, "count").value == 3
    assert modern_compile.written(program, entry="main", source=source)


def test_a_rejected_loop_copy_is_not_rebuilt_in_a_later_round(monkeypatch: pytest.MonkeyPatch) -> None:
    """nbody rebuilt and re-priced the same rejected unroll every fixed-point round."""
    from collections import Counter

    from qbopt.optimize import unroll

    real = unroll._rejection
    rejected: Counter = Counter()

    def recording(before, after, latch, count, where):
        why = real(before, after, latch, count, where)
        if why is not None:
            rejected[unroll._signature(before, latch, count, where)] += 1
        return why

    monkeypatch.setattr(unroll, "_rejection", recording)
    modern_compile.assembled(driver.parsed(NBODY), entry="main")

    assert rejected
    assert max(rejected.values()) == 1


def test_fixed_point_arithmetic_has_a_price(tmp_path: Path) -> None:
    """Unpriced FIXED_MUL left nbody unpriceable, so every loop copy was built only to be refused."""
    from qbopt import hir
    from qbopt.backend import cpu
    from qbopt.optimize import profit

    source = tmp_path / "fixed.mod"
    source.write_text(
        "type fixed16 = fixed i32, fraction=16\n\n"
        "fn scaled(left: fixed16, right: fixed16) -> fixed16:\n"
        "    return left * right / right\n\n"
        "fn main() -> i16:\n"
        "    scaled(1.5, 2.25)\n"
        "    return 0\n"
    )
    costs = cpu.profile("386").operations
    bodies = [one.body for one in hir.lower(driver.parsed(source))]
    kinds = {op.kind for body in bodies for block in body.blocks for op in block.ops}

    assert {mir.Kind.FIXED_MUL, mir.Kind.FIXED_DIV} <= kinds
    assert all(profit.static(body, costs) is not None for body in bodies)


def _returned(tmp_path: Path, text: str, entry: str = "value") -> object:
    source = tmp_path / "program.mod"
    source.write_text(text)
    return execute.run(driver.parsed(source), entry).value


@pytest.mark.parametrize(
    ("type_", "expression", "expected"),
    [
        ("i16", "6 & 3 | 8 ^ 1", 11),
        ("i16", "1 + 2 << 3", 24),
        ("i16", "~5", -6),
        ("i16", "1 << 15", -32768),
        ("i16", "-16 >> 2", -4),
        ("u16", "u16(65520) >> 4", 4095),
        ("i16", "i16(not 1 == 2)", 1),
        ("i16", "i16(true or false and false)", 1),
        ("i32", "i32(i16(-2))", -2),
        ("u32", "u32(u16(65535))", 65535),
        ("i8", "i8(i16(300))", 44),
        ("i16", "i16(-7.9)", -7),
        ("i16", "i16(true) + i16(false)", 1),
        ("u8", "u8('A')", 65),
        ("f32", "f32(3) / f32(2)", 1.5),
    ],
)
def test_operators_bind_and_conversions_convert_as_the_spec_says(
    tmp_path: Path, type_: str, expression: str, expected: object
) -> None:
    assert _returned(tmp_path, f"fn value() -> {type_}:\n    return {expression}\n") == expected


def test_and_or_evaluate_their_right_operand_only_when_it_decides(tmp_path: Path) -> None:
    text = (
        "fn boom(zero: i16) -> bool:\n"
        "    return 1 / zero == 0\n"
        "fn value() -> i16:\n"
        "    let zero: i16 = 0\n"
        "    return i16(false and boom(zero)) + i16(true or boom(zero))\n"
    )
    assert _returned(tmp_path, text) == 1


def test_every_bitwise_operator_has_a_compound_assignment(tmp_path: Path) -> None:
    text = (
        "fn value() -> u16:\n"
        "    var x: u16 = 1\n"
        "    x <<= 4\n"
        "    x |= 3\n"
        "    x ^= 1\n"
        "    x &= 255\n"
        "    x >>= 1\n"
        "    return x\n"
    )
    assert _returned(tmp_path, text) == 9


def test_a_repeat_literal_fills_a_fixed_array(tmp_path: Path) -> None:
    text = (
        "fn value() -> i32:\n"
        "    var a: [i32; 5] = [7; 5]\n"
        "    a[2] = 1\n"
        "    var total: i32 = 0\n"
        "    for item in a:\n"
        "        total += item\n"
        "    return total\n"
    )
    assert _returned(tmp_path, text) == 29


def test_a_repeat_literal_in_the_frame_is_one_string_fill(tmp_path: Path) -> None:
    """The fill loop stepped its byte address to zero under `!=`, which `fill` missed: 64 stores in a loop."""
    source = tmp_path / "frame_fill.mod"
    source.write_text(
        "fn value(k: i16) -> i32:\n"
        "    var a: [i32; 64] = [0; 64]\n"
        "    a[k] = 5\n"
        "    return a[k] + a[k + 1]\n"
        "fn main() -> i16:\n"
        "    return i16(value(3))\n"
    )
    assembly = masm.text(modern_compile.assembled(driver.parsed(source), entry="main"))
    body = assembly[assembly.index("_value proc") : assembly.index("_value endp")]

    assert "rep stosd" in body
    assert not re.search(r"\bj\w+\s", body)


@pytest.mark.parametrize(
    "body",
    [
        "    return i16(1 < 2 < 3)\n",
        "    return 1 << 16\n",
        "    return 1 << -1\n",
        "    let a: i16 = 1\n    let b: u16 = 1\n    return i16(a + b)\n",
        "    let a: i8 = -1\n    let b: u16 = 1\n    return i16(a < b)\n",
        "    return i16(bool(1))\n",
        "    return i16(u8(300))\n",
        "    return i16(f64(1) & f64(2))\n",
        "    let a: [i16; 4] = [0; 3]\n    return a[0]\n",
    ],
)
def test_ill_formed_operators_conversions_and_repeats_are_rejected(tmp_path: Path, body: str) -> None:
    source = tmp_path / "rejected.mod"
    source.write_text("fn value() -> i16:\n" + body)
    with pytest.raises(driver.FrontendError):
        driver.parsed(source)


def test_a_repeat_value_reads_the_names_outside_the_new_binding(tmp_path: Path) -> None:
    """The fill bound the new array first, so `[a[0]; 3]` read the uninitialized new `a`."""
    text = (
        "fn value() -> i16:\n"
        "    let a: [i16; 2] = [5, 6]\n"
        "    if true:\n"
        "        let a: [i16; 3] = [a[0]; 3]\n"
        "        return a[0] + a[2]\n"
        "    return 0\n"
    )
    assert _returned(tmp_path, text) == 10


def test_not_is_not_an_operand_of_a_tighter_operator(tmp_path: Path) -> None:
    """`a + not b == c` parsed as `a + (not (b == c))`; Python rejects it, and so does the spec's ladder."""
    source = tmp_path / "not.mod"
    source.write_text("fn value() -> bool:\n    return true == not false\n")
    with pytest.raises(driver.FrontendError):
        driver.parsed(source)


@pytest.mark.parametrize(
    ("type_", "body", "expected"),
    [
        ("i16", "    let a: u8 = 200\n    let b: u8 = 100\n    return a + b\n", 300),
        ("u8", "    let a: i16 = 300\n    return a\n", 44),
        ("i16", "    let a: u8 = 5\n    return -a\n", -5),
        ("i16", "    let a: i16 = -1\n    let b: u32 = 1\n    return i16(a < b)\n", 0),
        ("f32", "    let a: i8 = 3\n    let f: f32 = 0.5\n    return a * f\n", 1.5),
        ("i32", "    let a: i16 = 1\n    return a + 40000\n", 40001),
        ("u8", "    var x: u8 = 250\n    x += 10\n    return x\n", 4),
        ("i16", "    return half(7)\n", 3),
        ("i16", "    let a: i8 = -1\n    let b: u8 = 1\n    return a + b\n", 0),
        ("u16", "    let a: u8 = 1\n    let b: u16 = 65535\n    return a + b\n", 0),
        ("i32", "    let n: u32 = 15\n    return 1 << n\n", -32768),
        ("i16", "    let n: u8 = 3\n    var t: i16 = 0\n    for i in 0..n + n:\n        t += i\n    return t\n", 15),
        (
            "i16",
            "    let k: i16 = 1\n    let n: u8 = 4\n    var t: i16 = 0\n    for i in k..n:\n        t += i\n    return t\n",
            6,
        ),
    ],
)
def test_integers_and_floats_convert_implicitly_as_in_c(
    tmp_path: Path, type_: str, body: str, expected: object
) -> None:
    """Every one of these was a type mismatch: operands and destinations had to agree exactly."""
    text = f"fn half(x: f64) -> f64:\n    return x / 2\nfn value() -> {type_}:\n" + body
    assert _returned(tmp_path, text) == expected


@pytest.mark.parametrize(
    ("type_", "expression", "expected"),
    [
        ("i16", "i16(fix(3)) + i16(fix(2.75))", 5),
        ("i16", "i16(fix(-2.75))", -2),
        ("i32", "i32(fix(-0.5))", 0),
        ("i16", "i16(small(fix(5.5) - fix(2.75)) * 4)", 11),
        ("i16", "i16(fix(small(7.9375)) * 16)", 127),
        ("i16", "i16(fix(u8(200)))", 200),
    ],
)
def test_fixed_point_converts_explicitly_toward_zero(
    tmp_path: Path, type_: str, expression: str, expected: int
) -> None:
    """`fix(n)` was a call to an unknown function: fixed types had no conversions."""
    text = (
        "type fix = fixed i32, fraction=8\n"
        "type small = fixed i16, fraction=4\n"
        f"fn value() -> {type_}:\n    return {expression}\n"
    )
    assert _returned(tmp_path, text) == expected


def test_a_float_does_not_convert_to_fixed_point(tmp_path: Path) -> None:
    source = tmp_path / "float_fixed.mod"
    source.write_text(
        "type fix = fixed i32, fraction=8\nfn value() -> i16:\n    let x: f64 = 1.5\n    return i16(fix(x))\n"
    )
    with pytest.raises(driver.FrontendError):
        driver.parsed(source)


def test_ranked_arrays_index_fill_and_borrow_row_major() -> None:
    """Only rank one existed; `[T; 3, 3]`, `a[i, j]` and `&[T, 2]` were rejected."""
    program = driver.parsed(ROOT / "frontends" / "modern" / "fixtures" / "ranked.mod")
    assert program.array_order is hir.ArrayOrder.ROW_MAJOR
    assert execute.run(program, "main").output == "15 106 162 42 9 3 4\n"


@pytest.mark.parametrize(
    "body",
    [
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    return a[0]\n",
        "    let a: [i16; 2, 2, 2, 2, 2] = [0; 2, 2, 2, 2, 2]\n    return 0\n",
        "    let a: [i16; 2, 2] = [[1, 2], [3]]\n    return 0\n",
        "    let a: [i16; 2, 2] = [0; 2, 3]\n    return 0\n",
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    return a.dim(2)\n",
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    return first(&a)\n",
        "    let a: [i16; 2, 2] = [0; 2, 2]\n    var t: i16 = 0\n    for x in a:\n        t += x\n    return t\n",
    ],
)
def test_ranked_arrays_reject_the_wrong_rank_or_shape(tmp_path: Path, body: str) -> None:
    source = tmp_path / "ranked_rejected.mod"
    source.write_text("fn first(values: &[i16]) -> i16:\n    return values[0]\nfn value() -> i16:\n" + body)
    with pytest.raises(driver.FrontendError):
        driver.parsed(source)


@pytest.mark.parametrize("through", ["a[i, 1]", "at(&a, i)"])
def test_a_ranked_index_is_not_computed_in_a_narrow_index_type(tmp_path: Path, through: str) -> None:
    """A u8 first index made 19 * 20 + 1 wrap to 125 in u8 and read the wrong element."""
    text = (
        "fn at(m: &[i16, 2], i: u8) -> i16:\n"
        "    return m[i, 1]\n"
        "fn value() -> i16:\n"
        "    var a: [i16; 20, 20] = [0; 20, 20]\n"
        "    a[19, 1] = 7\n"
        "    let i: u8 = 19\n"
        f"    return {through}\n"
    )
    assert _returned(tmp_path, text) == 7


def test_a_view_reads_its_stored_stride(tmp_path: Path) -> None:
    """Views assumed contiguous elements: a stride-2 view summed 1 + 2 instead of 1 + 3."""
    source = tmp_path / "strided.mod"
    source.write_text(
        "fn sum(values: &[i16]) -> i16:\n"
        "    var total: i16 = 0\n"
        "    for value in &values:\n"
        "        total += value\n"
        "    return total\n"
        "fn value() -> i16:\n"
        "    let values: [i16; 4] = [1, 2, 3, 4]\n"
        "    return sum(&values[0:2])\n"
    )
    program = driver.parsed(source)
    assert execute.run(program, "value").value == 3

    (module,) = program.modules
    function = next(one for one in module.functions if one.name == "value")
    view = next(one.id for one in function.places if one.name == "$slice_values")

    patched = []

    def strided(instruction: hir.Instruction) -> hir.Instruction:
        target = instruction.operands[0] if instruction.operands else None
        if instruction.op is hir.Op.STORE and isinstance(target, hir.ProjectedPlace) and target.place == view:
            stride = instruction.operands[1]
            if target.offset == 4 and isinstance(stride, hir.Constant):  # [length][capacity][stride]
                patched.append(stride.value)
                return replace(instruction, operands=(target, replace(stride, value=2)))
        return instruction

    blocks = tuple(
        replace(block, instructions=tuple(strided(one) for one in block.instructions)) for block in function.blocks
    )
    assert patched == [1]
    functions = tuple(replace(one, blocks=blocks) if one is function else one for one in module.functions)
    program = replace(program, modules=(replace(module, functions=functions),))
    assert execute.run(program, "value").value == 4

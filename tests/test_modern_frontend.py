import re
import json
from pathlib import Path

import pytest

from qbopt import hir
from qbopt.model import mir
from qbopt.backend import masm
from qbopt.backend import lower_int64
from qbopt.frontend.modern import driver
from qbopt.frontend.qb import physicalize
from qbopt.frontend.modern import compile as modern_compile

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "frontends" / "modern" / "fixtures" / "control.mod"
PRIMITIVES = ROOT / "frontends" / "modern" / "fixtures" / "primitives.mod"
NBODY = ROOT / "frontends" / "modern" / "fixtures" / "nbody.mod"
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


def test_fixed_point_types_scale_literals_and_lower_through_wide_integer_mir() -> None:
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
    assert {mir.Kind.SIGN_EXTEND, mir.Kind.MUL, mir.Kind.SAR} <= product_kinds
    assert {mir.Kind.SIGN_EXTEND, mir.Kind.SHL, mir.Kind.DIVMOD} <= quotient_kinds


def test_narrow_view_of_legalized_fixed_product_uses_its_low_dword() -> None:
    """Native nbody printed every initial position unchanged after one step.

    Fixed multiplication computes an i64 product, shifts that wide value, and
    then takes its narrow i32 view.  Int64 legalization must redirect that
    view to the low dword it just made, rather than leave a use of the removed
    wide value for lowering to interpret as an unrelated live-in.
    """
    program = driver.parsed(NBODY)
    function = next(one for one in program.modules[0].functions if one.name == "nbody")
    lowered = next(one for one in hir.lower(program) if one.name == "nbody.nbody")
    physical = physicalize(program, function, lowered)

    source_shift = next(
        operation
        for block in physical.lowered.body.blocks
        for operation in block.ops
        if operation.kind is mir.Kind.SAR
        and isinstance(operation.args[0], mir.Held)
        and operation.args[0].width == 8
        and operation.results[0].width == 8
    )
    narrow = next(
        operation
        for block in physical.lowered.body.blocks
        for operation in block.ops
        if operation.kind is mir.Kind.COPY
        and isinstance(operation.args[0], mir.Held)
        and operation.args[0].value == source_shift.results[0].value
        and operation.args[0].width == 4
    )

    legalized = lower_int64.expanded(
        physical.lowered.body,
        physical.calls,
        physical.contracts,
        physical.hints,
    ).body
    low_result = next(
        operation.results[0]
        for block in legalized.blocks
        for operation in block.ops
        if operation.at == source_shift.at and operation.kind is mir.Kind.OR
    )
    legalized_narrow = next(
        operation
        for block in legalized.blocks
        for operation in block.ops
        if operation.at == narrow.at and operation.kind is mir.Kind.COPY
    )
    assert legalized_narrow.args == (low_result,)


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
        mir.Kind.DIVMOD,
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
    assembly = masm.text(modern_compile.assembled(driver.parsed(NBODY), entry="main"))
    loop = assembly.split("L0_18:\n", 1)[1].split("    jne L0_18\n", 1)[0]

    assert "mov si, ax" not in loop
    assert "shl si, 4" not in loop
    assert "lea di" not in loop
    assert "cmp ax, 6" not in loop
    assert "mov si, 65440\nL0_18:" in assembly  # -96 in a word
    assert "mov ebx, dword ptr [bp+si+8]" in loop
    assert "add dword ptr [bp+si], ebx" in loop
    assert "mov ebx, dword ptr [bp+si+12]" in loop
    assert "add dword ptr [bp+si+4], ebx" in loop
    assert "add si, 16\nL0_17:\n" in loop
    assert "or si, si" not in loop
    assert "cmp si" not in loop


def test_nbody_identity_uses_the_paired_byte_recurrences() -> None:
    """nbody carried scalar current/other indices beside two `index * 16`
    address chains because their identity comparison hid that both byte
    offsets are the same injective encoding of those indices.
    """
    assembly = masm.text(modern_compile.assembled(driver.parsed(NBODY), entry="main"))
    interaction = assembly.split("L0_7:\n", 1)[1].split("L0_2:\n", 1)[0]

    assert re.search(r"    shl (?:[sd]i|word ptr \[[^]]+\]), 4\n", interaction) is None
    assert ", 96\n" not in interaction
    assert len(re.findall(r"    add word ptr \[[^]]+\], 16\nL\d+_\d+:\n    jne L\d+_\d+\n", interaction)) == 2
    assert len(re.findall(r"    mov word ptr \[[^]]+\], 65440\n", assembly.split("L0_2:\n", 1)[0])) == 2


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

    assembly = masm.text(modern_compile.assembled(driver.parsed(source), entry="main"))

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

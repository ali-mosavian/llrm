import json
from pathlib import Path

import pytest

from qbopt import hir
from qbopt.model import mir
from qbopt.frontend.modern import driver

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "frontends" / "modern" / "fixtures" / "control.mod"
PRIMITIVES = ROOT / "frontends" / "modern" / "fixtures" / "primitives.mod"
NBODY = ROOT / "frontends" / "modern" / "fixtures" / "nbody.mod"


@pytest.fixture(scope="module")
def program() -> hir.Program:
    return driver.parsed(FIXTURE)


def test_frontend_document_crosses_the_strict_common_hir_boundary(program: hir.Program) -> None:
    assert program.dialect is hir.Dialect.MODERN
    assert program.runtime is hir.RuntimeProfile.FREESTANDING
    assert [function.name for function in program.modules[0].functions] == ["step", "count"]
    assert hir.decode(hir.encode(program)) == program


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


def test_nbody_arrays_strings_and_print_cross_hir_and_verify_in_mir() -> None:
    program = driver.parsed(NBODY)
    module = program.modules[0]
    types = {one.name: one for one in module.types}
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
    assert callables["__print_text"].defined is False
    assert callables["__print_i32"].defined is False
    assert callables["__print_newline"].defined is False

    [lowered] = hir.lower(program)
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


def test_nbody_string_places_point_after_the_descriptor() -> None:
    function = driver.parsed(NBODY).modules[0].functions[0]
    strings = [place for place in function.places if place.name.startswith("$string")]
    assert strings
    assert all(place.offset == 4 for place in strings)

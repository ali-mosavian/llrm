import json
from pathlib import Path

import pytest

from qbopt import hir
from qbopt.model import mir
from qbopt.frontend.modern import driver

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "frontends" / "modern" / "fixtures" / "control.mod"
PRIMITIVES = ROOT / "frontends" / "modern" / "fixtures" / "primitives.mod"


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
    assert set(types) == {"void", "bool", "char", "i8", "u8", "i16", "u16", "i32", "u32", "f32", "f64"}
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

    assert mir.Kind.UDIVMOD in kinds("unsignedDivide")
    assert mir.Kind.DIVMOD not in kinds("unsignedDivide")
    assert mir.Kind.UDIVMOD in kinds("unsignedRemainder")
    assert mir.Kind.FMUL in kinds("floatProduct")

    branch = next(
        operation
        for block in lowered["primitives.unsignedLess"].blocks
        for operation in block.ops
        if operation.kind is mir.Kind.BRANCH
    )
    assert branch.test is mir.Kind.BELOW

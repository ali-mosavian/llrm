import json
from pathlib import Path

import pytest

from qbopt import hir
from qbopt.frontend.modern import driver

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "frontends" / "modern" / "fixtures" / "control.mod"


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

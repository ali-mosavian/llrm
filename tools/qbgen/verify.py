"""Assert generated source contracts at the parser, semantic, and HIR stages."""

from __future__ import annotations

import tempfile
from pathlib import Path

from qbopt import hir
from qbopt.frontend.qb import driver
from tools.qbgen.model import render_bas
from qbopt.frontend.qb import FrontendError
from tools.qbgen.model import GeneratedCase


def _source(case: GeneratedCase, directory: Path, ordinal: int) -> Path:
    path = directory / case.filename(ordinal)
    path.write_bytes(render_bas(case.source).encode("ascii"))
    return path


def _ops(program: hir.Program) -> set[str]:
    return {
        instruction.op.value
        for module in program.modules
        for function in module.functions
        for block in function.blocks
        for instruction in block.instructions
    }


def _calls(program: hir.Program) -> set[str]:
    return {
        instruction.callee.upper()
        for module in program.modules
        for function in module.functions
        for block in function.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL and instruction.callee is not None
    }


def _bindings(program: hir.Program) -> dict[str, set[str]]:
    found: dict[str, set[str]] = {}
    for module in program.modules:
        types = {one.id: one.name.casefold() for one in module.types}
        for function in module.functions:
            for place in function.places:
                found.setdefault(place.name.upper(), set()).add(types[place.type])
    return found


def assert_hir_contract(program: hir.Program, case: GeneratedCase) -> None:
    """Check observable binding/type and HIR facts, never p-code details."""
    bindings = _bindings(program)
    for name, type_name in case.bindings:
        if type_name.casefold() == "array":
            values = {kind for kinds in bindings.values() for kind in kinds}
            if not any(kind.endswith("[]") or kind.startswith("array") for kind in values):
                raise AssertionError(f"{case.name}: expected an array binding for {name}, found {bindings}")
            continue
        if type_name.casefold() not in bindings.get(name.upper(), set()):
            raise AssertionError(f"{case.name}: {name} is not bound as {type_name}: {bindings}")
    missing_ops = set(case.hir_ops) - _ops(program)
    if missing_ops:
        raise AssertionError(f"{case.name}: missing HIR operations {sorted(missing_ops)}")
    expected_calls = {one.upper() for one in case.runtime_calls}
    missing_calls = expected_calls - _calls(program)
    if missing_calls:
        raise AssertionError(f"{case.name}: missing runtime calls {sorted(missing_calls)}")


def assert_case(case: GeneratedCase, ordinal: int, directory: Path) -> None:
    """Run the proper source stage for every supported dialect of one case."""
    path = _source(case, directory, ordinal)
    for outcome in case.outcomes:
        dialect = outcome.dialect
        runtime = dialect
        if outcome.result == "syntax-error":
            try:
                driver.syntax_checked(path, dialect=dialect, runtime=runtime)
            except FrontendError:
                continue
            raise AssertionError(f"{case.name}/{dialect}: expected syntax rejection")
        driver.syntax_checked(path, dialect=dialect, runtime=runtime)
        if outcome.result == "semantic-error":
            try:
                driver.parsed(path, dialect=dialect, runtime=runtime)
            except FrontendError:
                continue
            raise AssertionError(f"{case.name}/{dialect}: expected semantic rejection")
        assert_hir_contract(driver.parsed(path, dialect=dialect, runtime=runtime), case)


def verify(generated: tuple[GeneratedCase, ...]) -> None:
    """Full, deliberately opt-in source/HIR verification (one process per stage)."""
    with tempfile.TemporaryDirectory(prefix="qbgen-") as name:
        directory = Path(name)
        for ordinal, case in enumerate(generated, 1):
            assert_case(case, ordinal, directory)

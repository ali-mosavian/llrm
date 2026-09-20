"""Focused coverage for QB compiler stage observation."""

import runpy
from pathlib import Path

import pytest

from qbopt import hir
from qbopt.frontend.qb import compile as qb_compile


def _program() -> hir.Program:
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    body = hir.Block(1, (), hir.Terminator(hir.TerminatorKind.RETURN))
    function = hir.Function(1, "__main", 0, (), (), (body,), 1)
    statements = hir.DataObject(1, "$qb$statementTable", (), readonly=True)
    module = hir.Module(1, "capture", (void,), (function,), data=(statements,))
    return hir.Program(hir.Dialect.VBDOS, hir.RuntimeProfile.VBDOS, (module,))


def test_stage_observer_uses_one_compilation_and_preserves_object_bytes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """qbstages used to lower manually, then assemble the same HIR again for its final listing."""
    program = _program()
    uncaptured = qb_compile.object_bytes(program, "capture.bas")
    observed = []

    assert qb_compile.object_bytes(program, "capture.bas", observer=observed.append) == uncaptured
    assert [event.name for event in observed] == [
        "hir",
        "source-mir",
        "optimized-mir",
        "physical-mir",
        "optimized-physical-mir",
        "initial-lir",
        "machine:far-indirect-calls",
        "machine:floatalloc",
        "machine:phielim",
        "machine:twoaddr",
        "machine:coalesce",
        "machine:regalloc",
        "machine:parcopy",
        "machine:peephole",
        "machine:schedule",
        "machine:jumps",
        "final-lir",
        "emitted-assembly",
    ]
    final = next(event for event in observed if event.name == "final-lir")
    emitted = observed[-1]
    assert observed[0].value is program
    assert final.value is emitted.value.procedures[0].body

    source = tmp_path / "capture.bas"
    source.write_text("")
    stages = runpy.run_path("tools/qbstages.py")
    calls = {"parse": 0, "lower": 0}
    original_lower = hir.lower

    def parse(_source: Path, **_options: object) -> hir.Program:
        calls["parse"] += 1
        return program

    def lower_once(value: hir.Program) -> tuple[hir.Lowered, ...]:
        calls["lower"] += 1
        return original_lower(value)

    monkeypatch.setitem(stages["dumped"].__globals__, "parsed", parse)
    monkeypatch.setattr(hir, "lower", lower_once)
    output = stages["dumped"](source, tmp_path / "stages", dialect="vbdos", runtime="vbdos", includes=())

    assert calls == {"parse": 1, "lower": 1}
    assert (output / "01-hir.json").is_file()
    assert (output / "01-__main-02-mir.txt").is_file()
    assert (output / "99-emitted-asm.asm").is_file()

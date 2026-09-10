"""Exact conversion deletion keeps FPDEEP's pending-exception observation points."""

from dataclasses import replace

import pytest

from qbopt.analysis import consts
from qbopt.backend import lower
from qbopt.model import ir, mir
from qbopt.model.floating import Format, Precision, Rounding, Semantics
from qbopt.objectfile.module import Addr, Space
from qbopt.optimize import floatfold, transform


@pytest.mark.parametrize("guard", ["none", "live", "shared", "memory", "unknown", "barrier"])
def test_exact_pair_keeps_checks_and_refuses_observable_results(guard):
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 5), 4)
    load = mir.Op(0, ir.Operation.FLOAT_LOAD, "fld", (source,), (), kind=mir.Kind.FLOAD,
        args=(mir.Cell(cell),), results=(mir.Held(source, 10),), loads=(cell,),
        floating=Semantics((Format.BINARY32,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE))
    conversion = mir.Op(4, ir.Operation.FLOAT_STORE, "fistp", (result,), (source,), kind=mir.Kind.FSTORE,
        args=(mir.Held(source, 10),), results=(mir.Held(result, 4),),
        floating=Semantics((Format.EXTENDED80,), Format.SIGNED32, Precision.DESTINATION, Rounding.DYNAMIC))
    converted = {result: consts.Known(144, 4)}
    extra = []
    match guard:
        case "unknown": converted = {}
        case "memory": conversion = replace(conversion, stores=(cell,))
        case "barrier": conversion = replace(conversion, op=ir.Operation.BARRIER)
        case "live" | "shared":
            value = result if guard == "live" else source
            extra.append(mir.Op(8, ir.Operation.CALL, "call", (), (value,), kind=mir.Kind.CALL))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (load, conversion, *extra), ()),))
    changed = floatfold.discarded(body, converted)
    if guard != "none":
        assert changed == body
        return
    assert [op.kind for op in changed.blocks[0].ops] == [mir.Kind.FCHECK, mir.Kind.FCHECK]
    assert all(lower.semantics(op).name == "wait" for op in changed.blocks[0].ops)
    assert transform.dead(changed) == changed

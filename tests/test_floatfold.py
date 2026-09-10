"""Exact conversion deletion keeps FPDEEP's pending-exception observation points."""

from dataclasses import replace

import pytest

from qbopt.analysis import consts
from qbopt.backend import lower
from qbopt.model import ir, mir
from qbopt.model.floating import Format, Precision, Rounding, Semantics
from qbopt.objectfile.module import Addr, Space
from qbopt.optimize import floatfold, transform


def test_original_wait_is_an_explicit_checkpoint_with_encoding_provenance():
    """FPCSE's WAIT was called NOTHING in MIR, hiding an observation boundary from passes."""
    from pathlib import Path
    import corpus
    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    check = next(op for block in body.blocks for op in block.ops if op.at == 0x7a)
    assert check.kind is mir.Kind.FCHECK
    assert check.name == "" and check.node is not None and check.covers == (0x7a, 0x7c)
    assert lower.current(check).name == "wait"


@pytest.mark.parametrize("interruption", [mir.Kind.COPY, mir.Kind.STORE, mir.Kind.CALL, mir.Kind.OPAQUE, mir.Kind.FLOAD])
def test_repeated_checkpoint_needs_no_new_floating_effect(interruption):
    """Unrolled FPCSE kept checkpoints between constant stores although no FP work remained."""
    check = mir.Op(0, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.FCHECK)
    middle = mir.Op(1, ir.Operation.MOVE, "", (), (), kind=interruption)
    again = replace(check, at=2)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (check, middle, again), ()),))
    changed = floatfold.checks(body)
    assert changed.blocks[0].ops[0] == check
    assert changed.blocks[0].ops[-1].kind is (
        mir.Kind.NOTHING if interruption in (mir.Kind.COPY, mir.Kind.STORE) else mir.Kind.FCHECK)


def test_qb_fpcse_preserves_entry_when_first_load_disappears():
    """QB FPCSE falsely reported five overlapping bytes when entry 0x30 became source 0x35."""
    from pathlib import Path
    from qbopt import wholeseg
    result = wholeseg.emitted(Path("fixtures/omf/fpcse-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason


@pytest.mark.parametrize("number,expected", [(144, 0x43100000), (-6, 0xc0c00000),
                                            (16777217, None), ("1/3", None)])
def test_single_storage_requires_exact_bits(number, expected):
    """FPDEEP may store 144 directly; an inexact SINGLE checkpoint must still round."""
    from fractions import Fraction
    from qbopt.analysis import floatfacts
    source = mir.Value(1, 0)
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 5), 4)
    load = mir.Op(0, ir.Operation.FLOAT_LOAD, "fld", (source,), (), kind=mir.Kind.FLOAD,
        args=(mir.Cell(replace(cell, width=8)),), results=(mir.Held(source, 10),),
        floating=Semantics((Format.BINARY64,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE))
    store = mir.Op(4, ir.Operation.FLOAT_STORE, "fstp", (), (source,), kind=mir.Kind.FSTORE,
        args=(mir.Held(source, 10),), results=(mir.Cell(cell),), stores=(cell,),
        floating=Semantics((Format.EXTENDED80,), Format.BINARY32, Precision.DESTINATION, Rounding.DYNAMIC))
    body = mir.MirBody(0, (mir.MirBlock(0, (), (load, store), ()),))
    changed = floatfold.stored(body, {source: floatfacts.Finite(Fraction(number))})
    if expected is None:
        assert changed == body
        return
    assert [op.kind for op in changed.blocks[0].ops] == [mir.Kind.FCHECK, mir.Kind.FCHECK, mir.Kind.STORE]
    result = changed.blocks[0].ops[-1]
    assert result.args == (mir.Const(expected, 4),)
    assert result.stores == (cell,) and result.results == (mir.Cell(cell),)


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

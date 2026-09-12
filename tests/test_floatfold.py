"""Exact conversion deletion keeps FPDEEP's pending-exception observation points."""

from dataclasses import replace

import pytest

from qbopt.model import ir
from qbopt.model import mir
from qbopt.backend import lower
from qbopt.analysis import consts
from qbopt.optimize import floatfold
from qbopt.optimize import transform
from qbopt.model.floating import Format
from qbopt.objectfile.module import Addr
from qbopt.model.floating import Rounding
from qbopt.objectfile.module import Space
from qbopt.model.floating import Precision
from qbopt.model.floating import Semantics


def test_original_wait_is_an_explicit_checkpoint_with_encoding_provenance():
    """FPCSE's WAIT was called NOTHING in MIR, hiding an observation boundary from passes."""
    from pathlib import Path

    import corpus

    path = Path("fixtures/omf/fpcse-p-g2.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path), basic_semantics=True)[0][1]
    check = next(op for block in body.blocks for op in block.ops if op.at == 0x7A)
    assert check.kind is mir.Kind.FCHECK
    assert check.name == "" and check.node is not None and check.covers == (0x7A, 0x7C)
    assert lower.current(check).name == "wait"


def test_fpdeep_exact_double_stores_do_not_execute_floating_arithmetic():
    """QB FPDEEP still computed d=12 and e=6 on x87 after proving both exact."""
    from pathlib import Path

    from iced_x86 import Mnemonic

    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/fpdeep-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(
        one.mnemonic in {Mnemonic.FLD, Mnemonic.FMUL, Mnemonic.FMULP, Mnemonic.FDIV, Mnemonic.FDIVP}
        for one in instructions
    )
    assert not any(one.mnemonic == Mnemonic.WAIT for one in instructions)


@pytest.mark.parametrize(
    "interruption", [mir.Kind.COPY, mir.Kind.STORE, mir.Kind.CALL, mir.Kind.OPAQUE, mir.Kind.FLOAD]
)
def test_repeated_checkpoint_needs_no_new_floating_effect(interruption):
    """Unrolled FPCSE kept checkpoints between constant stores although no FP work remained."""
    check = mir.Op(0, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.FCHECK)
    middle = mir.Op(1, ir.Operation.MOVE, "", (), (), kind=interruption)
    again = replace(check, at=2)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (check, middle, again), ()),))
    changed = floatfold.checks(body)
    assert changed.blocks[0].ops[0] == check
    assert changed.blocks[0].ops[-1].kind is (
        mir.Kind.NOTHING if interruption in (mir.Kind.COPY, mir.Kind.STORE) else mir.Kind.FCHECK
    )


def test_qb_fpcse_preserves_entry_when_first_load_disappears():
    """QB FPCSE falsely reported five overlapping bytes when entry 0x30 became source 0x35."""
    from pathlib import Path

    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/fpcse-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason


def test_collapsed_fpcse_has_no_empty_jump_trampoline():
    """QB FPCSE's constant result still ran three jumps through its empty loop header."""
    from pathlib import Path

    from iced_x86 import Mnemonic

    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/fpcse-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert not any(
        one.insn.mnemonic == Mnemonic.JMP for block in corpus.partitioned(result.data) for one in block.insns
    )


@pytest.mark.parametrize("interruption", [mir.Kind.COPY, mir.Kind.CALL, mir.Kind.FLOAD])
def test_completed_fp_observation_crosses_only_proven_edges(interruption):
    """Collapsed FPCSE kept a second WAIT after integer-only control flow."""
    check = mir.Op(0, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.FCHECK)
    middle = replace(check, at=1, kind=interruption)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (check,), (1, 2)),
            mir.MirBlock(1, (), (middle,), (3,)),
            mir.MirBlock(2, (), (), (3,)),
            mir.MirBlock(3, (), (replace(check, at=3),), ()),
        ),
    )
    result = floatfold.checks(body)
    assert result.blocks[0].ops[0].kind is mir.Kind.FCHECK
    assert result.blocks[-1].ops[0].kind is (mir.Kind.NOTHING if interruption is mir.Kind.COPY else mir.Kind.FCHECK)


def test_loop_cannot_prove_its_first_fp_observation_redundant():
    """A check on the backedge cannot stand in for the first iteration's check."""
    check = mir.Op(1, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.FCHECK)
    body = mir.MirBody(0, (mir.MirBlock(0, (), (), (1,)), mir.MirBlock(1, (), (check,), (1,))))
    assert floatfold.checks(body) == body


@pytest.mark.parametrize(
    "format,width,number,expected",
    [
        (Format.BINARY32, 4, 144, 0x43100000),
        (Format.BINARY32, 4, -6, 0xC0C00000),
        (Format.BINARY32, 4, 16777217, None),
        (Format.BINARY32, 4, "1/3", None),
        (Format.BINARY64, 8, 12, 0x4028000000000000),
        (Format.BINARY64, 8, -6, 0xC018000000000000),
        (Format.BINARY64, 8, 9007199254740993, None),
        (Format.BINARY64, 8, "1/3", None),
    ],
)
def test_storage_requires_exact_bits(format, width, number, expected):
    """FPDEEP may store exact 144/12; inexact SINGLE/DOUBLE checkpoints must still round."""
    from fractions import Fraction

    from qbopt.analysis import floatfacts

    source = mir.Value(1, 0)
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 5), width)
    load = mir.Op(
        0,
        ir.Operation.FLOAT_LOAD,
        "fld",
        (source,),
        (),
        kind=mir.Kind.FLOAD,
        args=(mir.Cell(replace(cell, width=8)),),
        results=(mir.Held(source, 10),),
        floating=Semantics((Format.BINARY64,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE),
    )
    store = mir.Op(
        4,
        ir.Operation.FLOAT_STORE,
        "fstp",
        (),
        (source,),
        kind=mir.Kind.FSTORE,
        args=(mir.Held(source, 10),),
        results=(mir.Cell(cell),),
        stores=(cell,),
        floating=Semantics((Format.EXTENDED80,), format, Precision.DESTINATION, Rounding.DYNAMIC),
    )
    body = mir.MirBody(0, (mir.MirBlock(0, (), (load, store), ()),))
    changed = floatfold.stored(body, {source: floatfacts.Finite(Fraction(number))})
    if expected is None:
        assert changed == body
        return
    assert [op.kind for op in changed.blocks[0].ops] == [mir.Kind.FCHECK, mir.Kind.FCHECK, mir.Kind.STORE]
    result = changed.blocks[0].ops[-1]
    assert result.args == (mir.Const(expected, width),)
    assert result.stores == (cell,) and result.results == (mir.Cell(cell),)


@pytest.mark.parametrize("guard", ["none", "live", "shared", "memory", "unknown", "barrier"])
def test_exact_pair_keeps_checks_and_refuses_observable_results(guard):
    source, result = mir.Value(1, 0), mir.Value(2, 1)
    cell = mir.MemRef(Addr(Space.SEGMENT, 0, 5), 4)
    load = mir.Op(
        0,
        ir.Operation.FLOAT_LOAD,
        "fld",
        (source,),
        (),
        kind=mir.Kind.FLOAD,
        args=(mir.Cell(cell),),
        results=(mir.Held(source, 10),),
        loads=(cell,),
        floating=Semantics((Format.BINARY32,), Format.EXTENDED80, Precision.EXACT, Rounding.NONE),
    )
    conversion = mir.Op(
        4,
        ir.Operation.FLOAT_STORE,
        "fistp",
        (result,),
        (source,),
        kind=mir.Kind.FSTORE,
        args=(mir.Held(source, 10),),
        results=(mir.Held(result, 4),),
        floating=Semantics((Format.EXTENDED80,), Format.SIGNED32, Precision.DESTINATION, Rounding.DYNAMIC),
    )
    converted = {result: consts.Known(144, 4)}
    extra = []
    match guard:
        case "unknown":
            converted = {}
        case "memory":
            conversion = replace(conversion, stores=(cell,))
        case "barrier":
            conversion = replace(conversion, op=ir.Operation.BARRIER)
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

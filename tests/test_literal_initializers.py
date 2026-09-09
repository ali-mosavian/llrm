"""QuickBASIC's literal bytes are entry facts, not immutable caller memory."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.analysis import floatfacts
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.objectfile.module import Addr, Space


@pytest.mark.parametrize("width", [1, 2, 4, 8])
def test_literal_bytes_are_available_to_scalar_loads(width):
    """FPDEEP's copied DOUBLE needs integer literal reads, not only x87 reads."""
    from qbopt.analysis import consts
    from qbopt.frontend import raising_literals
    from qbopt.model import ir

    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    segments = omf.segments(found.records)
    _, segment, start, payload = next(item for item in omf.ledata(found.records)
                                      if segments[item[1]][0] == "BC_CN"
                                      and item[2] <= 0x22 and item[2] + len(item[3]) >= 0x2a)
    payload = payload[0x22 - start:]
    start = 0x22
    ref = mir.MemRef(Addr(Space.SEGMENT, start, segment), width)
    value = mir.Value(1, 0x30, variable=1, version=1)
    load = mir.Op(0x30, ir.Operation.MOVE, "mov", (value,), (), loads=(ref,),
                  kind=mir.Kind.LOAD, args=(mir.Cell(ref),), results=(mir.Held(value, width),))
    body = mir.MirBody(0x30, (mir.MirBlock(0x30, (), (load,), ()),))
    body = raising_literals.initialized(body, found)
    assert consts.known(body, found.dgroup, {})[value] == consts.Known(
        int.from_bytes(payload[:width], "little"), width)


@pytest.mark.parametrize("offset,location,admitted", [
    (-1, omf.LOC_OFF16, False), (0, omf.LOC_OFF16, False),
    (7, omf.LOC_OFF16, False), (8, omf.LOC_OFF16, True),
    (-3, omf.LOC_PTR32, False), (-4, omf.LOC_PTR32, True),
    (-1, omf.LOC_BASE, False), (-2, omf.LOC_BASE, True),
    (8, omf.LOC_PTR32, True), (8, 99, False),
])
def test_literal_relocation_exclusion_covers_the_whole_patch(monkeypatch, offset, location, admitted):
    """FPDEEP's DOUBLE shares a LEDATA record with relocated string descriptors."""
    from qbopt.frontend import raising_literals

    path = Path("fixtures/omf/fpdeep-p-g2.obj")
    found = corpus.loaded(path)
    segment = next(index for index, item in enumerate(omf.segments(found.records))
                   if item and item[0] == "BC_CN")
    ref = mir.MemRef(Addr(Space.SEGMENT, 0x22, segment), 8)
    from qbopt.model import ir
    read = mir.Op(0x30, ir.Operation.MOVE, "mov", (), (), loads=(ref,), kind=mir.Kind.LOAD)
    body = mir.MirBody(0x30, (mir.MirBlock(0x30, (), (read,), ()),))
    template = omf.fixups(found.records)[0]
    fixup = replace(template, seg=segment, offset=0x22 + offset, loc=location)
    monkeypatch.setattr(omf, "fixups", lambda records: [fixup])
    result = raising_literals.initialized(body, found)
    assert (ref in dict(result.initial)) is admitted


def test_fpbench_one_survives_unrelated_pointer_relocations():
    """FPBENCH lost its 1.0 literal fact because array descriptors elsewhere have far fixups."""
    path = Path("fixtures/bench/fpbench-v-g3.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    ref = mir.MemRef(Addr(Space.SEGMENT, 0, 9), 4)
    assert dict(body.initial)[ref] == mir.Const(0x3f800000, 4)


def test_quickbasic_literal_initializers_prove_the_same_floating_exit():
    """FPCSE's QB object retained ten iterations while PDS/VBDOS proved 487.5."""
    path = Path("fixtures/omf/fpcse-q-O.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    proofs = floatfacts.loop_exits(body, found.dgroup, found.calls)
    assert len(proofs) == 1 and proofs[0].count == 10
    assert any(fact.n == 0x43f3c000 for _, fact in proofs[0].stores)
    assert mir.resolved(body).initial == body.initial
    assert not floatfacts.known(body, found.dgroup, found.calls, initial={})


def test_unknown_write_invalidates_literal_entry_facts():
    path = Path("fixtures/omf/fpcse-q-O.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    entry = body.block(body.entry)
    clobber = replace(entry.ops[0], kind=mir.Kind.STORE, floating=None, floating_origin=None,
                      args=(mir.Const(0, 4),), results=(mir.Cell(mir.MemRef(None, 4)),),
                      stores=(mir.MemRef(None, 4),), loads=(), uses=(), defines=(), stack=None)
    body = replace(body, blocks=tuple(replace(block, ops=(clobber, *block.ops))
                                     if block is entry else block for block in body.blocks))
    assert not floatfacts.loop_exits(body, found.dgroup, found.calls)


@pytest.mark.parametrize("change", ["relocation", "missing_byte", "procedure"])
def test_literal_entry_requires_unmodified_complete_loader_bytes(change):
    from qbopt.frontend import raising_literals
    path = Path("fixtures/omf/fpcse-q-O.obj")
    found = corpus.loaded(path)
    body = replace(mir.bodies(found, corpus.partitioned(path))[0][1], initial=())
    ref = body.blocks[0].ops[0].loads[0]
    record, index, start, payload = next(item for item in omf.ledata(found.records)
                                       if item[1] == ref.addr.index and item[2] == ref.addr.disp)
    records = list(found.records)
    at = records.index(record)
    match change:
        case "relocation":
            template = next(fixup for fixup in omf.fixups(records) if fixup.seg == index)
            records.insert(at + 1, omf.fixupp_record([omf.reemit(template, offset=0)]))
        case "missing_byte":
            records[at] = omf.ledata_record(index, start + 1, payload[1:])
        case "procedure":
            body = replace(body, entry=body.entry + 1)
    result = raising_literals.initialized(body, replace(found, records=records))
    assert ref not in dict(result.initial)

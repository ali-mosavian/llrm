"""QuickBASIC's literal bytes are entry facts, not immutable caller memory."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.analysis import floatfacts
from qbopt.model import mir
from qbopt.objectfile import omf


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

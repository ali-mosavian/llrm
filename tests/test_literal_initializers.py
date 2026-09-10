"""QuickBASIC's literal bytes are entry facts, not immutable caller memory."""

from dataclasses import replace
from pathlib import Path

import corpus
import pytest

from qbopt.analysis import floatfacts
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.objectfile.module import Addr, Space


def test_nonreturning_call_does_not_justify_narrowing_unknown_memory_reads():
    """B$CENP may enter a No RESUME handler before exit; noreturn does not mean no reads."""
    from qbopt.abi import runtime
    routine = runtime.contract("B$CENP")
    assert routine.control is runtime.Control.NEVER
    assert routine.reads is runtime.Memory.ANY
    path = Path("fixtures/omf/fpcse-q-O.obj")
    found = corpus.loaded(path)
    body = mir.bodies(found, corpus.partitioned(path))[0][1]
    terminal = next(op for block in body.blocks for op in block.ops
                    if op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$CENP")
    stored = next(ref for block in body.blocks for op in block.ops for ref in op.stores
                  if ref.addr is not None and ref.addr.space is Space.SEGMENT)
    assert any(mir.overlapping(ref, stored, found.dgroup) for ref in terminal.loads)
    assert not mir._narrowed({0x30: "B$CENP"}, 0x30)
    assert mir._narrowed({0x30: "B$CEND"}, 0x30)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpdeep_mix_outputs_fold_across_string_prints(tag):
    """FPDEEP kept CLNG(q*1024) because PRINT invalidated the unrelated numeric literal."""
    from qbopt import wholeseg
    states = []
    def watch(stage, name, state):
        if stage == "mir-widen":
            states.append(state)
    result = wholeseg.emitted(Path(f"fixtures/omf/fpdeep-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    printed = {arg.n for body in states for block in body.blocks for op in block.ops
               if op.kind is mir.Kind.ARG for arg in op.args if isinstance(arg, mir.Const) and arg.width == 4}
    assert {512, 768, 896} <= printed
    for body in states:
        literal = next(ref for ref, value in body.initial if value.n == 0x44800000 and value.width == 4)
        assert not any(op.kind is mir.Kind.FMUL and any(ref.addr == literal.addr for ref in op.loads)
                       for block in body.blocks for op in block.ops)


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
@pytest.mark.parametrize("fault", ["escape", "payload", "addend", "overlap"])
def test_unknown_literal_pool_layout_does_not_exclude_call_writes(tag, fault, monkeypatch):
    """A malformed or overlapping descriptor must not protect numeric bytes from PRINT writes."""
    from qbopt.frontend import raising_literals
    from qbopt.objectfile import module
    path = Path(f"fixtures/omf/fpdeep-{tag}.obj")
    found = corpus.loaded(path)
    built = mir.bodies(found, corpus.partitioned(path))[0][1]
    literal = built.initial[0][0]
    # Exercise the layout proof with actual BC records, changing one premise.
    if fault == "escape":
        escapes = module.escaped(found) | {(literal.addr.index, literal.addr.disp)}
        monkeypatch.setattr(module, "escaped", lambda found: escapes)
    elif fault == "overlap":
        original = omf.fixups(found.records)
        field = next(item for item in original if item.seg == literal.addr.index and item.loc == omf.LOC_OFF16)
        monkeypatch.setattr(omf, "fixups", lambda records: [*original, replace(field, offset=field.offset + 1)])
    else:
        chunks = list(omf.ledata(found.records))
        # First near descriptor pointer / first far descriptor pointer.
        at = 2
        if fault == "payload":
            # Remove every payload byte, so even a valid relocation has no known object extent.
            chunks = [item for item in chunks if item[1] != (16 if tag == "v-g3" else 9)]
        else:
            changed = []
            for record, segment, start, payload in chunks:
                if segment == 9 and start <= at < start + len(payload):
                    payload = payload[:at-start] + b'\x01' + payload[at-start+1:]
                changed.append((record, segment, start, payload))
            chunks = changed
        monkeypatch.setattr(omf, "ledata", lambda records: iter(chunks))
    # Remove old annotations before re-running recognition on the changed object.
    built = replace(built, blocks=tuple(replace(block, ops=tuple(replace(op,
        stores=tuple(replace(ref, excludes=()) for ref in op.stores)) for op in block.ops)) for block in built.blocks))
    result = raising_literals.initialized(built, found)
    assert not any(ref.excludes for block in result.blocks for op in block.ops for ref in op.stores)


@pytest.mark.parametrize("offset,width,disjoint", [(0, 1, True), (3, 1, True), (0, 4, True), (-1, 2, False), (3, 2, False)])
def test_call_exclusion_requires_whole_byte_range(offset, width, disjoint):
    """A protected literal does not protect adjacent descriptor bytes or a straddling write."""
    start = Addr(Space.SEGMENT, 30, 9)
    effect = mir.MemRef(None, 0, excludes=((start, 4),))
    access = mir.MemRef(start.plus(offset), width)
    assert mir.overlapping(effect, access, frozenset()) is not disjoint


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

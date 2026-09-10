"""Branch-specific array stores should feed the following read through a value phi."""

from dataclasses import replace

import pytest

from qbopt.model import ir, mir
from qbopt.optimize import loadjoins


def diamond():
    pointer, left, right, loaded = (mir.Value(index, at) for index, at in [(1, 0), (2, 10), (3, 20), (4, 30)])
    cell = mir.MemRef(None, 4, base=pointer, pointer=True)
    def store(at, value):
        source = mir.Op(at, ir.Operation.MOVE, "", (value,), (), kind=mir.Kind.COPY,
                        args=(mir.Const(at, 4),), results=(mir.Held(value, 4),))
        write = mir.Op(at+1, ir.Operation.MOVE, "", (), (value, pointer), kind=mir.Kind.STORE,
                       args=(mir.Held(value, 4),), results=(mir.Cell(cell),), stores=(cell,))
        return source, write
    read = mir.Op(30, ir.Operation.MOVE, "", (loaded,), (pointer,), kind=mir.Kind.LOAD,
                  args=(mir.Cell(cell),), results=(mir.Held(loaded, 4),), loads=(cell,))
    return mir.MirBody(0, (mir.MirBlock(0, (), (), (10, 20)),
                           mir.MirBlock(10, (), store(10, left), (30,)),
                           mir.MirBlock(20, (), store(20, right), (30,)),
                           mir.MirBlock(30, (), (read,), ())))


@pytest.mark.parametrize("reverse", [False, True])
def test_join_load_uses_each_predecessors_stored_value(reverse):
    body = diamond()
    if reverse:
        body = replace(body, blocks=tuple(reversed(body.blocks)))
    after = loadjoins.reused(body)
    join = next(block for block in after.blocks if block.at == 30)
    assert not join.ops[0].loads
    phi, = join.phis
    assert join.ops[0].args == (mir.Held(phi.result, 4),)
    assert phi.incoming == {block.at: block.ops[0].defines[0] for block in body.blocks if block.at in (10, 20)}
    assert loadjoins.reused(after) == after


@pytest.mark.parametrize("where", [10, 20, 30])
@pytest.mark.parametrize("kind", [mir.Kind.CALL, mir.Kind.FCHECK])
def test_observable_call_or_exception_invalidates_the_join_value(where, kind):
    body = diamond()
    call = mir.Op(where, ir.Operation.CALL, "", (), (), kind=kind)
    body = replace(body, blocks=tuple(replace(block, ops=((call, *block.ops) if where == 30 else (*block.ops, call)))
                                    if block.at == where else block for block in body.blocks))
    assert loadjoins.reused(body) == body


def test_a_missing_edge_provider_keeps_the_load():
    body = diamond()
    body = replace(body, blocks=tuple(replace(block, ops=()) if block.at == 20 else block for block in body.blocks))
    assert loadjoins.reused(body) == body


@pytest.mark.parametrize("where", [10, 30])
def test_a_partial_overwrite_invalidates_the_whole_value(where):
    body = diamond()
    cell = body.blocks[-1].ops[0].loads[0]
    narrow = replace(cell, width=1)
    write = mir.Op(where, ir.Operation.MOVE, "", (), (cell.base,), kind=mir.Kind.STORE,
                   args=(mir.Const(99, 1),), results=(mir.Cell(narrow),), stores=(narrow,))
    body = replace(body, blocks=tuple(replace(block, ops=((write, *block.ops) if where == 30 else (*block.ops, write)))
                                    if block.at == where else block for block in body.blocks))
    assert loadjoins.reused(body) == body


def test_predecessor_loads_can_supply_the_join_without_stores():
    body = diamond()
    load = body.blocks[-1].ops[0]
    blocks = []
    for block in body.blocks:
        if block.at in (10, 20):
            value = block.ops[0].results[0]
            block = replace(block, ops=(replace(load, at=block.at, defines=(value.value,), results=(value,)),))
        blocks.append(block)
    body = replace(body, blocks=tuple(blocks))
    assert not loadjoins.reused(body).blocks[-1].ops[0].loads


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_emitted_memphi_reloads_read_destination_but_not_array_values(tag):
    """MEMPHI printed 10,10 for 10,9 when READ's escaped destination was incorrectly reused."""
    from pathlib import Path
    from qbopt import wholeseg
    from qbopt.frontend import blocks, declen
    from qbopt.objectfile import module, omf

    result = wholeseg.emitted(Path(f"fixtures/regressions/memphi-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str), mapped
    fields = {fix.offset: fix for fix in omf.fixups(found.records) if fix.seg == found.seg}
    loads = []
    for block in blocks.partition(found, mapped):
        for one in block.insns:
            if (one.disp_at in fields
                and any(access.access in declen.READS for access in declen.INFO.info(one.insn).used_memory())):
                fix = fields[one.disp_at]
                if fix.index == found.program_data:
                    loads.append(fix.disp)
    assert loads.count(8) == 2
    assert 14 not in loads and 22 not in loads

"""A read needed on both arms should execute only on the arm lacking its value."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.model import ir, mir
from qbopt.objectfile.module import Addr, Space
from qbopt.optimize import loadjoins


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_ldpre_true_arm_skips_the_remaining_memory_read(tag):
    """LDPRE unnecessarily reread x after the true arm had just stored it."""
    from iced_x86 import FlowControl
    from qbopt import wholeseg
    from qbopt.frontend import blocks, declen
    from qbopt.objectfile import module, omf
    result = wholeseg.emitted(Path(f"fixtures/regressions/ldpre-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str), mapped
    instructions = [one for block in blocks.partition(found, mapped) for one in block.insns]
    fields = {fix.offset: fix for fix in omf.fixups(found.records) if fix.seg == found.seg}
    reads = [one for one in instructions if one.disp_at in fields
             and any(access.access in declen.READS for access in declen.INFO.info(one.insn).used_memory())
             and fields[one.disp_at].index == found.program_data
             and fields[one.disp_at].disp == 10]  # x follows the two INTEGER variables.
    read, = reads
    assert any(one.at < read.at and one.insn.flow_control == FlowControl.UNCONDITIONAL_BRANCH
               and one.insn.near_branch_target == read.end for one in instructions)


def diamond():
    cell = mir.MemRef(Addr(Space.SEGMENT, 6, 5), 2)
    before, after = mir.Value(1, 10), mir.Value(2, 30)
    load = mir.Op(10, ir.Operation.MOVE, "", (before,), (), kind=mir.Kind.LOAD,
                  args=(mir.Cell(cell),), results=(mir.Held(before, 2),), loads=(cell,), covers=(10, 14))
    repeated = replace(load, at=30, defines=(after,), results=(mir.Held(after, 2),), covers=(30, 34))
    return mir.MirBody(0, (mir.MirBlock(0, (), (), (10, 20)),
                          mir.MirBlock(10, (), (load,), (30,)),
                          mir.MirBlock(20, (), (), (30,)),
                          mir.MirBlock(30, (), (repeated,), ())))


def test_missing_path_reads_once_and_existing_path_reuses_value():
    body = diamond()
    result = loadjoins.reused(body, insert=True)
    assert result.block(10).ops == body.block(10).ops
    inserted, = result.block(20).ops
    assert inserted.loads == body.block(30).ops[0].loads
    assert inserted.covers == (20, 20)
    assert not result.block(30).ops[0].loads
    phi, = result.block(30).phis
    assert phi.incoming == {10: body.block(10).ops[0].defines[0], 20: inserted.defines[0]}
    assert loadjoins.reused(result, insert=True) == result


@pytest.mark.parametrize("guard", ["critical", "call", "checkpoint", "store", "division", "no_provider"])
def test_load_insertion_cannot_speculate_or_cross_observable_operations(guard):
    body = diamond()
    if guard == "critical":
        body = replace(body, blocks=tuple(replace(b, succ=(30, 40)) if b.at == 20 else b for b in body.blocks)
                       + (mir.MirBlock(40, (), (), ()),))
    elif guard == "no_provider":
        body = replace(body, blocks=tuple(replace(b, ops=()) if b.at == 10 else b for b in body.blocks))
    else:
        kind = {"call": mir.Kind.CALL, "checkpoint": mir.Kind.FCHECK,
                "store": mir.Kind.STORE, "division": mir.Kind.DIV}[guard]
        effect = mir.Op(29, ir.Operation.NOTHING, "", (), (), kind=kind)
        body = replace(body, blocks=(*body.blocks[:-1], replace(body.blocks[-1], ops=(effect, *body.blocks[-1].ops))))
    assert loadjoins.reused(body, insert=True) == body


def test_inserted_address_uses_the_missing_edges_pointer():
    body = diamond()
    left, right, joined = (mir.Value(index, at) for index, at in ((5, 10), (6, 20), (7, 30)))
    branches = []
    for block, pointer in zip(body.blocks[1:3], (left, right), strict=True):
        define = mir.Op(block.at, ir.Operation.MOVE, "", (pointer,), (), kind=mir.Kind.COPY,
                        args=(mir.Const(block.at, 4),), results=(mir.Held(pointer, 4),))
        ops = tuple(replace(op, uses=(pointer,), loads=(mir.MemRef(None, 2, base=pointer, pointer=True),),
                            args=(mir.Cell(mir.MemRef(None, 2, base=pointer, pointer=True)),)) for op in block.ops)
        branches.append(replace(block, ops=(define, *ops)))
    join = body.blocks[-1]
    ref = mir.MemRef(None, 2, base=joined, pointer=True)
    join = replace(join, phis=(mir.Phi(joined, {10: left, 20: right}),),
                   ops=(replace(join.ops[0], uses=(joined,), loads=(ref,), args=(mir.Cell(ref),)),))
    body = replace(body, blocks=(body.blocks[0], *branches, join))
    result = loadjoins.reused(body, insert=True)
    assert result.block(20).ops[-1].loads[0].base == right
    assert result.block(20).ops[-1].uses == (right,)
    assert not result.block(30).ops[0].loads


def test_missing_explicit_critical_edge_gets_its_own_load_block():
    """The unrelated arm must not acquire a read that could fault or observe memory."""
    body = diamond()
    condition = mir.Op(20, ir.Operation.BRANCH, "", (), (), kind=mir.Kind.BRANCH,
                       target=30, test=mir.Kind.EQ, covers=(20, 22))
    body = replace(body, blocks=tuple(replace(block, ops=(condition,), succ=(30, 40))
                                     if block.at == 20 else block for block in body.blocks)
                   + (mir.MirBlock(40, (), (), ()),))
    result = loadjoins.reused(body, insert=True)
    assert not result.block(30).ops[0].loads
    parent = result.block(20)
    assert len(parent.ops) == 1 and 40 in parent.succ
    bridge = result.block(parent.ops[-1].target)
    assert bridge.at not in {block.at for block in body.blocks}
    assert bridge.ops[0].loads == body.block(30).ops[0].loads
    assert bridge.ops[-1].kind is mir.Kind.JUMP and bridge.ops[-1].target == 30
    assert bridge.at in result.block(30).phis[0].incoming
    assert 20 not in result.block(30).phis[0].incoming
    assert all(op.inserted for op in bridge.ops)
    assert loadjoins.reused(result, insert=True) == result


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_ldcrit_load_runs_only_on_the_missing_conditional_edge(tag):
    """LDCRIT reread x on true iterations (35 and -28) after already storing it."""
    from iced_x86 import FlowControl
    from qbopt import wholeseg
    from qbopt.frontend import blocks, declen
    from qbopt.objectfile import module, omf
    result = wholeseg.emitted(Path(f"fixtures/regressions/ldcrit-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str), mapped
    instructions = [one for block in blocks.partition(found, mapped) for one in block.insns]
    fields = {fix.offset: fix for fix in omf.fixups(found.records) if fix.seg == found.seg}
    read, = [one for one in instructions if one.disp_at in fields
             and any(access.access in declen.READS for access in declen.INFO.info(one.insn).used_memory())
             and fields[one.disp_at].index == found.program_data and fields[one.disp_at].disp == 10]
    jump, = [one for one in instructions if one.at == read.end]
    assert jump.insn.flow_control == FlowControl.UNCONDITIONAL_BRANCH
    assert jump.insn.near_branch_target < read.at
    assert any(one.insn.flow_control == FlowControl.CONDITIONAL_BRANCH
               and one.insn.near_branch_target == read.at for one in instructions)

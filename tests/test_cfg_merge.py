"""Linear CFG fragments must not prevent optimization across statements."""

from dataclasses import replace
from pathlib import Path

import pytest

from qbopt.model import ir, mir
from qbopt.optimize import cfg


def test_end_guards_have_no_return_edge_in_raised_control_flow():
    """UDTRNG's END arms falsely rejoined the guarded array accesses in MIR."""
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf
    path = Path("fixtures/regressions/udtrng-p-g2.obj")
    found = module.of(omf.parse(path.read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    exits = [block for block in body.blocks if block.ops and found.calls.get(block.ops[-1].at) == "B$CEND"]
    assert len(exits) == 2
    assert all(not block.succ for block in exits)
    parents = {at: {block.at for block in body.blocks if at in block.succ} for at in (0x60, 0x6c)}
    assert parents == {0x60: {0x30}, 0x6c: {0x60}}


def test_udtrng_bounds_compare_explicit_values():
    """UDTRNG's bounds guards hid their read inside CMP, leaving range analysis no SSA value to constrain."""
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf
    found = module.of(omf.parse(Path("fixtures/regressions/udtrng-p-g2.obj").read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    guards = [op for block in body.blocks for op in block.ops
              if op.at in (0x54, 0x60) and op.op is ir.Operation.COMPARE]
    assert len(guards) == 2
    assert all(not op.loads and isinstance(op.args[0], mir.Held) for op in guards)


def test_udtrng_guards_constrain_subsequent_reads_of_slot():
    """UDTRNG lost both slot bounds when the next statement reloaded the same cell."""
    from qbopt.frontend import arrayfacts, blocks
    from qbopt.analysis.ranges import Interval
    from qbopt.objectfile import module, omf
    found = module.of(omf.parse(Path("fixtures/regressions/udtrng-p-g2.obj").read_bytes()))
    body = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))[0][1]
    state, _ = arrayfacts._transfer(body.block(0x30), arrayfacts.State(), False)
    state = arrayfacts._edge(state, body.block(0x30), 0x60)
    state, _ = arrayfacts._transfer(body.block(0x60), state, False)
    state = arrayfacts._edge(state, body.block(0x60), 0x6c)
    cell = body.block(0x6c).ops[0].args[0]
    assert arrayfacts._read(cell, state) == Interval(0, 2, 2)


@pytest.mark.parametrize("known,terminal", [(False, True), (True, False)])
def test_only_established_terminal_contracts_remove_return_edges(known, terminal):
    """An unknown or returning END-shaped call must not erase a reachable path."""
    from qbopt.abi import runtime
    from qbopt.frontend import blocks, raising_control
    from qbopt.objectfile import module, omf
    found = module.of(omf.parse(Path("fixtures/regressions/udtrng-p-g2.obj").read_bytes()))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    contracts = runtime.for_module(found)
    original = [block for block in blocks.partition(found, mapped)
                if block.insns and found.calls.get(block.insns[-1].at) == "B$CEND"]
    assert original
    changed = {at: replace(contract, established=known,
               control=runtime.Control.NEVER if terminal else runtime.Control.RETURNS)
               for at, contract in contracts.items()}
    assert raising_control.terminal_edges(original, changed) == original


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_bools_constant_program_is_one_live_block(tag):
    """BOOLS still split four constant stores and PRINT across four live blocks."""
    from qbopt import wholeseg
    states = []

    def watch(stage, name, body):
        if isinstance(body, mir.MirBody):
            states.append(body)

    result = wholeseg.emitted(Path(f"fixtures/omf/bools-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert len(states[-1].blocks) == 1


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_localp_keeps_termination_after_interleaved_procedure(tag):
    """LOCALP printed 28/DONE in BC but nothing after optimization lost main's exit."""
    from qbopt import wholeseg
    from qbopt.objectfile import omf, module
    from qbopt.frontend import blocks
    result = wholeseg.emitted(Path(f"fixtures/regressions/localp-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str), mapped
    terminal, = [at for at, name in found.calls.items() if name == "B$CENP"]
    assert terminal in mapped.starts


def chain():
    value, joined = mir.Value(1, 0), mir.Value(2, 10)
    copy = mir.Op(0, ir.Operation.MOVE, "mov", (value,), (), kind=mir.Kind.COPY,
                  args=(mir.Const(7, 2),), results=(mir.Held(value, 2),), covers=(0, 3))
    jump = mir.Op(3, ir.Operation.JUMP, "jmp", (), (), kind=mir.Kind.JUMP, target=10, covers=(3, 5),
                  extra_covers=((5, 10),))
    argument = mir.Op(10, ir.Operation.PUSH, "push", (), (joined,), kind=mir.Kind.ARG,
                      args=(mir.Held(joined, 2),), covers=(10, 12))
    return mir.MirBody(0, (mir.MirBlock(0, (), (copy, jump), (10,)),
                          mir.MirBlock(10, (mir.Phi(joined, {0: value}),), (argument,), ())))


def test_single_entry_phi_is_replaced_and_jump_bytes_are_retained():
    """A constant passed through a statement join must remain the same PRINT argument."""
    result = cfg.merged(chain())
    assert len(result.blocks) == 1
    block = result.blocks[0]
    assert not block.phis and not block.succ
    assert block.ops[-1].args == (mir.Held(mir.Value(1, 0), 2),)
    assert block.ops[1].kind is mir.Kind.NOTHING and block.ops[1].covers == (3, 5)
    assert cfg.merged(result) == result


@pytest.mark.parametrize("guard", ["entry", "other_predecessor", "repetition", "intervening", "unowned_gap"])
def test_merge_preserves_alternate_entries_and_layout(guard):
    body = chain()
    if guard == "entry":
        body = replace(body, entry=10)
    if guard == "other_predecessor":
        body = replace(body, blocks=(*body.blocks, mir.MirBlock(20, (), (), (10,))))
    if guard == "repetition":
        body = replace(body, repetitions=((10, 2),))
    if guard == "intervening":
        body = replace(body, blocks=(*body.blocks, mir.MirBlock(5, (), (body.blocks[1].ops[0],), ())))
    if guard == "unowned_gap":
        first = body.blocks[0]
        body = replace(body, blocks=(replace(first, ops=(first.ops[0], replace(first.ops[1], extra_covers=()))),
                                     body.blocks[1]))
    assert cfg.merged(body) == body


def test_successor_phi_edge_is_renamed_to_the_surviving_block():
    """A later join must still receive the value from the merged path."""
    body = chain()
    value = body.blocks[0].ops[0].defines[0]
    joined = mir.Value(3, 20)
    body = replace(body, blocks=(body.blocks[0], replace(body.blocks[1], succ=(20,)),
        mir.MirBlock(20, (mir.Phi(joined, {10: value, 30: mir.Value(4, 30)}),), (), ()),
        mir.MirBlock(30, (), (), (20,))))
    result = cfg.merged(body)
    assert result.block(0).succ == (20,)
    assert result.block(20).phis[0].incoming == {0: value, 30: mir.Value(4, 30)}


def test_unreachable_ownership_between_blocks_moves_without_losing_spans():
    """BOOLS's eliminated arms still own original object bytes after merging."""
    body = chain()
    first = body.blocks[0]
    body = replace(body, blocks=(replace(first, ops=(first.ops[0], replace(first.ops[1], extra_covers=()))),
                                 body.blocks[1]))
    empty = mir.Op(5, ir.Operation.NOTHING, "", (), (), kind=mir.Kind.NOTHING, covers=(5, 10))
    body = replace(body, blocks=(*body.blocks, mir.MirBlock(5, (), (empty,), ())))
    result = cfg.merged(body)
    assert len(result.blocks) == 1
    assert [(op.at, op.covers) for op in result.blocks[0].ops] == [
        (0, (0, 3)), (3, (3, 5)), (5, (5, 10)), (10, (10, 10)), (10, (10, 12))]

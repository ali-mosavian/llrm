"""Stack argument writes must not erase disjoint compiler-local values."""

from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import pytest

from qbopt import wholeseg
from qbopt.abi import runtime
from qbopt.frontend import declen, raising_frame
from qbopt.model import mir
from qbopt.objectfile import module


def test_pl_move_memory_push_reads_the_pointer_not_its_stack_destination():
    """PL_MOVE refused emission at 2337: PUSH dword [BX] became a read of [SP-8]."""
    import corpus
    path = Path("fixtures/regressions/qrender-pl-move-v-g3.obj")
    found = corpus.loaded(path)
    bodies = mir.bodies(found, corpus.partitioned(path), runtime.for_module(found))
    op = next(op for _, body in bodies for block in body.blocks
              for op in block.ops if op.at == 0x2337)
    source = op.loads[0]
    assert source.addr is None
    assert source.space is not module.Space.STACK
    assert source.base is not None and source.width == 4
    assert op.args == (mir.Cell(source),)
    assert op.stores[0].addr == module.Addr(module.Space.STACK, -8)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_chain_constant_divisors_survive_argument_setup(tag):
    """CHAIN kept six IDIVs: pushing ONE's label forgot local constants before PRINT."""
    bodies = []
    def watch(stage, name, body):
        if stage == "mir-widen":
            bodies.append(body)
    result = wholeseg.emitted(Path(f"fixtures/omf/chain-{tag}.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert bodies
    assert not any(op.kind is mir.Kind.DIVMOD for body in bodies for block in body.blocks for op in block.ops)


def _block(raw, at=0x30, successors=()):
    code = bytes(at) + bytes.fromhex(raw)
    insns = []
    cursor = at
    while cursor < len(code):
        decoded = declen.decode(code, cursor)
        assert decoded is not None
        insns.append(decoded)
        cursor = decoded.end
    return SimpleNamespace(at=at, insns=insns, succ=successors)


@pytest.mark.parametrize("tag,floor", [("q-O", -34), ("p-g2", -42), ("v-g3", -44)])
def test_main_layout_uses_runtime_specific_fixed_prefix(tag, floor):
    assert raising_frame._layout(module.load(Path(f"fixtures/omf/chain-{tag}.obj"))) == (floor, 24)


@pytest.mark.parametrize("raw", ["55", "8bc5", "8bc4", "8d46e6", "8bec"])
def test_frame_pointer_values_are_not_private(raw):
    assert not raising_frame._private([_block(raw)])


def test_direct_frame_load_does_not_escape_the_frame():
    assert raising_frame._private([_block("8b46e6")])


@pytest.mark.parametrize("raw", ["8bec", "83c402", "8ed0", "cd21", "5c"])
def test_unmodeled_frame_stack_or_selector_change_loses_depth(raw):
    block = _block(raw + " 50")
    depths = raising_frame._depths([block], block.at, -42, {})
    assert depths[block.insns[-1].at] is None


def test_known_cleanup_restores_depth_but_unknown_call_does_not():
    block = _block("50 9a00000000 50")
    call = block.insns[1].at
    contract = runtime.contract("B$PSSD")
    assert raising_frame._depths([block], block.at, -42, {call: contract})[block.insns[-1].at] == -42
    for changed in (runtime.worst("unknown"), replace(contract, cleanup=None),
                    replace(contract, enters_user_code=True),
                    replace(contract, clobbers=contract.clobbers | {runtime.Reg.BP})):
        assert raising_frame._depths([block], block.at, -42, {call: changed})[block.insns[-1].at] is None


def test_conflicting_stack_depths_do_not_produce_a_join_fact():
    entry = _block("90", successors=(0x40, 0x50))
    left = _block("50", 0x40, (0x60,))
    right = _block("90", 0x50, (0x60,))
    join = _block("50", 0x60)
    assert raising_frame._depths([entry, left, right, join], entry.at, -42, {})[join.at] is None


def test_unbalanced_loop_loses_depth_instead_of_iterating_forever():
    block = _block("50", successors=(0x30,))
    assert raising_frame._depths([block], block.at, -42, {})[block.at] is None


@pytest.mark.parametrize("fixture", ["fixtures/omf/chain-p-evt.obj", "fixtures/omf/divmod-p-g2.obj"])
def test_event_and_error_modules_keep_their_original_alias_facts(fixture):
    found = module.load(Path(fixture))
    body = SimpleNamespace(entry=0x30)
    assert raising_frame.annotated(body, found, [], runtime.for_module(found)) is body


@pytest.mark.parametrize("pushing", [True, False])
def test_push_pop_frame_operand_is_not_its_implicit_stack_access(pushing):
    """PUSH [local] / POP [local] must not claim the explicit local access excludes itself."""
    found = module.load(Path("fixtures/omf/chain-p-g2.obj"))
    block = _block("ff76e6" if pushing else "50 8f46e6")
    at = block.insns[-1].at
    local = mir.MemRef(module.Addr(module.Space.FRAME, -26), 2, space=module.Space.STACK)
    stack = mir.MemRef(module.Addr(module.Space.STACK, -2), 2, space=module.Space.STACK)
    loads, stores = ((local,), (stack,)) if pushing else ((stack,), (local,))
    from qbopt.model import ir
    op = mir.Op(at, ir.Operation.PUSH if pushing else ir.Operation.POP, "", (), (),
                kind=mir.Kind.ARG if pushing else mir.Kind.COPY,
                loads=loads, stores=stores, args=(mir.Cell(loads[0]),), results=(mir.Cell(stores[0]),))
    body = mir.MirBody(0x30, (mir.MirBlock(0x30, (), (op,), ()),), {})
    result = raising_frame.annotated(body, found, [block], {}).blocks[0].ops[0]
    explicit = result.loads[0] if pushing else result.stores[0]
    implicit = result.stores[0] if pushing else result.loads[0]
    assert not explicit.excludes
    assert mir.overlapping(explicit, local, found.dgroup)
    assert implicit.excludes

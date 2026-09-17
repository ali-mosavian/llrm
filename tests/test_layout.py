"""
qbopt/backend/layout.py: a whole body emitted, and everything that moves with it.

The claim is not that the bytes match. select.py may choose an encoding BC
did not -- it emits the near form of every branch where BC often wrote the
short one -- so the body is a different length and every address in it
shifts. The claim is that it is the same program: the same instructions in
the same order, and every branch pointing at the instruction it pointed at
before rather than at whatever now sits at the old address.
"""

from pathlib import Path
import pytest
from iced_x86 import OpKind
from iced_x86 import Mnemonic

import corpus
from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import asm
from qbopt.model import mir
from qbopt.objectfile import omf
from qbopt.backend import layout
from qbopt.objectfile.module import SourceMap
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map


@pytest.mark.parametrize("conditional", [False, True])
def test_reordered_block_materializes_its_cfg_fallthrough(conditional):
    """Peeled IVARM's latch fell into its own header instead of the next iteration."""
    what = ir.Semantics(ir.Operation.BRANCH, "jne", target=30) if conditional else ir.Semantics(
        ir.Operation.NOTHING, "nop")
    op = lir.Insn(10, (10, 11), what, (), ())
    body = lir.LirBody("fallthrough", 10, (
        lir.LirBlock(10, (op,), (30, 40) if conditional else (40,)),
        lir.LirBlock(20, (), ()),
        lir.LirBlock(30, (), ()),
        lir.LirBlock(40, (), ()),
    ), {}, {})
    changed = layout._fallthroughs(body)
    jump = next(block for block in changed.blocks if block.at == 10).insns[-1]
    assert jump.what == ir.Semantics(ir.Operation.JUMP, "jmp", target=40)
    assert jump.covers == (10, 10)
    assert jump.node is None and jump.symbol is False
    assert layout._fallthroughs(changed) == changed


def test_empty_reordered_block_gets_its_own_jump_anchor():
    body = lir.LirBody("empty", 10, (lir.LirBlock(10, (), (30,)),
                            lir.LirBlock(20, (), ()), lir.LirBlock(30, (), ())), {}, {})
    changed = layout._fallthroughs(body)
    first = next(block for block in changed.blocks if block.at == 10).insns[0]
    assert layout._anchors(changed)[10] is first
    assert first.what.target == 30


@pytest.mark.parametrize("tag", ["q-O", "p-g2", "v-g3"])
def test_peeled_ivarm_emission_keeps_every_iteration_reachable(monkeypatch, tag):
    """PDS's peeled latch reentered itself, leaving emitted bytes 0x57..0x70 unreachable."""
    from qbopt import wholeseg
    from qbopt.analysis import loops
    from qbopt.objectfile import module
    from qbopt.optimize import lcssa, loopclone, transform

    original = transform.applied

    def candidate(body, *args, **kwargs):
        kwargs["unswitch_"] = False
        body = lcssa.closed(original(body, *args, **kwargs))
        loop, = loops.loops(body.blocks, body.entry)
        changed = loopclone.peeled(body, loop, 2)
        assert changed is not None
        return changed

    monkeypatch.setattr(transform, "applied", candidate)
    result = wholeseg.emitted(Path(f"fixtures/regressions/ivarm-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped


def test_pressx_has_no_jump_to_the_following_instruction():
    """PRESSX retained an unconditional jump to its exit immediately after loop elimination."""
    from qbopt import wholeseg
    result = wholeseg.emitted(Path("fixtures/omf/pressx-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    for block in corpus.partitioned(result.data):
        for one in block.insns:
            insn = one.insn
            if insn.mnemonic == Mnemonic.JMP and insn.op0_kind in (OpKind.NEAR_BRANCH16, OpKind.NEAR_BRANCH32):
                assert insn.near_branch_target != insn.next_ip


@pytest.mark.parametrize("shape", ["next", "chain", "self", "data"])
def test_fallthrough_relaxation_preserves_targets_and_intervening_data(shape):
    """Removing PRESSX's empty exit jump must not remove a loop or execute skipped data."""
    from types import SimpleNamespace
    def jump(at, to):
        return lir.Insn(at, (at, at + 2), ir.Semantics(ir.Operation.JUMP, "jmp", target=to), (), ())
    def ret(at):
        return lir.Insn(at, (at, at + 1), ir.Semantics(ir.Operation.RETURN, "ret"), (), ())
    ops = [jump(0, 2), ret(2)]
    code = bytes.fromhex("eb00c3")
    if shape == "chain":
        ops, code = [jump(0, 2), jump(2, 4), ret(4)], bytes.fromhex("eb00eb00c3")
    elif shape == "self":
        ops = [jump(0, 0), ret(2)]
    elif shape == "data":
        ops, code = [jump(0, 3), asm.Table(2, 3), ret(3)], bytes.fromhex("eb0190c3")
    found = SimpleNamespace(code=code, absorbed={}, fixup_at={}, calls={}, refs={}, float_protocols={})
    result = asm.assemble(ops, 0, found, source=SourceMap())
    assert not isinstance(result, str), result
    if shape in {"next", "chain"}:
        assert result.code == bytes.fromhex("c3")
    elif shape == "self":
        assert result.code == bytes.fromhex("ebfec3")
    else:
        assert result.code == bytes.fromhex("eb0190c3")


def test_emulator_load_uses_the_allocated_address() -> None:
    """nbody's copied FLD still read SI after allocation moved its pointer."""
    from types import SimpleNamespace
    from iced_x86 import Register
    from qbopt.frontend import declen

    raw = bytes.fromhex("cd3504")
    original = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),),
                            (ir.Mem(None, 4, through=Register.SI),))
    changed = ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ir.St(0),),
                           (ir.Mem(None, 4, through=Register.DI),))
    node = ir.Opaque(declen.decode(raw, 0), ir.NO_EFFECT, original)
    source = mir.Op(0, ir.Operation.FLOAT_LOAD, "fld", (), (), kind=mir.Kind.FLOAD,
                    source_backed=True, id=1, absorbed=(1,))
    op = lir.Insn(0, (0, 3), changed, (), (), op=source, node=node)
    found = SimpleNamespace(code=raw, absorbed={}, fixup_at={}, calls={}, refs={}, float_protocols={})
    done = asm.assemble([op], 0, found, source=SourceMap())
    assert not isinstance(done, str), done
    assert done.code == bytes.fromhex("cd3505")


def test_a_tangled_class_is_split_on_the_phi_edge() -> None:
    """A value arriving at a phi and still wanted after it is the lost-copy
    shape: the phi's result and its own argument are one congruence class
    and are live at the same moment, so no single register holds both and
    `colour` refuses to move the class. `untangled` puts `v' = v` at the
    end of the predecessor and the phi takes `v'`, which is the live-range
    split done where the split belongs.

    Constructed, because BC's own code never has one -- measured, zero
    across the corpus -- and a tangle only appears once a pass has moved a
    definition. This checks the three functions directly; that
    `layout.rebuild` calls them is not asserted here.
    """
    from dataclasses import replace

    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import mir
    from qbopt.legacy import regalloc

    made, carried = mir.Value(1, 0x10, 0, 1, 1), mir.Value(2, 0x20, 0, 1, 2)
    other = mir.Value(5, 0x11, 0, 4, 1)

    def op(at, kind, defines, uses, args, results):
        return mir.Op(
            at,
            ir.Operation.MOVE,
            "mov",
            defines=defines,
            uses=uses,
            loads=(),
            stores=(),
            kind=kind,
            args=args,
            results=results,
        )

    # `made` is defined before the join, arrives at the phi, and is read
    # again after it -- so it overlaps the phi's own result.
    body = mir.MirBody(
        0x10,
        (
            mir.MirBlock(
                0x10,
                (),
                (
                    op(0x10, mir.Kind.COPY, (made,), (), (mir.Const(1, 2),), (mir.Held(made, 2),)),
                    op(0x13, mir.Kind.COPY, (other,), (), (mir.Const(2, 2),), (mir.Held(other, 2),)),
                ),
                (0x20,),
            ),
            mir.MirBlock(
                0x20,
                (mir.Phi(carried, {0x10: made}),),
                (
                    op(
                        0x20,
                        mir.Kind.COPY,
                        (mir.Value(3, 0x20, 0, 2, 1),),
                        (carried,),
                        (mir.Held(carried, 2),),
                        (mir.Held(mir.Value(3, 0x20, 0, 2, 1), 2),),
                    ),
                    op(
                        0x23,
                        mir.Kind.COPY,
                        (mir.Value(4, 0x23, 0, 3, 1),),
                        (made,),
                        (mir.Held(made, 2),),
                        (mir.Held(mir.Value(4, 0x23, 0, 3, 1), 2),),
                    ),
                    op(
                        0x26,
                        mir.Kind.COPY,
                        (mir.Value(6, 0x26, 0, 5, 1),),
                        (other,),
                        (mir.Held(other, 2),),
                        (mir.Held(mir.Value(6, 0x26, 0, 5, 1), 2),),
                    ),
                ),
                (),
            ),
        ),
        # The class starts in ax, and `other` is held there too -- so the
        # class has to move, and while it is tangled nothing can move it.
        {made: Register.EAX, carried: Register.EAX, other: Register.EAX},
        {},
    )

    body = replace(body, pins={other: Register.EAX})
    tangled = regalloc._tangled(body)
    assert tangled, "the constructed body has no overlapping class; it witnesses nothing"
    assert isinstance(regalloc.colour(body, body.pins), str), "it coloured while tangled"

    fixed = regalloc.untangled(body)
    assert not isinstance(regalloc.colour(fixed, fixed.pins), str), "it does not colour after"
    # The copy goes on the edge, in the predecessor -- not anywhere that
    # would make this pass through an unrelated rewrite.
    before = next(block for block in fixed.blocks if block.at == 0x10)
    assert len(before.ops) == len(body.blocks[0].ops) + 1, "no copy was put on the edge"
    added = before.ops[-1]
    assert added.kind is mir.Kind.COPY and made.id in {one.id for one in added.uses}, (
        f"the copy on the edge reads {added.uses}"
    )
    was = {one.id for block in body.blocks for op in block.ops for one in (*op.defines, *op.uses)}
    assert {one.id for one in added.defines}.isdisjoint(was), "the edge copy reuses an existing value"
    join = next(block for block in fixed.blocks if block.at == 0x20)
    assert {one.id for phi in join.phis for one in phi.incoming.values()} == {one.id for one in added.defines}, (
        "the phi does not take the copy"
    )


def test_a_moved_operation_keeps_its_fixup() -> None:
    """Why the side table is keyed by the operation and not by its address.

    `ref` used to sit on the op, which is what made it survive the hoist
    re-seating something. Off the op it has to be keyed by an identity that
    moves with the operation; keyed by `at` the relocation stays behind and
    the address comes out a bare zero.
    """
    from dataclasses import replace

    from qbopt.model import mir
    from qbopt.backend import lower
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path("fixtures/omf/hotlop-p-g2.obj").read_bytes()))
    assert found is not None
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    bodies = mir.bodies(found, split.partition(found, mapped))
    found = bodies.source.applied(found)
    assert found.refs, "the raise recorded no relocations at all"

    selected = [
        lir.Insn(
            op.at,
            bodies.source.occurrences[op.id][0],
            lower.current(op, node=bodies.source.nodes.get(op.id)),
                tuple(one.id for one in op.defines),
                tuple(one.id for one in op.uses),
                op=op,
                node=bodies.source.nodes.get(op.id),
                symbol=op.symbol,
        )
        for _, body in bodies
        for block in body.blocks
        for op in block.ops
        if op.id in found.refs
    ]
    carried = [op for op in selected if asm._field_in(found, op) is not None]
    assert carried, "no operation carries a fixup; the test measures nothing"

    one = carried[0]
    was = asm._field_in(found, one)
    moved = replace(one, at=one.at + 0x100)
    assert asm._field_in(found, moved) == was, "the relocation stayed behind"


def test_an_operation_may_carry_a_fixup_for_each_instruction_it_stands_for() -> None:
    """One operation is not always one instruction.

    Absorbing a long divide is `mov eax,[a] / mov ecx,[b] / cdq / idiv ecx`
    and two of those four carry a fixup. select.Emitted reported one field
    because until now an operation was an instruction and an instruction
    relocates at most one -- select.restore emits three and gets away with
    it only by having no fixup at all.

    The plumbing carries as many as an operation has: Emitted.places says
    where each landed, the module's refs say which fixup each operand
    carries, and layout pairs them in the order the instructions came out.
    """
    from qbopt.backend import select

    # However it was built: _assemble reads the two offsets iced gives back,
    # an idiom says where its own fields are, and both answer `places`.
    plain = select.Emitted(b"\x8b\x06\x00\x00", displacement_at=2)
    assert plain.places == (2,) and plain.relocated_at == 2

    idiom = select.Emitted(b"\x66\xa1\x00\x00\x66\xb9\x00\x00", fields=(2, 6))
    assert idiom.places == (2, 6), idiom.places
    assert idiom.relocated_at == 2, "the first is what a caller with one wants"

    silent = select.Emitted(b"\x99")
    assert silent.places == () and silent.relocated_at is None


def test_a_jump_to_the_block_placed_next_emits_nothing():
    """A rotated loop's preheader kept `jmp short` to the instruction after it."""
    from qbopt.backend import layout

    jump = lir.Insn(0, (0, 2), ir.Semantics(ir.Operation.JUMP, "jmp", target=4), (), ())
    work = lir.Insn(4, (4, 6), ir.Semantics(ir.Operation.MOVE, "mov", (), ()), (), ())
    body = lir.LirBody("fall", 0, (lir.LirBlock(0, (jump,), (4,)), lir.LirBlock(4, (work,), ())), {}, {})
    (first, _) = layout._fallen(body).blocks
    assert first.insns[-1].what.op is ir.Operation.NOTHING and first.insns[-1].covers == (0, 2)


def test_no_branch_lands_inside_an_instruction_after_a_dropped_jump() -> None:
    """BC's dead `jmp short` after a WEND reached by GOTO must not survive a dropped jump.

    BC writes `add [pa],-360 / jmp next / jmp short back` and nothing reaches
    the short jump. The rebuild dropped the near `jmp` for a fall-through and
    carried the dead bytes after the `add`, so wrapping the angle ran them:
    `jmp short` into the middle of the `add`, whose last byte `FE` is an
    illegal opcode. DOSBox exited on it in deedlines' mark 8.
    """
    from qbopt import wholeseg
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path("fixtures/omf/wendgo-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped
    owners: dict[int, list[int]] = {}
    for block in split.partition(found, mapped):
        for one in block.insns:
            for at in range(one.at, one.end):
                owners.setdefault(at, []).append(one.at)
    assert not [at for at, who in owners.items() if len(set(who)) > 1]


def test_a_jump_over_a_block_holding_a_phi_copy_is_kept() -> None:
    """A block emptied in MIR can still emit the copy a phi left in it.

    deedlines' plasmablobs flips `rc%` in `IF ... THEN rc% = -1`. Its store
    was dead, so the THEN block held nothing but the phi's `mov cx,-1`, which
    rides on a NOTHING op. Layout asked the op's kind, saw an empty block and
    dropped the `jmp` over it: `jne` landed on the next instruction, rc%
    became -1 on both paths, and the plasma ramp came out inverted.
    """
    from iced_x86 import FlowControl

    from qbopt import wholeseg
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path("fixtures/omf/rcflip-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    insns = sorted((one for block in split.partition(found, code_map(found)) for one in block.insns), key=lambda one: one.at)
    collapsed = [
        f"{one.at:#x}: {one.insn}"
        for one, following in zip(insns, insns[1:])
        if one.insn.flow_control == FlowControl.CONDITIONAL_BRANCH and one.insn.near_branch_target == following.at
    ]
    assert not collapsed, collapsed

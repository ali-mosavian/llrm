"""
qbopt/ir.py's own gate: decode every Body to Nodes, re-emit, byte-identical.

Everything here runs against the real corpus, not a hand-built stand-in
where one exists -- the segment/DGROUP work in test_module.py and
test_omf.py, and the segment-override refusal in test_lift.py, are this
module's own dependencies and are tested there.
"""

from pathlib import Path

import pytest
from iced_x86 import Code
from iced_x86 import Register
from iced_x86 import Register_

from qbopt import ir
from qbopt import omf
from helpers import hx
from qbopt import module
from qbopt.flags import ALL
from qbopt.flags import Flag
from qbopt.blocks import Ends
from qbopt.declen import Insn
from qbopt.extent import Body
from qbopt.blocks import Block
from qbopt.declen import decode
from qbopt.blocks import CodeMap
from qbopt.blocks import code_map
from qbopt.extent import BodyKind
from qbopt.rewrite import rewrite


def _decode(path: Path) -> tuple[module.Module, tuple[ir.BodyIR, ...]]:
    found = module.load(path)
    assert found is not None
    result = ir.decode_module(found)
    assert not isinstance(result, str), result
    return found, result


def test_every_mapped_fixture_round_trips_byte_identical(mapped_obj: Path) -> None:
    found, bodies = _decode(mapped_obj)
    for body_ir in bodies:
        original = b"".join(found.code[lo:hi] for lo, hi in body_ir.body.ranges)
        assert ir.emit(found, body_ir.nodes) == original


def test_nodes_tile_every_range_with_no_gap_or_overlap(mapped_obj: Path) -> None:
    _found, bodies = _decode(mapped_obj)
    for body_ir in bodies:
        spans = [ir.span(node) for node in body_ir.nodes]
        index = 0
        for lo, hi in body_ir.body.ranges:
            at = lo
            while at < hi:
                assert index < len(spans), f"{mapped_obj.name}: ran out of nodes before {hi:#x}"
                node_lo, node_hi = spans[index]
                assert node_lo == at, f"{mapped_obj.name}: gap or overlap at {at:#x}"
                at = node_hi
                index += 1
        assert index == len(spans), f"{mapped_obj.name}: nodes left over past the body's own ranges"


def test_decode_is_deterministic(mapped_obj: Path) -> None:
    # Stands in for "IR -> code -> IR idempotence": commit 1's emit is always
    # the original bytes (see ir.py's own module docstring), so re-emitting
    # never produces different input to re-decode from. What is worth
    # checking now is that decoding the same bytes twice, independently,
    # gives the same node sequence -- the property later commits' own
    # idempotence will actually rest on once emit stops being verbatim.
    def signature(bodies: tuple[ir.BodyIR, ...]) -> tuple[tuple[str, int, int], ...]:
        return tuple((type(node).__name__, *ir.span(node)) for body_ir in bodies for node in body_ir.nodes)

    _found_a, bodies_a = _decode(mapped_obj)
    _found_b, bodies_b = _decode(mapped_obj)
    assert signature(bodies_a) == signature(bodies_b)


def test_a_wrong_node_type_cannot_corrupt_emitted_bytes(mapped_obj: Path) -> None:
    # The design's own central claim: emit reads only a node's span, never
    # its semantic fields, so misclassifying a node cannot break the
    # byte-identical gate -- only a wrong span can. Swap every real node for
    # an Opaque wrapping the same instruction (impossible for Restore/Data,
    # which are not single instructions, so those are left as-is) and
    # confirm the emitted bytes still match.
    found, bodies = _decode(mapped_obj)
    for body_ir in bodies:
        reclassified = tuple(
            ir.Opaque(node.insn, node.effects) if isinstance(node, ir.Long | ir.Call) else node
            for node in body_ir.nodes
        )
        original = b"".join(found.code[lo:hi] for lo, hi in body_ir.body.ranges)
        assert ir.emit(found, reclassified) == original


def test_every_named_call_becomes_a_call_node_and_no_others_do(operator_obj: Path) -> None:
    found, bodies = _decode(operator_obj)
    call_nodes = {node.insn.at for body_ir in bodies for node in body_ir.nodes if isinstance(node, ir.Call)}
    # Not just "every Call node's own name matches" -- that holds trivially,
    # since a Call node stores exactly module.calls[insn.at] by construction.
    # The real claim is completeness: every far call a fixup names inside a
    # reached body becomes a Call node, and nothing that is not one of those
    # addresses ever does.
    assert call_nodes, "an operator fixture has at least one runtime call"
    assert call_nodes == set(found.calls)
    assert {"B$CPI4", "B$DVI4"} & {found.calls[at] for at in call_nodes}


def test_jump_table_is_recognised_by_kind(fixtures: Path) -> None:
    # ON k GOTO L1, L2, L3 -- B$OGTA's own inline data, real jump targets.
    # Its own span opens on the count byte, one short of the first entry --
    # `lo` itself is never a fixup site here, so the entry count is exactly
    # (end - at - 1) // 2 regardless of whether `entries` includes `at`.
    found, bodies = _decode(fixtures / "jumptable.obj")
    tables = [node for body_ir in bodies for node in body_ir.nodes if isinstance(node, ir.Data)]
    assert tables, "jumptable.obj has an ON GOTO table"
    assert all(node.kind is ir.TableKind.JUMP for node in tables)
    assert all(len(node.entries) == (node.end - node.at - 1) // 2 for node in tables)


def test_resume_map_is_recognised_as_data_not_a_jump_table(fixtures: Path) -> None:
    # divmod-v-g3.obj's own /X RESUME map sits right where an unrelated call
    # (B$CENP) happens to end -- extent.py's own docstring names this fixture
    # for exactly this shape. Unlike a JUMP table, a MAP's own span opens
    # exactly on its first fixup site (blocks.unexplained_tables() returns
    # the first fixup offset itself), so `entries` must include `at`.
    found, bodies = _decode(fixtures / "divmod-v-g3.obj")
    tables = [node for body_ir in bodies for node in body_ir.nodes if isinstance(node, ir.Data)]
    assert tables
    assert all(node.kind is ir.TableKind.MAP for node in tables)
    assert all(node.entries and node.entries[0] == node.at for node in tables)


def test_every_data_node_is_a_table_span(mapped_obj: Path) -> None:
    found, bodies = _decode(mapped_obj)
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    table_spans = set(mapped.tables)
    for body_ir in bodies:
        for node in body_ir.nodes:
            if isinstance(node, ir.Data):
                assert (node.at, node.end) in table_spans


def test_restore_idiom_is_recognised_not_split_into_opaques() -> None:
    # calls.py's own idiom, byte-identical to lift.FIXUP[0]: push eax / pop
    # ax / pop dx. BC never emits this -- it only exists in an object this
    # pass has already rewritten once -- so this hand-built case is what
    # pins the shape down; test_restore_appears_after_rewriting_the_real_corpus
    # below confirms the same recognition fires on real, rewritten output.
    code = hx("90") + hx("66 50 58 5A") + hx("90")  # nop, restore, nop
    insns = []
    at = 0
    while at < len(code):
        insn = decode(code, at)
        assert insn is not None
        insns.append(insn)
        at = insn.end

    block = Block(0, len(code), tuple(insns), Ends.FALLS_THROUGH, ())
    found = module.Module([], 0, "T", code, 0, len(code))
    mapped = CodeMap(frozenset(insn.at for insn in insns), frozenset({0}))
    body = Body(BodyKind.MAIN, 0, None, ((0, len(code)),))

    nodes = ir.decode_body(found, mapped, [block], body)
    assert [type(n).__name__ for n in nodes] == ["Opaque", "Restore", "Opaque"]
    restore = nodes[1]
    assert isinstance(restore, ir.Restore)
    assert restore.pair == 0
    assert (restore.at, restore.end) == (1, 5)
    assert restore.effects.defs == frozenset({Register.EAX, Register.EDX})
    # both pops are partial writes of their own root -- see
    # _register_effects' own comment -- so both roots are also uses
    assert restore.effects.uses == frozenset({Register.EAX, Register.EDX})
    assert restore.effects.flags_written is Flag.NONE
    assert ir.emit(found, nodes) == code


def test_restore_appears_after_rewriting_the_real_corpus(fixtures: Path) -> None:
    # BC never emits calls.py's own restore idiom -- it exists only in code
    # this pass has already rewritten once. qbopt.rewrite.rewrite() is a
    # pure in-memory function already run over this same tracked corpus by
    # tests/test_rewrite.py; piping its own output back through
    # decode_module is a real-corpus confirmation with no dependency on the
    # untracked, mutable build/ tree the hand-built test above cannot reach.
    total_restores = 0
    for path in sorted(fixtures.glob("*.obj")):
        original = path.read_bytes()
        out, regions = rewrite(original, dry_run=False)
        if out == original or not any(region.taken for region in regions):
            continue
        rewritten = module.of(omf.parse(out))
        assert rewritten is not None
        result = ir.decode_module(rewritten)
        assert not isinstance(result, str), (path.name, result)
        for body_ir in result:
            original_slice = b"".join(rewritten.code[lo:hi] for lo, hi in body_ir.body.ranges)
            assert ir.emit(rewritten, body_ir.nodes) == original_slice, path.name
            total_restores += sum(1 for node in body_ir.nodes if isinstance(node, ir.Restore))
    assert total_restores > 0, "no rewritten fixture produced calls.py's own restore idiom"


def test_root_normalises_every_sub_register_of_the_ax_pair() -> None:
    assert ir.root(Register.AL) is Register.EAX
    assert ir.root(Register.AH) is Register.EAX
    assert ir.root(Register.AX) is Register.EAX
    assert ir.root(Register.EAX) is Register.EAX


def test_a_sub_register_write_is_normalised_in_a_real_instructions_own_effects() -> None:
    # Not just ir.root() in isolation: `mov al,5` is a real instruction, and
    # a def/use set that still said "al" rather than "eax" would push "does
    # a write to al kill dx" onto every future consumer instead of deciding
    # it once, here.
    insn = decode(hx("B0 05"), 0)
    assert insn is not None
    effects = ir.instruction_effects(insn, module.literal_only)
    assert effects.defs == frozenset({Register.EAX})
    # a partial write of eax is also a read of it -- the bits it does not
    # touch survive, so a consumer needs the old value too
    assert effects.uses == frozenset({Register.EAX})


def test_a_call_or_interrupt_gets_the_conservative_answer_not_iceds_own() -> None:
    # `call far` -- iced's own per-instruction info reports roughly {sp}, not
    # the callee's real clobbers, which this layer cannot know at all (that
    # is calls.py's own, routine-specific business). flags.written_by already
    # treats a call this way for the flags; the same conservatism applies
    # here to registers and memory.
    insn = decode(hx("9A 00 00 00 00"), 0)
    assert insn is not None
    effects = ir.instruction_effects(insn, module.literal_only)
    assert effects.defs is None
    assert effects.uses is None
    assert effects.flags_written is ALL
    assert effects.flags_read is ALL
    assert effects.loads == (ir.Mem(None, 0),)
    assert effects.stores == (ir.Mem(None, 0),)


def test_opaque_memory_effect_is_unnamed_for_a_segment_override() -> None:
    effects = _effects("26 8B 06 34 12")
    assert effects.touches_memory is True
    assert effects.loads == (ir.Mem(None, 2),)
    assert effects.stores == ()


# --- the operation vocabulary ------------------------------------------------

STATIC = module.Addr(module.Space.LITERAL, 0x1234)
LOCAL = module.Addr(module.Space.FRAME, -4)
AX = ir.Reg(Register.AX, 2)
DX = ir.Reg(Register.DX, 2)
EAX = ir.Reg(Register.EAX, 4)
ECX = ir.Reg(Register.ECX, 4)
EDX = ir.Reg(Register.EDX, 4)


def _insn(code: str) -> Insn:
    found = decode(hx(code), 0)
    assert found is not None
    return found


def _effects(code: str) -> ir.Effects:
    return ir.instruction_effects(_insn(code), module.literal_only)


def _semantics(code: str) -> ir.Semantics:
    return ir.instruction_semantics(_insn(code), module.literal_only)


def _defs(code: str) -> frozenset[Register_]:
    found = _effects(code).defs
    assert found is not None, "a real instruction, not a call"
    return found


def _uses(code: str) -> frozenset[Register_]:
    found = _effects(code).uses
    assert found is not None, "a real instruction, not a call"
    return found


@pytest.mark.parametrize(
    ("code", "expected"),
    [
        ("8B 06 34 12", ir.Semantics(ir.Operation.MOVE, "mov", (AX,), (ir.Mem(STATIC, 2),))),
        ("89 46 FC", ir.Semantics(ir.Operation.MOVE, "mov", (ir.Mem(LOCAL, 2),), (AX,))),
        ("03 C2", ir.Semantics(ir.Operation.BINARY, "add", (AX,), (AX, DX))),
        ("13 D3", ir.Semantics(ir.Operation.BINARY, "adc", (DX,), (DX, ir.Reg(Register.BX, 2)))),
        ("F7 D0", ir.Semantics(ir.Operation.UNARY, "not", (AX,), (AX,))),
        ("99", ir.Semantics(ir.Operation.EXTEND, "cwd", (DX,), (AX,))),
        ("FF 36 34 12", ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Mem(STATIC, 2),))),
        ("58", ir.Semantics(ir.Operation.POP, "pop", (AX,))),
        (
            "8D 7E E6",
            ir.Semantics(
                ir.Operation.ADDRESS,
                "lea",
                (ir.Reg(Register.DI, 2),),
                (ir.Address(module.Addr(module.Space.FRAME, -0x1A)),),
            ),
        ),
        ("83 3E 34 12 05", ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Mem(STATIC, 2), ir.Imm(5, 2)))),
        ("75 10", ir.Semantics(ir.Operation.BRANCH, "jne", target=0x12)),
        ("E9 00 01", ir.Semantics(ir.Operation.JUMP, "jmp", target=0x103)),
        ("C3", ir.Semantics(ir.Operation.RETURN, "ret")),
        ("9A 00 00 00 00", ir.Semantics(ir.Operation.CALL, "call")),
    ],
)
def test_one_instructions_own_operation_shape(code: str, expected: ir.Semantics) -> None:
    assert _semantics(code) == expected


@pytest.mark.parametrize(
    ("code", "expected"),
    [
        # Each of the three immediate encodings BC picks between (lift.py's
        # own IMM_FAMILY) reports the value it means, sign extension and all,
        # not the bytes it was encoded in.
        ("83 C0 FF", ir.Imm(-1, 2)),
        ("05 FF FF", ir.Imm(-1, 2)),
        ("6A FE", ir.Imm(-2, 2)),
        ("66 6A FE", ir.Imm(-2, 4)),
    ],
)
def test_an_immediate_is_reported_as_the_value_it_means(code: str, expected: ir.Imm) -> None:
    assert _semantics(code).sources[-1] == expected


def test_a_dword_immediate_store_carries_both_the_address_and_the_value() -> None:
    # /G3's own `mov dword [X],12345678h` -- one instruction where a constant
    # propagation pass has everything it needs and nothing to re-decode.
    assert _semantics("66 C7 06 34 12 78 56 34 12") == ir.Semantics(
        ir.Operation.MOVE, "mov", (ir.Mem(STATIC, 4),), (ir.Imm(0x12345678, 4),)
    )


def test_a_push_of_a_static_writes_a_stack_cell_that_is_not_that_static() -> None:
    # The reason loads and stores are separate: `push [X]` reads X and writes
    # somewhere below sp. A single "touches memory: X" would let a later pass
    # believe the push had overwritten X.
    effects = _effects("FF 36 34 12")
    assert effects.loads == (ir.Mem(STATIC, 2),)
    assert effects.stores == (ir.Mem(None, 2),)


def test_a_load_writes_no_memory_and_a_store_reads_none() -> None:
    assert _effects("8B 06 34 12").stores == ()
    assert _effects("89 46 FC").loads == ()
    assert _effects("89 46 FC").stores == (ir.Mem(LOCAL, 2),)


def test_lea_touches_no_memory_at_all() -> None:
    # The claim a later pass acts on destructively: `lea` computes an address
    # and reads nothing, so it neither aliases a store nor is killed by one.
    effects = _effects("8D 7E E6")
    assert effects.loads == ()
    assert effects.stores == ()
    assert effects.touches_memory is False
    assert effects.flags_written is Flag.NONE


def test_cwd_writes_dx_alone_and_reads_ax() -> None:
    # It does NOT write ax, which is what makes it safe to treat the ax half
    # of a widened pair as still holding what it held.
    assert _defs("99") == frozenset({Register.EDX})
    assert Register.EAX not in _defs("99")
    assert _uses("99") == frozenset({Register.EAX, Register.EDX})


def test_the_absorbed_divide_names_both_of_its_destinations() -> None:
    # AGENTS.md's "divide and remainder are C's": one `idiv ecx` produces the
    # quotient in eax and the remainder in edx. Naming only the quotient
    # would tell a value-numbering pass edx still held its old value.
    assert _semantics("66 F7 F9") == ir.Semantics(ir.Operation.DIVIDE, "idiv", (EAX, EDX), (EDX, EAX, ECX))
    assert _defs("66 F7 F9") == frozenset({Register.EAX, Register.EDX})
    assert Register.ECX not in _defs("66 F7 F9")


def test_the_absorbed_multiply_leaves_edx_alone() -> None:
    # The two-operand `imul` calls.py emits, not the one-operand form: edx is
    # untouched, and a pass that thought otherwise would refuse every region
    # an absorbed B$MUI4 sits in.
    assert _semantics("66 0F AF C1") == ir.Semantics(ir.Operation.MULTIPLY, "imul", (EAX,), (EAX, ECX))
    assert _defs("66 0F AF C1") == frozenset({Register.EAX})


def test_a_three_operand_multiply_does_not_read_its_own_destination() -> None:
    # Unlike every BINARY form, `imul eax,ecx,4` overwrites eax without
    # reading it -- which is exactly the difference Operation.MULTIPLY exists
    # to record.
    assert _semantics("66 6B C1 04") == ir.Semantics(ir.Operation.MULTIPLY, "imul", (EAX,), (ECX, ir.Imm(4, 4)))
    assert Register.EAX not in _uses("66 6B C1 04")


def test_a_branch_reads_the_flags_it_tests_and_writes_none() -> None:
    effects = _effects("75 10")
    assert effects.flags_read is Flag.ZF
    assert effects.flags_written is Flag.NONE
    assert effects.defs == frozenset()


def test_add_reads_no_flag_and_adc_reads_the_carry() -> None:
    # The pair BC emits for every long: the low half reads nothing, the high
    # half reads CF. Widening a region that splits them is exactly the bug
    # this distinction exists to make visible.
    assert _effects("03 C2").flags_read is Flag.NONE
    assert _effects("13 D3").flags_read is Flag.CF


def test_not_writes_no_flags() -> None:
    # lift.py picked NOT_RM16 for NOT/EQV/IMP precisely because of this.
    assert _effects("F7 D0").flags_written is Flag.NONE


@pytest.mark.parametrize(
    ("code", "why"),
    [
        ("CA 04 00", "retf n"),
        ("0E", "push cs"),
        ("16", "push ss"),
        ("07", "pop es"),
        ("F3 AB", "rep stosw"),
        ("C9", "leave"),
        ("EA 00 00 00 00", "a far jmp, whose target is not in the instruction"),
        ("F7 E9", "one-operand imul, which writes dx:ax"),
        ("26 8B 06 34 12", "a segment override"),
        ("CD 35 46 C8", "the x87 emulator's own int 35h"),
    ],
)
def test_the_deliberately_refused_shapes_stay_opaque(code: str, why: str) -> None:
    # Refusing is always safe -- a later pass declines the whole region. A
    # wrong effect is silently catastrophic, which is why none of these is
    # guessed at. See ir.SHAPE's own comment.
    assert _semantics(code) is ir.UNMODELLED, why


def test_an_opaque_node_still_carries_a_complete_effect() -> None:
    # The point of the split: a shape with no modelled operation still has an
    # iced-derived def/use, so liveness across it is exact rather than absent.
    effects = _effects("C9")
    assert _defs("C9") == frozenset({Register.EBP, Register.ESP})
    assert effects.loads == (ir.Mem(None, 2),)


def test_semantics_never_claims_a_register_or_cell_the_effects_do_not(mapped_obj: Path) -> None:
    # The invariant that makes the two views safe to use together: Semantics
    # is derived by hand, Effects comes from iced, and a builder that shaped
    # an instruction wrongly would name a destination iced does not report as
    # written -- or a source it does not report as read. Neither may happen.
    _found, bodies = _decode(mapped_obj)
    for body_ir in bodies:
        for node in body_ir.nodes:
            semantics, effects = node.semantics, node.effects
            if not ir.modelled(semantics):
                continue
            for where in semantics.dests:
                if isinstance(where, ir.Reg) and effects.defs is not None:
                    assert ir.root(where.register) in effects.defs, semantics
                if isinstance(where, ir.Mem):
                    assert where in effects.stores, semantics
            for where in semantics.sources:
                if isinstance(where, ir.Reg) and effects.uses is not None:
                    assert ir.root(where.register) in effects.uses, semantics
                if isinstance(where, ir.Mem):
                    assert where in effects.loads, semantics


# Every encoding in the corpus this pass deliberately declines to model. Not
# a coverage floor with slack in it: an encoding leaving this set is a
# regression, and one joining it is a decision to make deliberately.
REFUSED = {
    Code.RETFW_IMM16,
    Code.PUSHW_CS,
    Code.PUSHW_SS,
    Code.POPW_ES,
    Code.STOSW_M16_AX,
    Code.LEAVEW,
    Code.JMP_PTR1616,
}


def test_the_corpus_is_modelled_except_for_exactly_the_refused_encodings(fixtures: Path) -> None:
    unmodelled: set[int] = set()
    modelled = total = 0
    for path in sorted(fixtures.glob("*.obj")):
        found = module.load(path)
        assert found is not None
        result = ir.decode_module(found)
        if isinstance(result, str):
            continue
        for body_ir in result:
            for node in body_ir.nodes:
                total += 1
                if ir.modelled(node.semantics):
                    modelled += 1
                elif isinstance(node, ir.Opaque | ir.Long | ir.Call):
                    unmodelled.add(node.insn.code)
    assert unmodelled == REFUSED
    assert modelled / total > 0.99, f"{modelled}/{total}"


def test_a_restore_and_a_table_carry_their_own_operations() -> None:
    assert ir.RESTORE_IDIOM.op is ir.Operation.RESTORE
    assert ir.TABLE_DATA.op is ir.Operation.DATA
    assert ir.modelled(ir.UNMODELLED) is False

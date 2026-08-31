"""
qbopt/ir.py's own gate: decode every Body to Nodes, re-emit, byte-identical.

Everything here runs against the real corpus, not a hand-built stand-in
where one exists -- the segment/DGROUP work in test_module.py and
test_omf.py, and the segment-override refusal in test_lift.py, are this
module's own dependencies and are tested there.
"""

from pathlib import Path

import pytest
from iced_x86 import Register
from iced_x86 import Register_

import corpus
from iced_x86 import Code

from qbopt import ir
from helpers import hx
from qbopt import extent
from qbopt import module
from qbopt.flags import ALL
from qbopt.flags import Flag
from qbopt.blocks import Ends
from qbopt.declen import Insn
from qbopt.extent import Body
from qbopt.blocks import Block
from qbopt.declen import decode
from qbopt.blocks import CodeMap
from qbopt.extent import BodyKind


def _decode(source: Path | bytes) -> tuple[module.Module, tuple[ir.BodyIR, ...]]:
    found = corpus.loaded(source)
    assert found is not None
    result = corpus.bodies(source)
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
    _found, bodies = _decode(mapped_obj)
    mapped = corpus.mapped(mapped_obj)
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
        out, regions = corpus.rewritten(path, dry_run=False)
        if out == original or not any(region.taken for region in regions):
            continue
        rewritten = corpus.loaded(out)
        assert rewritten is not None
        result = corpus.bodies(out)
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


def test_a_barriers_memory_reach_is_unknown_in_both_directions() -> None:
    # `in al,dx` names no memory operand at all, and a barrier's memory is
    # widened past what the encoding reports anyway -- ir.instruction_effects'
    # own docstring says why. A port read's real effect is a device's, which
    # is nowhere in the instruction, so both directions are claimed rather
    # than decoded. That is what makes "write a promoted variable back before
    # a barrier, re-read it after" true without a consumer remembering to.
    #
    # Deliberately not an x87 site any more: the emulator's own int 34h-3Dh
    # encodings are modelled now, and carry their real named operand rather
    # than this widened one.
    effects = _effects("EC")
    assert effects.touches_memory is True
    assert effects.loads == ir.ANY_MEMORY
    assert effects.stores == ir.ANY_MEMORY


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
        ("D9 C1", "fld st(1), a stack duplicate -- no memory operand for _float_load to accept"),
        (
            "D8 C1",
            "fadd st(0),st(1), the non-popping register form -- a different shape under "
            "the same Mnemonic as the memory form _float_arith accepts",
        ),
        ("CD 21", "an ordinary software interrupt"),
        ("E4 40", "in -- what it does happens in a device"),
        ("EE", "out, likewise"),
        ("AB", "stosw with no rep prefix: a different shape, absent from this corpus"),
        ("F6 E9", "byte-wide imul, whose whole product lands in ax rather than a pair"),
        ("FF 2E 34 12", "an indirect far jmp, which reads its target out of memory"),
    ],
)
def test_the_deliberately_refused_shapes_stay_barriers(code: str, why: str) -> None:
    # A barrier claims nothing, which is always safe; a wrong effect is
    # silently catastrophic, which is why none of these is guessed at. See
    # ir.SHAPE's own comment for what each one would have cost.
    assert _semantics(code) is ir.UNMODELLED, why


ES = ir.Reg(Register.ES, 2)
CS = ir.Reg(Register.CS, 2)
BX = ir.Reg(Register.BX, 2)
FAR_BX = module.Addr(module.Space.FAR, 0, base=Register.BX, segment=Register.ES)


def test_a_far_pointer_load_is_modelled() -> None:
    # `mov ax,es:[bx]` -- 4025 of these in qb-qrender, the single biggest
    # shape in the segment-override barrier population. Addr's own `segment`
    # field is what makes this different from `mov ax,[bx]` through ds.
    assert _semantics("26 8B 07") == ir.Semantics(ir.Operation.MOVE, "mov", (AX,), (ir.Mem(FAR_BX, 2),))


def test_a_far_pointer_store_is_modelled() -> None:
    # `mov es:[bx],ax` -- 1783 of these, the write side of the same pattern.
    assert _semantics("26 89 07") == ir.Semantics(ir.Operation.MOVE, "mov", (ir.Mem(FAR_BX, 2),), (AX,))


def test_a_descriptor_load_into_a_segment_register_is_modelled() -> None:
    # `mov es,[si+2]` -- 17,385 of these, the load that primes every es:bx
    # access after it. A segment register is a Reg like any other now, so
    # _move needs nothing special to build this.
    found = module.Addr(module.Space.LITERAL, 2, base=Register.SI)
    assert _semantics("8E 44 02") == ir.Semantics(ir.Operation.MOVE, "mov", (ES,), (ir.Mem(found, 2),))


def test_a_redundant_ds_prefix_is_not_an_override_to_record() -> None:
    # `mov es,ds:[0]` -- 208 of these. The `ds:` prefix names exactly the
    # segment `[0]` already defaults to, so it resolves through the ordinary
    # LITERAL path rather than becoming a second Space.FAR to track.
    literal = module.Addr(module.Space.LITERAL, 0)
    assert _semantics("3E 8E 06 00 00") == ir.Semantics(ir.Operation.MOVE, "mov", (ES,), (ir.Mem(literal, 2),))


def test_reading_a_segment_register_is_modelled_too() -> None:
    # `mov ax,cs` -- a pure register-to-register copy, no memory and no
    # override involved at all, so there is nothing left this pass cannot
    # say about it once a segment register is a Reg like any other.
    assert _semantics("8C C8") == ir.Semantics(ir.Operation.MOVE, "mov", (AX,), (CS,))


def test_a_segment_override_this_pass_cannot_express_still_refuses_the_address() -> None:
    # `mov ax,es:[si+2]` -- measured against qb-qrender, every one of the
    # 11,150 segment-override instructions is bx with no index; this shape
    # is not among them, so lift.operand() refuses to resolve it. The
    # instruction is still modelled (ax is a real, nameable destination),
    # but its source is Mem(None, ...) -- unnamed, and never provably
    # disjoint from anything, rather than a whole-node barrier.
    found = _semantics("26 8B 44 02")
    assert found == ir.Semantics(ir.Operation.MOVE, "mov", (AX,), (ir.Mem(None, 2),))
    assert ir.modelled(found) is True


def test_xchg_is_a_genuine_two_way_swap() -> None:
    # `xchg bx,ax` -- both operands read AND written, each landing in the
    # other's old value. 54 of qb-qrender's 74 xchg sites are this shape.
    found = _semantics("93")
    assert found == ir.Semantics(ir.Operation.EXCHANGE, "xchg", (BX, AX), (AX, BX))
    assert _defs("93") == frozenset({Register.EBX, Register.EAX})
    assert _uses("93") == frozenset({Register.EBX, Register.EAX})


def test_a_barrier_still_carries_a_complete_effect() -> None:
    # The point of the split, and the point of a barrier: a shape with no
    # modelled operation still has an iced-derived def/use, so liveness
    # across it is exact rather than absent. `in al,40h` -- PITSNAP's own
    # port read, the shape this pass will never model.
    assert _defs("E4 40") == frozenset({Register.EAX})
    assert _effects("E4 40").flags_written is Flag.NONE


# --- the x87 vocabulary -------------------------------------------------

FRAME = module.Addr(module.Space.FRAME, -0x38)
ST0 = ir.St(0)
ST1 = ir.St(1)


@pytest.mark.parametrize(
    ("code", "expected"),
    [
        # fld/fild: push the one real memory operand; nothing iced itself
        # names as the destination (there is no existing register to name
        # the new top with), so St(0) is this module's own convention.
        ("D9 46 C8", ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ST0,), (ir.Mem(FRAME, 4),))),
        ("DD 46 C8", ir.Semantics(ir.Operation.FLOAT_LOAD, "fld", (ST0,), (ir.Mem(FRAME, 8),))),
        ("DB 46 C8", ir.Semantics(ir.Operation.FLOAT_LOAD, "fild", (ST0,), (ir.Mem(FRAME, 4),))),
        ("DF 46 C8", ir.Semantics(ir.Operation.FLOAT_LOAD, "fild", (ST0,), (ir.Mem(FRAME, 2),))),
        # fstp/fistp: the current top, written out, then popped.
        ("D9 5E C8", ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (ir.Mem(FRAME, 4),), (ST0,))),
        ("DD 5E C8", ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (ir.Mem(FRAME, 8),), (ST0,))),
        ("DF 5E C8", ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (ir.Mem(FRAME, 2),), (ST0,))),
        ("DB 5E C8", ir.Semantics(ir.Operation.FLOAT_STORE, "fistp", (ir.Mem(FRAME, 4),), (ST0,))),
        # fadd/fsub/fmul/fdiv/fidiv/fisub, memory form: st0 combined with
        # the one real operand, in place -- dest IS sources[0], same as
        # BINARY's own rule.
        ("D8 46 C8", ir.Semantics(ir.Operation.FLOAT_ARITH, "fadd", (ST0,), (ST0, ir.Mem(FRAME, 4)))),
        ("D8 66 C8", ir.Semantics(ir.Operation.FLOAT_ARITH, "fsub", (ST0,), (ST0, ir.Mem(FRAME, 4)))),
        ("D8 4E C8", ir.Semantics(ir.Operation.FLOAT_ARITH, "fmul", (ST0,), (ST0, ir.Mem(FRAME, 4)))),
        ("D8 76 C8", ir.Semantics(ir.Operation.FLOAT_ARITH, "fdiv", (ST0,), (ST0, ir.Mem(FRAME, 4)))),
        ("DE 66 C8", ir.Semantics(ir.Operation.FLOAT_ARITH, "fisub", (ST0,), (ST0, ir.Mem(FRAME, 2)))),
        ("DE 76 C8", ir.Semantics(ir.Operation.FLOAT_ARITH, "fidiv", (ST0,), (ST0, ir.Mem(FRAME, 2)))),
        # faddp/fsubp/fmulp/fdivp st(1),st(0): combines the two, writes the
        # result to st(1) -- pre-pop numbering -- then pops.
        ("DE C1", ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "faddp", (ST1,), (ST1, ST0))),
        ("DE E9", ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "fsubp", (ST1,), (ST1, ST0))),
        ("DE C9", ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "fmulp", (ST1,), (ST1, ST0))),
        ("DE F9", ir.Semantics(ir.Operation.FLOAT_ARITH_POP, "fdivp", (ST1,), (ST1, ST0))),
        # fchs/fabs/fsqrt: the top, transformed in place.
        ("D9 E0", ir.Semantics(ir.Operation.FLOAT_UNARY, "fchs", (ST0,), (ST0,))),
        ("D9 E1", ir.Semantics(ir.Operation.FLOAT_UNARY, "fabs", (ST0,), (ST0,))),
        ("D9 FA", ir.Semantics(ir.Operation.FLOAT_UNARY, "fsqrt", (ST0,), (ST0,))),
        # wait: a synchronisation point, not arithmetic -- Operation.NOTHING,
        # same as a nop. declen.py's own account of int 3Dh is why.
        ("9B", ir.Semantics(ir.Operation.NOTHING, "wait")),
    ],
)
def test_the_x87_vocabulary_is_modelled_in_its_one_measured_shape(code: str, expected: ir.Semantics) -> None:
    assert _semantics(code) == expected


def test_an_emulated_x87_site_models_identically_to_its_literal_encoding() -> None:
    # `CD 35 46 C8` is declen.py's own int 34h-3Bh stand-in for exactly
    # `D9 46 C8` -- the same fld this module already models -- so nothing
    # here has to special-case the emulated shape: instruction_semantics
    # and instruction_effects both read insn.insn, which declen.emulated()
    # already reconstructed to the real opcode.
    literal = _insn("D9 46 C8")
    emulated = _insn("CD 35 46 C8")
    assert emulated.length == 4
    assert ir.instruction_semantics(emulated, module.literal_only) == ir.instruction_semantics(
        literal, module.literal_only
    )
    real_effects = ir.instruction_effects(literal, module.literal_only)
    emulated_effects = ir.instruction_effects(emulated, module.literal_only)
    assert emulated_effects.loads == real_effects.loads == (ir.Mem(FRAME, 4),)
    assert emulated_effects.stores == real_effects.stores == ()


def test_an_x87_loads_memory_reach_is_the_real_operand_not_any_memory() -> None:
    # The point of the whole change: a load's own Effects name exactly the
    # cell it reads, not ir.ANY_MEMORY -- so an integer variable elsewhere
    # in the same body is never poisoned by a float instruction next to it.
    effects = _effects("D9 46 C8")
    assert effects.loads == (ir.Mem(FRAME, 4),)
    assert effects.stores == ()
    assert effects.fp_stack is True
    assert effects.flags_written is Flag.NONE
    assert effects.flags_read is Flag.NONE


def test_an_x87_stores_memory_reach_is_the_real_operand_too() -> None:
    effects = _effects("DD 5E C8")
    assert effects.loads == ()
    assert effects.stores == (ir.Mem(FRAME, 8),)
    assert effects.fp_stack is True


def test_x87_arithmetic_touches_the_fp_stack_without_naming_it_as_memory() -> None:
    # fadd m32fp neither pushes nor pops (fpu_stack_increment_info's own
    # increment is 0), but it still reads and writes st(0) in place, which
    # is exactly what fp_stack -- not loads/stores -- is for.
    effects = _effects("D8 46 C8")
    assert effects.fp_stack is True
    assert effects.loads == (ir.Mem(FRAME, 4),)
    assert effects.stores == ()


def test_wait_touches_no_resource_this_module_tracks() -> None:
    # Confirms _touches_fp_stack's own docstring: wait reads no st(i) and
    # iced's own fpu_stack_increment_info agrees it moves nothing, so it is
    # exactly as inert as a nop, not a hidden coupling to neighbouring FP
    # instructions.
    effects = _effects("9B")
    assert effects.fp_stack is False
    assert effects.defs == frozenset()
    assert effects.uses == frozenset()
    assert effects.loads == ()
    assert effects.stores == ()
    assert effects.flags_written is Flag.NONE


def test_a_call_may_reach_the_fp_stack_too() -> None:
    # Symmetric with the callee's own unknown memory/register reach: the
    # callee could push or pop floats internally, so this stays True the
    # same way loads/stores stay ANY_MEMORY.
    assert _effects("9A 00 00 00 00").fp_stack is True


@pytest.mark.parametrize(
    ("code", "expected"),
    [
        # BC builds the far pointer it hands B$OEGA as `push cs / push ax`,
        # and PDS 7.1's /Ot prologue points es at the frame with
        # `push ss / pop es` before its own `rep stosw`.
        ("0E", ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Reg(Register.CS, 2),))),
        ("16", ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Reg(Register.SS, 2),))),
        ("07", ir.Semantics(ir.Operation.POP, "pop", (ir.Reg(Register.ES, 2),))),
    ],
)
def test_a_segment_register_is_pushed_and_popped_like_any_other(code: str, expected: ir.Semantics) -> None:
    assert _semantics(code) == expected


def test_leave_names_both_registers_it_writes_and_the_one_it_reads() -> None:
    # `leave` is `mov sp,bp` then `pop bp`. Naming only sp would tell a
    # value-numbering pass that bp still held the caller's frame pointer.
    assert _semantics("C9") == ir.Semantics(
        ir.Operation.LEAVE, "leave", (ir.Reg(Register.SP, 2), ir.Reg(Register.BP, 2)), (ir.Reg(Register.BP, 2),)
    )
    assert _defs("C9") == frozenset({Register.EBP, Register.ESP})
    assert _effects("C9").flags_written is Flag.NONE


def test_a_rep_fill_writes_a_cell_it_cannot_name_and_reads_its_count() -> None:
    # `rep stosw` writes cx words through es:di. The destination is unnamed
    # and unsized -- iced reports that access at MemorySize.UNKNOWN, because
    # the extent is cx words rather than one -- and an unnamed cell aliases
    # everything, which is the answer a fill wants.
    found = _semantics("F3 AB")
    assert found == ir.Semantics(
        ir.Operation.FILL,
        "stosw",
        (ir.Mem(None, 0),),
        (AX, ir.Reg(Register.CX, 2), ir.Reg(Register.DI, 2), ir.Reg(Register.ES, 2)),
    )
    assert _effects("F3 AB").stores == (ir.Mem(None, 0),)
    assert _effects("F3 AB").loads == ()


def test_the_widening_multiply_names_both_halves_of_its_product() -> None:
    # `imul cx` is dx:ax <- ax * cx -- two destinations, the shape
    # Semantics.dests already carries for the absorbed divide. Naming only ax
    # would tell a value-numbering pass dx still held what it held before.
    assert _semantics("F7 E9") == ir.Semantics(ir.Operation.MULTIPLY, "imul", (AX, DX), (AX, ir.Reg(Register.CX, 2)))
    assert _semantics("66 F7 E9") == ir.Semantics(ir.Operation.MULTIPLY, "imul", (EAX, EDX), (EAX, ECX))
    assert _defs("F7 E9") == frozenset({Register.EAX, Register.EDX})


def test_a_far_jump_is_control_leaving_the_body_with_no_target_invented() -> None:
    # Where it goes is a segment:offset a fixup writes, not something in the
    # instruction -- so no edge is built and none is guessed. All 14 in this
    # corpus are an event-poll stub's own tail.
    found = _semantics("EA 00 00 00 00")
    assert found == ir.Semantics(ir.Operation.ESCAPE, "jmp")
    assert found.target is None
    assert ir.modelled(found) is True


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


# Every encoding in the corpus this pass deliberately carries as a barrier
# rather than modelling. It was empty, and measured: `push cs`, `push ss`,
# `pop es`, `rep stosw`, `leave` and the far `jmp` were the whole of it -- 47
# of 17970 instructions -- and all six were modelled. Not a coverage floor
# with slack in it: an encoding joining this set is a decision to make
# deliberately, and one leaving it is a regression.
#
# movsw is the one deliberate refusal. suite/fpdeep.bas brought it in -- BC
# copies its SINGLE array initialiser with four of them -- and it is 44 of
# 21978 instructions, all in that one program's setup and none in a loop.
# Modelling it would mean giving Semantics a shape it does not have: a load
# and a store through two different segments, plus si and di stepping by a
# width the opcode implies and the operands never name. And it would buy
# nothing, because there is no other way to encode a movsw -- select.py's
# only correct answer is the byte that is already there, which is exactly
# what layout.py carries for a barrier.
REFUSED: set[int] = {Code.MOVSW_M16_M16}


def test_the_corpus_is_modelled_except_for_exactly_the_refused_encodings(fixtures: Path) -> None:
    unmodelled: set[int] = set()
    modelled = total = 0
    for path in sorted(fixtures.glob("*.obj")):
        found = corpus.loaded(path)
        assert found is not None
        result = corpus.bodies(path)
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
    # and the refusal is 44 instructions, not a share that could drift
    assert total - modelled == 44


def test_a_restore_and_a_table_carry_their_own_operations() -> None:
    assert ir.RESTORE_IDIOM.op is ir.Operation.RESTORE
    assert ir.TABLE_DATA.op is ir.Operation.DATA
    # the two predicates are one another's negation, on every operation there
    # is -- so "may I reason about this" and "must I carry this" can never
    # both be true, and can never both be false
    for op in ir.Operation:
        found = ir.Semantics(op)
        assert ir.modelled(found) is not ir.barrier(found)
    assert ir.barrier(ir.UNMODELLED) is True
    assert ir.modelled(ir.UNMODELLED) is False


@pytest.mark.parametrize(
    ("code", "op", "name"),
    [
        ("CB", ir.Operation.RETURN, "retf"),
        ("CA 04 00", ir.Operation.RETURN, "retf"),
        ("C1 E6 02", ir.Operation.BINARY, "shl"),
        ("D1 EE", ir.Operation.BINARY, "shr"),
        ("C1 FE 02", ir.Operation.BINARY, "sar"),
        ("90", ir.Operation.NOTHING, "nop"),
    ],
)
def test_the_frame_and_shift_forms_are_modelled(code: str, op: ir.Operation, name: str) -> None:
    found = _semantics(code)
    assert found.op is op
    assert found.name == name


def test_a_shift_reads_its_own_destination() -> None:
    """shl si,2 is dest <- dest shl 2 -- the shape _binary already models, and
    a value number keyed on it has to see si on both sides."""
    found = _semantics("C1 E6 02")
    assert found.dests[0] == found.sources[0]
    assert isinstance(found.sources[1], ir.Imm)


def test_a_far_return_carries_the_bytes_it_pops() -> None:
    """`retf 4` is callee cleanup, and the count is the calling convention --
    the one thing a lowering pass must reproduce exactly."""
    found = _semantics("CA 04 00")
    assert found.sources == (ir.Imm(value=4, width=2),)


def test_every_body_of_every_kind_is_fully_modelled(fixtures: Path) -> None:
    """Per-body liftability, split by kind, and exact rather than a floor.

    The history is the point. Every procedure ends in `retf n`, so leaving
    RETF unmodelled refused all 30 of them; modelling it left one, procs-p-ot's
    TWICE, whose /Ot prologue zeroes its frame with push ss / pop es /
    rep stosw and closes with `leave`. The 15 main bodies that refused all
    refused on the `push cs` under B$OEGA's far pointer, and all 14 event
    stubs on `push cs` and the far `jmp` in their own trampoline. So five
    encodings held 30 of 171 bodies, and one unmodelled instruction refuses
    the whole body it sits in either way -- which is the reason barriers
    exist and the reason these five were worth modelling instead.

    The eleven that refuse now are suite/fpdeep.bas's main bodies, on the
    movsw BC initialises its array with -- the one encoding REFUSED above
    names, for the reasons recorded there. They are refused as *fully
    modelled* and nothing more: all fifteen fpdeep objects still rebuild
    whole and run right on all twelve configurations, because carrying an
    instruction verbatim is what a barrier is.

    A body joining or leaving this is a decision, not a number to relax.
    """
    liftable: dict[extent.BodyKind, int] = {kind: 0 for kind in extent.BodyKind}
    refused: dict[extent.BodyKind, int] = {kind: 0 for kind in extent.BodyKind}
    for path in sorted(fixtures.glob("*.obj")):
        found = module.load(path)
        assert found is not None
        result = ir.decode_module(found)
        if isinstance(result, str):
            continue
        for body_ir in result:
            counted = liftable if all(ir.modelled(node.semantics) for node in body_ir.nodes) else refused
            counted[body_ir.body.kind] += 1
    assert liftable == {extent.BodyKind.MAIN: 144, extent.BodyKind.PROCEDURE: 30, extent.BodyKind.EVENT_STUB: 20}
    assert refused == {
        extent.BodyKind.MAIN: 11,
        extent.BodyKind.PROCEDURE: 0,
        extent.BodyKind.EVENT_STUB: 0,
    }


def test_a_barrier_is_carried_rather_than_refusing_the_body_it_sits_in() -> None:
    """The whole of Part 1, on the one shape this pass will never model.

    `in al,40h` is PITSNAP's own port read: its effect is a device's, so
    nothing about it is claimed. It still becomes a node with a complete
    Effects, its bytes still emit verbatim, and everything around it is
    modelled -- which is what lets a caller lift the body and leave this one
    instruction where it is.
    """
    code = hx("B8 01 00") + hx("E4 40") + hx("C3")  # mov ax,1 / in al,40h / ret
    insns = []
    at = 0
    while at < len(code):
        insn = decode(code, at)
        assert insn is not None
        insns.append(insn)
        at = insn.end

    block = Block(0, len(code), tuple(insns), Ends.RETURN, ())
    found = module.Module([], 0, "T", code, 0, len(code))
    mapped = CodeMap(frozenset(insn.at for insn in insns), frozenset())
    nodes = ir.decode_body(found, mapped, [block], Body(BodyKind.MAIN, 0, None, ((0, len(code)),)))

    port = nodes[1]
    assert ir.barrier(port.semantics) is True
    assert ir.modelled(port.semantics) is False
    assert [ir.modelled(node.semantics) for node in nodes] == [True, False, True]
    assert ir.emit(found, nodes) == code
    # a barrier's registers are the encoding's own, so the allocator may not
    # rename them; a modelled node pins nothing, which is what modelling buys
    assert ir.pinned(port) == frozenset({Register.EAX})
    assert ir.pinned(nodes[0]) == frozenset()
    # and its memory reach is unknown in both directions, so anything
    # promoted to a register is written back before it and re-read after
    assert port.effects.loads == ir.ANY_MEMORY
    assert port.effects.stores == ir.ANY_MEMORY


def test_a_barrier_whose_reach_is_wholly_unknown_pins_every_register() -> None:
    # `int 21h`: flow puts its registers beyond iced's own per-instruction
    # answer (Effects' None), and pinned() carries that through as None rather
    # than as an empty set a caller would read as "rename freely".
    code = hx("CD 21")
    insn = decode(code, 0)
    assert insn is not None
    node = ir.Opaque(insn, ir.instruction_effects(insn, module.literal_only), ir.UNMODELLED)
    assert node.effects.defs is None
    assert ir.pinned(node) is None

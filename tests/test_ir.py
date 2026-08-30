"""
qbopt/ir.py's own gate: decode every Body to Nodes, re-emit, byte-identical.

Everything here runs against the real corpus, not a hand-built stand-in
where one exists -- the segment/DGROUP work in test_module.py and
test_omf.py, and the segment-override refusal in test_lift.py, are this
module's own dependencies and are tested there.
"""

from pathlib import Path

from iced_x86 import Register

from qbopt import ir
from qbopt import omf
from helpers import hx
from qbopt import module
from qbopt.flags import ALL
from qbopt.flags import Flag
from qbopt.blocks import Ends
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
    assert effects.touches_memory is True
    assert effects.memory is None


def test_opaque_memory_effect_is_none_for_a_segment_override() -> None:
    insn = decode(hx("26 8B 06 34 12"), 0)
    assert insn is not None
    effects = ir.instruction_effects(insn, module.literal_only)
    assert effects.touches_memory is True
    assert effects.memory is None

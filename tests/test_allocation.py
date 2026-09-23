"""What an allocation has to get right, and six ways it did not.

Three of these fail today and say so. They are the defects the register
allocation work is for, written down while they were understood rather than
kept in a patch file: each names the program that printed the wrong number
and what the fix is. `strict=True`, so when one starts passing the suite
says so instead of quietly agreeing.

Each of these is a program that printed the wrong number while every host
test passed. They are gathered here rather than spread across the modules
they blame because they describe one thing: moving a value between
registers is only correct when everything that reads it moves with it, and
the machine has requirements about where it may go at all.
"""

from pathlib import Path

import pytest
from iced_x86 import Decoder
from iced_x86 import Register
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.backend import asm
from qbopt.backend import select
from qbopt.backend import target
from qbopt.objectfile import omf
from qbopt.legacy import regalloc
from qbopt.objectfile import module
from qbopt.frontend import blocks as split
from qbopt.frontend.blocks import code_map

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


def test_harr_keeps_its_inner_loop_value_out_of_a_spill_slot() -> None:
    """harr-p-g2 acquired three stack accesses per 10x10 loop iteration.

    A long interval took eax before the multiply's fixed inputs were
    assigned. Their eviction sent it directly toward spilling although a
    different initial placement fits the entire body in registers.
    Modeled cost grew from 11274 to 12876, worse than BC's 12454.
    """
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/harr-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    mapped = code_map(found)
    assert not isinstance(mapped, str), mapped
    reads = [
        insn
        for at in sorted(mapped.starts)
        for insn in (Decoder(16, found.code[at:], ip=at).decode(),)
        if insn.memory_base == Register.BP
    ]
    assert not reads, f"harr introduced frame accesses: {reads}"


def _shown(code: bytes) -> str:
    formatter = Formatter(FormatterSyntax.NASM)
    return "; ".join(formatter.format(one) for one in Decoder(16, code, ip=0))


def test_moving_a_preserved_value_does_not_rewrite_the_destination() -> None:
    """`mov ax,1` defines one value and preserves another in the same register.

    Writing ax leaves eax's high half alone, so MIR records a use of the
    eax before it. When an allocation moves *that* value, a single register
    map for the instruction says eax becomes ebx and rewrites the
    destination too -- which belongs to the value being defined, and has
    not moved anywhere. hotlop's counter became `mov bx,1` and was clobbered
    by the very value the move was meant to make room for.

    One map cannot say two things about one register. The remap is per side:
    destinations from what the operation defines, sources from what it uses.
    """
    kept = mir.Value(1, 0x10)
    older = mir.Value(2, 0x08)

    ax = ir.Reg(register=Register.AX, width=2)
    what = ir.Semantics(ir.Operation.MOVE, "mov", dests=(ax,), sources=(ir.Imm(value=1, width=2),))
    op = mir.Op(0x10, ir.Operation.MOVE, "mov", (kept,), (older,))

    origin = {kept: Register.EAX, older: Register.EAX}
    # The older value moves; the one this instruction defines does not.
    where = asm._where(op, {kept: Register.EAX, older: Register.EBX}, origin)
    made = select.emit(what, at=0, where=where)
    assert made is not None
    assert _shown(made.code) == "mov ax,1", f"the destination moved with a value that is not it: {_shown(made.code)}"


def test_a_lowered_origin_lets_the_assembler_remap_a_moved_value() -> None:
    """Lower keyed `origin` by `mir.Value` and the assembler looked up the
    instruction's value ids, so a value allocated away from where BC had it
    found no origin and was emitted in BC's register."""
    from qbopt.backend import lower

    made = mir.Value(1, 0x10, variable=7)
    copy = mir.Op(
        0x10,
        ir.Operation.MOVE,
        "mov",
        (made,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(1, 2),),
        results=(mir.Held(made, 2),),
    )
    body = mir.MirBody(0x10, (mir.MirBlock(0x10, (), (copy,), ()),))
    low = lower.lowered("origin", body, {}, set(), {}, hints=mir.AllocationHints(origins={7: Register.EAX}))
    (one,) = low.insns
    where = asm._where(one, {made.id: Register.EBX}, low.origin)
    assert where is not None and where[0][Register.AX] == Register.BX, where


def test_lir_says_a_two_address_operand_is_one_register() -> None:
    """`add ax,[c]` is ax at two moments, not two places."""
    ax = ir.Reg(register=Register.AX, width=2)
    cell = ir.Mem(None, 2)

    what = ir.Semantics(ir.Operation.BINARY, "add", dests=(ax,), sources=(ax, cell))
    assert target.tied(what) is Register.EAX, "the destination and the first source are one register"

    apart = ir.Semantics(ir.Operation.BINARY, "add", dests=(ax,), sources=(ir.Reg(register=Register.BX, width=2), cell))
    assert target.tied(apart) is None, "and a three-operand form ties nothing"

    on_stack = ir.Semantics(ir.Operation.FLOAT_ARITH, "fadd", dests=(ir.St(0),), sources=(ir.St(0),))
    assert target.tied(on_stack) is None, "x87 shares no register with the rest"


def test_a_two_address_operation_keeps_both_halves_in_one_register() -> None:
    """`add ax,[c]` is ax at two moments, not two places.

    x86 says so by naming the same operand twice and nothing else does, so
    an allocation that moves the value it defines without the value it reads
    emits `add di,[c]` -- which adds to whatever di held. matrix printed
    -2076 for 380.
    """
    ax = ir.Reg(register=Register.AX, width=2)
    cell = ir.Mem(None, 2)
    from types import SimpleNamespace

    what = ir.Semantics(ir.Operation.BINARY, "add", dests=(ax,), sources=(ax, cell))
    assert target.tied(what) is Register.EAX, "the destination and the first source are one register"

    apart = ir.Semantics(ir.Operation.BINARY, "add", dests=(ax,), sources=(ir.Reg(register=Register.BX, width=2), cell))
    assert target.tied(apart) is None, "and a three-operand form ties nothing"

    # Tied operands put the two values in one congruence class, so an
    # allocation moves both or neither.
    made = mir.Value(1, 0x10)
    read = mir.Value(2, 0x08)
    ref = mir.MemRef(None, 2)
    op = mir._RaisedOp(
        0x10,
        ir.Operation.BINARY,
        "add",
        (made,),
        (read,),
        loads=(ref,),
        kind=mir.Kind.ADD,
        args=(mir.Held(read, 2), mir.Cell(ref)),
        results=(mir.Held(made, 2),),
        node=SimpleNamespace(semantics=what),
    )
    body = mir._RaisedBody(0x10, (mir.MirBlock(0x10, (), (op,), ()),), origin={made: Register.EAX, read: Register.EAX})
    klass = regalloc.congruent(body)
    # Both present, not both absent: `.get` on two values neither of which
    # is in the map returns None twice, which compares equal and says
    # nothing at all.
    assert made in klass and read in klass, "neither value is in a class, so this proves nothing"
    assert klass[made] is klass[read], "a two-address instruction ties what it reads to what it writes"


def test_an_allocation_keeps_every_value_where_it_was_unless_forced() -> None:
    """Only what conflicts moves, and the invariant says which.

    A value may leave the register BC put it in when an interfering
    neighbour is genuinely sitting there, and not otherwise. Assigning in
    degree order, and counting only neighbours already assigned, let a class
    processed late find its own register taken and take someone else's --
    and that one did the same. A pin that had to displace one value
    displaced twelve in lngmix, and somewhere down that cascade a value and
    its readers stopped agreeing.
    """
    seen = 0
    for obj in FIXTURES[:40]:
        found = module.of(omf.parse(obj.read_bytes()))
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue

        raised = mir.bodies(found, split.partition(found, mapped))
        for body_name, public in raised:
            body = mir._with_hints(public, raised.hints[public.entry])
            graph = regalloc.interference(body)
            victim = next((one for one in graph if not one.flags and one in body.origin), None)
            if victim is None:
                continue

            # Whichever register it will take. Asking for one no value uses
            # finds nothing: BC's loops occupy all six, which is why the
            # first version of this test never pinned anything at all and
            # reported success by doing nothing.
            for want in target.AVAILABLE:
                if want is body.origin[victim]:
                    continue
                got = regalloc.colour(body, {victim: want})
                if isinstance(got, str):
                    continue
                seen += 1
                klass = regalloc.congruent(body)
                homes: dict = {}
                for one in got:
                    homes.setdefault(klass.get(one, one), set()).add(body.origin.get(one))
                pinned = klass.get(victim, victim)
                for value, where in got.items():
                    was = body.origin.get(value)
                    if was is None or where is was or value is victim:
                        continue
                    # A phi ties a class together and it moves as one, so a
                    # member of the pinned value's own class did not move on
                    # its own account.
                    if klass.get(value, value) is pinned:
                        continue
                    # And a class whose members disagree about where BC put
                    # them has no home to keep, so moving it is not unforced.
                    if len(homes[klass.get(value, value)]) != 1:
                        continue
                    # It moved. Something the class interferes with has to
                    # be in the register it left, or nothing forced it --
                    # the class, because a class moves as one: addrm's v1_1
                    # has no neighbours at all and left eax because its
                    # sibling v1_12 interferes with the pinned value now
                    # sitting there. Asked of the value alone this reported
                    # an unforced move that never happened.
                    kin = {value} | {one for one in klass if klass.get(one, one) is klass.get(value, value)}
                    assert [one for near in kin for one in graph.get(near, ()) if got.get(one) is was], (
                        f"{obj.stem} {body_name}: {value} left {regalloc.NAMES.get(was, was)} with nothing in it"
                    )
                break
    assert seen, "no body took a pin, so this proves nothing"


def test_resolving_after_a_move_relinks_by_register() -> None:
    """Which is why a pass that moves code must not ask for it.

    resolved() rebuilds def-use the way raise_body does, from registers,
    because that is all machine code carries. After a definition has moved
    the use is re-linked to whatever the block last wrote to that register,
    and the value the pass moved is simply gone. hotlop printed 1656 for 630
    with no register named anywhere.

    Recorded rather than fixed: this is the right behaviour when raising and
    the wrong thing to call afterwards.
    """
    first = mir.Value(1, 0x10)
    second = mir.Value(2, 0x14)
    third = mir.Value(3, 0x18)

    # Two definitions of the same register, and a reader of the first.
    head = mir.MirBlock(
        0x10,
        (),
        (
            mir.Op(0x10, ir.Operation.MOVE, "mov", (first,), (), (mir.MemRef(None, 2),)),
            mir.Op(0x14, ir.Operation.MOVE, "mov", (second,), (), (mir.MemRef(None, 2),)),
            mir.Op(0x18, ir.Operation.BINARY, "add", (third,), (first,), (mir.MemRef(None, 2),)),
        ),
        (),
    )
    body = mir._RaisedBody(0x10, (head,), origin={first: Register.EAX, second: Register.EAX, third: Register.EAX})

    got = mir.resolved(body, {})
    assert not isinstance(got, str), got
    reader = got.blocks[0].ops[2]
    assert first not in reader.uses, (
        "resolved() kept a use pointing at a value the last write to that register replaced"
    )


def test_the_requirements_table_says_what_the_encoding_permits() -> None:
    """Legal and assignable are different questions.

    16-bit addressing reaches memory through bx, bp, si or di; regalloc may
    not hand out bp, because it is the frame pointer. Answering both with
    one set made every `[bp-12h]` in the corpus look like a violated
    requirement, and intersecting lir's 16-bit names with regalloc's 32-bit
    roots gave the empty set -- so nothing satisfied the class at all, and
    two programs quietly stopped hoisting.
    """
    assert Register.BP in target.ADDRESSING, "a frame slot is reached through bp"
    assert Register.DX not in target.ADDRESSING, "`[dx+0Ah]` has no encoding"
    assert target.BASES, "and the assignable set is not empty"
    assert all(one in target.AVAILABLE for one in target.BASES)
    assert not any(one is Register.EBP for one in target.BASES), "bp is the frame pointer"


def _through_regalloc(body, pinned=None):
    """The whole phase, which is where the constraint splitter runs."""
    from qbopt.backend import allocate
    from qbopt.backend import frame as frames

    return allocate.RegAlloc(pinned or {}, frames.of(body)).transform(body)


def _one_block(*insns):
    return lir.LirBody("one", 0, (lir.LirBlock(at=0, insns=insns, succ=()),), origin={}, pins={})


def _mov(into: int, value: int, at: int):
    from qbopt.model import ir

    return lir.Insn(
        at=at,
        covers=(at, at + 2),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(into, 2),), (ir.Imm(value, 2),)),
        defines=(into,),
        uses=(),
        op=None,
    )


def _shl(result: int, count: int, at: int):
    from qbopt.model import ir

    what = ir.Semantics(ir.Operation.BINARY, "shl", (ir.Held(result, 2),), (ir.Held(result, 2), ir.Held(count, 2)))
    return lir.Insn(at=at, covers=(at, at + 2), what=what, defines=(result,), uses=(result, count), op=None)


def _named(body, name: str):
    from qbopt.model import ir

    return [
        one.what
        for block in body.blocks
        for one in block.insns
        if one.what and one.what.name == name and not isinstance(one.what.sources[0], ir.Imm)
    ]


def test_a_widening_multiply_puts_its_halves_in_ax_and_dx() -> None:
    """pressx printed R= 6460 for 7500: the halves went to cx and ax and
    the `add` after the multiply read a register it never wrote."""
    from iced_x86 import Register

    from qbopt.model import ir

    what = ir.Semantics(
        ir.Operation.MULTIPLY,
        "imul",
        (ir.Held(1, 2), ir.Held(2, 2)),
        (ir.Held(1, 2), ir.Held(3, 2)),
    )
    imul = lir.Insn(at=0x100, covers=(0x100, 0x102), what=what, defines=(1, 2), uses=(1, 3), op=None)
    got = _through_regalloc(_one_block(_mov(1, 3, 0xFC), _mov(3, 5, 0xFE), imul))
    (made,) = _named(got, "imul")
    assert ir.ROOT[made.dests[0].register] is Register.EAX, made.dests
    assert ir.ROOT[made.dests[1].register] is Register.EDX, made.dests
    assert made.sources[0] == made.dests[0], "the tie was broken"


def test_a_half_register_and_its_whole_are_the_same_register() -> None:
    """dx is edx's low half, so a value in edx does not survive a write to dx.

    lngmix printed S= 771897293 for 142900 with both its divides hoisted:
    the long the second one returned was given edx, the restore idiom
    beside it delivers its high half in dx, and the two were counted as
    different registers -- so the idiom overwrote the answer the loop
    went on to read.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.backend import allocate

    # Three longer-lived values take the registers ahead of edx, so the
    # long below is placed in it, and a half-width value pinned to dx is
    # defined while the long is still live.
    holds = [_mov(one, one, 0xF0 + 2 * index) for index, one in enumerate((3, 4, 5))]
    wide = lir.Insn(
        at=0x100,
        covers=(0x100, 0x104),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 4),), (ir.Imm(7, 4),)),
        defines=(1,),
        uses=(),
        op=None,
    )
    half = _mov(2, 3, 0x104)
    read = lir.Insn(
        at=0x106,
        covers=(0x106, 0x10A),
        what=ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(1, 4),), (ir.Held(1, 4), ir.Held(2, 2))),
        defines=(1,),
        uses=(1, 2),
        op=None,
    )
    last = lir.Insn(
        at=0x10A,
        covers=(0x10A, 0x10C),
        what=ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(3, 2),), (ir.Held(3, 2), ir.Held(4, 2))),
        defines=(3,),
        uses=(3, 4, 5),
        op=None,
    )
    got = allocate.allocate(_one_block(*holds, wide, half, read, last), pinned={2: Register.DX})
    long, pinned = got.where.get(1), got.where.get(2)
    assert long is not None or pinned is not None, got.where
    if long is not None and pinned is not None:
        assert ir.ROOT.get(long, long) is not ir.ROOT.get(pinned, pinned), (
            f"the long was given {long!r} and the pinned value {pinned!r}, which are one register"
        )


def test_a_soft_preference_changes_free_register_order_without_becoming_a_pin() -> None:
    """r_walk needs to compare counter placements without declaring one ABI-fixed.

    A role trial must be able to try DX before EAX for a freely placeable
    value, but DX is still only a preference: if it is unavailable, ordinary
    allocation may choose another legal register instead of reporting an
    impossible fixed assignment.
    """
    from qbopt.backend import allocate

    one = allocate.allocate(_one_block(_mov(1, 1, 0x100)), preferred={1: Register.EDX})
    assert one.where[1] is Register.EDX
    overlap = lir.Insn(
        at=0x104,
        covers=(0x104, 0x106),
        what=ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(1, 2),), (ir.Held(1, 2), ir.Held(2, 2))),
        defines=(1,),
        uses=(1, 2),
        op=None,
    )
    held = allocate.allocate(
        _one_block(_mov(2, 2, 0x100), _mov(1, 1, 0x102), overlap),
        pinned={2: Register.EDX},
        preferred={1: Register.EDX},
    )
    assert held.where[1] is not Register.EDX


def test_an_explicit_soft_preference_beats_an_ordinary_copy_hint() -> None:
    """A role trial asked for DX, but a dead source's AX copy hint won.

    Copy hints are profitable tiebreakers only.  They must not silently
    reverse the explicit, still-fallible register role that a complete
    pressure candidate is evaluating.  The source dies at the copy, so both
    AX and DX are legal for its successor; the requested DX has to win.
    """
    from qbopt.backend import allocate

    source = _mov(2, 2, 0x100)
    copy = lir.Insn(
        at=0x102,
        covers=(0x102, 0x104),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(1, 2),), (ir.Held(2, 2),)),
        defines=(1,),
        uses=(2,),
        op=None,
    )
    use = lir.Insn(
        at=0x104,
        covers=(0x104, 0x106),
        what=ir.Semantics(ir.Operation.BINARY, "add", (ir.Held(1, 2),), (ir.Held(1, 2), ir.Imm(1, 2))),
        defines=(1,),
        uses=(1,),
        op=None,
    )
    got = allocate.allocate(
        _one_block(source, copy, use),
        pinned={2: Register.EAX},
        preferred={1: Register.EDX},
    )
    assert got.where[1] is Register.EDX, got.where


def test_folded_spill_cost_depends_on_the_selected_cpu() -> None:
    """A folded source is not free on a 386.

    Seven values are live at the final add, so one must spill.  Value 7 dies
    in an operand that the spiller can fold.  Treating that read as free on
    every target made the allocator spill it on a 386 even though
    ``add reg,[mem]`` costs exactly the ``add reg,reg`` plus explicit load it
    replaced.  Core prices both ALU forms equally, so value 7 really is the
    cheapest spill there.
    """
    from qbopt.model import ir
    from qbopt.backend import allocate

    holds = tuple(_mov(value, value, 0x100 + 2 * (value - 1)) for value in range(1, 8))
    folded = lir.Insn(
        at=0x110,
        covers=(0x110, 0x112),
        what=ir.Semantics(
            ir.Operation.BINARY,
            "add",
            (ir.Held(1, 2),),
            (ir.Held(1, 2), ir.Held(7, 2)),
        ),
        defines=(1,),
        uses=(1, 7),
        op=None,
    )
    final = lir.Insn(
        at=0x112,
        covers=(0x112, 0x114),
        what=None,
        defines=(),
        uses=(1, 2, 3, 4, 5, 6),
        widths=tuple((value, 2) for value in range(1, 7)),
        op=None,
    )
    body = _one_block(*holds, folded, final)

    on_386 = allocate.allocate(body, cpu="386")
    on_core = allocate.allocate(body, cpu="Core")

    assert 7 not in on_386.spilled, on_386
    assert on_core.spilled == frozenset({7}), on_core


def test_a_fixed_source_that_is_not_the_multiply_pair_is_honoured() -> None:
    """A variable shift counts from cl and names it nowhere."""
    from iced_x86 import Register

    from qbopt.model import ir

    got = _through_regalloc(_one_block(_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104)))
    (made,) = _named(got, "shl")
    assert ir.ROOT[made.sources[1].register] is Register.ECX, made.sources


def test_a_value_required_in_two_registers_gets_one_fresh_value_per_site() -> None:
    """One value cannot be in two registers at once, so each site gets its
    own short-lived value and the original keeps one place."""
    from iced_x86 import Register

    from qbopt.model import ir

    cwd = lir.Insn(
        at=0x106,
        covers=(0x106, 0x107),
        what=ir.Semantics(ir.Operation.EXTEND, "cwd", (ir.Held(8, 2),), (ir.Held(7, 2),)),
        defines=(8,),
        uses=(7,),
        op=None,
    )
    got = _through_regalloc(_one_block(_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104), cwd))
    (shift,) = _named(got, "shl")
    (extend,) = _named(got, "cwd")
    assert ir.ROOT[shift.sources[1].register] is Register.ECX, shift.sources
    assert ir.ROOT[extend.sources[0].register] is Register.EAX, extend.sources


def test_an_origin_pin_does_not_override_what_the_instruction_requires() -> None:
    """A pin says where a value already was; a shift count in anything but
    cl is not an instruction. They are different values, so neither can
    override the other."""
    from iced_x86 import Register

    from qbopt.model import ir

    body = _one_block(_mov(9, 1, 0x100), _mov(7, 2, 0x102), _shl(9, 7, 0x104))
    got = _through_regalloc(body, {7: Register.EBX})
    (made,) = _named(got, "shl")
    assert ir.ROOT[made.sources[1].register] is Register.ECX, made.sources


@pytest.mark.parametrize("stem", ["matrix-p-g2", "nested-p-g2"])
def test_the_rewriter_hands_on_the_bytes_a_dropped_copy_stood_for(stem: str) -> None:
    """matrix and nested refused with `2 bytes between the ops are not
    instructions` at a `mov bx,ax`. The rewriter drops such a copy once
    both ends land in one register -- and the bytes it stood for went with
    it, so layout could not account for them."""
    from pathlib import Path

    from qbopt import flow
    from qbopt.model import mir
    from qbopt.backend import lower
    from qbopt.objectfile import omf
    from qbopt.objectfile import module
    from qbopt.optimize import transform
    from qbopt.backend import frame as frames
    from qbopt.frontend import blocks as split
    from qbopt.frontend.blocks import code_map

    found = module.of(omf.parse(Path(f"fixtures/omf/{stem}.obj").read_bytes()))
    blocks = split.partition(found, code_map(found))
    raised = mir.bodies(found, blocks)
    name, body = next(iter(raised))
    body = transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
    low = lower.lowered(
        name,
        body,
        found.calls,
        set(found.absorbed),
        runtime.for_module(found),
        nodes=raised.source.nodes,
        hints=raised.hints[body.entry],
    )
    owned = lambda one: {  # noqa: E731
        at for block in one.blocks for i in block.insns if i.covers for at in range(*i.covers)
    }
    was = owned(low)
    for phase in flow.machine(low.pins, frames.of(low), found.calls):
        low = phase.transform(low)
    lost = sorted(was - owned(low))
    assert not lost, f"bytes owned by nothing: {[hex(x) for x in lost[:4]]}"


def _based_cell(through=None):
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    where = Addr(Space.SEGMENT, 0x10, base=Register.SI)
    cell = ir.Mem(where, 2, through if through is not None else Register.NONE, 0, 2, base=ir.Held(21, 2))
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(30, 2),), (cell,))
    return lir.Insn(at=0x100, covers=(0x100, 0x104), what=what, defines=(30,), uses=(21,), op=None)


def test_a_placed_cell_reaches_memory_by_the_register_its_value_got() -> None:
    """`through` follows the assignment and `base` stays: arrprm stored one
    element through the other's address because the cell kept BC's own
    register whatever the allocator chose."""
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.backend import allocate

    for register in (Register.EBX, Register.ESI):
        got = allocate._settled(_based_cell().what.sources[0], {21: register})
        assert isinstance(got, ir.Mem)
        assert got.through == target.named(register, 2), f"{register}: {got.through}"
        assert got.base == ir.Held(21, 2), "the cell stopped naming its value"
        for field in ("addr", "width", "offset", "disp_width"):
            assert getattr(got, field) == getattr(_based_cell().what.sources[0], field), field


def test_a_call_result_used_as_a_base_leaves_the_register_it_was_delivered_in() -> None:
    """nbodys placed `fld [si]`'s base in AX, where the call left it.

    The allocator released the whole-range AX pin, but constrain still found
    it in `body.pins` and judged the result already where the call delivers
    it, so nothing split it and `[ax]`, which has no 16-bit encoding, was
    emitted.
    """
    from dataclasses import replace

    result = ir.Held(21, 2)
    call = lir.Insn(
        at=0xF0,
        covers=(0xF0, 0xF3),
        what=ir.Semantics(ir.Operation.CALL, "call", (), ()),
        defines=(21,),
        uses=(),
        op=None,
        delivers=((result, Register.AX),),
    )
    body = replace(_one_block(call, _based_cell()), pins={21: Register.EAX})
    placed = _through_regalloc(body, {21: Register.EAX})
    load = next(one for one in placed.insns if one.at == 0x100 and isinstance(one.what.sources[0], ir.Mem))
    assert load.what.sources[0].through in target.ADDRESSING, load.what.sources[0].through


def test_a_cell_whose_address_nothing_placed_is_refused() -> None:
    """Guessing a base register is how arrprm printed ' 0  0' for ' 7  8'."""
    from qbopt.backend import select

    assert select.emit(_based_cell().what) is None


def test_a_placed_cell_emits_the_register_it_was_given() -> None:
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.backend import select

    cell = _based_cell(Register.SI).what.sources[0]
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (cell,))
    made = select.emit(what)
    assert made is not None, "a placed cell was refused"
    assert made.code.hex().startswith("8b84"), made.code.hex()  # mov ax,[si+disp]


def test_a_based_cell_keeps_the_register_the_allocation_gave_its_base() -> None:
    """`_placed` threw its own answer away.

    `Mem.through` is `compare=False`, so a cell resolved from `through=NONE`
    to a register compares equal to the one it came from, and the equality
    early-return in `_placed` returned the untouched instruction. The
    allocator resolved arrprm's cell twice and both results were dropped,
    so select saw an unplaced base and the whole body fell back to MIR.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import allocate
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    cell = ir.Mem(where, 2, Register.NONE, 2, 1, base=ir.Held(17, 2))
    load = lir.Insn(
        at=0xA2,
        covers=(0xA2, 0xA4),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (cell,)),
        defines=(),
        uses=(17,),
        op=None,
    )
    body = lir.LirBody(
        name="one",
        entry=0,
        blocks=(lir.LirBlock(at=0, insns=(load,), succ=()),),
        origin={},
        pins={},
    )
    got = allocate.applied(
        body, allocate.Assignment(where={17: Register.BX}, spilled=frozenset(), cost=0.0, optimal=True)
    )
    read = got.blocks[0].insns[0].what.sources[0]
    assert read.through == Register.BX, f"the cell is still reached through {read.through}"
    assert read.base == ir.Held(17, 2), "the cell stopped saying which value reached it"
    assert (read.addr, read.width, read.offset, read.disp_width) == (where, 2, 2, 1)


def test_a_fixed_call_argument_reaches_its_register_through_the_whole_phase() -> None:
    """The pin `constrained` hands back is the only record of where an
    operand-free requirement went.

    `constrain.required` re-derives a requirement from the operand holding
    the fresh value, which works for `imul`'s tie and cannot work for a
    call: it names no operand, `constrained` consumes the requirement, and
    the fresh value's register lived only in the map RegAlloc dropped. So
    B$ENRA's arguments were coloured like any other value -- arrprm put
    them in dx and di where the runtime reads cx and bx.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    # Loaded, not a constant: a constant is simply made in cx.
    made = lir.Insn(
        at=0,
        covers=(0, 3),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(2, 2),), (ir.Mem(Addr(Space.SEGMENT, 0x10), 2),)),
        defines=(2,),
        uses=(),
        op=None,
    )
    call = lir.Insn(
        at=3,
        covers=(3, 8),
        what=ir.Semantics(ir.Operation.CALL, "call", (), ()),
        defines=(),
        uses=(2,),
        op=None,
        requires=((ir.Held(2, 2), Register.CX),),
    )
    body = _one_block(made, call)
    # The argument itself lives in si, which is not where the call reads it.
    got = _through_regalloc(body, {2: Register.SI})
    insns = got.blocks[0].insns
    assert len(insns) == 3, f"the pre-call copy is missing: {len(insns)} instructions"
    copy = insns[1]
    assert copy.what.dests[0] == ir.Reg(Register.CX, 2), (
        f"the fresh value went to {copy.what.dests[0]}, not the register the call reads"
    )
    assert copy.what.sources[0] == ir.Reg(Register.SI, 2), f"the argument moved off si: {copy.what.sources[0]}"
    assert made_dest(insns[0]) == ir.Reg(Register.SI, 2), "the original was pinned away from si"


def made_dest(one):
    return one.what.dests[0]


def test_a_call_result_nothing_reads_becomes_a_clobber() -> None:
    """A call defines every register the raise cannot prove it preserves,
    and three of B$EVCK's six in bools-q-evt are read by nothing.

    Kept as values they carry a singleton pin each, so eax, ecx and edx
    are reserved for results that do not exist; the spiller then chose one
    of them every round -- 118, then 119, then 120 -- storing a dead value
    and freeing nothing. As clobbers they still keep a live range out of
    those registers, which is the fact worth keeping.
    """
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.backend import allocate

    call = lir.Insn(
        at=0x10,
        covers=(0x10, 0x15),
        what=ir.Semantics(ir.Operation.CALL, "call", (), ()),
        defines=(11, 12),
        uses=(),
        op=None,
        clobbers=frozenset({Register.ESI}),
    )
    read = lir.Insn(
        at=0x15,
        covers=(0x15, 0x17),
        what=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Held(20, 2),), (ir.Held(12, 2),)),
        defines=(20,),
        uses=(12,),
        op=None,
    )
    body = _one_block(call, read)
    got, pins = allocate.narrowed(body, {11: Register.EAX, 12: Register.ECX})
    after = got.blocks[0].insns[0]
    assert after.defines == (12,), f"the dead result survived: {after.defines}"
    assert 11 not in pins, f"its pin survived: {pins}"
    assert pins.get(12) is Register.ECX, "the read result lost its pin"
    assert Register.EAX in after.clobbers, f"the register it destroyed was forgotten: {after.clobbers}"
    assert Register.ESI in after.clobbers, "an existing clobber was dropped"


def test_a_value_minted_for_a_fixed_register_is_not_spilled_out_of_it() -> None:
    """The only reason it exists is to be in that register.

    constrain.py splits a value an instruction requires in a register it
    names nowhere into a value of its own, live across that instruction
    and nothing else. Spilled, the reload that replaced it carried no
    requirement at all: nested and harr reached emission with their
    fixed-register input in no register, and whatever the reload landed in
    is what the instruction read.
    """
    from pathlib import Path

    from qbopt import wholeseg
    from qbopt.backend import constrain
    from qbopt.backend import allocate as alloc

    minted: dict = {}
    last: list = []
    was_c, was_a = constrain.constrained, alloc.allocate

    def note_constrained(body, pinned):
        got, fixed = was_c(body, pinned)
        minted.update(fixed)
        return got, fixed

    def note_allocate(body, pinned=None, unspillable=None, *, cpu="386"):
        got = was_a(body, pinned, unspillable, cpu=cpu)
        last.append(got)
        return got

    for name in ("harr-p-g2", "segld-p-g2"):
        minted.clear()
        last.clear()
        constrain.constrained = note_constrained
        alloc.constrain = constrain
        alloc.allocate = note_allocate
        try:
            wholeseg.rebuilt(Path(f"fixtures/omf/{name}.obj").read_bytes())
        finally:
            constrain.constrained = was_c
            alloc.constrain = constrain
            alloc.allocate = was_a
        assert minted, f"{name} requires nothing in a register it names nowhere; this proves nothing"
        where = last[-1].where
        wrong = {one: want for one, want in minted.items() if where.get(one) is not want}
        assert not wrong, f"{name}: {sorted(wrong)} were minted for a register and did not get it"


def test_conflicting_hard_register_assignments_are_unplaceable_not_spills() -> None:
    """Two simultaneous DX requirements must fail honestly.

    A fixed assignment is a hardware fact, rather than an allocation
    preference.  Greedy allocation used to reserve DX for the first value,
    then quietly return the second in ``spilled`` when DX was still occupied.
    The spiller would reload it into an arbitrary register, turning an
    impossible encoding requirement into malformed code instead of naming the
    impossible allocation.
    """
    from qbopt.backend import allocate

    first = _mov(1, 1, 0)
    second = _mov(2, 2, 2)
    both = _shl(1, 2, 4)

    with pytest.raises(allocate.Unplaced, match=r"value#2 cannot be placed"):
        allocate.allocate(_one_block(first, second, both), pinned={1: Register.DX, 2: Register.DX})


def test_a_hard_register_assignment_is_not_an_eviction_victim() -> None:
    """A later expensive range may not take a hard register assignment.

    The far-owner allocation experiment fixed an owner in DI.  A hotter
    recurrence then evicted that fixed interval because greedy eviction
    charged its low spill weight, as though a reload in any register could
    still meet DI's requirement.  A hard assignment has no such recovery;
    this must be reported as pressure or resolved by an explicit split.
    """
    from qbopt.backend import allocate
    from qbopt.analysis import intervals

    held = intervals.Interval(1, (intervals.Segment(0, 4),), weight=0.1)
    incoming = intervals.Interval(2, (intervals.Segment(0, 4),), weight=10.0)

    got = allocate._evict(
        incoming,
        (Register.DI,),
        {allocate._whole(Register.DI): [1]},
        {1: held, 2: incoming},
        [],
        protected=frozenset({1}),
    )
    assert got is None, got


def test_an_unspillable_range_without_a_register_is_unplaceable() -> None:
    """farloadloop's forced pressure plan left reload v118 unassigned.

    Every general register was occupied by a hard range and the short reload
    could not be spilled again.  The allocator retried that impossible value
    until its queue budget expired, then returned it in neither ``where`` nor
    ``spilled``; final rewriting failed much later with ``no register``.
    The allocation boundary must report the impossible pressure directly.
    """
    from qbopt.model import ir
    from qbopt.backend import target
    from qbopt.backend import allocate

    holds = tuple(_mov(value, value, 2 * (value - 1)) for value in range(1, 8))
    use_all = lir.Insn(
        at=14,
        covers=(14, 16),
        what=ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        defines=(),
        uses=tuple(range(1, 8)),
        op=None,
    )
    pins = {value: register for value, register in zip(range(1, 7), target.AVAILABLE, strict=True)}

    with pytest.raises(allocate.Unplaced, match=r"value#7 cannot be spilled"):
        allocate.allocate(_one_block(*holds, use_all), pinned=pins, unspillable=frozenset({7}))


def test_a_rejected_allocation_trial_leaves_no_frame_slot(monkeypatch: pytest.MonkeyPatch) -> None:
    """sum_three.c's loop-base trial spilled its index, lost, and kept the slot.

    The index was later spilled for real into that stale slot, already shared
    with the first array's base, and the program returned 330 for 1110. A
    value that ends in a register must hold no frame slot.
    """
    from qbopt.backend import allocate
    from qbopt.cfront import compile as cfront

    seen = []
    transform = allocate.RegAlloc.transform

    def recorded(self, body):
        out = transform(self, body)
        seen.append(self.frame)
        return out

    applied = allocate.applied
    placed = []

    def recording(body, got):
        placed.append(got.where)
        return applied(body, got)

    monkeypatch.setattr(allocate.RegAlloc, "transform", recorded)
    monkeypatch.setattr(allocate, "applied", recording)
    source = Path(__file__).resolve().parents[1] / "bench" / "parity" / "sum_three.c"
    cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    assert seen and len(seen) == len(placed)
    for frame, where in zip(seen, placed):
        assert not {value for value in frame.slots if value in where}

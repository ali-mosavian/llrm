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
from iced_x86 import Formatter
from iced_x86 import FormatterSyntax
from iced_x86 import Register

from qbopt import ir
from qbopt import lir
from qbopt import mir
from qbopt import omf
from qbopt import layout
from qbopt import module
from qbopt import regalloc
from qbopt import transform
from qbopt import select
from qbopt import blocks as split
from qbopt.blocks import code_map

import corpus

FIXTURES = sorted(Path("fixtures/omf").glob("*.obj"))


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
    op = mir.Op(0x10, ir.Operation.MOVE, "mov", (kept,), (older,), made=what)

    origin = {kept: Register.EAX, older: Register.EAX}
    # The older value moves; the one this instruction defines does not.
    where = layout._where(op, {kept: Register.EAX, older: Register.EBX}, origin)
    made = select.emit(what, at=0, where=where)
    assert made is not None
    assert _shown(made.code) == "mov ax,1", f"the destination moved with a value that is not it: {_shown(made.code)}"


def test_lir_says_a_two_address_operand_is_one_register() -> None:
    """`add ax,[c]` is ax at two moments, not two places."""
    ax = ir.Reg(register=Register.AX, width=2)
    cell = ir.Mem(None, 2)

    what = ir.Semantics(ir.Operation.BINARY, "add", dests=(ax,), sources=(ax, cell))
    assert lir.tied(what) is Register.EAX, "the destination and the first source are one register"

    apart = ir.Semantics(
        ir.Operation.BINARY, "add", dests=(ax,), sources=(ir.Reg(register=Register.BX, width=2), cell)
    )
    assert lir.tied(apart) is None, "and a three-operand form ties nothing"

    on_stack = ir.Semantics(ir.Operation.FLOAT_ARITH, "fadd", dests=(ir.St(0),), sources=(ir.St(0),))
    assert lir.tied(on_stack) is None, "x87 shares no register with the rest"


def test_a_two_address_operation_keeps_both_halves_in_one_register() -> None:
    """`add ax,[c]` is ax at two moments, not two places.

    x86 says so by naming the same operand twice and nothing else does, so
    an allocation that moves the value it defines without the value it reads
    emits `add di,[c]` -- which adds to whatever di held. matrix printed
    -2076 for 380.
    """
    ax = ir.Reg(register=Register.AX, width=2)
    cell = ir.Mem(None, 2)

    what = ir.Semantics(ir.Operation.BINARY, "add", dests=(ax,), sources=(ax, cell))
    assert lir.tied(what) is Register.EAX, "the destination and the first source are one register"

    apart = ir.Semantics(
        ir.Operation.BINARY, "add", dests=(ax,), sources=(ir.Reg(register=Register.BX, width=2), cell)
    )
    assert lir.tied(apart) is None, "and a three-operand form ties nothing"

    # Tied operands put the two values in one congruence class, so an
    # allocation moves both or neither.
    made = mir.Value(1, 0x10)
    read = mir.Value(2, 0x08)
    op = mir.Op(0x10, ir.Operation.BINARY, "add", (made,), (read,), made=what)
    body = mir.MirBody(
        0x10, (mir.MirBlock(0x10, (), (op,), ()),), {made: Register.EAX, read: Register.EAX}, {}
    )
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

        for body_name, body in mir.bodies(found, split.partition(found, mapped)):
            graph = regalloc.interference(body)
            victim = next((one for one in graph if not one.flags and one in body.origin), None)
            if victim is None:
                continue

            # Whichever register it will take. Asking for one no value uses
            # finds nothing: BC's loops occupy all six, which is why the
            # first version of this test never pinned anything at all and
            # reported success by doing nothing.
            for want in regalloc.AVAILABLE:
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
                    # It moved. Something it interferes with has to be in
                    # the register it left, or nothing forced it.
                    assert [one for one in graph.get(value, ()) if got.get(one) is was], (
                        f"{obj.stem} {body_name}: {value} left "
                        f"{regalloc.NAMES.get(was, was)} with nothing in it"
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
    ax = ir.Reg(register=Register.AX, width=2)
    cell = ir.Mem(None, 2)
    load = ir.Semantics(ir.Operation.MOVE, "mov", dests=(ax,), sources=(cell,))
    use = ir.Semantics(ir.Operation.BINARY, "add", dests=(ax,), sources=(ax, cell))

    first = mir.Value(1, 0x10)
    second = mir.Value(2, 0x14)
    third = mir.Value(3, 0x18)

    # Two definitions of the same register, and a reader of the first.
    head = mir.MirBlock(
        0x10,
        (),
        (
            mir.Op(0x10, ir.Operation.MOVE, "mov", (first,), (), (mir.MemRef(None, 2),), made=load),
            mir.Op(0x14, ir.Operation.MOVE, "mov", (second,), (), (mir.MemRef(None, 2),), made=load),
            mir.Op(0x18, ir.Operation.BINARY, "add", (third,), (first,), (mir.MemRef(None, 2),), made=use),
        ),
        (),
    )
    body = mir.MirBody(
        0x10, (head,), {first: Register.EAX, second: Register.EAX, third: Register.EAX}, {}
    )

    got = mir.resolved(body, {})
    assert not isinstance(got, str), got
    reader = got.blocks[0].ops[2]
    assert first not in reader.uses, (
        "resolved() kept a use pointing at a value the last write to that register replaced"
    )


@pytest.mark.parametrize("obj", FIXTURES[:60], ids=lambda p: p.stem)
def test_a_body_no_pass_touched_is_allocated_to_the_identity(obj: Path) -> None:
    """Running an allocation over code that was already correct is not free.

    Every move it makes is a move nothing asked for, and one of them was
    wrong: lngmix printed 142850 for 142900 on four configurations without
    any pass having hoisted anything at all.
    """
    found = module.of(omf.parse(obj.read_bytes()))
    if found is None:
        return
    mapped = code_map(found)
    if isinstance(mapped, str):
        return

    for name, body in mir.bodies(found, split.partition(found, mapped)):
        if body.pins:
            continue
        got = regalloc.colour(body, {})
        if isinstance(got, str):
            continue
        assert not regalloc.moved(body, got), (
            f"{obj.stem} {name}: an untouched body allocated away from where BC had it"
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
    assert Register.BP in lir.ADDRESSING, "a frame slot is reached through bp"
    assert Register.DX not in lir.ADDRESSING, "`[dx+0Ah]` has no encoding"
    assert regalloc.ADDRESSING, "and the assignable set is not empty"
    assert all(one in regalloc.AVAILABLE for one in regalloc.ADDRESSING)
    assert not any(one is Register.EBP for one in regalloc.ADDRESSING), "bp is the frame pointer"


def test_the_allocator_honours_a_register_an_operation_demands() -> None:
    """lir says which register an operand must be in. Something has to ask.

    The table has been right since it was written and nothing read it. That
    was safe only by accident: the identity assignment puts every value back
    where BC had it, so all 1,153 of these requirements were already met and
    none was ever tested. It stops being safe the moment a value moves.
    """
    seen = 0
    for obj in sorted(Path("fixtures/omf").glob("*.obj")):
        found = corpus.loaded(obj)
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for name, body in mir.bodies(found, split.partition(found, mapped)):
            demanded = regalloc.required(body)
            seen += len(demanded)
            for value, where in demanded.items():
                was = ir.ROOT.get(body.origin.get(value, -1), -1)
                assert was is where, (
                    f"{obj.stem}/{name}: {value} is required in "
                    f"{regalloc.NAMES.get(where, where)} but BC had it in "
                    f"{regalloc.NAMES.get(was, was)}"
                )
    assert seen > 1000, f"only {seen} requirements found, so this proves little"


def test_a_pin_against_what_the_machine_demands_is_refused() -> None:
    """Asking for a value somewhere its own instruction cannot read it.

    A refusal, not a preference. The hoist asks for registers and must be
    told no rather than quietly given one the multiply will not look in --
    that is how hotlop printed 0 for 630: the load was renamed to cx and
    `imul` went on multiplying by ax.
    """
    tried = 0
    for obj in sorted(Path("fixtures/omf").glob("*.obj")):
        found = corpus.loaded(obj)
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for name, body in mir.bodies(found, split.partition(found, mapped)):
            for value, where in regalloc.required(body).items():
                other = next(one for one in regalloc.AVAILABLE if one is not where)
                got = regalloc.colour(body, {value: other})
                assert isinstance(got, str), (
                    f"{obj.stem}/{name}: {value} must be in "
                    f"{regalloc.NAMES.get(where, where)} but was allocated to "
                    f"{regalloc.NAMES.get(other, other)} on request"
                )
                tried += 1
                if tried >= 5:
                    return
    assert tried, "no fixed requirement to contradict, so this proves nothing"


def test_a_copy_on_the_phi_edge_untangles_a_class() -> None:
    """The live range split, where the split belongs.

    colour() refuses to move a class two of whose members are live at once
    -- nothing runs on a phi edge, so its result and arguments must already
    share a register. The way out is to put something on the edge.

    addrm is the shape: v4 = phi(v10, v31) with v31 still wanted after the
    phi, so the class cannot move and `v31 is wanted across its own phi`.
    """
    found = corpus.loaded(Path("fixtures/omf/addrm-p-g2.obj"))
    mapped = code_map(found)
    assert not isinstance(mapped, str)
    blocks = split.partition(found, mapped)
    seen = 0
    for _who, body in mir.bodies(found, blocks):
        # The same pipeline wholeseg runs: widening is not a pass and goes
        # after every one of them, just before lowering.
        done = transform.widened(
            transform.applied(body, found.dgroup, found.calls, blocks=blocks, found=found)
        )
        if not regalloc._tangled(done):
            continue
        seen += 1
        assert isinstance(regalloc.colour(done, done.pins), str), "it refuses while tangled"
        fixed = regalloc.untangled(done)
        assert not regalloc._tangled(fixed), "and the copy breaks the class"
        assert not isinstance(regalloc.colour(fixed, fixed.pins), str), "so it can be coloured"
    assert seen, "addrm no longer tangles, so this proves nothing"

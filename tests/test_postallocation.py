"""Small Tier-1 checks for physical-register rewrites after allocation."""

from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import select
from qbopt.backend import peephole
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def _pair(operation: ir.Operation, name: str, tail: tuple[lir.Insn, ...]) -> tuple[lir.LirBody, lir.Insn]:
    left = ir.Reg(Register.EBX, 4)
    right = ir.Reg(Register.ECX, 4)
    combined = lir.Insn(
        1,
        (1, 3),
        ir.Semantics(operation, name, (left,), (left, right)),
        (10,),
        (1, 2),
    )
    copied = lir.Insn(
        3,
        (3, 3),
        ir.Semantics(ir.Operation.MOVE, "mov", (right,), (left,)),
        (11,),
        (10,),
    )
    return lir.LirBody("pair", 0, (lir.LirBlock(0, (combined, copied, *tail), ()),), {}, {}), copied


def test_dword_constant_is_narrowed_when_the_abi_reads_only_its_low_word() -> None:
    """C SCALAR emitted ``mov eax,1789`` where BASIC needed only AX."""
    value = mir.Value(1, 1)
    source = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(
            ir.Operation.MOVE,
            "mov",
            (ir.Reg(Register.EAX, 4),),
            (ir.Imm(1789, 4),),
        ),
        (value.id,),
        (),
    )
    returned = mir.Op(
        2,
        ir.Operation.RETURN,
        "ret",
        (),
        (value,),
        kind=mir.Kind.RETURN,
        args=(mir.Held(value, 2),),
        reads_complete=True,
    )
    finish = lir.Insn(
        2,
        (2, 2),
        ir.Semantics(ir.Operation.RETURN, "ret", (), ()),
        (),
        (value.id,),
        requires=((ir.Held(value.id, 2), Register.AX),),
        op=returned,
    )
    # Source/symbol ownership anchors from the unrolled frontend body must be
    # transparent to physical liveness even though they constrain layout.
    anchor = lir.Insn(
        2,
        (2, 2),
        ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        (),
        (),
        symbol=True,
    )
    body = lir.LirBody("return-low", 1, (lir.LirBlock(1, (source, anchor, finish), ()),), {}, {})

    result = peephole.narrowed_moves(body)

    assert result.insns[0].what == ir.Semantics(
        ir.Operation.MOVE,
        "mov",
        (ir.Reg(Register.AX, 2),),
        (ir.Imm(1789, 2),),
    )


def test_dword_constant_stays_wide_when_any_upper_lane_is_live() -> None:
    wide = ir.Reg(Register.EAX, 4)
    source = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.MOVE, "mov", (wide,), (ir.Imm(1789, 4),)),
        (1,),
        (),
    )
    use = lir.Insn(
        2,
        (2, 2),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (wide, ir.Imm(0, 4))),
        (),
        (1,),
    )
    body = lir.LirBody("return-wide", 1, (lir.LirBlock(1, (source, use), ()),), {}, {})

    assert peephole.narrowed_moves(body) == body


def test_register_high_extract_uses_one_double_shift_for_dx_ax_return() -> None:
    """Frontend-parity ALGEBRA returned its high word with push/pop/pop.

    C originally selected ``mov edx,ebx; shr edx,16`` for the same value.
    ``shld edx,ebx,16`` puts EBX's high word in DX while leaving the low return
    word in AX alone.  Once physical liveness proves EDX's upper lanes and the
    shift flags dead, neither frontend needs the two-instruction spelling.
    """
    source = ir.Reg(Register.EBX, 4)
    discarded = ir.Reg(Register.BX, 2)
    high = ir.Reg(Register.DX, 2)
    marker = mir.Op(10, ir.Operation.RESTORE, "extract", (), (), kind=mir.Kind.EXTRACT)
    parts = (
        lir.Insn(
            10,
            (10, 10),
            ir.Semantics(ir.Operation.PUSH, "push", (), (source,)),
            (),
            (1,),
            op=marker,
        ),
        lir.Insn(
            10,
            (10, 10),
            ir.Semantics(ir.Operation.POP, "pop", (discarded,), ()),
            (2,),
            (),
        ),
        lir.Insn(
            10,
            (10, 10),
            ir.Semantics(ir.Operation.POP, "pop", (high,), ()),
            (3,),
            (),
        ),
        lir.Insn(
            11,
            (11, 11),
            ir.Semantics(ir.Operation.COMPARE, "cmp", (), (high, ir.Imm(0, 2))),
            (),
            (3,),
        ),
        lir.Insn(
            12,
            (12, 12),
            ir.Semantics(
                ir.Operation.MOVE,
                "mov",
                (ir.Reg(Register.EDX, 4),),
                (ir.Imm(0, 4),),
            ),
            (4,),
            (),
        ),
    )
    body = lir.LirBody("return-high", 10, (lir.LirBlock(10, parts, ()),), {}, {})

    result = peephole.high_extracts(body, cpu="386").insns

    assert [one.what.name for one in result] == ["shld", "cmp", "mov"]
    assert result[0].what == ir.Semantics(
        ir.Operation.FUNNEL,
        "shld",
        (ir.Reg(Register.EDX, 4),),
        (ir.Reg(Register.EDX, 4), source, ir.Imm(16, 1)),
    )
    assert result[0].defines == (3,)
    assert result[0].uses == (1,)


def test_selected_move_shift_high_extract_uses_the_same_double_shift() -> None:
    """Frontend-parity ALGEBRA's C path retained MOV EDX,ECX; SHR EDX,16.

    That is the same high-word extraction as BASIC's portable stack join.
    Final selection must canonicalize both shapes to SHLD so frontend spelling
    cannot survive into generated code.
    """
    source = ir.Reg(Register.ECX, 4)
    high = ir.Reg(Register.EDX, 4)
    returned = mir.Op(
        12,
        ir.Operation.RETURN,
        "return",
        (),
        (),
        kind=mir.Kind.RETURN,
        reads_complete=True,
    )
    parts = (
        lir.Insn(
            10,
            (10, 10),
            ir.Semantics(ir.Operation.MOVE, "mov", (high,), (source,)),
            (3,),
            (1,),
        ),
        lir.Insn(
            11,
            (11, 11),
            ir.Semantics(ir.Operation.BINARY, "shr", (high,), (high, ir.Imm(16, 1))),
            (3,),
            (3,),
        ),
        lir.Insn(
            12,
            (12, 12),
            ir.Semantics(ir.Operation.RETURN, "retf", (), (ir.Reg(Register.AX, 2), ir.Reg(Register.DX, 2))),
            (),
            (4, 3),
            op=returned,
            requires=((ir.Held(4, 2), Register.AX), (ir.Held(3, 2), Register.DX)),
        ),
    )
    body = lir.LirBody("c-return-high", 10, (lir.LirBlock(10, parts, ()),), {}, {})

    result = peephole.high_extracts(body, cpu="386").insns

    assert [one.what.name for one in result] == ["shld", "retf"]
    assert result[0].what == ir.Semantics(
        ir.Operation.FUNNEL,
        "shld",
        (high,),
        (high, source, ir.Imm(16, 1)),
    )
    assert result[0].defines == (3,)
    assert result[0].uses == (1,)


def test_register_high_extract_shifts_a_dying_return_root_in_place() -> None:
    """Frontend-parity LOOP returned EDX through push/pop/pop.

    AX already held the low word and DX was both the source root and the high
    return register.  The source's upper lanes died at the extraction, so one
    in-place SHR is the complete high-half operation.
    """
    source = ir.Reg(Register.EDX, 4)
    high = ir.Reg(Register.DX, 2)
    marker = mir.Op(10, ir.Operation.RESTORE, "extract", (), (), kind=mir.Kind.EXTRACT)
    parts = (
        lir.Insn(
            10,
            (10, 10),
            ir.Semantics(ir.Operation.PUSH, "push", (), (source,)),
            (),
            (1,),
            op=marker,
        ),
        lir.Insn(
            10,
            (10, 10),
            ir.Semantics(ir.Operation.POP, "pop", (ir.Reg(Register.BX, 2),), ()),
            (2,),
            (),
        ),
        lir.Insn(
            10,
            (10, 10),
            ir.Semantics(ir.Operation.POP, "pop", (high,), ()),
            (3,),
            (),
        ),
        lir.Insn(
            11,
            (11, 11),
            ir.Semantics(ir.Operation.COMPARE, "cmp", (), (high, ir.Imm(0, 2))),
            (),
            (3,),
        ),
        lir.Insn(
            12,
            (12, 12),
            ir.Semantics(ir.Operation.MOVE, "mov", (source,), (ir.Imm(0, 4),)),
            (4,),
            (),
        ),
    )
    body = lir.LirBody("return-high-in-place", 10, (lir.LirBlock(10, parts, ()),), {}, {})

    result = peephole.high_extracts(body, cpu="386").insns

    assert [one.what.name for one in result] == ["shr", "cmp", "mov"]
    assert result[0].what == ir.Semantics(
        ir.Operation.BINARY,
        "shr",
        (source,),
        (source, ir.Imm(16, 1)),
    )
    assert result[0].defines == (3,)
    assert result[0].uses == (1,)


def test_frontend_parity_listing_keeps_blocks_after_an_early_basic_exit() -> None:
    """The first parity report hid BRANCH's positive arm after B$EXSA.

    Frame-exit calls are per-edge ABI scaffolding, not an end marker for the
    laid-out procedure.  The raw comparison must omit that call while keeping
    every later block and its arithmetic.
    """
    from tools.frontend_parity import basic_core

    call = lambda at: mir.Op(at, ir.Operation.CALL, "call", (), (), kind=mir.Kind.CALL)  # noqa: E731
    blocks = (
        lir.LirBlock(
            1,
            (
                lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.CALL, "call"), (), (), op=call(1)),
                lir.Insn(
                    2,
                    (2, 2),
                    ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Imm(1, 2),)),
                    (),
                    (),
                ),
                lir.Insn(3, (3, 3), ir.Semantics(ir.Operation.CALL, "call"), (), (), op=call(3)),
            ),
            (10,),
        ),
        lir.LirBlock(
            10,
            (
                lir.Insn(
                    10,
                    (10, 10),
                    ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Imm(2, 2),)),
                    (),
                    (),
                ),
            ),
            (),
        ),
    )
    body = lir.LirBody("branch", 1, blocks, {}, {})

    result = basic_core(body, {1: "B$ENRA", 3: "B$EXSA"})

    assert [line for _block, line in result] == ["mov ax, 1", "mov ax, 2"]


def test_dead_flags_crossing_increment_do_not_block_zero_idiom() -> None:
    """PARITYCONTROL kept ``mov eax,0`` because a later DEC preserved dead CF.

    When every arithmetic flag is dead after INC/DEC, its preserved carry is
    dead before it too; a still later logical operation may therefore make an
    earlier XOR-zeroing form safe.
    """
    eax = ir.Reg(Register.EAX, 4)
    bx = ir.Reg(Register.BX, 2)
    cx = ir.Reg(Register.CX, 2)
    insns = (
        lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (eax,), (ir.Imm(0, 4),)), (1,), ()),
        lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.UNARY, "dec", (bx,), (bx,)), (2,), (2,)),
        lir.Insn(2, (2, 2), ir.Semantics(ir.Operation.BINARY, "and", (cx,), (cx, cx)), (3,), (3,)),
        lir.Insn(3, (3, 3), ir.Semantics(ir.Operation.BRANCH, "jne", (), (), 1), (), ()),
    )
    body = lir.LirBody("zero-before-dec", 0, (lir.LirBlock(0, insns, (1,)),), {}, {})

    result = peephole.zeroes(body)

    assert result.insns[0].what == ir.Semantics(ir.Operation.BINARY, "xor", (eax,), (eax, eax))


def test_return_high_extraction_drops_redundant_low_word_shuttle() -> None:
    """PARITYMEMORY returned EAX as AX:DX through two needless BX moves.

    Extracting the high word through EDX neither changes EAX nor needs the
    saved AX value.  The virtual extract/call chain remains anchored even
    when the physical save and restore emit no instructions.
    """
    eax = ir.Reg(Register.EAX, 4)
    ax = ir.Reg(Register.AX, 2)
    bx = ir.Reg(Register.BX, 2)
    edx = ir.Reg(Register.EDX, 4)
    dx = ir.Reg(Register.DX, 2)
    parts = (
        lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (bx,), (ax,)), (2,), (1,)),
        lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (edx,), (eax,)), (3,), (1,)),
        lir.Insn(
            2,
            (2, 2),
            ir.Semantics(ir.Operation.BINARY, "shr", (edx,), (edx, ir.Imm(16, 1))),
            (4,),
            (3,),
        ),
        lir.Insn(3, (3, 3), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (bx,)), (5,), (2,)),
        lir.Insn(
            4,
            (4, 4),
            ir.Semantics(ir.Operation.NOTHING, "", (), ()),
            (),
            (),
            clobbers=frozenset({Register.EBX}),
        ),
        lir.Insn(
            5,
            (5, 5),
            ir.Semantics(ir.Operation.RETURN, "retf", (), (ax, dx)),
            (),
            (5, 4),
            requires=((ir.Held(5, 2), Register.AX), (ir.Held(4, 2), Register.DX)),
        ),
    )
    body = lir.LirBody("return-pair", 0, (lir.LirBlock(0, parts, ()),), {}, {})

    result = peephole.Peephole().transform(body)
    emitted = [one.what for one in result.insns if one.what and one.what.op is not ir.Operation.NOTHING]

    assert emitted == [parts[1].what, parts[2].what, parts[5].what]


def test_unit_add_selects_inc_only_when_carry_is_dead() -> None:
    """Frontend-parity LOOP used C's ADD 1 but BASIC's equivalent INC.

    The frontend spelling is irrelevant after MIR.  INC is the compact
    allocated form only when no later instruction observes ADD's carry.
    """
    counter = ir.Reg(Register.CX, 2)
    add = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.BINARY, "add", (counter,), (counter, ir.Imm(1, 2))),
        (1,),
        (1,),
    )
    compare = lir.Insn(
        2,
        (2, 2),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (counter, ir.Imm(8, 2))),
        (),
        (1,),
    )
    dead = lir.LirBody("counter", 1, (lir.LirBlock(1, (add, compare), ()),), {}, {})
    branch = lir.Insn(2, (2, 2), ir.Semantics(ir.Operation.BRANCH, "jb", (), (), 3), (), ())
    live = lir.LirBody(
        "carry",
        1,
        (lir.LirBlock(1, (add, branch), (3,)), lir.LirBlock(3, (), ())),
        {},
        {},
    )

    assert peephole.increments(dead).insns[0].what.name == "inc"
    assert peephole.increments(live).insns[0].what.name == "add"


@pytest.mark.parametrize(
    "operation,name",
    [
        (ir.Operation.BINARY, "add"),
        (ir.Operation.BINARY, "and"),
        (ir.Operation.BINARY, "or"),
        (ir.Operation.BINARY, "xor"),
        (ir.Operation.MULTIPLY, "imul"),
    ],
)
def test_commutative_result_copy_uses_the_dying_other_operand(operation: ir.Operation, name: str) -> None:
    """Mandel's inner loop emitted ``imul ebx,ecx; mov ecx,ebx``.

    Both input values die at the multiply and ECX is immediately overwritten
    with its result.  The equivalent ``imul ecx,ebx`` leaves the only live
    physical result in the same place without executing the copy.
    """
    left = ir.Reg(Register.EBX, 4)
    right = ir.Reg(Register.ECX, 4)
    shift = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.BINARY, "sar", (right,), (right, ir.Imm(7, 1))),
        (12,),
        (11,),
    )
    overwrite = lir.Insn(
        5,
        (5, 7),
        ir.Semantics(ir.Operation.MOVE, "mov", (left,), (ir.Imm(0, 4),)),
        (13,),
        (),
    )
    body, copied = _pair(operation, name, (shift, overwrite))

    result = peephole.transferred(body)

    assert result.insns[0].what == ir.Semantics(operation, name, (right,), (right, left))
    assert result.insns[1].what.op is ir.Operation.NOTHING
    assert result.insns[1].defines == copied.defines
    assert result.insns[2:] == (shift, overwrite)


def test_commutative_result_copy_keeps_a_still_live_first_operand() -> None:
    """Changing which multiply input is destroyed is legal only when the old destination dies."""
    left = ir.Reg(Register.EBX, 4)
    use = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (left, ir.Imm(0, 4))),
        (),
        (10,),
    )
    body, _copied = _pair(ir.Operation.MULTIPLY, "imul", (use,))

    assert peephole.transferred(body) == body


def test_commutative_result_copy_keeps_source_owned_copy_bytes() -> None:
    """A real input instruction is not the synthetic transfer this rewrite may erase."""
    left = ir.Reg(Register.EBX, 4)
    overwrite = lir.Insn(
        5,
        (5, 7),
        ir.Semantics(ir.Operation.MOVE, "mov", (left,), (ir.Imm(0, 4),)),
        (13,),
        (),
    )
    body, copied = _pair(ir.Operation.MULTIPLY, "imul", (overwrite,))
    owned = replace(copied, covers=(3, 5))
    body = replace(body, blocks=(replace(body.blocks[0], insns=(body.insns[0], owned, overwrite)),))

    assert peephole.transferred(body) == body


def _high_extract(tail: tuple[lir.Insn, ...]) -> lir.LirBody:
    value = mir.Value(1, 1)
    source = mir.Value(2, 1)
    operation = mir.Op(
        1,
        ir.Operation.BINARY,
        "shr",
        (value,),
        (source,),
        kind=mir.Kind.SHR,
        args=(mir.Held(source, 4), mir.Const(16, 1)),
        results=(mir.Held(value, 4),),
    )
    wide = ir.Reg(Register.EDX, 4)
    cell = ir.Mem(Addr(Space.FRAME, -4), 4, through=Register.BP)
    load = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.MOVE, "mov", (wide,), (cell,)),
        (1,),
        (),
        op=operation,
        symbol=False,
    )
    shift = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.BINARY, "shr", (wide,), (wide, ir.Imm(16, 1))),
        (1,),
        (1,),
        op=operation,
    )
    return lir.LirBody("extract", 0, (lir.LirBlock(0, (load, shift, *tail), ()),), {}, {})


def _return_high() -> lir.Insn:
    value = mir.Value(1, 1)
    operation = mir.Op(
        2,
        ir.Operation.RETURN,
        "",
        (),
        (value,),
        kind=mir.Kind.RETURN,
        args=(mir.Held(value, 2),),
        reads_complete=True,
    )
    return lir.Insn(
        2,
        (2, 3),
        ir.Semantics(ir.Operation.RETURN, "retf", (), ()),
        (),
        (1,),
        op=operation,
        requires=((ir.Held(1, 2), Register.DX),),
    )


def test_synthetic_high_extract_loads_only_the_high_word() -> None:
    """Mandel loaded a spilled dword and shifted it solely to return the high word."""
    low = ir.Reg(Register.DX, 2)
    returned = _return_high()

    result = peephole.high_extracts(_high_extract((returned,)))

    load, anchor = result.insns[:2]
    assert load.what == ir.Semantics(
        ir.Operation.MOVE,
        "mov",
        (low,),
        (ir.Mem(Addr(Space.FRAME, -2), 2, through=Register.BP),),
    )
    assert anchor.what.op is ir.Operation.NOTHING
    assert anchor.defines == (1,)
    assert result.insns[2:] == (returned,)


@pytest.mark.parametrize("hazard", ["full-result", "flags", "source-load"])
def test_high_extract_keeps_observable_wide_load_or_shift_effects(hazard: str) -> None:
    """Narrowing is legal only for a synthetic load with dead upper lanes and flags."""
    if hazard == "full-result":
        tail = (
            lir.Insn(
                2,
                (2, 4),
                ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Reg(Register.EDX, 4), ir.Imm(0, 4))),
                (),
                (1,),
            ),
            _return_high(),
        )
    elif hazard == "flags":
        tail = (lir.Insn(2, (2, 4), ir.Semantics(ir.Operation.BRANCH, "je", (), (), 9), (), ()),)
    else:
        tail = (_return_high(),)
    body = _high_extract(tail)
    if hazard == "source-load":
        load = replace(body.insns[0], covers=(1, 3), symbol=None)
        body = replace(body, blocks=(replace(body.blocks[0], insns=(load, *body.insns[1:])),))

    assert peephole.high_extracts(body) == body


def test_discarded_x87_result_has_a_register_pop_encoding() -> None:
    """qcport gib.c's ``(void)gib_crandom(rng)`` reached fresh OMF output as
    ``fstp st(0)``.  The allocated instruction was valid but selection knew
    only memory stores, so the complete optimized build was unencodable.
    """
    what = ir.Semantics(ir.Operation.FLOAT_STORE, "fstp", (ir.St(0),), (ir.St(0),))

    made = select.emit(what)

    assert made is not None
    assert made.code == bytes.fromhex("ddd8")

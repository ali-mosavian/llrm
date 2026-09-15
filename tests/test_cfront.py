"""qcport's modules through the C path, from streams wccq recorded.

pal and qglsurf built this way render dm3ish to the Borland build's md5.
"""

from pathlib import Path

from qbopt.cfront import compile as cfront

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


def _asm(module: str) -> list[str]:
    text = cfront.compiled((FIXTURES / f"{module}.cgs").read_text(), module)
    return [line.strip() for line in text.splitlines()]


def test_implicit_conversion_is_the_raise_s():
    """wcc leaves `(long) byte - short` as mixed-width operands; the raise
    refused pal_bestfit with `used at width 4` until it converted them."""
    lines = _asm("pal")
    at = lines.index("movzx ax, byte ptr [bx]")
    assert lines[at + 1 : at + 5] == ["movzx eax, ax", "mov bx, word ptr [bp+6]", "movsx ebx, bx", "sub eax, ebx"]


def test_far_pointer_return_in_dx_ax():
    """pal_current's `(PalRgb far *) pal_now` crashed the return on an address
    that was not yet a value."""
    lines = _asm("pal")
    body = lines[lines.index("_pal_current proc far") : lines.index("_pal_current endp")]
    assert "mov ax, DGROUP" in body and "mov bx, offset _pal_now" in body
    assert body[-6:-3] == ["mov eax, dword ptr [bp-4]", "mov edx, eax", "shr edx, 16"]


def test_choose_joins_both_arms_in_one_cell():
    """`c ? a : b` and `p != 0` as a value: the raise refused CGChoose and a
    value-producing CGCompare, which ls_init has 17 of."""
    lines = _asm("choose")
    pick = lines[lines.index("_pick proc near") : lines.index("_pick endp")]
    assert pick.count("mov word ptr [bp-4], ax") == 2 and any(one.startswith("jmp L0_") for one in pick)
    body = lines[lines.index("_choose proc far") : lines.index("_choose endp")]
    at = body.index("mov word ptr [bp-4], 1")
    assert body[at - 3] == "cmp ax, 0" and body[at - 2].startswith("je L2_") and body[at - 1].endswith(":")
    assert body[at + 3] == "mov word ptr [bp-4], 0"


def test_data_pointer_to_a_literal():
    """ls's style table points at its pattern strings; DGBackPtr was refused."""
    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "ls.cgs").read_text()))
    lines = dict(cfront._data(unit))
    assert "    dw L_b2" in next(items for items in lines.values() if "    dw L_b3" in items)


def test_negative_data_fits_its_width():
    """`short sbar_health = -1` printed as `dw 4294967295`: the shim writes a
    negative item as 32 bits, and jwasm refused the initializer."""
    unit = cfront.hir.Unit()
    unit.segments[1] = cfront.hir.Segment(1, "_DATA", 0, [("DGInteger", ("4294967295", "TY_INT_2"))])
    assert dict(cfront._data(unit))["_DATA"] == ("    dw 65535",)


def test_float_moves_as_its_bits():
    """`ls_animate(&ls, 0.05f)` pushes the single's four bytes; CGFloat was refused."""
    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "ls.cgs").read_text()))
    proc = next(one for one in unit.procs if unit.symbols[one.symbol].name == "ls_selftest")
    body = cfront.raise_hir.raised(unit, proc).body
    pushed = [op.args[0] for block in body.blocks for op in block.ops if op.kind.name == "ARG"]
    assert cfront.mir.Const(0x3D4CCCCD, 4) in pushed


def test_raised_mir_names_no_instruction():
    """MIR says what each operation computes; lowering picks the instruction.
    The raise wrote `mov`, `lea` and `fistp` into every op it made, `call`
    and `retf` as machine semantics, `add sp` naming SP, and ES and BX into
    every far cell."""
    ir, Space = cfront.raise_hir.ir, cfront.raise_hir.Space
    for module in ("pal", "qglsurf", "choose", "ls"):
        unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / f"{module}.cgs").read_text()))
        for proc in unit.procs:
            for block in cfront.raise_hir.raised(unit, proc).body.blocks:
                for op in block.ops:
                    assert (op.op, op.name, op.made) == (ir.Operation.NOTHING, "", None), (module, op)
                    for ref in (*op.loads, *op.stores):
                        if ref.addr is not None and ref.addr.space is Space.FAR:
                            assert (ref.addr.base, ref.addr.segment) == (0, 0), (module, ref)


def test_float_cast_truncates():
    """`(long)(anim_time * 10.0f)`: fistp rounds by the control word, so it is
    set to toward-zero around the store and put back. The raise refused
    float O_TIMES."""
    lines = _asm("ls")
    body = lines[lines.index("_ls_animate proc far") : lines.index("_ls_animate endp")]
    at = body.index("fld dword ptr [bp+8]")
    assert body[at + 1] == "fimul word ptr [bp-10]"
    assert body[at + 2 : at + 8] == [
        "fnstcw word ptr [bp-22]",
        "fnstcw word ptr [bp-24]",
        "or word ptr [bp-24], 3072",
        "fldcw word ptr [bp-24]",
        "fistp dword ptr [bp-16]",
        "fldcw word ptr [bp-22]",
    ]


def test_register_convention_is_refused():
    """A callee taking arguments in registers: the raise pushed them anyway,
    and ls linked against `strlen_`, Watcom's register-convention strlen."""
    try:
        _asm("regs")
    except cfront.hir.Unsupported as refused:
        assert "_twice has a register calling convention" in str(refused)
    else:
        raise AssertionError("a register convention was compiled as a stack one")


def test_runtime_is_the_target_s_cdecl():
    """borland.h moves Open Watcom's runtime declarations to cdecl."""
    assert "call far ptr _strlen" in _asm("ls")


def test_optimised_return_arrives_in_ax():
    """Under --opt ls_lchar left its result in BX: the raise pinned the value
    it returned, forwarding replaced the return's operand with an unpinned
    one, and ls_selftest failed with -1."""
    text = cfront.compiled((FIXTURES / "ls.cgs").read_text(), "ls", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    body = lines[lines.index("_ls_lchar proc near") : lines.index("_ls_lchar endp")]
    epilogue = body.index("mov sp, bp")
    assert body[epilogue - 1].startswith("mov ax,"), body[epilogue - 3 : epilogue]


def test_optimised_locals_lose_their_dead_stores():
    """A C local whose address is never taken is gone when the body returns,
    so a store nothing reads again is dead. The optimiser kept every one:
    pal_bestfit wrote dr, dg, db and d to the frame on every iteration."""
    text = cfront.compiled((FIXTURES / "pal.cgs").read_text(), "pal", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    body = lines[lines.index("_pal_bestfit proc far") : lines.index("_pal_bestfit endp")]
    stored = [one for one in body if one.startswith("mov dword ptr [bp-") and one.split("[bp-")[1].split("]")[0] in ("14", "18", "22", "26")]
    assert stored == []


def _proc(lines: list[str], name: str) -> list[str]:
    return lines[lines.index(f"{name} proc far") : lines.index(f"{name} endp")]


def test_float_compare_moves_the_status_word_into_the_flags():
    """`a < b` on floats, 66 of qcport's procedures, was refused. x87 C0 and C3
    reach CF and ZF through sahf, so the branch is an unsigned one."""
    lines = _asm("floats")
    pick = _proc(lines, "_pick")
    at = pick.index("fnstsw ax")
    assert pick[at - 2 : at + 3] == ["fld dword ptr [bp+6]", "fcomp dword ptr [bp+10]", "fnstsw ax", "sahf", "jae L1_10"]
    sign = _proc(lines, "_sign")
    at = sign.index("fcompp")
    assert sign[at - 2 : at + 4] == ["fldz", "fld qword ptr [bp+6]", "fcompp", "fnstsw ax", "sahf", "jb L2_9"]


def test_float_literal_is_read_from_dgroup():
    """`d * 0.5`: CGFloat TY_DOUBLE was refused, and a single that is no small
    integer had nowhere to be loaded from."""
    lines = _asm("floats")
    assert "fmul dword ptr L_f0" in _proc(lines, "_half")
    at = lines.index("L_f0 label byte")
    assert lines[at + 1] == "db 000h,000h,000h,03fh"


def test_float_results_arrive_and_leave_in_st0():
    """A float call's result was taken for DX:AX and a float return refused.
    ext_scale's result waits in memory across half's call; scaled returns its
    sum on the x87."""
    body = _proc(_asm("floats"), "_scaled")
    at = body.index("call far ptr _ext_scale")
    assert body[at + 1 : at + 3] == ["add sp, 4", "fstp tbyte ptr [bp-22]"]
    at = body.index("call far ptr _half")
    assert body[at - 4 : at] == ["mov eax, dword ptr [bp-8]", "push eax", "mov eax, dword ptr [bp-12]", "push eax"]
    assert body[at + 1 : at + 4] == ["add sp, 8", "fld tbyte ptr [bp-22]", "faddp st(1), st(0)"]
    assert body[body.index("mov sp, bp") - 1] == "fld dword ptr [bp-4]"


def test_library_math_calls_the_runtime():
    """OW's front end makes `sqrt` an operator, O_SQRT, which was refused;
    Borland's library has `_sqrt`, taking a double and returning in st(0)."""
    lines = _asm("floats")
    body = _proc(lines, "_mag")
    at = body.index("call far ptr _sqrt")
    assert body[at + 1] == "add sp, 8" and "fabs" in body
    assert "extern _sqrt:far" in lines


def test_switch_reaches_its_default():
    """A switch was refused, 19 of qcport's procedures. Expanded into compares,
    its last one fell through into the first case: nothing printed the jump
    to a successor that is not the next block, so `default` never ran."""
    body = _proc(_asm("control"), "_pick")
    at = body.index("cmp ax, 9")
    assert body[at + 1].startswith("je ") and body[at + 2].startswith("jmp ")
    default = body.index(body[at + 2].split()[1] + ":")
    assert body[default + 1] == "mov word ptr [bp-2], -1"


def test_shift_counts_from_cl():
    """`v << n` was refused for any count but a literal."""
    body = _proc(_asm("control"), "_shifted")
    assert {"shl eax, cl", "sar ebx, cl", "shr bx, cl"} <= set(body)


def test_unsigned_division_is_div():
    """`offset / 7` on an unsigned short and `a / b` on unsigned longs were
    refused; a signed division reads 0x8000 and up as negative."""
    lines = _asm("control")
    per = _proc(lines, "_per")
    assert per[per.index("div bx") - 1] == "mov dx, 0"
    ratio = _proc(lines, "_ratio")
    assert ratio[ratio.index("div ebx") - 1] == "mov edx, 0"


def test_unsigned_long_loads_as_a_quad():
    """`(float) t` for an unsigned long was refused: fild reads a dword signed."""
    body = _proc(_asm("control"), "_wide")
    at = body.index("fild qword ptr [bp-12]")
    assert body[at - 3 : at] == ["mov eax, dword ptr [bp+6]", "mov dword ptr [bp-12], eax", "mov dword ptr [bp-8], 0"]


def test_constant_far_address_loads_its_selector_through_the_stack():
    """`*(unsigned long far *) 0x0040006C` was refused, then printed `mov es, 64`,
    which x86 has no encoding for."""
    body = _proc(_asm("control"), "_far_ticks")
    at = body.index("pop es")
    assert body[at - 1 : at + 3] == ["pushw 64", "pop es", "mov bx, 108", "mov eax, dword ptr es:[bx]"]


def test_compound_assignment_widens_its_source():
    """`total += step`, a long and a short: the raise refused a word used at width 4."""
    body = _proc(_asm("control"), "_grow")
    at = body.index("movsx ebx, bx")
    assert body[at + 1 : at + 3] == ["add eax, ebx", "mov dword ptr [bp+6], eax"]


def test_address_of_a_float_is_a_pointer():
    """`(void far *) &cell`: the name node is typed by the float it names, and the
    conversion loaded it onto the x87."""
    body = _proc(_asm("control"), "_where")
    assert body[body.index("mov bx, offset _cell") - 1] == "mov ax, DGROUP"


def test_inline_assembly_names_locals_at_their_frame_offsets():
    """`__asm { fld f; fnstcw cw; ... }` was refused as a call with a register
    convention. Its code goes in place with each local's BP offset patched in."""
    body = _proc(_asm("inline"), "_chop")
    at = body.index("db 0d9h,086h,006h,000h,0d9h,0beh,0fch,0ffh,0dfh,09eh,0fah,0ffh,0d9h,0aeh,0fch,0ffh")
    assert body[at + 1] == "mov ax, word ptr [bp-6]"


def test_emit_lays_down_its_bytes():
    """`__emit__( 0x0f, 0x31 )` became a far call to `___emit__`, which no library has."""
    lines = _asm("inline")
    body = lines[lines.index("_raw proc near") : lines.index("_raw endp")]
    assert body[body.index("db 0b8h,001h,000h") + 1 : body.index("db 0bah,002h,000h")] == ["db 090h"]
    assert not any("___emit__" in one for one in lines)


def test_value_less_return_returns_what_the_code_left():
    """`raw` ends in inline code and `return;`-less: its MIR returned nothing,
    and DX:AX reached the caller only because nothing was emitted after."""
    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "inline.cgs").read_text()))
    proc = next(one for one in unit.procs if unit.symbols[one.symbol].name == "raw")
    body = cfront.raise_hir.raised(unit, proc).body
    returned = next(op for block in body.blocks for op in block.ops if op.kind.name == "RETURN")
    assert len(returned.args) == 2


def test_calls_push_in_convention_order():
    """cdecl pushes last first and pops after; pascal pushes first first."""
    lines = _asm("qglsurf")
    at = lines.index("call far ptr _asset_seek")
    assert lines[at - 3 : at + 2] == ["lea bx, [bp-10]", "push bx", "push ax", "call far ptr _asset_seek", "add sp, 4"]
    at = lines.index("call far ptr QGLSFNEW")
    assert lines[at - 6 : at] == [
        "mov ax, word ptr [bp+10]",
        "mov ebx, dword ptr [bp-14]",
        "mov cx, word ptr [bp+8]",
        "push cx",
        "push bx",
        "push ax",
    ]

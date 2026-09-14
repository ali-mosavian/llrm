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

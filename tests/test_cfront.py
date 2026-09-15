"""qcport's modules through the C path, from streams wccq recorded.

pal and qglsurf built this way render dm3ish to the Borland build's md5.
"""

from pathlib import Path

import pytest

from qbopt.cfront import compile as cfront

FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "c"


def _asm(module: str) -> list[str]:
    text = cfront.compiled((FIXTURES / f"{module}.cgs").read_text(), module)
    return [line.strip() for line in text.splitlines()]


def test_implicit_conversion_is_the_raise_s():
    """wcc leaves `(long) byte - short` as mixed-width operands; the raise
    refused pal_bestfit with `used at width 4` until it converted them."""
    lines = _asm("pal")
    at = lines.index("movzx ax, byte ptr _pal_now[bx]")
    assert lines[at + 1 : at + 5] == ["movzx eax, ax", "mov bx, word ptr [bp+6]", "movsx ebx, bx", "sub eax, ebx"]


def test_far_pointer_return_in_dx_ax():
    """pal_current's `(PalRgb far *) pal_now` crashed the return on an address
    that was not yet a value."""
    lines = _asm("pal")
    body = lines[lines.index("_pal_current proc far") : lines.index("_pal_current endp")]
    assert "mov ax, DGROUP" in body and "mov bx, offset _pal_now" in body
    epilogue = body.index("leave")
    assert body[epilogue - 2 : epilogue] == ["mov ax, word ptr [bp-4]", "mov dx, word ptr [bp-2]"]


def test_choose_joins_both_arms_in_one_cell():
    """`c ? a : b` and `p != 0` as a value: the raise refused CGChoose and a
    value-producing CGCompare, which ls_init has 17 of."""
    lines = _asm("choose")
    pick = lines[lines.index("_pick proc near") : lines.index("_pick endp")]
    assert pick.count("mov word ptr [bp-4], ax") == 2 and any(one.startswith("jmp L0_") for one in pick)
    body = lines[lines.index("_choose proc far") : lines.index("_choose endp")]
    at = body.index("mov word ptr [bp-4], 1")
    assert body[at - 3] == "or ax, ax" and body[at - 2].startswith("je L2_") and body[at - 1].endswith(":")
    assert _reached(body, body.index(body[at - 2].split()[1] + ":")) == "mov word ptr [bp-4], 0"


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
    # The constant from the pool: as an integer it was first written to a slot, `mov word ptr [bp-10], 10`.
    assert body[at + 1].startswith("fmul dword ptr L_f") and "mov word ptr [bp-10], 10" not in body
    assert [line.split()[0] for line in body[at + 2 : at + 5]] == ["fldcw", "fistp", "fldcw"]
    saves = [index for index, line in enumerate(body) if line.startswith("fnstcw")]
    assert len(saves) == 2 and saves[-1] < at


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
    epilogue = next(i for i, line in enumerate(body) if line.startswith("ret"))
    while body[epilogue - 1] in ("leave", "pop bp"):
        epilogue -= 1
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
    assert body[body.index("leave") - 1] == "fld dword ptr [bp-4]"


def test_library_math_calls_the_runtime():
    """OW's front end makes `sqrt` an operator, O_SQRT, which was refused;
    Borland's library has `_sqrt`, taking a double and returning in st(0)."""
    lines = _asm("floats")
    body = _proc(lines, "_mag")
    at = body.index("call far ptr _sqrt")
    assert body[at + 1] == "add sp, 8" and "fabs" in body
    assert "extern _sqrt:far" in lines


def _reached(body, at):
    """The first instruction run from `at`, through labels and jumps."""
    while body[at].endswith(":") or body[at].startswith("jmp "):
        at = body.index(body[at].split()[1] + ":") if body[at].startswith("jmp ") else at + 1
    return body[at]


def test_switch_reaches_its_default():
    """A switch was refused, 19 of qcport's procedures. Expanded into compares,
    its last one fell through into the first case: nothing printed the jump
    to a successor that is not the next block, so `default` never ran."""
    body = _proc(_asm("control"), "_pick")
    at = body.index("cmp ax, 9")
    assert body[at + 1].startswith("je ")
    assert _reached(body, at + 2) == "mov word ptr [bp-2], -1"


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


def test_private_segment_data_goes_through_its_selector():
    """r_portal's `static short far stk_leaf[]` sits outside DGROUP; the raise
    addressed it through DS and jwasm refused `mov word ptr _stk_leaf, ax`."""
    lines = _asm("fardata")
    body = lines[lines.index("_peek proc far") : lines.index("_peek endp")]
    at = body.index("mov word ptr es:[bx+si], ax")
    assert "pushw seg _stack" in body[:at] and "pop es" in body[:at] and "mov bx, offset _stack" in body[:at]
    assert "fardata13_DATA segment word public 'FAR_DATA'" in lines


def test_const2_is_in_dgroup():
    """d_faces's `static const float lm_recip[]` is in CONST2, which Open Watcom
    groups with DGROUP; jwasm's medium model does not, so DS reached the wrong frame."""
    assert "DGROUP group CONST2" in _asm("fardata")


def test_float_check_is_named_fwait():
    """floatfold keeps an FCHECK where it drops an unused inexact conversion;
    the C path named it nothing, and --opt refused view.c's v_update_camera."""
    check = cfront.mir.Op(5, cfront.lower.ir.Operation.NOTHING, "", (), (), kind=cfront.mir.Kind.FCHECK)
    body = cfront.mir.MirBody(5, (cfront.mir.MirBlock(5, (), (check,), ()),))
    (named,) = cfront.lower.named(body).blocks[0].ops
    assert (named.op, named.name) == (cfront.lower.ir.Operation.NOTHING, "fwait")


def test_global_array_cell_is_named_through_its_index():
    """`sy[j] = sx[j]` addressed `offset _sy + j` as an unnamed near pointer: its
    store could reach the frame, so j round-tripped through its slot and every
    access took three instructions."""
    text = cfront.compiled((FIXTURES / "indexed.cgs").read_text(), "indexed", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    body = lines[lines.index("_copy proc far") : lines.index("_copy endp")]
    assert not any("offset _s" in line or "[bp-" in line for line in body)
    assert any(line.startswith("mov word ptr _sy[") for line in body)


def test_near_pointer_field_is_named_through_its_pointer():
    """`scale->y` and `m[3]` added a constant to a loaded near pointer as plain
    arithmetic: each sum was hoisted and spilled, so every access took
    `mov bx, [bp-n]` before `fmul [bx]`."""
    text = cfront.compiled((FIXTURES / "pointers.cgs").read_text(), "pointers", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    body = lines[lines.index("_transform proc far") : lines.index("_transform endp")]
    assert "fmul dword ptr [di+12]" in body
    assert not any(line.startswith("mov bx, word ptr [bp-") for line in body)


def test_private_store_dies_across_float_operations():
    """Every float op cleared what dead-store analysis knew, as if its exception
    handler could read this frame: the promoted counter still wrote its slot
    each pass, `mov word ptr [bp-4], ax`, for nothing to read."""
    text = cfront.compiled((FIXTURES / "pointers.cgs").read_text(), "pointers", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    body = lines[lines.index("_transform proc far") : lines.index("_transform endp")]
    assert not any("[bp-4]" in line for line in body)


def test_local_stored_before_a_call_is_promoted_after_it():
    """A call with no stated effects wrote all memory, this frame too: `k = 0`
    before it was not available after, so the loop reloaded k's slot every
    pass and the allocator spilled its register copy to a second slot."""
    from qbopt.optimize import transform

    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "calls.cgs").read_text()))
    (proc,) = unit.procs
    raised = cfront.raise_hir.raised(unit, proc, cfront.raise_hir.Shared())
    body = transform.applied(raised.body, frozenset(), raised.calls, found=None)
    local = [
        op.at
        for block in body.blocks
        for op in block.ops
        if op.kind is cfront.mir.Kind.LOAD
        and any(ref.addr is not None and ref.addr.space is cfront.raise_hir.Space.FRAME and ref.addr.disp < 0 for ref in op.loads)
    ]
    assert local == []


def test_symbol_argument_is_pushed_as_a_literal():
    """Fold put a number into the argument that read it but not a symbol, so
    `tag` stood as `mov di, offset _tag`, left the loop, and took a register
    or a slot where `push offset _tag` needs neither."""
    text = cfront.compiled((FIXTURES / "loopaddr.cgs").read_text(), "loopaddr", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_sum")
    assert "push offset _tag" in body and not any(line.startswith("mov ") and "offset _tag" in line for line in body), body


def test_stored_symbol_is_forwarded_to_its_reload():
    """Once a store took `offset _pal_now` as a literal, the reload after it was
    served nothing: pal_current built a frame to store the far pointer it
    returned and read it straight back."""
    text = cfront.compiled((FIXTURES / "pal.cgs").read_text(), "pal", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_pal_current")
    assert not any("[bp-" in line for line in body), body


@pytest.mark.parametrize("name", ["_dot", "_fill"])
def test_loop_tests_at_its_bottom(name):
    """Every C loop tested at its top and jumped back, `cmp bx,40 / je out /
    ... / jmp top`: two branches a pass where bcc, gcc and clang take one."""
    text = cfront.compiled((FIXTURES / "rotate.cgs").read_text(), "rotate", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], name)
    labels = {line[:-1]: index for index, line in enumerate(body) if line.endswith(":")}
    back = [line for index, line in enumerate(body) if line.startswith("jmp ") and labels.get(line.split()[1], index) < index]
    assert back == [], body


def test_short_value_crosses_a_call_in_si_or_di():
    """A call was said to destroy every register, so the running total lived
    in a slot and each pass wrote `add word ptr [bp-4], ax`. The callee keeps
    SI and DI as 16-bit registers."""
    text = cfront.compiled((FIXTURES / "crosscall.cgs").read_text(), "crosscall", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_total")
    assert any(line in ("add si, ax", "add di, ax") for line in body), body


def test_loaded_far_pointer_moves_whole():
    """A far pointer read from memory was split into offset and segment and
    never put back together: stored as two words, reloaded as a dword, split
    again and rejoined through `push bx / push ax / pop eax` to test it."""
    text = cfront.compiled((FIXTURES / "farptr.cgs").read_text(), "farptr", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    body = lines[lines.index("_copy proc far") : lines.index("_copy endp")]
    assert not any("_pts+2" in line or line == "pop eax" for line in body), body


def test_far_pointer_in_memory_is_read_as_its_two_words():
    """`table->x + table->y` loaded the pointer as a dword and split it:
    `mov ebx,[_table]; mov ecx,ebx; shr ecx,16; mov es,cx`. 1,076 splits
    over qcport where bcc writes `les bx,[_table]`."""
    text = cfront.compiled((FIXTURES / "farderef.cgs").read_text(), "farderef", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_sum")
    assert not any(line.startswith("shr") or line.startswith("mov e") for line in body), body
    assert body[2] == "les bx, dword ptr _table", body


@pytest.mark.parametrize("name", ["_grab", "_pass"])
def test_long_call_result_is_consumed_as_its_two_words(name):
    """A long returned in DX:AX was joined through `push dx; push ax; pop eax`
    only to be stored, tested against zero or pushed: 202 joins over qcport."""
    text = cfront.compiled((FIXTURES / "longret.cgs").read_text(), "longret", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], name)
    assert not any(line.startswith("pop e") for line in body), body


def test_indexed_cell_reaches_its_whole_symbol():
    """An index value with no register in the address read as element zero
    alone, so a store to `sy[j]` did not reach `sy[2]`."""
    Addr, Space = cfront.raise_hir.Addr, cfront.raise_hir.Space
    j = cfront.mir.Value(1, 10)
    element = cfront.mir.MemRef(Addr(Space.SEGMENT, 0, 5), 2, base=j, space=Space.SEGMENT, base_width=2)
    fixed = cfront.mir.MemRef(Addr(Space.SEGMENT, 4, 5), 2, space=Space.SEGMENT)
    assert cfront.mir.overlapping(fixed, element, frozenset())


def test_pointer_store_misses_a_frame_whose_address_stays_home():
    """snd_fetch's `out[j] = ...` could reach any frame slot, so j and out were
    reloaded from the frame every sample. C reaches a local through a pointer
    only where the body took its address; keep takes one, and t is reread."""
    text = cfront.compiled((FIXTURES / "indexed.cgs").read_text(), "indexed", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    fill = lines[lines.index("_fill proc far") : lines.index("_fill endp")]
    assert sum("[bp+6]" in line for line in fill) <= 1 and not any(", word ptr [bp-4]" in line for line in fill)
    keep = lines[lines.index("_keep proc far") : lines.index("_keep endp")]
    assert "mov ax, word ptr [bp-6]" in keep


def test_confined_reload_moves_the_one_in_its_register():
    """sc_lru_use's `sc->ltail[c] = b` wants the reloaded sc in BX, the only
    base register, where the reloaded b already sat: `value#119 cannot be
    spilled and no register is free for it`, though b fit in SI or DI."""
    text = cfront.compiled((FIXTURES / "indexed.cgs").read_text(), "indexed", optimise=True)
    lines = [line.strip() for line in text.splitlines()]
    body = lines[lines.index("_lru_use proc far") : lines.index("_lru_use endp")]
    assert any(line.startswith("mov word ptr es:[bx+") for line in body)


def test_loop_limit_follows_the_start_it_adds_to():
    """sc_init's `sc->desc[i] = 0`: indvars put the new loop limit before the
    last operation of a preheader that fell through, the one defining the
    start it adds to. Lowering read the limit from an unset SI; sc_selftest hung."""
    from qbopt.optimize import transform

    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "indexed.cgs").read_text()))
    proc = next(one for one in unit.procs if unit.symbols[one.symbol].name == "clear")
    raised = cfront.raise_hir.raised(unit, proc)
    body = transform.applied(raised.body, frozenset(), raised.calls, found=None)
    for block in body.blocks:
        later = {value for op in block.ops for value in op.defines}
        for op in block.ops:
            assert not later.intersection(op.uses), f"{op.at:#x} {op.kind} reads {later.intersection(op.uses)}"
            later.difference_update(op.defines)


def test_verify_reports_a_use_before_its_definition_in_one_block():
    """verify asked only whether the defining block dominates, so the loop limit
    read before its start, in the same block, passed as SSA."""
    mir, ir = cfront.mir, cfront.raise_hir.ir
    start, limit = mir.Value(1, 1), mir.Value(2, 1)
    add = mir.Op(1, ir.Operation.NOTHING, "", (limit,), (start,), kind=mir.Kind.ADD,
                 args=(mir.Held(start, 2), mir.Const(100, 2)), results=(mir.Held(limit, 2),))
    copy = mir.Op(1, ir.Operation.NOTHING, "", (start,), (), kind=mir.Kind.COPY,
                  args=(mir.Const(0, 2),), results=(mir.Held(start, 2),))
    body = mir.MirBody(1, (mir.MirBlock(1, (), (add, copy), ()),))
    assert any("before its definition" in problem for problem in mir.verify(body))


@pytest.mark.parametrize("name", ["_half", "_eighth", "_gaps"])
def test_signed_word_division_by_a_power_of_two_shifts(name):
    """Only a long divided by a power of two became shifts; a word went to
    `idiv`, which shellsort's `gap /= 2` paid on every halving."""
    text = cfront.compiled((FIXTURES / "halve.cgs").read_text(), "halve", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], name)
    assert not any(line.startswith("idiv") for line in body), body


@pytest.mark.parametrize(
    ("name", "strings"),
    [("_fill_bytes", ("rep stosb",)), ("_fill_words", ("rep stosw",)), ("_fill_far", ("rep stosw", "rep stosb")),
     ("_fill_counted", ("rep stosw",)), ("_fill_local", ("rep stosw",)),
     ("_fill_through", ("rep stosw",)), ("_fill_voices", ("rep stosw",))],
)
def test_counted_store_of_one_value_is_a_string_fill(name, strings):
    """A loop storing one value into consecutive cells took a compare, a
    branch, a store and an add per cell: sieve's reset was 32,764 cycles a
    pass where bcc's `rep stosw` is 4,099. Far statics, qcport's `reached`
    and `used`, stayed loops: the store's selector aliased the bound, and the
    counter was read after the loop. A local array whose address a call takes
    left every local in memory; a store through `int far *` reloaded the pointer, and one through `&acc`
    reloaded a static bound."""
    text = cfront.compiled((FIXTURES / "fill.cgs").read_text(), "fill", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], name)
    labels = {line[:-1]: index for index, line in enumerate(body) if line.endswith(":")}
    back = [line for index, line in enumerate(body) if line.startswith("j") and labels.get(line.split()[-1], index + 1) <= index]
    assert all(one in body for one in strings) and back == [], body


def test_a_test_known_to_fail_is_gone():
    """A branch never taken stayed when it stood for no bytes: decide hands a
    removed branch's bytes to the op before it, and the C path's have none to
    hand, so the fill's guard compared a known 0 with 99 on every call."""
    text = cfront.compiled((FIXTURES / "fill.cgs").read_text(), "fill", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_fill_bytes")
    assert not any(line.startswith(("cmp", "jg")) for line in body), body


def test_one_expression_through_twin_counters_reaches_a_fixed_point():
    """qcport's mod_link_anims stopped compiling once its counters left memory:
    `t[j]` read four times through two counters stepping alike, and strength
    gave each read its own recurrence, one a round, for more than 16 rounds."""
    text = cfront.compiled((FIXTURES / "anims.cgs").read_text(), "anims", optimise=True)
    assert "_link_anims" in text

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
    refused pal_bestfit with `used at width 4` until it converted them.

    Both narrow operands may be extended straight into their 32-bit
    destinations. Requiring either older register-to-register extension pair
    made this regression reject a strictly better lowering.
    """
    lines = _asm("pal")
    at = lines.index("movzx eax, byte ptr _pal_now[bx]")
    assert lines[at + 1 : at + 3] == ["movsx ebx, word ptr [bp+6]", "sub eax, ebx"]


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


def test_optimized_branch_result_is_promoted_to_a_phi():
    """pick's two arms stored 7/9 to a frame temporary and reloaded it at
    their join. Restricting scalar promotion to loops left ordinary
    straight-line mem2reg work behind and blocked later inlining.
    """
    lines = [
        line.strip()
        for line in cfront.compiled((FIXTURES / "choose.cgs").read_text(), "choose", optimise=True).splitlines()
    ]
    # Promotion now exposes the one-use helper to inlining, so the strongest
    # result has no _pick body to inspect.  A regression leaves either the
    # helper or its inlined temporary behind; inspect whichever body owns the
    # choice instead of requiring one particular inlining decision.
    if "_pick proc near" in lines:
        body = lines[lines.index("_pick proc near") : lines.index("_pick endp")]
    else:
        body = lines[lines.index("_choose proc far") : lines.index("_choose endp")]
    assert not any("[bp-" in line for line in body)


def test_data_pointer_to_a_literal():
    """ls's style table points at its pattern strings; DGBackPtr was refused."""
    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "ls.cgs").read_text()))
    segments = dict(cfront._data(unit))
    pointer = cfront.masm.Pointer
    assert pointer("L_b2", 0, False) in next(items for items in segments.values() if pointer("L_b3", 0, False) in items)


def test_negative_data_fits_its_width():
    """`short sbar_health = -1` printed as `dw 4294967295`: the shim writes a
    negative item as 32 bits, and jwasm refused the initializer."""
    unit = cfront.hir.Unit()
    unit.segments[1] = cfront.hir.Segment(1, "_DATA", 0, [("DGInteger", ("4294967295", "TY_INT_2"))])
    assert dict(cfront._data(unit))["_DATA"] == (b"\xff\xff",)


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
                    assert (op.op, op.name) == (ir.Operation.NOTHING, ""), (module, op)
                    for ref in (*op.loads, *op.stores):
                        if ref.addr is not None and ref.addr.space is Space.FAR:
                            assert (ref.addr.base, ref.addr.segment) == (0, 0), (module, ref)


def test_constant_return_propagates_across_a_direct_call():
    """add_answer used the opaque AX result of answer() even though every
    returning path produces 37.  Local SCCP cannot cross that procedure
    boundary; the caller must receive the module summary's constant.
    """
    lines = [
        line.strip()
        for line in cfront.compiled((FIXTURES / "ipconst.cgs").read_text(), "ipconst", optimise=True).splitlines()
    ]
    body = lines[lines.index("_add_answer proc far") : lines.index("_add_answer endp")]
    assert "call _answer" not in body
    assert any(line.startswith("add ") and line.endswith(", 37") for line in body)
    assert "_answer proc near" not in lines


def test_global_dce_keeps_an_address_taken_private_procedure():
    """qcport item.c lost every item_take callback from an initialized table.

    A static function absent from the direct-call graph may still be reached
    through a function pointer or an initialized relocation.
    """
    unit = cfront.hir.Unit()
    unit.symbols[1] = cfront.hir.Symbol(1, "callback", "callback", "_*", cfront.hir.FE_PROC)
    unit.nodes[1] = cfront.hir.Node("CGFEName", ("y1", "TY_CODE_PTR"))
    unit.calls[1] = cfront.hir.Call("n1", "TY_INT_2", 1)
    assert cfront._address_taken_procedures(unit) == frozenset()

    unit.nodes[2] = cfront.hir.Node("CGUnary", ("O_CONVERT", "n1", "TY_POINTER"))
    assert cfront._address_taken_procedures(unit) == frozenset({"_callback"})

    del unit.nodes[2]
    unit.backs[1] = 1
    assert cfront._address_taken_procedures(unit) == frozenset({"_callback"})

    data_root = cfront.hir.Unit()
    data_root.symbols[1] = cfront.hir.Symbol(1, "callback", "callback", "_*", cfront.hir.FE_PROC)
    data_root.segments[1] = cfront.hir.Segment(1, "callbacks", 0, [("DGFEPtr", ("y1", "TY_NEAR_POINTER", "0"))])
    assert cfront._address_taken_procedures(data_root) == frozenset({"_callback"})


def test_private_constant_argument_specializes_before_local_sccp():
    """twice is called only as twice(21), but its frame load hid 42 from
    caller SCCP. The specialized pure call should collapse without losing
    the same-address constant copy that defines AX.
    """
    lines = [
        line.strip()
        for line in cfront.compiled((FIXTURES / "iparg.cgs").read_text(), "iparg", optimise=True).splitlines()
    ]
    assert "_twice proc near" not in lines
    body = lines[lines.index("_answer_from_argument proc far") : lines.index("_answer_from_argument endp")]
    assert "call _twice" not in body
    assert "mov ax, 42" in body


def test_int64_stream_raises_whole_signed_and_unsigned_mir_values():
    """Watcom emits TY_{U,}INT_8, but qbopt stopped at `no scalar width`.

    Keep the value whole at the MIR boundary.  Argument and result splitting
    is a target ABI decision for lowering, not a reason to represent C's
    arithmetic as four unrelated words in the optimizer.
    """
    from qbopt.model import floating
    from qbopt.optimize import transform

    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "mir" / "int64.cgs").read_text()))
    bodies = {}
    for proc in unit.procs:
        name = unit.symbols[proc.symbol].name
        raised = cfront.raise_hir.raised(unit, proc)
        assert not cfront.mir.verify(raised.body), name
        bodies[name] = raised.body
        optimised = transform.applied(raised.body, frozenset(), raised.calls, found=None)
        assert not cfront.mir.verify(optimised), name

    def operations(name):
        return [op for block in bodies[name].blocks for op in block.ops]

    add = next(op for op in operations("i64Add") if op.kind is cfront.mir.Kind.ADD)
    assert [one.width for one in (*add.args, *add.results)] == [8, 8, 8]

    unsigned = operations("u64Math")
    assert any(op.kind is cfront.mir.Kind.MUL and op.results[0].width == 8 for op in unsigned)
    assert any(op.kind is cfront.mir.Kind.UDIVMOD and all(one.width == 8 for one in op.results) for op in unsigned)
    assert any(op.kind is cfront.mir.Kind.SHR and op.args[0].width == 8 for op in unsigned)

    signed_test = next(op for op in operations("i64Less") if op.kind is cfront.mir.Kind.BRANCH)
    unsigned_test = next(op for op in operations("u64Less") if op.kind is cfront.mir.Kind.BRANCH)
    # The value-producing comparison branches to its false arm, hence the
    # inverse tests.  The important distinction is signed GE against the
    # unsigned ABOVE_EQ over the same eight-byte operands.
    assert signed_test.test is cfront.mir.Kind.GE
    assert unsigned_test.test is cfront.mir.Kind.ABOVE_EQ

    signed_extend = next(op for op in operations("i64Extend") if op.kind is cfront.mir.Kind.SIGN_EXTEND)
    unsigned_extend = next(op for op in operations("u64Extend") if op.kind is cfront.mir.Kind.ZERO_EXTEND)
    assert (signed_extend.args[0].width, signed_extend.results[0].width) == (2, 8)
    assert (unsigned_extend.args[0].width, unsigned_extend.results[0].width) == (2, 8)

    narrow_store = next(
        op for op in operations("i64Narrow") if op.kind is cfront.mir.Kind.STORE and op.stores[0].width == 4
    )
    assert narrow_store.args[0].width == 4

    called = operations("i64Call")
    assert any(op.kind is cfront.mir.Kind.ARG and op.args[0].width == 8 for op in called)
    assert any(op.kind is cfront.mir.Kind.CALL and op.results[0].width == 8 for op in called)
    assert any(op.kind is cfront.mir.Kind.RETURN and op.args[0].width == 8 for op in called)

    signed_load = next(op for op in operations("i64ToDouble") if op.kind is cfront.mir.Kind.FLOAD)
    unsigned_load = next(op for op in operations("u64ToDouble") if op.kind is cfront.mir.Kind.FLOAD)
    signed_store = next(op for op in operations("doubleToI64") if op.kind is cfront.mir.Kind.FSTORE)
    unsigned_store = next(op for op in operations("doubleToU64") if op.kind is cfront.mir.Kind.FSTORE)
    assert signed_load.floating.inputs == (floating.Format.SIGNED64,)
    assert unsigned_load.floating.inputs == (floating.Format.UNSIGNED64,)
    assert signed_store.floating.result is floating.Format.SIGNED64
    assert unsigned_store.floating.result is floating.Format.UNSIGNED64
    assert signed_store.results[0].width == unsigned_store.results[0].width == 8

    # Constant evaluation must not round a 64-bit dividend through Python's
    # binary64 float on the way to C's toward-zero quotient and remainder.
    dividend = -(2**63) + 17
    quotient = -1844674407370955158
    assert cfront.raise_hir._fold("O_DIV", dividend, 5, True) == quotient
    assert cfront.raise_hir._fold("O_MOD", dividend, 5, True) == dividend - quotient * 5

    from qbopt.backend import lower_int64

    legalized = lower_int64.expanded(bodies["i64Add"])
    assert not any(
        getattr(arg, "width", None) == 8
        for block in legalized.body.blocks
        for op in block.ops
        for arg in (*op.args, *op.results)
    )
    cfront.lower.lowered("int64", legalized.body, legalized.calls, (), legalized.contracts)


@pytest.mark.parametrize(
    ("module", "kinds"),
    [
        ("mix64", {cfront.mir.Kind.SHR, cfront.mir.Kind.XOR, cfront.mir.Kind.MUL}),
        ("euclid64", {cfront.mir.Kind.UDIVMOD, cfront.mir.Kind.DIVMOD, cfront.mir.Kind.MUL}),
        ("fib64", {cfront.mir.Kind.ADD}),
    ],
)
def test_int64_number_crunching_programs_survive_the_mir_pipeline(module, kinds):
    """Self-checking kernels used to stop at the first TY_{U,}INT_8 value.

    Each fixture has a known-answer `*Check` procedure that returns zero.
    Every procedure raises and optimizes as valid SSA, the arithmetic kernel
    retains the distinct 64-bit work it was written to test, and target
    lowering emits all procedures.  The DOS semantic gate lives beside this
    test in test_cfront_int64_e2e.py.
    """
    from qbopt.optimize import transform

    path = FIXTURES / "mir" / f"{module}.cgs"
    unit = cfront.hir.unit(cfront.stream.parse(path.read_text()))
    observed = set()
    checks = []
    for proc in unit.procs:
        raised = cfront.raise_hir.raised(unit, proc)
        assert not cfront.mir.verify(raised.body), (module, raised.name)
        optimised = transform.applied(raised.body, frozenset(), raised.calls, found=None)
        assert not cfront.mir.verify(optimised), (module, raised.name)
        ops = [op for block in raised.body.blocks for op in block.ops]
        observed.update(
            op.kind
            for op in ops
            if any(
                isinstance(value, (cfront.mir.Held, cfront.mir.Const)) and value.width == 8
                for value in (*op.args, *op.results)
            )
        )
        if raised.name.endswith("Check"):
            checks.extend(ops)

    assert kinds <= observed
    assert any(op.kind is cfront.mir.Kind.CALL and op.results and op.results[0].width == 8 for op in checks)
    assert any(op.kind is cfront.mir.Kind.BRANCH for op in checks)

    # These are programs, not parser samples: the machine half must accept
    # the same optimized bodies and emit every procedure, including main.
    assembly = cfront.compiled(path.read_text(), module, optimise=True)
    assert "64Check proc" in assembly
    assert "_main proc" in assembly


def test_int64_helper_result_placement_is_an_external_hint() -> None:
    """euclid64's remainder lived in EBX:ECX by mutating ``MirBody.origin``.

    Int64 legalization is the machine boundary, so its fixed helper result is
    a backend allocation hint keyed by the new scalar variables.  Public MIR
    must remain unchanged; otherwise removing ``MirBody.origin`` would silently
    discard the helper ABI and euclid64 would miscompile after allocation.
    """
    from iced_x86 import Register

    from qbopt.backend import lower_int64

    unit = cfront.hir.unit(cfront.stream.parse((FIXTURES / "mir" / "euclid64.cgs").read_text()))
    raised = next(
        cfront.raise_hir.raised(unit, proc) for proc in unit.procs if unit.symbols[proc.symbol].object_name == "_gcd64"
    )
    legalized = lower_int64.expanded(raised.body, raised.calls, raised.contracts, raised.hints)

    assert not hasattr(legalized.body, "origin")
    added = {
        variable: register
        for variable, register in legalized.hints.origins.items()
        if variable not in raised.hints.origins
    }
    assert set(added.values()) == {Register.EBX, Register.ECX}
    assert len(added) == 2


def test_int64_hot_helpers_do_work_proportional_to_the_operands() -> None:
    """signedCrunch64 used to run 64 restoring rounds for every `/ 7`.

    Its `* 5` and `* 11` also computed three full dword products although a
    dword constant has only one cross product.  Keep the generated helper
    shapes honest: constant division uses hardware DIV, general division has
    no fixed 64-round counter, and narrow multiplication emits two products.
    """
    from iced_x86 import Decoder
    from iced_x86 import Mnemonic

    def helpers(module: str) -> dict[str, list[bytes]]:
        path = FIXTURES / "mir" / f"{module}.cgs"
        built = cfront.assembled(path.read_text(), module, optimise=True)
        found: dict[str, list[bytes]] = {}
        for procedure in built.procedures:
            for callee in procedure.callees.values():
                if callee.code:
                    found.setdefault(callee.name, []).append(b"".join(callee.code))
        return found

    mix = helpers("mix64")
    euclid = helpers("euclid64")

    general_multiply = list(Decoder(16, mix["__U8M"][0]))
    narrow_multiply = list(Decoder(16, euclid["__U8M32"][0]))
    assert len(general_multiply) == 7
    assert sum(one.mnemonic in (Mnemonic.IMUL, Mnemonic.MUL) for one in general_multiply) == 3
    assert len(narrow_multiply) == 4
    assert sum(one.mnemonic in (Mnemonic.IMUL, Mnemonic.MUL) for one in narrow_multiply) == 2

    constant_divide = euclid["__I8D32"][0]
    general_divide = euclid["__U8D"][0]
    signed_divide = euclid["__I8D"][0]
    assert sum(one.mnemonic is Mnemonic.DIV for one in Decoder(16, constant_divide)) == 4
    assert sum(one.mnemonic is Mnemonic.DIV for one in Decoder(16, general_divide)) == 2
    assert sum(one.mnemonic is Mnemonic.DIV for one in Decoder(16, signed_divide)) == 2
    assert b"\xb9\x40\x00" not in constant_divide
    assert b"\xb9\x40\x00" not in general_divide
    assert b"\xb9\x40\x00" not in signed_divide


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
    assert len(saves) == 1 and saves[0] < at
    masks = [
        line
        for line in body
        if line.startswith("or ") and not line.startswith("or word ptr") and line.endswith((", 3072", ", 12"))
    ]
    assert len(masks) == 1


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
    # Argument extensions may legitimately spill before the loop, and their
    # offsets change with allocation.  The dead dr/dg/db/d temporaries were
    # stores between the first palette load and the distance comparison.
    first_component = next(index for index, line in enumerate(body) if "byte ptr _pal_now" in line)
    comparison = next(index for index in range(first_component, len(body)) if body[index].startswith("jge "))
    assert not any(line.startswith("mov dword ptr [bp-") for line in body[first_component:comparison])


def test_strlen_does_not_force_a_counter_reload() -> None:
    """ls_init reloaded ``i`` after strlen although strlen only reads its string argument."""
    text = cfront.compiled((FIXTURES / "ls.cgs").read_text(), "ls", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_ls_init")
    call = body.index("call far ptr _strlen")
    length_store = next(
        index
        for index in range(call + 1, len(body))
        if body[index].startswith("mov word ptr [") and body[index].endswith("+2], ax")
    )

    assert not any(line.startswith("mov ") and "[bp-" in line for line in body[call + 1 : length_store]), body


def _proc(lines: list[str], name: str) -> list[str]:
    return lines[lines.index(f"{name} proc far") : lines.index(f"{name} endp")]


def test_float_compare_moves_the_status_word_into_the_flags():
    """`a < b` on floats, 66 of qcport's procedures, was refused. x87 C0 and C3
    reach CF and ZF through sahf, so the branch is an unsigned one."""
    lines = _asm("floats")
    pick = _proc(lines, "_pick")
    at = pick.index("fnstsw ax")
    assert pick[at - 2 : at + 3] == [
        "fld dword ptr [bp+6]",
        "fcomp dword ptr [bp+10]",
        "fnstsw ax",
        "sahf",
        "jae L1_10",
    ]
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
    at = next(index for index, line in enumerate(body) if line.startswith("movsx ") and "word ptr [bp+10]" in line)
    widened = body[at].split()[1].rstrip(",")
    assert body[at + 1 : at + 3] == [f"add eax, {widened}", "mov dword ptr [bp+6], eax"]


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
    """cdecl pushes last first and pops after; pascal pushes first first.

    Assert the values and order, not which free register happens to carry the
    address or whether a frame argument was folded into ``push``.
    """
    lines = _asm("qglsurf")
    at = lines.index("call far ptr _asset_seek")
    address = lines[at - 3]
    assert address.startswith("lea ") and address.endswith(", [bp-10]")
    register = address.removeprefix("lea ").split(",", 1)[0]
    assert lines[at - 2 : at + 2] == [
        f"push {register}",
        "push word ptr [bp+6]",
        "call far ptr _asset_seek",
        "add sp, 4",
    ]
    at = lines.index("call far ptr QGLSFNEW")
    assert lines[at - 3] == "push word ptr [bp+8]"
    assert lines[at - 2].startswith("push ") and "ptr" not in lines[at - 2]
    assert lines[at - 1] == "push word ptr [bp+10]"


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
        and any(
            ref.addr is not None and ref.addr.space is cfront.raise_hir.Space.FRAME and ref.addr.disp < 0
            for ref in op.loads
        )
    ]
    assert local == []


def test_symbol_argument_is_pushed_as_a_literal():
    """Fold put a number into the argument that read it but not a symbol, so
    `tag` stood as `mov di, offset _tag`, left the loop, and took a register
    or a slot where `push offset _tag` needs neither."""
    text = cfront.compiled((FIXTURES / "loopaddr.cgs").read_text(), "loopaddr", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_sum")
    assert "push offset _tag" in body and not any(line.startswith("mov ") and "offset _tag" in line for line in body), (
        body
    )


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
    back = [
        line
        for index, line in enumerate(body)
        if line.startswith("jmp ") and labels.get(line.split()[1], index) < index
    ]
    assert back == [], body


def test_short_value_crosses_a_call_in_si_or_di():
    """crosscall's total and counter were merged because both started at zero.

    Besides producing the wrong loop condition, that left `add ax,si / mov
    si,ax` in place of an update to the callee-saved accumulator. Parallel
    phi destinations must remain distinct, and the total should cross MAP in
    SI or DI without a frame round trip or a result copy.
    """
    text = cfront.compiled((FIXTURES / "crosscall.cgs").read_text(), "crosscall", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_total")
    updates = [line for line in body if line in ("add si, ax", "add di, ax")]
    assert len(updates) == 1, body
    accumulator = updates[0].split()[1].rstrip(",")
    assert not any(line == f"mov {accumulator}, ax" for line in body), body


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


def test_hot_loop_loads_far_fields_before_owner_address_splits():
    """r_walk reloaded each far field as two words after allocation had
    rematerialized its near owner separately.  BCC selects `les` while the
    words still share their owner, avoiding one address reconstruction per
    field per iteration."""
    text = cfront.compiled((FIXTURES / "farloadloop.cgs").read_text(), "farloadloop", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_mark")
    loads = [line for line in body if line.startswith("les ")]
    assert len(loads) >= 2, body
    # `world` and `rdr` are adjacent *near* arguments.  They must never be
    # mistaken for the two words of one far pointer merely because their
    # frame slots touch.
    assert not any("[bp+6]" in line for line in loads), body


def test_hot_loop_retains_invariant_far_field_owners_under_pressure():
    """r_walk loaded `world` and `rdr` from BP on every marked face.

    Both pointers are loop-invariant near owners of far fields.  Once the
    displaced word indexes can fold into their dying field bases, keeping the
    owners costs no extra reload register; the allocator must choose that
    complete pressure plan rather than rematerializing the owners per pass.
    """
    text = cfront.compiled((FIXTURES / "farloadloop.cgs").read_text(), "farloadloop", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_mark")
    jump = next(line for line in body if line.startswith("jl "))
    loop_label = jump.split()[1] + ":"
    loop = body[body.index(loop_label) : body.index(jump)]

    assert not any("[bp+6]" in line or "[bp+8]" in line for line in loop), loop
    before = body[: body.index(loop_label)]
    assert any("[bp+6]" in line for line in before), body
    assert any("[bp+8]" in line for line in before), body


def test_hot_loop_does_not_carry_a_scaled_index_that_spills() -> None:
    """farloadloop grew a second induction variable for ``i * 2``.

    The derived value was updated in a frame slot on every iteration, making
    the strength-reduced loop 75 bytes / weighted 386 cost 141; recomputing
    the cheap shift produced 71 / 133.  Formula selection must reject a
    recurrence whose backedge lifetime costs more than the work it removes.
    """
    text = cfront.compiled((FIXTURES / "farloadloop.cgs").read_text(), "farloadloop", optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_mark")
    jump = next(line for line in body if line.startswith("jl "))
    loop_label = jump.split()[1] + ":"
    loop = body[body.index(loop_label) : body.index(jump)]

    assert not any(line.startswith("add word ptr [bp-") and line.endswith(", 2") for line in loop), loop


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
    add = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (limit,),
        (start,),
        kind=mir.Kind.ADD,
        args=(mir.Held(start, 2), mir.Const(100, 2)),
        results=(mir.Held(limit, 2),),
    )
    copy = mir.Op(
        1,
        ir.Operation.NOTHING,
        "",
        (start,),
        (),
        kind=mir.Kind.COPY,
        args=(mir.Const(0, 2),),
        results=(mir.Held(start, 2),),
    )
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
    [
        ("_fill_bytes", ("rep stosb",)),
        ("_fill_words", ("rep stosw",)),
        ("_fill_far", ("rep stosw", "rep stosb")),
        ("_fill_counted", ("rep stosw",)),
        ("_fill_local", ("rep stosw",)),
        ("_fill_through", ("rep stosw",)),
        ("_fill_voices", ("rep stosw",)),
    ],
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
    back = [
        line
        for index, line in enumerate(body)
        if line.startswith("j") and labels.get(line.split()[-1], index + 1) <= index
    ]
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


def test_nonvolatile_far_byte_or_is_one_read_modify_write() -> None:
    """A byte bitmap update widened to three word temporaries before allocation.

    That exhausted the address-register file in QCport's r_walk marked-face
    loop, preventing the normal hottest-loop live-range split.  The source is
    intentionally standalone: the rule is a non-volatile byte compound
    assignment, not a renderer or bitmap-specific exception.
    """
    source = FIXTURES / "rmwbyte.c"
    text = cfront.compiled(cfront.recorded(source, []), source.stem, optimise=True)
    body = _proc([line.strip() for line in text.splitlines()], "_mark_bit")

    assert any(line.startswith("or byte ptr es:[") for line in body), body

from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import peephole


@pytest.mark.parametrize("width,register,value", [(2, Register.AX, 1), (4, Register.ECX, 0x12345678)])
def test_screen_argument_reuses_its_required_register_constant(width, register, value):
    """SCREEN's duplicate PUSH 1/MOV AX,1 contributed to E1M1 exhausting its far heap."""
    from qbopt.backend import select

    operand = ir.Imm(value, width)
    dest = ir.Reg(register, width)
    push = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.PUSH, "push", (), (operand,)), (), ())
    move = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (operand,)), (7,), ())
    body = lir.LirBody("screen", 0, (lir.LirBlock(0, (push, move)),), {}, {})
    result = peephole.Peephole().transform(body)
    expected = (move.what, replace(push.what, sources=(dest,)))
    assert b"".join(select.emit(one.what).code for one in result.insns) == b"".join(
        select.emit(what).code for what in expected
    )
    assert result.insns[0].defines == (7,)
    assert result.insns[1].uses == (7,)


@pytest.mark.parametrize(
    "barrier",
    [
        "different",
        "width",
        "stack",
        "frame",
        "relocation",
        "covered",
        "gap",
        "group",
        "symbol",
        "clobber",
        "requires",
        "block",
    ],
)
def test_argument_materialization_does_not_cross_observable_boundaries(barrier):
    """SCREEN's stack argument must not change when its register setup cannot move before it."""
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    literal = ir.Imm(1, 2)
    push = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.PUSH, "push", (), (literal,)), (), ())
    move = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (literal,)), (7,), ())
    match barrier:
        case "different":
            move = replace(move, what=replace(move.what, sources=(ir.Imm(2, 2),)))
        case "width":
            move = replace(move, what=replace(move.what, dests=(ir.Reg(Register.EAX, 4),)))
        case "stack" | "frame":
            move = replace(
                move, what=replace(move.what, dests=(ir.Reg(Register.SP if barrier == "stack" else Register.BP, 2),))
            )
        case "relocation":
            symbol = replace(literal, address=Addr(Space.SEGMENT, 0, 1))
            push = replace(push, what=replace(push.what, sources=(symbol,)))
            move = replace(move, what=replace(move.what, sources=(symbol,)))
        case "covered":
            move = replace(move, covers=(1, 4))
        case "gap":
            move = replace(move, at=2, covers=(2, 2))
        case "group":
            move = replace(move, group=1)
        case "symbol":
            move = replace(move, symbol=True)
        case "clobber":
            push = replace(push, clobbers=frozenset({Register.AX}))
        case "requires":
            push = replace(push, requires=((ir.Held(5, 2), Register.AX),))
    blocks = (
        (lir.LirBlock(0, (push,), (1,)), lir.LirBlock(1, (move,)))
        if barrier == "block"
        else (lir.LirBlock(0, (push, move)),)
    )
    body = lir.LirBody("screen", 0, blocks, {}, {})
    assert peephole.pushed_constants(body) == body


def test_nbody_does_not_reload_unchanged_array_index():
    """NBODY reloaded SI from its index spill at 0x23a after already loading it at 0x22a."""
    from qbopt import wholeseg

    states = []

    def watch(stage, name, body):
        if stage == "peephole" and body.entry == 0x30:
            states.append(body)

    result = wholeseg.emitted(Path("fixtures/bench/nbody-v-g3.obj").read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert not any(one.at == 0x23A and one.spill_reload for one in states[0].insns)


@pytest.mark.parametrize("mismatch", ["none", "register", "slot", "width", "clobber", "missing", "unowned", "entry"])
def test_entry_reload_requires_agreement_on_every_edge(mismatch):
    """NBODY's header reload is redundant only when all paths carry the exact stored bits."""
    from qbopt.backend import spillforward
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    register = ir.Reg(Register.EAX, 4)
    cell = ir.Mem(Addr(Space.FRAME, -4), 4, through=Register.BP)
    store = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (cell,), (register,)), (), ())
    other = store
    match mismatch:
        case "register":
            other = replace(store, what=replace(store.what, sources=(ir.Reg(Register.ECX, 4),)))
        case "slot":
            other = replace(store, what=replace(store.what, dests=(replace(cell, addr=cell.addr.plus(-4)),)))
        case "width":
            other = replace(store, what=replace(store.what, dests=(replace(cell, width=2),)))
        case "clobber":
            other = replace(store, clobbers=frozenset({Register.EAX}))
        case "missing":
            other = replace(store, what=None)
    reload = lir.Insn(
        3,
        (3, 3),
        ir.Semantics(ir.Operation.MOVE, "mov", (register,), (cell,)),
        (),
        (),
        spill_reload=mismatch != "unowned",
    )
    body = lir.LirBody(
        "join",
        3 if mismatch == "entry" else 0,
        (
            lir.LirBlock(0, (), (1, 2)),
            lir.LirBlock(1, (store,), (3,)),
            lir.LirBlock(2, (other,), (3,)),
            lir.LirBlock(3, (reload,), ()),
        ),
        {},
        {},
    )
    done = spillforward.forwarded(body)
    kept = any(one.spill_reload or one.what == reload.what for one in done.blocks[-1].insns)
    assert kept == (mismatch not in ("none", "unowned"))


def test_forwarded_spill_reload_retains_its_virtual_definition():
    """BASIC nbody's store read value#979 after spill forwarding removed its reload."""
    from qbopt.backend import verify
    from qbopt.backend import spillforward
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    register = ir.Reg(Register.EAX, 4)
    source = ir.Mem(Addr(Space.FRAME, -4), 4, through=Register.BP)
    destination = ir.Mem(Addr(Space.SEGMENT, 8), 4)
    establish = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (source,), (register,)), (), ())
    reload = lir.Insn(
        2,
        (2, 2),
        ir.Semantics(ir.Operation.MOVE, "mov", (register,), (source,)),
        (2,),
        (),
        spill_reload=True,
    )
    consume = lir.Insn(
        3,
        (3, 4),
        ir.Semantics(ir.Operation.MOVE, "mov", (destination,), (register,)),
        (),
        (2,),
    )
    body = lir.LirBody("forwarded-definition", 1, (lir.LirBlock(1, (establish, reload, consume)),), {}, {})

    done = spillforward.forwarded(body)

    assert not verify.verify(done)
    assert sum(one.what.op is ir.Operation.MOVE for one in done.insns) == 2


def test_forwarded_register_copy_retains_its_virtual_definition():
    """PITSNAP's IN read value#188 after copy propagation removed its AL setup."""
    from qbopt.backend import verify
    from qbopt.backend import copyprop

    al = ir.Reg(Register.AL, 1)
    ah = ir.Reg(Register.AH, 1)
    establish = lir.Insn(
        1,
        (1, 2),
        ir.Semantics(ir.Operation.MOVE, "mov", (al,), (ah,)),
        (),
        (),
    )
    define = replace(establish, at=2, covers=(2, 3), defines=(2,))
    consume = lir.Insn(3, (3, 4), None, (3,), (2,))
    body = lir.LirBody(
        "forwarded-copy-definition",
        1,
        (lir.LirBlock(1, (establish, define, consume)),),
        {},
        {},
    )

    done = copyprop.forwarded(body)

    assert not verify.verify(done)
    assert sum(one.what is not None and one.what.op is ir.Operation.MOVE for one in done.insns) == 1


def test_nbody_accumulator_does_not_copy_its_addend_over_its_running_sum():
    """NBODY copied ECX to ESI and EAX to ECX before ADD ECX,ESI on every force pair."""
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/bench/nbody-v-g3.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert not any(
        instructions[index : index + 3] == ["mov esi,ecx", "mov ecx,eax", "add ecx,esi"]
        for index in range(len(instructions) - 2)
    )


@pytest.mark.parametrize("name", ["add", "and", "or", "xor", "sub", "adc"])
@pytest.mark.parametrize("width", [2, 4])
def test_commuted_accumulator_keeps_the_saved_value(name, width):
    """NBODY's saved accumulator must remain valid even when a later instruction reads it."""
    registers = (Register.CX, Register.SI, Register.AX) if width == 2 else (Register.ECX, Register.ESI, Register.EAX)
    accumulator, temporary, term = (ir.Reg(register, width) for register in registers)
    semantics = (
        ir.Semantics(ir.Operation.MOVE, "mov", (temporary,), (accumulator,)),
        ir.Semantics(ir.Operation.MOVE, "mov", (accumulator,), (term,)),
        ir.Semantics(ir.Operation.BINARY, name, (accumulator,), (accumulator, temporary)),
    )
    insns = tuple(lir.Insn(index, (index, index + 1), what, (), ()) for index, what in enumerate(semantics))
    body = lir.LirBody("accumulator", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    result = peephole.commuted(body)
    if name in {"sub", "adc"}:
        assert result == body
        return
    assert len(result.insns) == 2
    assert result.insns[0].what == semantics[0]
    assert result.insns[1].what.sources == (accumulator, term)
    for seed, addend in ((0, 0), (1, 2), (0x7FFF, 1), (0xFFFFFFFF, 1)):
        mask = (1 << (width * 8)) - 1

        def execute(operations):
            values = {accumulator: seed & mask, temporary: 42, term: addend & mask}
            for one in operations:
                operands = [values[arg] for arg in one.what.sources]
                match one.what.name:
                    case "mov":
                        value = operands[0]
                    case "add":
                        value = sum(operands)
                    case "and":
                        value = operands[0] & operands[1]
                    case "or":
                        value = operands[0] | operands[1]
                    case "xor":
                        value = operands[0] ^ operands[1]
                values[one.what.dests[0]] = value & mask
            return values

        assert execute(insns) == execute(result.insns)


def test_fpcse_pushes_constant_single_as_one_dword():
    """QB FPCSE pushed 487.5 as 43F3h then C000h instead of one dword."""
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/fpcse-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert "pushd 43F3C000h" in instructions
    assert "push 43F3h" not in instructions


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpcse_passes_literal_addresses_without_register_shuffles(tag):
    """FPCSE materialized both PRINT literal addresses in AX solely to push them."""
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/omf/fpcse-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert "push ax" not in instructions


@pytest.mark.parametrize("high,low", [(0x43F3, 0xC000), (-1, -2), (0, 0), (0x8000, 0x7FFF)])
def test_constant_push_pair_preserves_stack_bytes(high, low):
    from iced_x86 import Decoder

    from qbopt.backend import select

    def push(at, number):
        return lir.Insn(at, (at, at + 3), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(number, 2),)), (), ())

    pair = (push(0, high), push(3, low))
    body = lir.LirBody("arguments", 0, (lir.LirBlock(0, pair, ()),), {}, {})
    result = peephole.pushes(body).insns
    assert len(result) == 1 and result[0].covers == (0, 6)
    operand = result[0].what.sources[0]
    assert operand.value.to_bytes(4, "little") == (low & 0xFFFF).to_bytes(2, "little") + (high & 0xFFFF).to_bytes(
        2, "little"
    )
    assert next(iter(Decoder(16, select.emit(result[0].what).code))).stack_pointer_increment == -4


@pytest.mark.parametrize("barrier", ["relocation", "gap", "block", "instruction"])
def test_constant_push_fusion_stops_at_boundaries(barrier):
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    first = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Imm(1, 2),)), (), ())
    second = replace(first, at=3, covers=(3, 6))
    match barrier:
        case "relocation":
            first = replace(first, what=replace(first.what, sources=(ir.Imm(1, 2, Addr(Space.SEGMENT, 0, 5)),)))
        case "gap":
            second = replace(second, at=4, covers=(4, 7))
    insns = (first, lir.Insn(3, (3, 3), None, (), ()), second) if barrier == "instruction" else (first, second)
    blocks = (
        (lir.LirBlock(0, (first,), (3,)), lir.LirBlock(3, (second,), ()))
        if barrier == "block"
        else (lir.LirBlock(0, insns, ()),)
    )
    body = lir.LirBody("boundary", 0, blocks, {}, {})
    assert peephole.pushes(body) == body


def test_fpcse_drops_unused_allocator_reload():
    """QB FPCSE printed 487.5 correctly but restored AX only to overwrite it."""
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/fpcse-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert "mov ax,[bp-2]" not in instructions
    assert "sub sp,2" not in instructions


@pytest.mark.parametrize("owned", [False, True])
@pytest.mark.parametrize("read", [False, True])
def test_dead_reload_requires_allocator_ownership_and_no_read(owned, read):
    """FPCSE's dead spill is removable, but source loads and live spills are not."""
    from qbopt.backend.frame import Frame

    ax = ir.Reg(Register.AX, 2)
    load = lir.Insn(
        0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (Frame(0).cell(1, 2),)), (), (), spill_reload=owned
    )
    use = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.BX, 2),), (ax,)), (), ())
    write = lir.Insn(2, (2, 2), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ir.Imm(4, 2),)), (), ())
    insns = (load, use, write) if read else (load, write)
    body = lir.LirBody("reload", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    assert (load not in peephole.overwritten(body).insns) == (owned and not read)


def test_overwritten_reload_keeps_virtual_definition() -> None:
    """mdl_draw_tris kept a zero-cost copy of a spilled selector after its
    reload was physically dead; dropping the reload orphaned that value."""
    from qbopt.backend import verify
    from qbopt.backend.frame import Frame

    ax = ir.Reg(Register.AX, 2)
    load = lir.Insn(
        0,
        (0, 0),
        ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (Frame(0).cell(1, 2),)),
        (1,),
        (),
        spill_reload=True,
    )
    copy = lir.anchor(lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ax,)), (2,), (1,)))
    overwrite = lir.Insn(2, (2, 2), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ir.Imm(0, 2),)), (), ())
    body = lir.LirBody("reload", 0, (lir.LirBlock(0, (load, copy, overwrite), ()),), {}, {})

    result = peephole.overwritten(body)
    assert not verify.verify(result, in_ssa=False)
    assert any(one.what.op is ir.Operation.NOTHING and one.defines == (1,) for one in result.insns)


@pytest.mark.parametrize("middle,removed", [(Register.CX, True), (Register.AL, False), (Register.AH, False)])
def test_overwritten_register_copy_respects_byte_reads(middle, removed):
    """FPDEEP retains AX/SI allocation shuffles overwritten before any use."""
    ax, si = ir.Reg(Register.AX, 2), ir.Reg(Register.SI, 2)
    copy = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (si,)), (), ())
    width = 1 if middle in (Register.AL, Register.AH) else 2
    read = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(
            ir.Operation.MOVE,
            "mov",
            (ir.Reg(Register.BL if width == 1 else Register.BX, width),),
            (ir.Reg(middle, width),),
        ),
        (),
        (),
    )
    overwrite = lir.Insn(2, (2, 2), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ir.Imm(4, 2),)), (), ())
    body = lir.LirBody("copies", 0, (lir.LirBlock(0, (copy, read, overwrite), ()),), {}, {})
    result = peephole.overwritten(body)
    assert (copy not in result.insns) is removed


@pytest.mark.parametrize("tag", ["p-g2", "v-g3"])
def test_fpdeep_discards_overwritten_copy_shuffles(tag):
    """FPDEEP emitted six AX/SI and BX/DI shuffles around its four MOVSWs."""
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/omf/fpdeep-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert instructions.count("mov ax,si") + instructions.count("mov bx,di") <= 1


@pytest.mark.parametrize("dest", [Register.AL, Register.AH])
def test_word_copy_survives_partial_overwrite(dest):
    """Writing AL or AH alone cannot make a prior AX definition dead."""
    ax, si = ir.Reg(Register.AX, 2), ir.Reg(Register.SI, 2)
    copy = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (si,)), (), ())
    write = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(dest, 1),), (ir.Imm(0, 1),)), (), ())
    body = lir.LirBody("partial", 0, (lir.LirBlock(0, (copy, write), ()),), {}, {})
    assert peephole.overwritten(body) == body


def test_addrm_index_scale_uses_one_lea():
    """ADDRM QB copied and shifted SI on every iteration instead of one LEA."""
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path("fixtures/omf/addrm-q-O.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    import re

    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert any(re.fullmatch(r"lea \w\w,\[(e\w\w)\+\1\]", one) for one in instructions), instructions


@pytest.mark.parametrize("following", ["add", "adc", "inc", "shl", "call", "je"])
def test_index_lea_preserves_observed_shift_flags(following):
    """ADDRM's copy/shift can become LEA only before a complete flag overwrite."""
    dest, source = ir.Reg(Register.SI, 2), ir.Reg(Register.BX, 2)
    copy = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (source,)), (2,), (1,))
    shift = lir.Insn(0, (0, 2), ir.Semantics(ir.Operation.BINARY, "shl", (dest,), (dest, ir.Imm(1, 1))), (2,), (2,))
    last = lir.Insn(2, (2, 4), ir.Semantics(ir.Operation.BINARY, following, (source,), (source, ir.Imm(1, 2))), (), ())
    body = lir.LirBody("index", 0, (lir.LirBlock(0, (copy, shift, last), ()),), {}, {})
    result = peephole.addresses(body).insns
    assert result[0].what.name == ("lea" if following == "add" else "mov")
    if following == "add":
        assert result[0].covers == (0, 2)
        assert result[0].uses == (1,)
        assert result[0].defines == (2,)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_hotlpx_uses_scaled_address_for_factor_five(tag):
    """HOTLPX's factor twenty expanded to copy/shift/add/shift instead of LEA/shift."""
    from iced_x86 import Mnemonic

    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/omf/hotlpx-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    insns = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert any(one.mnemonic == Mnemonic.LEA and one.memory_index_scale == 4 for one in insns)


@pytest.mark.parametrize("amount", [1, 2, 3])
@pytest.mark.parametrize("following", ["cmp", "adc", "je"])
@pytest.mark.parametrize("width", [2, 4])
def test_scaled_lea_does_not_require_another_shift(amount, following, width):
    """HOTLPX's scale idiom needed three instructions when followed by CMP instead of SHL."""
    dest = ir.Reg(Register.BX if width == 2 else Register.EBX, width)
    source = ir.Reg(Register.CX if width == 2 else Register.ECX, width)

    def insn(at, kind, name, args):
        return lir.Insn(at, (at, at), ir.Semantics(kind, name, (dest,), args), (), ())

    copy = insn(0, ir.Operation.MOVE, "mov", (source,))
    shift = insn(1, ir.Operation.BINARY, "shl", (dest, ir.Imm(amount, 1)))
    add = insn(2, ir.Operation.BINARY, "add", (dest, source))
    last = insn(3, ir.Operation.COMPARE if following == "cmp" else ir.Operation.BINARY, following, (dest, ir.Imm(0, 2)))
    body = lir.LirBody("scale", 0, (lir.LirBlock(0, (copy, shift, add, last), ()),), {}, {})
    result = peephole.addresses(body).insns
    expected = (
        ["lea", "cmp"]
        if following == "cmp"
        else (["lea", "add", following] if amount == 1 else ["mov", "shl", "add", following])
    )
    assert [one.what.name for one in result] == expected
    if following == "cmp":
        from qbopt.backend import select
        from qbopt.frontend.declen import decode

        emitted = decode(select.emit(result[0].what).code, 0).insn
        assert emitted.memory_index_scale == 1 << amount


@pytest.mark.parametrize(
    "guard", ["none", "dword", "carry", "zero_shift", "bytes", "wrong_source", "same", "stack", "relocation"]
)
def test_scaled_address_requires_dead_flags_and_exact_allocated_operands(guard):
    """HOTLPX's LEA must retain low-word arithmetic without losing flags or owned bytes."""
    width = 4 if guard == "dword" else 2
    dest = ir.Reg(Register.EBX if width == 4 else Register.BX, width)
    source = ir.Reg(Register.ECX if width == 4 else Register.CX, width)
    if guard == "same":
        source = dest
    if guard == "stack":
        source = ir.Reg(Register.SP, width)

    def insn(at, kind, name, sources):
        return lir.Insn(at, (at, at), ir.Semantics(kind, name, (dest,), sources), (), ())

    copy = insn(0, ir.Operation.MOVE, "mov", (source,))
    shift = insn(1, ir.Operation.BINARY, "shl", (dest, ir.Imm(2, 1)))
    add = insn(2, ir.Operation.BINARY, "add", (dest, source))
    last = insn(3, ir.Operation.BINARY, "shl", (dest, ir.Imm(2, 1)))
    if guard == "carry":
        last = replace(last, what=replace(last.what, name="adc"))
    if guard == "zero_shift":
        last = replace(last, what=replace(last.what, sources=(dest, ir.Imm(0, 1))))
    if guard == "bytes":
        shift = replace(shift, covers=(1, 2))
    if guard == "wrong_source":
        add = replace(add, what=replace(add.what, sources=(dest, ir.Reg(Register.DX, 2))))
    if guard == "relocation":
        copy = replace(copy, symbol=True)
    result = peephole._scaled_address((copy, shift, add, last))
    if guard not in {"none", "dword"}:
        assert result is None
        return
    assert result.what.dests == (dest,)
    assert result.what.sources[0].scale == 4
    mask = (1 << (8 * width)) - 1
    for bits in (0, 1, 0x1234FFFF, 0x80008000, 0xFFFFFFFF):
        original = (((bits & mask) << 2) + (bits & mask)) & mask
        assert ((bits + bits * 4) & mask) == original


@pytest.mark.parametrize(
    "following,zeroed",
    [("cmp", True), ("add", True), ("adc", False), ("inc", False), ("shl", False), ("call", False), ("je", False)],
)
def test_zeroing_requires_flags_overwritten_before_observation(following, zeroed):
    """HARR-style zeroing is safe before CMP, but not before a carry consumer."""
    dest = ir.Reg(Register.AX, 2)
    first = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (ir.Imm(0, 2),)), (), ())
    last = lir.Insn(
        3,
        (3, 5),
        ir.Semantics(
            ir.Operation.COMPARE if following == "cmp" else ir.Operation.BINARY, following, (), (dest, ir.Imm(1, 2))
        ),
        (),
        (),
    )
    body = lir.LirBody("zero", 0, (lir.LirBlock(0, (first, last), ()),), {}, {})
    assert peephole.Peephole().transform(body).insns[0].what.name == ("xor" if zeroed else "mov")


def test_harr_uses_short_zeroing_before_overwritten_flags():
    """HARR's CX initialization cost three bytes despite ADD replacing its flags."""
    import re

    import corpus
    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path("fixtures/omf/harr-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    fixed = {fixup.offset for fixup in omf.fixups(found.records) if fixup.seg == found.seg}
    insns = [one for block in corpus.partitioned(result.data) for one in block.insns]
    assert any(re.fullmatch(r"xor (\w\w),\1", str(one.insn)) for one in insns)
    # A zero left as `mov` is an address the linker fills in.
    assert all(one.imm_at in fixed for one in insns if re.fullmatch(r"mov \w\w,0", str(one.insn)))


@pytest.mark.parametrize("variant", ["plain", "dword", "byte", "relocation", "boundary", "unknown", "clobber"])
def test_zeroing_preserves_width_relocations_and_unknown_flag_observers(variant):
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    width = 4 if variant == "dword" else 1 if variant == "byte" else 2
    register = {1: Register.AL, 2: Register.AX, 4: Register.EAX}[width]
    dest = ir.Reg(register, width)
    address = Addr(Space.SEGMENT, 0, 5) if variant == "relocation" else None
    first = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (ir.Imm(0, width, address),)), (), ())
    middle = lir.Insn(3, (3, 4), ir.Semantics(ir.Operation.NOTHING, ""), (), ())
    if variant == "unknown":
        middle = replace(middle, what=None)
    if variant == "clobber":
        middle = replace(middle, clobbers=frozenset({Register.EAX}))
    last = lir.Insn(4, (4, 6), ir.Semantics(ir.Operation.COMPARE, "cmp", (), (dest, ir.Imm(1, width))), (), ())
    blocks = (lir.LirBlock(0, (first, middle, last), ()),)
    if variant == "boundary":
        blocks = (lir.LirBlock(0, (first,), (3,)), lir.LirBlock(3, (middle, last), ()))
    result = peephole.zeroes(lir.LirBody("zero", 0, blocks, {}, {})).insns[0]
    # "boundary": the next block's cmp overwrites every flag before anything reads one.
    assert result.what.name == ("xor" if variant in {"plain", "dword", "boundary"} else "mov")
    assert result.what.dests == (dest,)
    assert result.covers == first.covers


@pytest.mark.parametrize("successor,zeroed", [("cmp", True), ("jb", False), (None, False)])
def test_zeroing_before_a_jump_asks_what_the_target_reads(successor, zeroed):
    """`mov bx,0; jmp` stayed three bytes: every block end was taken to have
    its flags read, whatever the block jumped to did first."""
    dest = ir.Reg(Register.BX, 2)
    zero = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (ir.Imm(0, 2),)), (), ())
    jump = lir.Insn(3, (3, 5), ir.Semantics(ir.Operation.JUMP, "jmp", (), (), target=5), (), ())
    match successor:
        case "cmp":
            first = ir.Semantics(ir.Operation.COMPARE, "cmp", (), (dest, ir.Imm(1, 2)))
        case "jb":
            first = ir.Semantics(ir.Operation.BRANCH, "jb", (), (), target=9)
        case None:
            first = None
    blocks = (lir.LirBlock(0, (zero, jump), (5,)), lir.LirBlock(5, (lir.Insn(5, (5, 7), first, (), ()),), ()))
    result = peephole.zeroes(lir.LirBody("zero", 0, blocks, {}, {})).insns[0]
    assert result.what.name == ("xor" if zeroed else "mov")


@pytest.mark.parametrize(
    "variant,rewritten",
    [
        ("plain", True),
        ("moves", True),
        ("call", True),
        ("return", True),
        ("relocated", True),
        ("x87", True),
        ("between", False),
        ("pushf", False),
        ("adjust", False),
        ("memory", False),
    ],
)
def test_zero_compare_before_its_branch_is_or(variant, rewritten):
    """`cmp ax,0; jl` is three bytes where `or ax,ax; jl` is two. Only AF
    differs, so the branch must read the flags next and nothing may read AF.
    No convention passes AF, yet a call or return after counted as reading it:
    791 of qcport's 805 zero tests right before their branch stayed three bytes.
    A relocated move or an x87 compare the decoder cannot encode read as every
    flag too, and held 215 more."""
    from qbopt.objectfile.module import Space

    ax = ir.Reg(Register.AX, 2)
    tested = ir.Mem(ir.Addr(Space.FRAME, -2), 2, Register.BP, 0, 2) if variant == "memory" else ax
    compare = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.COMPARE, "cmp", (), (tested, ir.Imm(0, 2))), (), ())
    move = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.BX, 2),), (ir.Reg(Register.CX, 2),))
    between = lir.Insn(3, (3, 5), None if variant == "between" else move, (), ())
    branch = lir.Insn(6, (6, 8), ir.Semantics(ir.Operation.BRANCH, "jl", (), (), target=9), (), ())
    after = ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ax, ir.Imm(1, 2)))
    from qbopt.objectfile.module import Addr

    last = {
        "adjust": None,
        "call": ir.Semantics(ir.Operation.CALL, "call"),
        "return": ir.Semantics(ir.Operation.RETURN, ""),
        "relocated": ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ir.Imm(0, 2, Addr(Space.SEGMENT, 0, 1)),)),
        "x87": ir.Semantics(
            ir.Operation.COMPARE, "fcomp", (), (ir.Mem(ir.Addr(Space.FRAME, -4), 4, Register.BP, 0, 2),)
        ),
        "pushf": ir.Semantics(ir.Operation.NOTHING, "pushf", (), (ir.Imm(0, 2, Addr(Space.SEGMENT, 0, 1)),)),
    }.get(variant, after)
    head = (compare, between, branch) if variant in ("moves", "between") else (compare, branch)
    blocks = (
        lir.LirBlock(0, head, (8, 9)),
        lir.LirBlock(8, (lir.Insn(8, (8, 9), after, (), ()),), ()),
        lir.LirBlock(9, (lir.Insn(9, (9, 10), last, (), (), symbol=variant in ("relocated", "pushf")),), ()),
    )
    result = peephole.zero_compares(lir.LirBody("zero", 0, blocks, {}, {})).insns[0]
    if rewritten:
        assert (result.what.op, result.what.name, result.what.dests, result.what.sources) == (
            ir.Operation.BINARY,
            "or",
            (ax,),
            (ax, ax),
        )
    else:
        assert result.what == compare.what


@pytest.mark.parametrize(
    "variant,printed",
    [
        ("add", ["add word ptr [bp-4], 1"]),
        ("register", ["add word ptr [bp-4], cx"]),
        ("unary", ["neg word ptr [bp-4]"]),
        ("compare", ["cmp word ptr [bp-4], 5", "jl L0_9"]),
        ("zero", ["cmp word ptr [bp-4], 0", "jl L0_9"]),
        ("signed byte", ["cmp byte ptr [bp-4], 0", "jl L0_9"]),
        ("unsigned byte", ["cmp byte ptr [bp-4], 0", "je L0_9"]),
        ("unsigned byte, sign read", None),
        ("live", None),
        ("addressed", None),
        ("bytes", None),
    ],
)
def test_register_round_trip_through_memory_is_one_instruction(variant, printed):
    """`mov bx,[bp-4]; add bx,1; mov [bp-4],bx` where bcc writes `add word ptr [bp-4],1`,
    and `mov bx,[bp-4]; cmp bx,5` where it writes `cmp word ptr [bp-4],5`: 183 and 265
    sites over qcport. Refused while the register is read after, when the cell is
    addressed through it, and for instructions that stand for bytes of an object.
    `movzx bx,byte ptr [bp-4]; cmp bx,0` too, sieve's flag test once per slot, unless
    the branch reads SF: the extension clears it where the byte compare copies bit 7."""
    from qbopt.backend import masm
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    extension = {"signed byte": "movsx", "unsigned byte": "movzx", "unsigned byte, sign read": "movzx"}.get(variant)
    compares = variant in ("compare", "zero") or extension is not None
    bx, cx = ir.Reg(Register.BX, 2), ir.Reg(Register.CX, 2)
    cell = ir.Mem(Addr(Space.FRAME, -4), 1 if extension else 2, Register.BP, 0, 2)
    if variant == "addressed":
        cell = ir.Mem(None, 2, Register.BX, 2, 1)
    spans = (0, 3) if variant == "bytes" else None

    def insn(at, op, name, dests=(), sources=(), target=None, covers=None):
        return lir.Insn(at, covers, ir.Semantics(op, name, dests, sources, target), (), ())

    load = insn(0, ir.Operation.MOVE, "mov", (bx,), (cell,), covers=spans)
    if extension:
        load = insn(0, ir.Operation.EXTEND, extension, (bx,), (cell,))
    work = {
        "register": insn(1, ir.Operation.BINARY, "add", (bx,), (bx, cx)),
        "unary": insn(1, ir.Operation.UNARY, "neg", (bx,), (bx,)),
        "compare": insn(1, ir.Operation.COMPARE, "cmp", (), (bx, ir.Imm(5, 2))),
    }.get(variant, insn(1, ir.Operation.BINARY, "add", (bx,), (bx, ir.Imm(1, 2))))
    if variant == "zero" or extension:
        work = insn(1, ir.Operation.COMPARE, "cmp", (), (bx, ir.Imm(0, 2)))
    if compares:
        head = (load, work, insn(2, ir.Operation.BRANCH, "je" if variant == "unsigned byte" else "jl", target=9))
    else:
        head = (load, work, insn(2, ir.Operation.MOVE, "mov", (cell,), (bx,)))
    reread = insn(8, ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (bx,))
    rewrite = insn(8, ir.Operation.MOVE, "mov", (bx,), (cx,))
    # Back to the top rather than a return: liveness reads a return as reading every register.
    blocks = (
        lir.LirBlock(0, head, (8, 9) if compares else (8,)),
        lir.LirBlock(8, (reread if variant == "live" else rewrite, insn(9, ir.Operation.JUMP, "jmp", target=0)), (0,)),
        lir.LirBlock(9, (rewrite, insn(10, ir.Operation.JUMP, "jmp", target=0)), (0,)),
    )
    result = peephole.fused(lir.LirBody("fused", 0, blocks, {}, {})).blocks[0]
    if printed is None:
        assert result.insns == head
    else:
        assert [line for one in result.insns for line in masm._instruction(one.what, {}, 0)] == printed


def test_fusion_does_not_cross_a_virtual_dataflow_anchor():
    """qcport's ls_animate lost value#207 when fusion skipped an anchor,
    removed the spill reload defining it, and left the anchor reading it."""
    from qbopt.backend import verify
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    bx, cx = ir.Reg(Register.BX, 2), ir.Reg(Register.CX, 2)
    cell = ir.Mem(Addr(Space.FRAME, -4), 2, Register.BP, 0, 2)
    load = lir.Insn(
        1,
        None,
        ir.Semantics(ir.Operation.MOVE, "mov", (bx,), (cell,)),
        (2,),
        (),
        spill_reload=True,
    )
    anchor = lir.Insn(2, None, ir.Semantics(ir.Operation.NOTHING, "", (), ()), (3,), (2,))
    compare = lir.Insn(
        3,
        None,
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (bx, ir.Imm(0, 2))),
        (),
        (3,),
    )
    branch = lir.Insn(4, None, ir.Semantics(ir.Operation.BRANCH, "je", (), (), 2), (), ())
    overwrite = lir.Insn(5, None, ir.Semantics(ir.Operation.MOVE, "mov", (bx,), (cx,)), (), ())
    body = lir.LirBody(
        "anchored-fusion",
        0,
        (
            lir.LirBlock(0, (load, anchor, compare, branch), (1, 2)),
            lir.LirBlock(1, (overwrite,), ()),
            lir.LirBlock(2, (overwrite,), ()),
        ),
        {},
        {},
    )

    done = peephole.fused(body)

    assert not verify.verify(done)


def test_indirect_call_target_is_physically_live_into_the_call():
    """qcport's mdl_ai lost value#47 after fusion removed the function
    pointer load: call BX was counted only as clobbering BX, not reading it."""
    from qbopt.backend import verify
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    bx, cx = ir.Reg(Register.BX, 2), ir.Reg(Register.CX, 2)
    cell = ir.Mem(Addr(Space.FRAME, -4), 2, Register.BP, 0, 2)
    load = lir.Insn(1, None, ir.Semantics(ir.Operation.MOVE, "mov", (bx,), (cell,)), (47,), ())
    compare = lir.Insn(
        2,
        None,
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (bx, ir.Imm(0, 2))),
        (),
        (47,),
    )
    branch = lir.Insn(3, None, ir.Semantics(ir.Operation.BRANCH, "je", (), (), 2), (), ())
    call = lir.Insn(
        4,
        None,
        ir.Semantics(ir.Operation.CALL, "call", (), (bx,)),
        (),
        (47,),
        clobbers=frozenset({Register.EBX}),
    )
    overwrite = lir.Insn(5, None, ir.Semantics(ir.Operation.MOVE, "mov", (bx,), (cx,)), (), ())
    body = lir.LirBody(
        "indirect-call-liveness",
        0,
        (
            lir.LirBlock(0, (load, compare, branch), (1, 2)),
            lir.LirBlock(1, (call,), ()),
            lir.LirBlock(2, (overwrite,), ()),
        ),
        {},
        {},
    )

    done = peephole.fused(body)

    assert not verify.verify(done)


@pytest.mark.parametrize(
    "variant,printed",
    [
        ("offset first", "les bx, dword ptr [bp-8]"),
        ("segment first", "les bx, dword ptr [bp-8]"),
        ("fs", "lfs bx, dword ptr [bp-8]"),
        ("through the offset", None),
        ("through the offset, segment first", "les bx, dword ptr [bx]"),
        ("override", "les bx, dword ptr es:[si+16]"),
        ("override, segment first", None),
        ("another cell", None),
    ],
)
def test_far_pointer_loaded_in_one_instruction(variant, printed):
    """`mov bx,[bp-8]; mov es,[bp-6]` where bcc writes `les bx,[bp-8]`: 862 pairs
    over qcport, and 18 more left as `mov bx,es:[si+16]; mov es,es:[si+18]`
    because ES also reached the cell. Refused only when the register written
    first addresses the second read, and for words of two different cells."""
    from iced_x86 import Decoder

    from qbopt.backend import masm
    from qbopt.backend import select
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    bx = ir.Reg(Register.BX, 2)
    segment = ir.Reg(Register.FS if variant == "fs" else Register.ES, 2)
    low = ir.Mem(Addr(Space.FRAME, -8), 2, Register.BP, 0, 2)
    high = ir.Mem(Addr(Space.FRAME, -4 if variant == "another cell" else -6), 2, Register.BP, 0, 2)
    base = Register.BP
    if variant.startswith("through the offset"):
        low, high = ir.Mem(None, 2, Register.BX, 0, 1), ir.Mem(None, 2, Register.BX, 2, 1)
        base = Register.BX
    if variant.startswith("override"):
        # As lowered and allocated: the offset value placed in si.
        low, high = (
            ir.Mem(Addr(Space.FAR, disp, segment=Register.ES), 2, Register.SI, 0, 2, base=ir.Held(1, 2))
            for disp in (16, 18)
        )
        base = Register.SI

    def move(at, dest, cell):
        return lir.Insn(at, None, ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (cell,)), (), ())

    pair = (move(0, bx, low), move(1, segment, high))
    if variant.endswith("segment first"):
        pair = pair[::-1]
    block = lir.LirBlock(0, pair, ())
    result = peephole.far_loads(lir.LirBody("far", 0, (block,), {}, {})).blocks[0].insns
    if printed is None:
        assert result == pair
        return
    assert [line for one in result for line in masm._instruction(one.what, {}, 0)] == [printed]
    from iced_x86 import Mnemonic

    decoded = next(iter(Decoder(16, select.emit(result[0].what).code)))
    expected = Mnemonic.LFS if variant == "fs" else Mnemonic.LES
    assert (decoded.mnemonic, decoded.op0_register, decoded.memory_base) == (expected, Register.BX, base)


@pytest.mark.parametrize("middle", ["", "mov", "fnstsw", "fninit", None, "block"])
def test_wait_elimination_does_not_cross_observable_work(middle):
    """FPCSEX's redundant waits may disappear, but integer observers still need completion."""

    def instruction(at, name):
        what = None if name is None else ir.Semantics(ir.Operation.NOTHING, name, (), ())
        return lir.Insn(at, (at, at + 1), what, (), ())

    first, between, last = instruction(0, "wait"), instruction(1, middle), instruction(2, "fld")
    blocks = (lir.LirBlock(0, (first, between, last), ()),)
    if middle == "block":
        blocks = (lir.LirBlock(0, (first,), (2,)), lir.LirBlock(2, (last,), ()))
    result = peephole.waits(lir.LirBody("waits", 0, blocks, {}, {}))
    assert any(one.what is not None and one.what.name == "wait" for one in result.insns) == (middle != "")


@pytest.mark.parametrize("tag,waits", [("p-g2", 1), ("q-O", 2), ("v-g3", 1)])
def test_fpcsex_keeps_only_waits_before_integer_work(tag, waits):
    """Runtime-input FPCSEX issued three waits per iteration; two preceded waiting FP instructions."""
    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/omf/fpcsex-{tag}.obj").read_bytes(), basic_semantics=True)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(str(one) == "wait" for one in instructions) == waits


@pytest.mark.parametrize("change", [None, Register.CH, Register.AH])
def test_repeated_copy_requires_unchanged_source_and_destination(change):
    """LNGMXX copied ECX into EAX twice around CDQ; partial writes must prevent reuse."""
    move = lir.Insn(
        0,
        (0, 1),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.EAX, 4),), (ir.Reg(Register.ECX, 4),)),
        (),
        (),
    )
    extend = lir.Insn(
        1,
        (1, 2),
        ir.Semantics(ir.Operation.EXTEND, "cdq", (ir.Reg(Register.EDX, 4),), (ir.Reg(Register.EAX, 4),)),
        (),
        (),
    )
    if change is not None:
        extend = replace(extend, clobbers=frozenset({change}))
    final = replace(move, at=2, covers=(2, 3))
    body = lir.LirBody("copies", 0, (lir.LirBlock(0, (move, extend, final), ()),), {}, {})
    result = peephole.constants(body)
    assert sum(one.what == move.what for one in result.insns) == (1 if change is None else 2)


def test_copied_value_survives_overwriting_its_original_register():
    """A copied value is a snapshot, not an alias of the register it came from."""

    def copy(at, dest, source):
        return lir.Insn(at, (at, at + 1), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(dest, 4),), (source,)), (), ())

    insns = (
        copy(0, Register.EAX, ir.Reg(Register.ECX, 4)),
        copy(1, Register.EDX, ir.Reg(Register.ECX, 4)),
        copy(2, Register.ECX, ir.Imm(7, 4)),
        copy(3, Register.EAX, ir.Reg(Register.EDX, 4)),
        copy(4, Register.EAX, ir.Reg(Register.ECX, 4)),
    )
    body = lir.LirBody("snapshot", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    result = peephole.constants(body)
    emitted = [one.what for one in result.insns if one.what.op is not ir.Operation.NOTHING]
    assert emitted == [one.what for one in insns if one.at != 3]


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_lngmxx_does_not_reload_dividend_after_sign_extension(tag):
    """LNGMXX's CDQ leaves its dividend intact, but lowering reloaded it before IDIV."""
    from iced_x86 import Mnemonic

    import corpus
    from qbopt import wholeseg

    result = wholeseg.emitted(Path(f"fixtures/omf/lngmxx-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    insns = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    divides = [index for index, one in enumerate(insns) if one.mnemonic == Mnemonic.IDIV]
    assert len(divides) == 1
    assert insns[divides[0] - 1].mnemonic == Mnemonic.CDQ


def test_nbody_repeated_fixed_constant_is_removed():
    """Nbody materialized 512 twice before one divide, with a non-clobbering CDQ between them."""
    from iced_x86 import Code

    from qbopt import wholeseg
    from qbopt.objectfile import omf
    from qbopt.frontend import blocks
    from qbopt.objectfile import module

    result = wholeseg.emitted(Path("fixtures/regressions/nbody-stack-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert (
        sum(one.insn.code == Code.MOV_R32_IMM32 and one.insn.immediate32 == 512 for one in blocks.instructions(found))
        == 1
    )


def test_partial_write_invalidates_constant():
    def move(at, dest, source):
        return lir.Insn(
            at=at,
            covers=(at, at + 1),
            defines=(),
            uses=(),
            what=ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (source,)),
        )

    first = move(0, ir.Reg(Register.EAX, 4), ir.Imm(512, 4))
    change = move(1, ir.Reg(Register.AH, 1), ir.Imm(0, 1))
    again = move(2, ir.Reg(Register.EAX, 4), ir.Imm(512, 4))
    body = lir.LirBody("partial", 0, (lir.LirBlock(0, (first, change, again), ()),), {}, {})
    assert len(peephole.constants(body).blocks[0].insns) == 3


@pytest.mark.parametrize("clobbers", [frozenset(), frozenset({Register.AX})])
def test_empty_ownership_marker_preserves_register_knowledge(clobbers):
    """Expanded FPDEEP emitted MOV AX,0 twice, separated only by a removed instruction's marker."""
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Imm(0, 2),))
    first = lir.Insn(0, (0, 3), what, (), ())
    marker = lir.Insn(3, (3, 5), ir.Semantics(ir.Operation.NOTHING, "", (), ()), (), (), clobbers=clobbers)
    last = replace(first, at=5, covers=(5, 8))
    body = lir.LirBody("marker", 0, (lir.LirBlock(0, (first, marker, last), ()),), {}, {})
    result = peephole.constants(body)
    assert sum(one.what == what for one in result.insns) == (2 if clobbers else 1)


def test_virtual_identity_marker_does_not_reload_nbody_dividend_constant():
    """C nbody emitted MOV EAX,512 twice around CDQ before one IDIV.

    Allocation retains an elided identity's virtual definition as an unnamed
    NOTHING marker.  That metadata changes no physical register and therefore
    must not erase the constant known to remain in EAX across CDQ.
    """
    from qbopt.backend import verify

    move = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.EAX, 4),), (ir.Imm(512, 4),))
    first = lir.Insn(0, (0, 0), move, (1,), ())
    extend = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.EXTEND, "cdq", (ir.Reg(Register.EDX, 4),), (ir.Reg(Register.EAX, 4),)),
        (2,),
        (1,),
    )
    marker = lir.Insn(2, (2, 2), ir.Semantics(ir.Operation.NOTHING, "", (), ()), (3,), (1,))
    again = lir.Insn(3, (3, 3), move, (4,), ())
    body = lir.LirBody("nbody-dividend", 0, (lir.LirBlock(0, (first, extend, marker, again), ()),), {}, {})

    result = peephole.constants(body)

    assert sum(one.what == move for one in result.insns) == 1
    assert not verify.verify(result)


def test_crc32_reads_a_byte_directly_into_its_dword_value() -> None:
    """C CRC32 emitted `movzx dx,[buf+bx]; movzx edx,dx` for every byte.

    The two unsigned extensions are one target instruction.  Keep the final
    SSA definition on that instruction so removing the second encoding does
    not leave a later virtual use without a definition.
    """
    from qbopt.backend import verify
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(Addr(Space.SEGMENT, 1, 0), 1, through=Register.BX)
    narrow = lir.Insn(
        0,
        (0, 3),
        ir.Semantics(ir.Operation.EXTEND, "movzx", (ir.Reg(Register.DX, 2),), (cell,)),
        (1,),
        (),
        symbol=True,
    )
    wide = lir.Insn(
        3,
        (3, 6),
        ir.Semantics(
            ir.Operation.EXTEND,
            "movzx",
            (ir.Reg(Register.EDX, 4),),
            (ir.Reg(Register.DX, 2),),
        ),
        (2,),
        (1,),
    )
    use = lir.Insn(
        6,
        (6, 6),
        ir.Semantics(
            ir.Operation.BINARY,
            "xor",
            (ir.Reg(Register.EAX, 4),),
            (ir.Reg(Register.EAX, 4), ir.Reg(Register.EDX, 4)),
        ),
        (3,),
        (2,),
    )
    body = lir.LirBody("crc32", 0, (lir.LirBlock(0, (narrow, wide, use)),), {}, {})

    result = peephole.extensions(body)
    emitted = [one for one in result.insns if one.what.op is not ir.Operation.NOTHING]

    assert emitted[0].what == ir.Semantics(
        ir.Operation.EXTEND,
        "movzx",
        (ir.Reg(Register.EDX, 4),),
        (cell,),
    )
    assert emitted[0].defines == (2,)
    assert emitted[0].symbol is True
    assert len(emitted) == 2
    assert not verify.verify(result)


@pytest.mark.parametrize("guard", ["signedness", "register", "shared", "clobber", "symbol"])
def test_transitive_extension_preserves_nonlocal_machine_state(guard: str) -> None:
    cell = ir.Mem(None, 1, through=Register.BX)
    narrow = lir.Insn(
        0,
        (0, 3),
        ir.Semantics(ir.Operation.EXTEND, "movzx", (ir.Reg(Register.DX, 2),), (cell,)),
        (1,),
        (),
    )
    wide = lir.Insn(
        3,
        (3, 6),
        ir.Semantics(
            ir.Operation.EXTEND,
            "movzx",
            (ir.Reg(Register.EDX, 4),),
            (ir.Reg(Register.DX, 2),),
        ),
        (2,),
        (1,),
    )
    tail = ()
    match guard:
        case "signedness":
            wide = replace(wide, what=replace(wide.what, name="movsx"))
        case "register":
            wide = replace(wide, what=replace(wide.what, dests=(ir.Reg(Register.EAX, 4),)))
        case "shared":
            tail = (
                lir.Insn(
                    6,
                    (6, 6),
                    ir.Semantics(ir.Operation.PUSH, "push", (), (ir.Reg(Register.DX, 2),)),
                    (),
                    (1,),
                ),
            )
        case "clobber":
            narrow = replace(narrow, clobbers=frozenset({Register.AX}))
        case "symbol":
            wide = replace(wide, symbol=True)
    body = lir.LirBody("guarded", 0, (lir.LirBlock(0, (narrow, wide, *tail)),), {}, {})

    assert peephole.extensions(body) == body


@pytest.mark.parametrize(
    "interruption",
    ["none", "extend", "extend_write", "extend_clobber", "call", "clobber", "unknown", "relocation", "block"],
)
def test_constant_knowledge_is_local_and_invalidated(interruption):
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    source = ir.Imm(512, 4)
    if interruption == "relocation":
        source = ir.Imm(512, 4, Addr(Space.SEGMENT, 0, 5))
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.EAX, 4),), (source,))
    first = lir.Insn(0, (0, 1), what, (), ())
    last = replace(first, at=2, covers=(2, 3))
    middle = lir.Insn(
        1, (1, 2), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.BX, 2),), (ir.Imm(7, 2),)), (), ()
    )
    if interruption == "call":
        middle = replace(middle, what=ir.Semantics(ir.Operation.CALL, "call", (), ()))
    if interruption in ("extend", "extend_clobber"):
        middle = replace(
            middle,
            what=ir.Semantics(ir.Operation.EXTEND, "cdq", (ir.Reg(Register.EDX, 4),), (ir.Reg(Register.EAX, 4),)),
        )
        if interruption == "extend_clobber":
            middle = replace(middle, clobbers=frozenset({Register.AH}))
    if interruption == "extend_write":
        middle = replace(
            middle,
            what=ir.Semantics(ir.Operation.EXTEND, "movsx", (ir.Reg(Register.EAX, 4),), (ir.Reg(Register.AX, 2),)),
        )
    if interruption == "clobber":
        middle = replace(middle, clobbers=frozenset({Register.EAX}))
    if interruption == "unknown":
        middle = replace(middle, what=None)
    blocks = (lir.LirBlock(0, (first, middle, last), ()),)
    if interruption == "block":
        blocks = (lir.LirBlock(0, (first, middle), (2,)), lir.LirBlock(2, (last,), ()))
    result = peephole.constants(lir.LirBody("constants", 0, blocks, {}, {}))
    assert sum(one.what == what for block in result.blocks for one in block.insns) == (
        1 if interruption in ("none", "extend") else 2
    )


def test_a_string_fill_reading_the_direction_flag_leaves_zero_as_xor():
    """`rep stosb` reads DF, and a zero was written `xor` only where no flag at
    all was live after it: every body with a fill kept `mov ax,0` though xor
    leaves DF as it is."""
    ax, bx = ir.Reg(Register.AX, 2), ir.Reg(Register.BX, 2)

    def insn(at, op, name, dests=(), sources=(), target=None):
        return lir.Insn(at, (at, at), ir.Semantics(op, name, dests, sources, target), (), ())

    blocks = (
        lir.LirBlock(
            0,
            (insn(0, ir.Operation.MOVE, "mov", (ax,), (ir.Imm(0, 2),)), insn(1, ir.Operation.JUMP, "jmp", target=2)),
            (2,),
        ),
        lir.LirBlock(
            2,
            (
                insn(2, ir.Operation.FILL, "stosb", (ir.Mem(None, 0),)),
                insn(3, ir.Operation.COMPARE, "cmp", (), (ax, bx)),
                insn(4, ir.Operation.JUMP, "jmp", target=0),
            ),
            (0,),
        ),
    )
    result = peephole.zeroes(lir.LirBody("zero", 0, blocks, {}, {})).blocks[0].insns[0]
    assert (result.what.op, result.what.name) == (ir.Operation.BINARY, "xor")

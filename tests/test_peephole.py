from pathlib import Path
from dataclasses import replace

import pytest

from iced_x86 import Register

from qbopt.model import ir, lir
from qbopt.backend import peephole


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
    load = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (Frame(0).cell(1, 2),)),
                    (), (), spill_reload=owned)
    use = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.BX, 2),), (ax,)), (), ())
    write = lir.Insn(2, (2, 2), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (ir.Imm(4, 2),)), (), ())
    insns = (load, use, write) if read else (load, write)
    body = lir.LirBody("reload", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    assert (load not in peephole.overwritten(body).insns) == (owned and not read)


@pytest.mark.parametrize("middle,removed", [(Register.CX, True), (Register.AL, False), (Register.AH, False)])
def test_overwritten_register_copy_respects_byte_reads(middle, removed):
    """FPDEEP retains AX/SI allocation shuffles overwritten before any use."""
    ax, si = ir.Reg(Register.AX, 2), ir.Reg(Register.SI, 2)
    copy = lir.Insn(0, (0, 0), ir.Semantics(ir.Operation.MOVE, "mov", (ax,), (si,)), (), ())
    width = 1 if middle in (Register.AL, Register.AH) else 2
    read = lir.Insn(1, (1, 1), ir.Semantics(ir.Operation.MOVE, "mov",
        (ir.Reg(Register.BL if width == 1 else Register.BX, width),), (ir.Reg(middle, width),)), (), ())
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
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert "lea si,[ebx+ebx]" in instructions


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
    import corpus
    from iced_x86 import Mnemonic
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
    last = insn(3, ir.Operation.COMPARE if following == "cmp" else ir.Operation.BINARY,
                following, (dest, ir.Imm(0, 2)))
    body = lir.LirBody("scale", 0, (lir.LirBlock(0, (copy, shift, add, last), ()),), {}, {})
    result = peephole.addresses(body).insns
    expected = ["lea", "cmp"] if following == "cmp" else (
        ["lea", "add", following] if amount == 1 else ["mov", "shl", "add", following])
    assert [one.what.name for one in result] == expected
    if following == "cmp":
        from qbopt.backend import select
        from qbopt.frontend.declen import decode
        emitted = decode(select.emit(result[0].what).code, 0).insn
        assert emitted.memory_index_scale == 1 << amount


@pytest.mark.parametrize("guard", ["none", "dword", "carry", "zero_shift", "bytes", "wrong_source", "same", "stack", "relocation"])
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


@pytest.mark.parametrize("following,zeroed", [("cmp", True), ("add", True),
    ("adc", False), ("inc", False), ("shl", False), ("call", False), ("je", False)])
def test_zeroing_requires_flags_overwritten_before_observation(following, zeroed):
    """HARR-style zeroing is safe before CMP, but not before a carry consumer."""
    dest = ir.Reg(Register.AX, 2)
    first = lir.Insn(0, (0, 3), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (ir.Imm(0, 2),)), (), ())
    last = lir.Insn(3, (3, 5), ir.Semantics(ir.Operation.COMPARE if following == "cmp" else ir.Operation.BINARY,
                    following, (), (dest, ir.Imm(1, 2))), (), ())
    body = lir.LirBody("zero", 0, (lir.LirBlock(0, (first, last), ()),), {}, {})
    assert peephole.Peephole().transform(body).insns[0].what.name == ("xor" if zeroed else "mov")


def test_harr_uses_short_zeroing_before_overwritten_flags():
    """HARR's CX initialization cost three bytes despite ADD replacing its flags."""
    import corpus
    from qbopt import wholeseg
    result = wholeseg.emitted(Path("fixtures/omf/harr-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [str(one.insn) for block in corpus.partitioned(result.data) for one in block.insns]
    assert "xor cx,cx" in instructions


@pytest.mark.parametrize("variant", ["plain", "dword", "byte", "relocation", "boundary", "unknown", "clobber"])
def test_zeroing_preserves_width_relocations_and_unknown_flag_observers(variant):
    from qbopt.objectfile.module import Addr, Space
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
    assert result.what.name == ("xor" if variant in {"plain", "dword"} else "mov")
    assert result.what.dests == (dest,)
    assert result.covers == first.covers


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
    result = wholeseg.emitted(Path(f"fixtures/omf/fpcsex-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(str(one) == "wait" for one in instructions) == waits


@pytest.mark.parametrize("change", [None, Register.CH, Register.AH])
def test_repeated_copy_requires_unchanged_source_and_destination(change):
    """LNGMXX copied ECX into EAX twice around CDQ; partial writes must prevent reuse."""
    move = lir.Insn(0, (0, 1), ir.Semantics(ir.Operation.MOVE, "mov",
                    (ir.Reg(Register.EAX, 4),), (ir.Reg(Register.ECX, 4),)), (), ())
    extend = lir.Insn(1, (1, 2), ir.Semantics(ir.Operation.EXTEND, "cdq",
                      (ir.Reg(Register.EDX, 4),), (ir.Reg(Register.EAX, 4),)), (), ())
    if change is not None:
        extend = replace(extend, clobbers=frozenset({change}))
    final = replace(move, at=2, covers=(2, 3))
    body = lir.LirBody("copies", 0, (lir.LirBlock(0, (move, extend, final), ()),), {}, {})
    result = peephole.constants(body)
    assert len(result.insns) == (2 if change is None else 3)


def test_copied_value_survives_overwriting_its_original_register():
    """A copied value is a snapshot, not an alias of the register it came from."""
    def copy(at, dest, source):
        return lir.Insn(at, (at, at + 1), ir.Semantics(ir.Operation.MOVE, "mov",
                        (ir.Reg(dest, 4),), (source,)), (), ())
    insns = (
        copy(0, Register.EAX, ir.Reg(Register.ECX, 4)),
        copy(1, Register.EDX, ir.Reg(Register.ECX, 4)),
        copy(2, Register.ECX, ir.Imm(7, 4)),
        copy(3, Register.EAX, ir.Reg(Register.EDX, 4)),
        copy(4, Register.EAX, ir.Reg(Register.ECX, 4)),
    )
    body = lir.LirBody("snapshot", 0, (lir.LirBlock(0, insns, ()),), {}, {})
    result = peephole.constants(body)
    assert [one.what for one in result.insns] == [one.what for one in insns if one.at != 3]


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_lngmxx_does_not_reload_dividend_after_sign_extension(tag):
    """LNGMXX's CDQ leaves its dividend intact, but lowering reloaded it before IDIV."""
    import corpus
    from iced_x86 import Mnemonic
    from qbopt import wholeseg
    result = wholeseg.emitted(Path(f"fixtures/omf/lngmxx-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    insns = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    divides = [index for index, one in enumerate(insns) if one.mnemonic == Mnemonic.IDIV]
    assert len(divides) == 1
    assert insns[divides[0] - 1].mnemonic == Mnemonic.CDQ


def test_nbody_repeated_fixed_constant_is_removed():
    """Nbody materialized 512 twice before one divide, with a non-clobbering CDQ between them."""
    from qbopt import wholeseg
    from qbopt.objectfile import module, omf
    from qbopt.frontend import blocks
    from iced_x86 import Code
    result = wholeseg.emitted(Path("fixtures/regressions/nbody-stack-p-g2.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    found = module.of(omf.parse(result.data))
    assert sum(one.insn.code == Code.MOV_R32_IMM32 and one.insn.immediate32 == 512
               for one in blocks.instructions(found)) == 1


def test_partial_write_invalidates_constant():
    def move(at, dest, source):
        return lir.Insn(at=at, covers=(at, at+1), defines=(), uses=(),
                        what=ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (source,)))
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


@pytest.mark.parametrize("interruption", ["none", "extend", "extend_write", "extend_clobber", "call", "clobber", "unknown", "relocation", "block"])
def test_constant_knowledge_is_local_and_invalidated(interruption):
    from qbopt.objectfile.module import Addr, Space
    source = ir.Imm(512, 4)
    if interruption == "relocation":
        source = ir.Imm(512, 4, Addr(Space.SEGMENT, 0, 5))
    what = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.EAX, 4),), (source,))
    first = lir.Insn(0, (0, 1), what, (), ())
    last = replace(first, at=2, covers=(2, 3))
    middle = lir.Insn(1, (1, 2), ir.Semantics(ir.Operation.MOVE, "mov",
                     (ir.Reg(Register.BX, 2),), (ir.Imm(7, 2),)), (), ())
    if interruption == "call":
        middle = replace(middle, what=ir.Semantics(ir.Operation.CALL, "call", (), ()))
    if interruption in ("extend", "extend_clobber"):
        middle = replace(middle, what=ir.Semantics(ir.Operation.EXTEND, "cdq",
                         (ir.Reg(Register.EDX, 4),), (ir.Reg(Register.EAX, 4),)))
        if interruption == "extend_clobber":
            middle = replace(middle, clobbers=frozenset({Register.AH}))
    if interruption == "extend_write":
        middle = replace(middle, what=ir.Semantics(ir.Operation.EXTEND, "movsx",
                         (ir.Reg(Register.EAX, 4),), (ir.Reg(Register.AX, 2),)))
    if interruption == "clobber":
        middle = replace(middle, clobbers=frozenset({Register.EAX}))
    if interruption == "unknown":
        middle = replace(middle, what=None)
    blocks = (lir.LirBlock(0, (first, middle, last), ()),)
    if interruption == "block":
        blocks = (lir.LirBlock(0, (first, middle), (2,)), lir.LirBlock(2, (last,), ()))
    result = peephole.constants(lir.LirBody("constants", 0, blocks, {}, {}))
    assert sum(len(block.insns) for block in result.blocks) == (2 if interruption in ("none", "extend") else 3)

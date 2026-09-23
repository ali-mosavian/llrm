from types import SimpleNamespace
from iced_x86 import Register

from qbopt.backend import asm
from qbopt.model import ir, lir, mir
from qbopt.objectfile.module import SourceMap


def _selected(op: mir.Op, source=None) -> lir.Insn:
    from qbopt.backend import lower

    node = source.nodes.get(op.id) if source is not None else None
    ranges = source.occurrences.get(op.id, ()) if source is not None else ()
    return lir.Insn(
        op.at,
        ranges[0] if ranges else (op.at, op.at),
        lower.current(op, node=node),
        tuple(one.id for one in op.defines),
        tuple(one.id for one in op.uses),
        op=op,
        node=node,
        symbol=op.symbol,
    )


def test_load_hoisted_to_call_does_not_acquire_call_fixup():
    """Qrender mov at 0941 inherited B$PER4's target and refused with no relocation field."""
    from dataclasses import replace
    from pathlib import Path
    from qbopt.objectfile import module, omf
    from qbopt.frontend import blocks
    found = module.of(omf.parse(Path("fixtures/regressions/qrender-view-v-g3.obj").read_bytes()))
    bodies = mir.bodies(found, blocks.partition(found, blocks.code_map(found)))
    ops = [op for _, body in bodies for block in body.blocks for op in block.ops]
    call = next(op for op in ops if op.at == 0x941)
    load = next(op for op in ops if op.at == 0x946)
    fields = frozenset({0x942})
    assert asm._field_in(found, _selected(call, bodies.source), fields, bodies.source) == 0x942
    assert asm._field_in(found, _selected(load, bodies.source), fields, bodies.source) is None
    assert asm._field_in(
        found,
        replace(_selected(load, bodies.source), at=call.at),
        fields,
        bodies.source,
    ) is None


def test_promoted_symbolic_load_drops_its_old_fixup():
    """VBDOS nbody refused 0x1b7: promotion left a fixup on a register-to-register move."""
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    op = mir.Op(0x1b7, ir.Operation.MOVE, "mov", (result,), (source,), kind=mir.Kind.COPY,
                args=(mir.Held(source, 4),), results=(mir.Held(result, 4),), id=1, symbol=True)
    found = SimpleNamespace(refs={1: (0x1b9,)})
    source_map = SourceMap(refs=found.refs)
    assert asm._fields_in(found, _selected(op, source_map), source=source_map) == ()


def test_inserted_instruction_never_reads_original_interrupt_bytes():
    """VBDOS nbody crashed emission when a synthetic instruction's address exceeded BC's bytes."""
    op = lir.Insn(
        100,
        (100, 100),
        ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Imm(1, 2),)),
        (),
        (),
    )
    found = SimpleNamespace(code=b"\x90", absorbed={}, fixup_at={}, calls={}, refs={}, float_protocols={})
    done = asm.assemble([op], 0, found, source=SourceMap())
    assert not isinstance(done, str), done
    assert done.code == bytes.fromhex("b80100")


def test_a_generated_read_modify_write_binds_its_one_symbolic_field():
    """VBDOS nbody refused 0x02b7: "cannot bind a generated symbolic memory operand".

    `add [x],eax` names its cell as destination and source; counted twice,
    one field could not take both.
    """
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    cell = ir.Mem(Addr(Space.SEGMENT, 0xA, 5), 4, disp_width=2)
    what = ir.Semantics(ir.Operation.BINARY, "add", (cell,), (cell, ir.Reg(Register.EAX, 4)))
    op = lir.Insn(0x10, (0x10, 0x10), what, (), ())
    found = SimpleNamespace(code=b"\x90", seg=0, absorbed={}, fixup_at={}, calls={}, refs={}, float_protocols={})
    done = asm.assemble([op], 0, found, source=SourceMap())
    assert not isinstance(done, str), done
    assert [addr for _where, addr in done.symbols] == [cell.addr]


def test_a_register_sum_drops_the_fixup_of_the_read_it_replaced():
    """matrix refused 0x0048: "add has 1 fixups and 0 fields to put them in".

    `add ax,[d]` was served from a register and then became `lea` of two
    registers. An Address with no `addr` is arithmetic and has no field.
    """
    source, other, result = mir.Value(1, 0x48), mir.Value(2, 0x48), mir.Value(3, 0x48)
    op = mir.Op(0x48, ir.Operation.BINARY, "add", (result,), (source, other), kind=mir.Kind.ADD, id=1, symbol=True)
    lea = ir.Semantics(
        ir.Operation.ADDRESS,
        "lea",
        (ir.Reg(Register.AX, 2),),
        (ir.Address(None, through=Register.EBX, index=Register.ECX),),
    )
    insn = lir.Insn(0x48, (0x48, 0x4C), lea, (3,), (1, 2), op=op, symbol=True)
    refs = {1: (0x4A,)}
    found = SimpleNamespace(
        code=bytes(0x4C), seg=0, absorbed={}, fixup_at={}, calls={}, refs=refs, float_protocols={}
    )
    done = asm.assemble([insn], 0x48, found, fields=frozenset({0x4A}), source=SourceMap(refs=refs))
    assert not isinstance(done, str), done
    assert done.relocations == ()

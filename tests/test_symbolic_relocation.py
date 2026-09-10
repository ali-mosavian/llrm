from types import SimpleNamespace
from iced_x86 import Register

from qbopt.backend import asm
from qbopt.model import ir, mir


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
    assert asm._field_in(found, call, fields) == 0x942
    assert asm._field_in(found, load, fields) is None
    assert asm._field_in(found, replace(load, at=call.at), fields) is None


def test_promoted_symbolic_load_drops_its_old_fixup():
    """VBDOS nbody refused 0x1b7: promotion left a fixup on a register-to-register move."""
    source, result = mir.Value(1, 0), mir.Value(2, 0)
    op = mir.Op(0x1b7, ir.Operation.MOVE, "mov", (result,), (source,), kind=mir.Kind.COPY,
                args=(mir.Held(source, 4),), results=(mir.Held(result, 4),), id=1, symbol=True)
    found = SimpleNamespace(refs={1: (0x1b9,)})
    assert asm._fields_in(found, op) == ()


def test_inserted_instruction_never_reads_original_interrupt_bytes():
    """VBDOS nbody crashed emission when a synthetic instruction's address exceeded BC's bytes."""
    op = mir.Op(100, ir.Operation.MOVE, "mov", (), (), kind=mir.Kind.COPY, covers=(100, 100),
                made=ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Imm(1, 2),)))
    found = SimpleNamespace(code=b"\x90", absorbed={}, fixup_at={}, calls={}, refs={}, float_protocols={})
    done = asm.assemble([op], 0, found)
    assert not isinstance(done, str), done
    assert done.code == bytes.fromhex("b80100")

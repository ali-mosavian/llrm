from pathlib import Path
from dataclasses import replace

from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import Register

import corpus
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import asm
from qbopt.backend import lower
from qbopt.backend import layout
from qbopt.backend import peephole
from qbopt.objectfile import objwrite


def test_zeroing_stays_before_the_comparison_in_emitted_bytes() -> None:
    """Qrender h_frame reported build time zero: layout moved XOR across CMP."""
    found = corpus.loaded(Path("fixtures/omf/harr-v-g3.obj"))
    assert found is not None
    body = lower.lowered("flags", mir.MirBody(0x30, (mir.MirBlock(0x30, (), (), ()),)), {}, {}, {})
    dest = ir.Reg(Register.AX, 2)
    zero = lir.Insn(0x33, (0x33, 0x33), ir.Semantics(ir.Operation.MOVE, "mov", (dest,), (ir.Imm(0, 2),)), (), ())
    compare = lir.Insn(
        0x30,
        (0x30, 0x30),
        ir.Semantics(ir.Operation.COMPARE, "cmp", (), (ir.Reg(Register.BX, 2), ir.Imm(0, 2))),
        (),
        (),
    )
    body = replace(body, blocks=(lir.LirBlock(0x30, (zero, compare)),))
    body = peephole.zeroes(body)
    laid = layout.rebuild(
        found,
        [(body.name, objwrite._as_mir(body))],
        ordered_entries=frozenset({body.entry}) if body.ordered else frozenset(),
    )
    assert isinstance(laid, asm.Laid), laid
    instructions = list(Decoder(16, laid.code))
    assert [one.mnemonic for one in instructions] == [Mnemonic.XOR, Mnemonic.TEST]

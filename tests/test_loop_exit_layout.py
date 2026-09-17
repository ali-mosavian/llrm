from pathlib import Path

from iced_x86 import Decoder
from iced_x86 import Mnemonic
from iced_x86 import Register

import corpus
from qbopt.model import ir
from qbopt.model import lir
from qbopt.backend import asm
from qbopt.backend import layout


def test_split_exit_executes_its_reload_before_the_increment() -> None:
    # SCREEN's BG_BAND skipped a split exit and incremented x as the next row.
    found = corpus.loaded(Path("fixtures/omf/harr-v-g3.obj"))
    assert found is not None
    bridge = 0x100000001

    def op(at: int, meaning: ir.Semantics) -> lir.Insn:
        return lir.Insn(at, (at, at), meaning, (), (), symbol=False)

    body = lir.LirBody(
        "split exit",
        0x30,
        (
            lir.LirBlock(0x30, (op(0x30, ir.Semantics(ir.Operation.BRANCH, "jle", target=0x30)),), (0x30, bridge)),
            lir.LirBlock(
                0x40,
                (
                    op(
                        0x40,
                        ir.Semantics(
                            ir.Operation.UNARY,
                            "inc",
                            dests=(ir.Reg(Register.AX, 2),),
                            sources=(ir.Reg(Register.AX, 2),),
                        ),
                    ),
                    op(0x40, ir.Semantics(ir.Operation.RETURN, "ret")),
                ),
                (),
            ),
            lir.LirBlock(
                bridge,
                (
                    op(
                        bridge,
                        ir.Semantics(
                            ir.Operation.MOVE, "mov", dests=(ir.Reg(Register.AX, 2),), sources=(ir.Imm(7, 2),)
                        ),
                    ),
                    op(bridge, ir.Semantics(ir.Operation.JUMP, "jmp", target=0x40)),
                ),
                (0x40,),
            ),
        ),
        {},
        {},
    )
    laid = layout.rebuild(found, [("split exit", body)])
    assert isinstance(laid, asm.Laid), laid
    instructions = list(Decoder(16, laid.code, ip=0x30))
    assert instructions[0].mnemonic == Mnemonic.JLE
    assert instructions[1].mnemonic == Mnemonic.JMP
    assert instructions[1].near_branch_target == laid.moved[bridge]

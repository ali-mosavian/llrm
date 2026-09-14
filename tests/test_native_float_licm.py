import shutil
import subprocess
from pathlib import Path
from dataclasses import replace

import pytest
from iced_x86 import Mnemonic

import corpus
from qbopt import wholeseg
from qbopt.model import ir
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.optimize import transform
from qbopt.model.floating import Format
from qbopt.model.floating import Rounding
from tools.references import fpcsex_check
from qbopt.model.floating import Precision
from qbopt.model.floating import Semantics
from qbopt.model.floating import Exceptions


@pytest.mark.parametrize("obstacle", [None, "empty", "bypass", "cycle", "call", "opaque", "checkpoint"])
def test_floating_motion_requires_execution_and_unchanged_environment(obstacle: str | None) -> None:
    value = mir.Value(1, 30)
    operation = mir.Op(
        30,
        ir.Operation.FLOAT_ARITH,
        "",
        (value,),
        (),
        kind=mir.Kind.FADD,
        floating=Semantics(
            (Format.EXTENDED80, Format.EXTENDED80),
            Format.EXTENDED80,
            Precision.DYNAMIC,
            Rounding.DYNAMIC,
            exceptions=Exceptions.DEFERRED,
        ),
    )
    middle = ()
    if obstacle in ("call", "opaque", "checkpoint"):
        kind = {"call": mir.Kind.CALL, "opaque": mir.Kind.OPAQUE, "checkpoint": mir.Kind.FCHECK}[obstacle]
        middle = (mir.Op(20, ir.Operation.NOTHING, "", (), (), kind=kind),)
    body = mir.MirBody(
        0,
        (
            mir.MirBlock(0, (), (), (10,)),
            mir.MirBlock(10, (), (), (20, 50)),
            mir.MirBlock(20, (), middle, (30,)),
            mir.MirBlock(30, (), (operation,), (40,)),
            mir.MirBlock(40, (), (), (10,)),
            mir.MirBlock(50, (), (), ()),
        ),
    )
    if obstacle in ("bypass", "cycle"):
        body = replace(
            body,
            blocks=tuple(
                replace(block, succ=(30, 40 if obstacle == "bypass" else 20)) if block.at == 20 else block
                for block in body.blocks
            ),
        )
    loop = loops.Loop(10, frozenset({40}), frozenset({10, 20, 30, 40}))
    allowed = transform._guaranteed_float_work(body, loop, nonempty=obstacle != "empty")
    assert (id(operation) in allowed) is (obstacle is None)


@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
@pytest.mark.parametrize("basic", [False, True])
def test_fpcsex_invariant_multiply_and_divide_leave_loop(tag: str, basic: bool) -> None:
    result = wholeseg.emitted(
        Path(f"fixtures/omf/fpcsex-{tag}.obj").read_bytes(), basic_semantics=basic
    )
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    blocks = corpus.partitioned(result.data)
    inside = {at for loop in loops.loops(blocks, blocks[0].at) for at in loop.body}
    assert inside
    arithmetic = [
        block.at for block in blocks for one in block.insns if one.insn.mnemonic in (Mnemonic.FMUL, Mnemonic.FDIV)
    ]
    assert len(arithmetic) == 2
    assert bool(inside.intersection(arithmetic)) is basic


@pytest.mark.e2e
def test_hoisted_fpcsex_matches_original_with_owned_float_slots(tmp_path: Path) -> None:
    result = wholeseg.emitted(Path("fixtures/omf/fpcsex-p-g2.obj").read_bytes(), native_fpu=True)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    (tmp_path / "REF.OBJ").write_bytes(result.data)
    subprocess.run(["uv", "run", "python", "tools/references/fpcsex_check.py", str(tmp_path), "--qemu"], check=True)
    qemu = shutil.which("qemu-system-i386")
    assert qemu is not None
    executed = subprocess.run(
        [
            qemu,
            "-accel",
            "tcg",
            "-cpu",
            "pentium3",
            "-m",
            "16",
            "-drive",
            f"file={tmp_path / 'qemu.img'},format=raw,if=floppy",
            "-boot",
            "a",
            "-display",
            "none",
            "-serial",
            "none",
            "-monitor",
            "none",
            "-debugcon",
            f"file:{tmp_path / 'QEMU.BIN'}",
            "-global",
            "isa-debugcon.iobase=0xe9",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-no-reboot",
        ],
        timeout=15,
        check=False,
    )
    assert executed.returncode == 33
    assert fpcsex_check.checked((tmp_path / "QEMU.BIN").read_bytes()) == 144

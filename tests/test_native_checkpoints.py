from pathlib import Path

import pytest
from iced_x86 import Mnemonic

import corpus
from qbopt import wholeseg


@pytest.mark.parametrize("program", ["fpcsex", "fpdeep"])
def test_native_float_output_has_no_basic_checkpoints(program: str) -> None:
    result = wholeseg.emitted(Path(f"fixtures/omf/{program}-p-g2.obj").read_bytes(), native_fpu=True)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert all(one.mnemonic != Mnemonic.WAIT for one in instructions)


def test_basic_float_output_keeps_exception_checkpoints() -> None:
    result = wholeseg.emitted(Path("fixtures/omf/fpcsex-p-g2.obj").read_bytes(), basic_semantics=True)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert any(one.mnemonic == Mnemonic.WAIT for one in instructions)

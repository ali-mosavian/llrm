from pathlib import Path

import pytest
from iced_x86 import Mnemonic

import corpus
from qbopt import wholeseg


@pytest.mark.parametrize("basic_semantics, adds", [(False, 3), (True, 4)])
@pytest.mark.parametrize("tag", ["p-g2", "q-O", "v-g3"])
def test_fpcsex_reuses_sum_only_under_native_policy(basic_semantics: bool, adds: int, tag: str) -> None:
    result = wholeseg.emitted(
        Path(f"fixtures/omf/fpcsex-{tag}.obj".lower()).read_bytes(),
        basic_semantics=basic_semantics,
    )
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    assert sum(one.mnemonic in (Mnemonic.FADD, Mnemonic.FADDP) for one in instructions) == adds

"""Byte ownership must not enlarge an instruction copied from the input."""

from pathlib import Path

from iced_x86 import Mnemonic

import corpus
from qbopt import wholeseg


def test_nbody_port_read_does_not_copy_neighbor_instructions():
    """NBODY printed an unprintable error: a copied IN acquired XOR and a stray MOV opcode."""
    result = wholeseg.emitted(Path("fixtures/bench/nbody-v-g3.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one.insn for block in corpus.partitioned(result.data) for one in block.insns]
    reads = [index for index, insn in enumerate(instructions) if insn.mnemonic == Mnemonic.IN]
    assert reads
    for index in reads:
        assert instructions[index + 1].mnemonic == Mnemonic.AND

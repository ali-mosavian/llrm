import pytest
from qbopt.backend import arithmetic


@pytest.mark.parametrize("cpu", ["386", "486", "P5", "P6"])
def test_constant_chains_preserve_product(cpu):
    for factor in (3, 7, 15, 20, 31, 45, 63, 85, 127, 1000, 32769):
        chain = arithmetic.scale(factor, cpu)
        if chain is None:
            continue
        for source in (0, 1, -1, -32768, 32767, -2147483648, 2147483647):
            value = source
            for name, count in chain:
                match name:
                    case "shl": value <<= count
                    case "add": value += source
                    case "sub": value -= source
            assert value == source * factor


def test_fast_multiply_changes_break_even():
    assert arithmetic.scale(20, "386") is not None
    assert arithmetic.scale(20, "486") is not None
    assert arithmetic.scale(20, "P5") is not None
    assert arithmetic.scale(20, "P6") is None
    assert arithmetic.scale(7, "386") == (("shl", 3), ("sub", 0))


def test_unknown_cpu_is_not_silently_defaulted():
    with pytest.raises(ValueError, match="CPU target"):
        arithmetic.scale(20, "unknown")


def test_386_small_immediate_multiply_is_not_costed_as_unknown_dword():
    """The selector expanded x*85 using a 22-clock guess; its immediate multiply costs 13."""
    assert arithmetic.scale(85, "386") is None
    assert arithmetic.scale(10, "386") is not None


@pytest.mark.parametrize("number,clocks", [(0, 9), (1, 9), (8, 9), (9, 10), (20, 11), (85, 13), (127, 13)])
def test_386_positive_immediate_early_out(number, clocks):
    assert arithmetic.immediate_multiply("386", number) == clocks


@pytest.mark.parametrize("cpu,multiplies", [("386", 1), ("486", 1), ("P5", 1), ("P6", 2)])
def test_hotlpx_emission_obeys_selected_cpu(cpu, multiplies):
    """HOTLPX's factor twenty should not become a slower chain on a fast multiplier."""
    from pathlib import Path
    from iced_x86 import Mnemonic
    import corpus
    from qbopt import wholeseg
    result = wholeseg.emitted(Path("fixtures/omf/hotlpx-p-g2.obj").read_bytes(), cpu=cpu)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert sum(one.insn.mnemonic == Mnemonic.IMUL for block in corpus.partitioned(result.data)
               for one in block.insns) == multiplies

from itertools import count
import pytest
from qbopt.backend import division
from qbopt.model import ir


@pytest.mark.parametrize("divisor", [3, 7, 10, 31, 1000, 2147483647])
def test_reciprocal_preserves_signed_quotient_and_remainder(divisor):
    """LNGMXX's q+r needs both answers, including negative truncation and INT_MIN."""
    source = ir.Held(1, 4)
    results = (ir.Held(2, 4), ir.Held(3, 4))
    parts = division.reciprocal(source, divisor, results, count(4).__next__, "P5")
    assert parts is not None
    signed = lambda value: ((value & 0xffffffff) ^ 0x80000000) - 0x80000000
    for number in (-2147483648, -divisor, -divisor+1, -1, 0, 1, divisor-1, divisor, 2147483647):
        values = {1: number}
        for part in parts:
            args = [values[arg.value] if isinstance(arg, ir.Held) else arg.value for arg in part.sources]
            match part.name:
                case "mov": answer = args[0]
                case "imul" if len(part.dests) == 2:
                    value = args[0] * args[1]
                    values[part.dests[0].value] = signed(value)
                    values[part.dests[1].value] = signed(value >> 32)
                    continue
                case "imul": answer = args[0] * args[1]
                case "add": answer = args[0] + args[1]
                case "sub": answer = args[0] - args[1]
                case "shl": answer = args[0] << args[1]
                case "shr": answer = (args[0] & 0xffffffff) >> args[1]
                case "sar": answer = args[0] >> args[1]
                case _: pytest.fail(part.name)
            values[part.dests[0].value] = signed(answer)
        quotient = abs(number) // divisor * (-1 if number < 0 else 1)
        assert values[2] == quotient
        assert values[3] == number - quotient * divisor


def test_slow_multiply_keeps_division():
    assert division.reciprocal(ir.Held(1, 4), 7, (ir.Held(2, 4), ir.Held(3, 4)), count(4).__next__, "386") is None


@pytest.mark.parametrize("cpu,divides", [("386", 1), ("486", 1), ("P5", 0), ("P6", 1)])
def test_lngmxx_selects_division_for_cpu(cpu, divides):
    """LNGMXX kept IDIV even where a signed reciprocal is cheaper, including its remainder."""
    from pathlib import Path
    from iced_x86 import Mnemonic
    import corpus
    from qbopt import wholeseg
    result = wholeseg.emitted(Path('fixtures/omf/lngmxx-p-g2.obj').read_bytes(), cpu=cpu)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert sum(one.insn.mnemonic == Mnemonic.IDIV for block in corpus.partitioned(result.data)
               for one in block.insns) == divides

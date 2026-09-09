from itertools import count
import pytest
from qbopt.backend import division, timing
from qbopt.model import ir


@pytest.mark.parametrize("cpu,width,low,high", [("386",2,9,22),("386",4,9,38),("486",2,13,26),("486",4,13,42),("P5",2,11,11),("P5",4,10,10)])
def test_full_product_bounds_follow_operand_width(cpu, width, low, high):
    assert timing.signed_multiply(cpu, width, full=True) == timing.Clocks(low, high)


def test_486_reciprocal_does_not_win_using_midpoint_multiply():
    """LNGMXX was selected on 486 using 26 clocks for a product that may take 42."""
    assert division.reciprocal(ir.Held(1,4), 7, (ir.Held(2,4),ir.Held(3,4)), count(4).__next__, "486") is None


def test_unverified_p6_forms_are_not_silently_priced():
    assert timing.signed_multiply("P6",4,full=True) is None
    assert timing.signed_divide("P6",4) is None


def test_quotient_only_reciprocal_omits_remainder_reconstruction():
    """A quotient-only divide should not multiply back and subtract for a dead remainder."""
    quotient, remainder = ir.Held(2, 4), ir.Held(3, 4)
    parts = division.reciprocal(ir.Held(1, 4), 7, (quotient, remainder), count(4).__next__, "P5", remainder=False)
    assert parts is not None
    assert parts[-1].dests == (quotient,)
    assert not any(remainder in one.dests for one in parts)
    assert not any(one.name == "sub" for one in parts)

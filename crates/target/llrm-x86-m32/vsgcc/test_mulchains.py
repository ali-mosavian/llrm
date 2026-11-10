"""mulchains.py: the least chains the M32-MUL-O1 row quotes."""

import mulchains


def test_1600_least_chain_is_nine_bytes_four_clocks():
    assert mulchains.search(1600, 3) == [(9, 4)]


def test_a_dense_constant_has_no_short_chain():
    assert mulchains.search(1664525, 3) == []

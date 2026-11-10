"""mulchains.py: the least chains the M32-MUL-O1 row quotes."""

import mulchains


def test_1600_least_chain_is_nine_bytes_four_clocks():
    assert mulchains.search(1600, 3) == [(9, 4)]


def test_1664525_has_no_chain_of_four_ops_or_fewer():
    # the row says "no chain of <= 4 ops": the claim is the depth searched, so the depth is the test's
    assert mulchains.search(1664525, 4) == []


def test_a_negative_constant_needs_four_ops_not_three():
    # the search can find a chain at depth 4 and is empty at depth 3: an empty result is not a search that never ran
    assert mulchains.search(-700, 3) == []
    assert mulchains.search(-700, 4) == [(11, 5)]

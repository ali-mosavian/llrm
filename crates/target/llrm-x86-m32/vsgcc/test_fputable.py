import fputable


def test_a_row_with_a_missing_reference_column_is_refused():
    """z80's table understated gaps by up to 1.7 kB when a row was judged against the next-best column."""
    assert fputable.missing_reference([(100, 10), None])
    assert fputable.missing_reference([None, (100, 10)])
    assert fputable.missing_reference([])
    assert not fputable.missing_reference([(100, 10), (90, 12)])


def test_an_o1_row_smaller_in_clocks_but_bigger_in_bytes_is_worse():
    """O1 judged on clocks alone passed a cell that was bytes behind the best reference."""
    refs = [(1000, 200), (1100, 190)]
    assert fputable.worse("O1", (990, 195), refs)
    assert fputable.worse("O1", (1005, 190), refs)
    assert not fputable.worse("O1", (1000, 190), refs)


def test_the_other_levels_have_one_metric():
    assert fputable.worse("O2", (1000, 210), [(1000, 205)])
    assert not fputable.worse("O2", (990, 210), [(1000, 205)])  # 990 + 0.312*210 < 1000 + 0.312*205
    assert fputable.worse("Os", (1, 163), [(999, 162)])
    assert not fputable.worse("Os", (999, 162), [(1, 162)])

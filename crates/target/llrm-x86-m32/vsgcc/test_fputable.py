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


def test_a_row_no_single_compiler_wins_on_both_axes_is_virtual_and_passes_when_pareto_and_not_behind_the_pin():
    """x_iir O1: gcc has the clocks (60,034), the pin the bytes (126); ours 130,528 clk / 115 B is Pareto-optimal and equals the pin on clocks."""
    assert fputable.verdict("O1", (130528, 115), (60034, 172), (130542, 126)) == "VIRTUAL-OK"


def test_a_row_one_compiler_wins_both_axes_is_worse_even_when_ours_is_ahead_on_the_other_axis():
    """x_jpeg_single O1: gcc has both (78.26M clk, 15,886 B); ours is ahead on clocks and 1,638 B behind on bytes."""
    assert fputable.verdict("O1", (75281236, 17524), (78258540, 15886), (85045697, 21184)) == "WORSE"


def test_a_virtual_row_the_pin_beats_on_an_axis_is_worse():
    """x_matmul_f O1: gcc has the clocks, the pin the bytes (235); ours is 11 B behind the pin."""
    assert fputable.verdict("O1", (67111, 246), (68373, 296), (71203, 235)) == "WORSE"


def test_a_row_dominated_by_a_compiler_is_worse_and_a_row_at_or_below_the_best_is_ok():
    assert fputable.verdict("O1", (18734, 307), (18060, 296), (19336, 303)) == "WORSE"
    assert fputable.verdict("O1", (990, 190), (1000, 200), (1100, 195)) == "ok"
    assert fputable.verdict("Os", (1, 163), (1, 162), (1, 170)) == "WORSE"


def test_the_virtual_rule_is_the_shared_module_not_a_copy():
    """Two copies of the rule drift: the 6502 table and this one must judge the same cell the same way."""
    assert fputable.virtual_ok.__file__.endswith("tools/virtual_ok.py")
    assert "def is_virtual" not in open(fputable.__file__).read()

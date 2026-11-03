"""abdiff: two result sets of the same programs, a row worse in any one counter is listed."""
import abdiff


def row(clocks=100, ins=50, code=40):
    return {'clocks': clocks, 'ins': ins, 'code': code}


def test_a_row_whose_clocks_alone_regress_is_listed_worse():
    """A bytes-only comparison printed nothing for a clock-only regression (6502, #6582)."""
    base = {('p', 'llrmOs'): row()}
    new = {('p', 'llrmOs'): row(clocks=106)}
    assert abdiff.worse(base, new) == [(('p', 'llrmOs'), 'clocks', 100, 106)]


def test_a_row_whose_instructions_alone_regress_is_listed_worse():
    assert abdiff.worse({('p', 'v'): row()}, {('p', 'v'): row(ins=51)}) == [(('p', 'v'), 'ins', 50, 51)]


def test_a_row_better_in_one_counter_and_worse_in_another_is_listed_for_the_worse_one():
    got = abdiff.worse({('p', 'v'): row()}, {('p', 'v'): row(code=30, clocks=101)})
    assert got == [(('p', 'v'), 'clocks', 100, 101)]


def test_an_unchanged_row_and_a_row_with_an_error_are_not_listed():
    assert abdiff.worse({('p', 'v'): row(), ('q', 'v'): {'error': 'x'}}, {('p', 'v'): row(), ('q', 'v'): row()}) == []

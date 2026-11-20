"""The clock instrument's parts: perf's report and the pair comparison."""

import clockpair
import runsweep


def test_a_perf_report_gives_the_user_instructions():
    """`perf stat -x,` writes `count,unit,event,...`; a run that never started prints `<not counted>`."""
    assert runsweep.retired("123456,,instructions:u,1000,100.00,,\n") == 123456
    assert runsweep.retired("<not counted>,,instructions:u,0,0.00,,\n") is None
    assert runsweep.retired("") is None


def test_a_pair_is_compared_over_the_cells_both_counted():
    """A cell one side lacks must not read as a saving: the comparison covers the common cells only."""
    base = {"a@-Os": 100, "b@-Os": 50, "c@-Os": 7}
    new = {"a@-Os": 90, "b@-Os": 60, "d@-Os": 1}
    before, after, deltas = clockpair.compare(base, new)
    assert (before, after) == (150, 150)
    assert deltas == [(-10, "a@-Os"), (10, "b@-Os")]

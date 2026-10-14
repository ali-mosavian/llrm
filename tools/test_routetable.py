"""Regression tests of the route table's reading of the allocator's debug lines."""
import routetable

STDERR = """[regalloc] f: 50 insns, 3 spilled, cost 120.5, 0 forced
[regalloc]   trial Whole (splitting false): 2 spilled, cost 110.0, 0 forced
[regalloc]   kept the trial
[routes] f: all 100.0 traffic 5.0000 spill 2.0000 loop-spill 1.0000 frame-cells 3.0000 spill-insns 4 insns 50
[candidates] f: spiller Some(100.0) / Some(7.0), allocator alone Some(110.0) / Some(7.0)
"""


def test_a_compile_gives_a_run_with_its_trial_and_a_route_with_its_features():
    rows = routetable.parse("prog", "-O2", STDERR)
    route = next(row for row in rows if row["kind"] == "route")
    run = next(row for row in rows if row["kind"] == "run")
    assert run["trials"] == [("Whole", False, 2, 110.0)] and run["kept"] == 0 and run["base"] == 120.5
    assert (route["alone"], route["spiller"], route["loopspill"], route["alone_run"]) == (110.0, 100.0, 1.0, 0)

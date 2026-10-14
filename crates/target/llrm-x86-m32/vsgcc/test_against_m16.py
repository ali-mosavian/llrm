"""against_m16.py: the table that measures `m32 no worse than m16` reads the right counters and flags the right rows."""
import against_m16 as t

BENCH = {
    "sum/c/O2": {"instructions": 100, "memory_operands": 40, "clocks": 200},
    "sum/bq16/O2": {"instructions": 1000, "memory_operands": 500, "clocks": 3000},
    "sum/bq32/O2": {"instructions": 1500},
    "worse/c/O2": {"instructions": 100, "memory_operands": 10, "clocks": 100},
    "gone/c/O2": {"instructions": 5, "memory_operands": 1, "clocks": 9},
    "sum/c/O1": {"instructions": 1, "memory_operands": 1, "clocks": 1},
    "c-group/O2": {"instructions": 1},
}
M32 = [
    {"prog": "sum", "variant": "llrm", "ins": 80, "mem": 20, "clocks": 150, "ok": True},
    {"prog": "worse", "variant": "llrm", "ins": 90, "mem": 0, "clocks": 130, "ok": True},
    {"prog": "sum", "variant": "llrmO1", "ins": 9, "mem": 9, "clocks": 9, "ok": True},
]


def test_a_program_is_paired_with_its_other_target_at_the_same_level_only():
    """-O2 is vsgcc's `llrm`, and a BASIC program is its `bq16` against its `bq32`: neither group key nor another level is a row."""
    found = t.rows(BENCH, M32, "O2")
    assert [(one["program"], one["language"]) for one in found] == [("sum", "c"), ("worse", "c"), ("sum", "qb")]
    assert found[0]["clk"] == (200, 150)


def test_a_counter_one_side_does_not_measure_is_none_and_ratios_of_none_count_read_as_one():
    """bench.py counts BASIC at m32 for instructions only; a missing clock was a KeyError. A zero count made the geomean's log fail."""
    qb = t.rows(BENCH, M32, "O2")[2]
    assert qb["clk"] is None and t.ratio(qb, "clk") == 0.0 and t.ratio(qb, "ins") == 1.5
    worse = t.rows(BENCH, M32, "O2")[1]
    assert t.ratio(worse, "mem") == 0.1


def test_the_requirement_is_clocks_where_measured_else_instructions():
    """C rows fail on clocks (130 over 100), not on a lower instruction count; BASIC's on instructions (1.5)."""
    found = {(one["program"], one["language"]): one for one in t.rows(BENCH, M32, "O2")}
    assert t.fails(found[("worse", "c")]) and t.fails(found[("sum", "qb")]) and not t.fails(found[("sum", "c")])


def test_the_table_lists_the_worst_first_and_ends_with_each_languages_geomean_and_worst():
    lines = t.table(t.rows(BENCH, M32, "O2"), "O2")
    body = [line for line in lines if line.startswith(("sum", "worse"))]
    assert [line.split()[:2] for line in body] == [["sum", "qb"], ["worse", "c"], ["sum", "c"]] and body[0].endswith("<")
    assert any("c n=2" in line and "clk" in line and "worst 1.30 (worse)" in line for line in lines), lines

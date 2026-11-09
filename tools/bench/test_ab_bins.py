import ab_bins

CELLS = [("bench/a/a.c", "bench_a"), ("bench/b/b.c", "bench_b")]


def fake(weights):
    """A build is named by its RA-CLK weight; `weights` is {(weight, program): (bytes, clocks)}."""

    def run(job):
        bindir, source, _, level = job
        return (*weights[(bindir, source.split("/")[1])], "ok")

    return run


def test_a_weight_change_that_moves_one_cell_is_that_cells_verdict():
    """The weight 3 -> 1 sweep: matmul -Os +5 B was only visible against the whole corpus. One cell moves; the rest stay `same`."""
    run = fake({("w3", "a"): (289, 23807), ("w1", "a"): (284, 23679), ("w3", "b"): (160, 100), ("w1", "b"): (160, 100)})
    rows = ab_bins.compare("w3", "w1", CELLS, ["-Os"], 500, run=run)
    assert [(r[0], r[4]) for r in rows] == [("bench/a/a.c", "better"), ("bench/b/b.c", "same")]


def test_the_o2_rate_prices_a_byte_in_clocks():
    """At -O2 one clock is two bytes: -6 B for +2 clk is better, -6 B for +4 clk worse."""
    assert ab_bins.verdict("-O2", 500, (293, 23467), (287, 23469)) == "better"
    assert ab_bins.verdict("-O2", 500, (293, 23467), (287, 23471)) == "worse"


def test_os_judges_bytes_alone_and_o1_both_axes():
    assert ab_bins.verdict("-Os", 0, (100, 10), (101, 1)) == "worse"
    assert ab_bins.verdict("-O1", 0, (100, 10), (101, 1)) == "trade"
    assert ab_bins.verdict("-O1", 0, (100, 10), (99, 10)) == "better"


def test_builds_that_print_different_output_get_no_verdict():
    """A miscompile that is also smaller must not read as a win."""
    def run(job):
        return (1, 1, "x" if job[0] == "base" else "y")

    assert ab_bins.compare("base", "head", CELLS[:1], ["-O2"], 500, run=run)[0][4] == "output differs"


def test_an_unmeasured_cell_is_reported_not_dropped():
    assert ab_bins.compare("base", "head", CELLS[:1], ["-O2"], 500, run=lambda job: "compile: boom")[0][4] == "unmeasured"

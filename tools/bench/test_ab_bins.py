import ab_bins

CELLS = [("bench/a/a.c", "bench_a"), ("bench/b/b.c", "bench_b")]


def fake(weights):
    """A build is named by its RA-CLK weight; `weights` is {(weight, program): (bytes, clocks)}."""

    def run(job):
        bindir, source, _, level, _env = job
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


def test_the_same_dist_with_another_environment_is_a_second_build():
    """The weight A/B runs one dist twice, the knob in the environment: the environment reaches that build's compile only."""
    seen = []

    def run(job):
        seen.append((job[0], job[4]))
        return (1, 1, "ok")

    ab_bins.compare("d", "d", CELLS[:1], ["-O2"], 500, run=run, base_env=[("W", "3")], head_env=[("W", "1")])
    assert seen == [("d", (("W", "3"),)), ("d", (("W", "1"),))]


def test_two_builds_in_one_run_each_link_from_their_own_llrm_bin(monkeypatch):
    """dosbatch reads LLRM_BIN once, at import and links that dist's runtime; one process measuring two dists could only ever use one (since #8501 it
    refuses, before it shared one silently). Each measurement runs in its own process with its own dist as LLRM_BIN."""
    seen = []

    class Done:
        stdout = '[1, 1, "ok"]'
        stderr = ""

    monkeypatch.setattr(ab_bins.subprocess, "run", lambda argv, **kw: (seen.append((argv[2:3], kw["env"]["LLRM_BIN"])), Done())[1])
    ab_bins.compare("/dist/base", "/dist/head", CELLS[:1], ["-O2"], 500)
    assert seen == [(["--measure"], "/dist/base"), (["--measure"], "/dist/head")]

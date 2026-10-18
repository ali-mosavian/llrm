import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import perf_main  # noqa: E402


def test_it_speaks_only_when_the_state_changes():
    """A red every three hours would be noise nobody reads; a silent first red or a silent recovery would be the hole this closes."""
    assert perf_main.decide("green", "red") == "comment-red"
    assert perf_main.decide("red", "green") == "comment-green"
    assert perf_main.decide("red", "red") == ""
    assert perf_main.decide("green", "green") == ""
    assert perf_main.decide("", "red") == "comment-red", "no state yet: a first red is news"
    assert perf_main.decide("", "green") == ""


def test_the_report_names_the_failing_tests_and_the_range():
    log = "FAILED tools/test_budgets.py::test_no_step_grows_faster_than_the_program_does[cases] - AssertionError: cases at N=128 -> 2N: lir coalesce 27.2 -> 62.6\nFAILED tools/test_x.py::t\n"
    body = perf_main.report("", "b2fd0b0eb1234", log)
    assert "red at b2fd0b0eb: 2 failing test(s)" in body
    assert "test_no_step_grows_faster_than_the_program_does[cases]: AssertionError: cases at N=128 -> 2N: lir coalesce 27.2 -> 62.6" in body
    assert "none recorded" in body


def test_the_workflow_runs_once_a_day_overnight():
    """The user decided daily only: a run every three hours was eight a day on the machine the measurements share."""
    import re

    workflow = (Path(__file__).parents[2] / ".github/workflows/perf-main.yml").read_text()
    crons = re.findall(r'cron: "([^"]+)"', workflow)
    assert len(crons) == 1 and re.fullmatch(r"\d+ \d+ \* \* \*", crons[0]), crons

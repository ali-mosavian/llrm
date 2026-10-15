import budgets
import pytest

LINE = "noise\n[counts] pass_runs=10 pass_run_instructions=500 analysis_computed=12 analysis_updated=3 analysis_kept=1 analysis_hits=40 module_computed=2 function_forks=0 allocations=4 lir_pipeline_runs=2 spills=7 splits=5\n"


def test_the_counts_line_is_read_name_by_name():
    counts = budgets.parse(LINE)
    assert counts["pass_runs"] == 10 and counts["splits"] == 5


def test_a_compile_without_the_counts_line_is_an_error_not_zero_counts():
    """A harness that read nothing would report every budget met."""
    with pytest.raises(ValueError):
        budgets.parse("[time] wall 1 ms\n")


def test_the_ratios_the_budgets_are_stated_in():
    ratios = budgets.derived(budgets.parse(LINE))
    assert ratios["analysis_per_pass_run"] == 1.5
    assert ratios["instructions_per_pass_run"] == 50
    assert ratios["allocations_per_pipeline"] == 2.0

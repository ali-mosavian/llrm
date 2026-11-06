"""A pytest plugin: a test skipped because a binary this repository builds "is not built" fails.

The gate builds every binary its tests use. tests/test_readme.py skipped its C case on `llrm-c is not built` after the gate's build stopped
making llrm-c (`c-legacy` is off by default), and examples/dot regressed at m16 with the gate green. A host capability (a tool the
repository does not build: the pin's llc, QCport) is declared in tiers.toml instead."""
import re

import pytest

NOT_BUILT = re.compile(r"\bis not built\b")


@pytest.hookimpl(hookwrapper=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    report = outcome.get_result()
    if report.skipped and (reason := str(report.longrepr[2]) if isinstance(report.longrepr, tuple) else "") and NOT_BUILT.search(reason):
        report.outcome = "failed"
        report.longrepr = f"{item.nodeid}: skipped for a binary the gate builds: {reason}"

"""The landing runner's environment does not leak into the tools' tests (see conftest.py)."""

import os

import conftest


def test_the_runners_settings_are_gone_when_the_tools_tests_run():
    """Four tests of parity_squash.py failed under the runner's LAND_FAST=1 and BATCH_CAP and passed in a plain shell: #5681 was refused for it."""
    assert [name for name in conftest.RUNNER_ENV if name in os.environ] == []

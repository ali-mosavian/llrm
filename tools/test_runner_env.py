"""The landing runner's environment does not leak into the tools' tests (see conftest.py)."""

import os
import subprocess
import sys
from pathlib import Path

import conftest


def test_the_runners_settings_are_gone_when_the_tools_tests_run():
    """Four tests of parity_squash.py failed under the runner's LAND_FAST=1 and BATCH_CAP and passed in a plain shell: #5681 was refused for it."""
    assert [name for name in conftest.RUNNER_ENV if name in os.environ] == []


def test_a_buildslot_test_passes_inside_the_landing_batch_environment():
    """The batch sets BUILDSLOT_LANDING=1 for its tool tests: test_buildslot got the bare argv and refused the 07:14 pass for a test the head passes."""
    here = Path(__file__).resolve().parent
    done = subprocess.run([sys.executable, "-m", "pytest", "-q", "-p", "no:cacheprovider", "test_buildslot.py::test_a_cargo_argv_or_shell_string_is_wrapped_and_anything_else_is_not"], cwd=here, env={**os.environ, "BUILDSLOT_LANDING": "1"}, capture_output=True, text=True)
    assert done.returncode == 0, done.stdout[-400:]

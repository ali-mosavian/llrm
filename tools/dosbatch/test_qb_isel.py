"""The QB compiles of qbbuild and qb32 pass $LLRM_ISEL as --isel.

Without it `LLRM_ISEL=old` built the flat-target programs with the default selector: an A/B of the selectors on QB compared the
default with itself (bench-dag.json and bench-old.json: 123 identical rows).
"""

import os
import sys
import tempfile
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qb32  # noqa: E402
import qbbuild  # noqa: E402


def command_of(call) -> list[str]:
    with tempfile.TemporaryDirectory() as where, mock.patch.object(dosbatch.subprocess, "run") as run, mock.patch.dict(os.environ, {"LLRM_ISEL": "old"}):
        run.return_value = mock.Mock(returncode=1, stderr="", stdout="")
        try:
            call(Path(where))
        except (dosbatch.BuildError, RuntimeError):
            pass
    return run.call_args[0][0]


def test_the_flat_build_of_a_program_passes_the_selector_the_run_asks_for():
    source = dosbatch.ROOT / "bench" / "fib" / "fib.bas"
    command = command_of(lambda where: qbbuild.one_target(source, where, {}, ("-O2",), qbbuild.FLAT))
    assert "--isel=old" in command, command


def test_the_flat_runtime_test_compile_passes_the_selector_the_run_asks_for():
    source = dosbatch.ROOT / "bench" / "fib" / "fib.bas"
    command = command_of(lambda where: qb32._compile_basic(source, where / "fib.obj", ("-O2",)))
    assert "--isel=old" in command, command

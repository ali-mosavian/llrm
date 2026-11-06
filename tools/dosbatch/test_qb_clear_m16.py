"""CLEAR on the llrm runtime at m16 (clear_variables.bas) ran to "Illegal function call": the call passed no table of module
variables, then a far pointer where the C body takes a near one. The Microsoft runtime path of run_tests never saw it."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbbuild  # noqa: E402


def test_clear_zeroes_the_module_variables_on_the_llrm_runtime_at_m16(tmp_path):
    source = dosbatch.ROOT / "tests" / "run" / "qb" / "clear_variables.bas"
    objects = {qbbuild.REAL: qbbuild.runtime16(tmp_path / "rt")}
    built = qbbuild.one_target(source, tmp_path, objects, ("-O2",), qbbuild.REAL)
    ran = qbbuild.run(built, tmp_path / "run")
    assert ran.output.decode("latin-1").replace("\r\n", "\n") == source.with_suffix(".out").read_text()

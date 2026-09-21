"""Modern source through the minimal runtime, DOS LINK and a real 386."""

from pathlib import Path

import pytest
from configs import CONFIGS
from modernexe import build
from modernexe import _jwasm
from dosbox import dosbox_bin
from modernexe import BuildError

from qbopt.hir import execute
from qbopt.frontend.modern import driver

ROOT = Path(__file__).resolve().parents[1]
NBODY = ROOT / "frontends" / "modern" / "fixtures" / "nbody.mod"

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x"),
    pytest.mark.skipif(not CONFIGS["v-g3"].available, reason="no DOS linker toolchain"),
]


def test_native_nbody_matches_hir_through_bootstrap_runtime_and_main(tmp_path: Path) -> None:
    """Native nbody once printed unchanged bodies, then missed its last damping.

    Build the shipped assembly bootstrap, C-frontend runtime and modern module
    as distinct OMF objects.  The linked program must stay within the small
    real-mode budget and print exactly what the executable HIR specifies.
    """
    try:
        _jwasm()
    except BuildError as error:
        pytest.skip(str(error))

    expected = execute.run(driver.parsed(NBODY), "main").output
    made = build(NBODY, tmp_path / "NBODY.EXE", run=True)

    assert made.size < 5 * 1024
    assert made.output == expected

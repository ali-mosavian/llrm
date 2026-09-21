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


def test_native_dynamic_fixed_i32_arithmetic_preserves_wrapping_results(tmp_path: Path) -> None:
    """The helper-free two-DIV path must agree at signs and wrapped overflow."""
    source = tmp_path / "fixed_i32.mod"
    source.write_text(
        "type scalar = fixed i32, fraction=9\n"
        "fn product(left: scalar, right: scalar) -> scalar:\n"
        "    return left * right\n"
        "fn quotient(left: scalar, right: scalar) -> scalar:\n"
        "    return left / right\n"
        "fn main() -> i16:\n"
        "    print(quotient(1, 0.001953125))\n"
        "    print(quotient(-1, 0.001953125))\n"
        "    print(quotient(4194303.998046875, 0.001953125))\n"
        "    print(quotient(-4194304, -1))\n"
        "    print(product(100000, 0.5))\n"
        "    return 0\n"
    )

    expected = execute.run(driver.parsed(source), "main").output
    assert expected == "512.0\n-512.0\n-1.0\n-4194304.0\n50000.0\n"

    made = build(source, tmp_path / "FIXED32.EXE", run=True)

    assert made.output == expected

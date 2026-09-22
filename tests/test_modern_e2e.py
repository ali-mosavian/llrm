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
SUM = ROOT / "frontends" / "modern" / "fixtures" / "sum.mod"
DESCRIPTORS = ROOT / "frontends" / "modern" / "fixtures" / "descriptors.mod"
COLLECTIONS = ROOT / "frontends" / "modern" / "fixtures" / "collections.mod"
SUM_THREE = ROOT / "frontends" / "modern" / "fixtures" / "sum_three.mod"

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


def test_native_borrowed_array_uses_a_direct_mutable_payload_pointer(tmp_path: Path) -> None:
    """The real-mode call ABI must not pass or mutate the descriptor address."""
    source = tmp_path / "array_borrow.mod"
    source.write_text(
        "fn bump(values: &mut [i16]) -> void:\n"
        "    values[1] += 3\n"
        "fn main() -> i16:\n"
        "    var values: [i16; 3] = [10, 20, 30]\n"
        "    bump(&mut values)\n"
        "    if values[1] == 23:\n"
        '        print("ok")\n'
        "    else:\n"
        '        print("bad")\n'
        "    return 0\n"
    )

    expected = execute.run(driver.parsed(source), "main").output
    assert expected == "ok\n"

    made = build(source, tmp_path / "BORROW.EXE", run=True)

    assert made.output == expected


def test_native_sum_reads_length_from_the_prefix_descriptor(tmp_path: Path) -> None:
    """The one-pointer sum ABI must preserve initialized payload stores and return 21."""
    expected = execute.run(driver.parsed(SUM), "main").output
    assert expected == "ok\n"

    made = build(SUM, tmp_path / "SUM.EXE", run=True)

    assert made.output == expected


def test_native_three_array_sum_shares_one_index_without_losing_frame_base(tmp_path: Path) -> None:
    """sum_three once addressed initialized locals through EAX+SI instead of BP+SI."""
    oracle = execute.run(driver.parsed(SUM_THREE), "main")
    assert (oracle.output, oracle.value) == ("sum_three: ok\n", 1110)

    made = build(SUM_THREE, tmp_path / "SUM3.EXE", run=True)

    assert made.output == oracle.output
    assert made.size < 3 * 1024


@pytest.mark.parametrize(
    ("source", "executable", "expected"),
    [
        (DESCRIPTORS, "DESCRIPT.EXE", "descriptors: ok\n"),
        (COLLECTIONS, "COLLECT.EXE", "collections: ok\n"),
    ],
)
def test_native_modern_collection_examples_match_hir(
    tmp_path: Path, source: Path, executable: str, expected: str
) -> None:
    """Descriptor views and bounded collections must survive real OMF linking."""
    oracle = execute.run(driver.parsed(source), "main")
    assert (oracle.output, oracle.value) == (expected, 0)

    made = build(source, tmp_path / executable, run=True)

    assert made.output == expected
    assert made.size < 3 * 1024


def test_native_integers_print_as_the_hir_executor_prints_them(tmp_path: Path) -> None:
    """The runtime had no integer formatter: printing an i32 failed to link on `_pi4`."""
    source = tmp_path / "integers.mod"
    source.write_text(
        "fn main() -> i16:\n"
        "    let a: i8 = -128\n"
        "    let b: u8 = 255\n"
        "    let c: i16 = -32768\n"
        "    let d: u16 = 65535\n"
        "    let e: i32 = -2147483648\n"
        "    let f: u32 = 4294967295\n"
        "    let g: i32 = 0\n"
        "    print(f\"{a} {b} {c} {d} {e} {f} {g}\")\n"
        "    return 0\n"
    )

    expected = execute.run(driver.parsed(source), "main").output
    assert expected == "-128 255 -32768 65535 -2147483648 4294967295 0\n"
    assert build(source, tmp_path / "INTS.EXE", run=True).output == expected

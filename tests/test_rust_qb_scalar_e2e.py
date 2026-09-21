"""Rust QB scalar OMF must link and run through the VBDOS runtime."""

import shutil
import subprocess
from pathlib import Path

import pytest
from dosbox import launch
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "bench" / "parity" / "scalar.bas"
GOLDEN = ROOT / "bench" / "parity" / "golden" / "scalar.txt"
PARITY_SOURCE = ROOT / "bench" / "parity" / "parity.bas"
PARITY_GOLDEN = ROOT / "bench" / "parity" / "golden" / "parity.txt"
ALGEBRA_SOURCE = ROOT / "bench" / "parity" / "algebra.bas"
ALGEBRA_GOLDEN = ROOT / "bench" / "parity" / "golden" / "algebra.txt"
BRANCH_SOURCE = ROOT / "bench" / "parity" / "branch.bas"
BRANCH_GOLDEN = ROOT / "bench" / "parity" / "golden" / "branch.txt"
MEMORY_SOURCE = ROOT / "bench" / "parity" / "memory.bas"
MEMORY_GOLDEN = ROOT / "bench" / "parity" / "golden" / "memory.txt"
LOOP_SOURCE = ROOT / "bench" / "parity" / "loop.bas"
LOOP_GOLDEN = ROOT / "bench" / "parity" / "golden" / "loop.txt"
CFG = CONFIGS["v-g3"]

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.skipif(shutil.which("cargo") is None, reason="cargo is not installed"),
    pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x"),
    pytest.mark.skipif(not CFG.available, reason="no v-g3 DOS linker toolchain"),
]


def _llrm_qb() -> Path:
    binary = ROOT / "target" / "debug" / "llrm-qb"
    build = subprocess.run(
        ["cargo", "build", "--quiet", "--bin", "llrm-qb"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert build.returncode == 0, build.stdout + build.stderr
    assert binary.is_file()
    return binary


def _normalized_dos(text: str) -> str:
    """Remove DOS line endings, blank lines, and terminal display padding."""
    return "\n".join(line.rstrip() for line in text.replace("\r\n", "\n").split("\n") if line.strip())


def _assert_program_matches_oracle(tmp_path: Path, source: Path, golden: Path, stem: str) -> None:
    object_file = tmp_path / f"{stem}.OBJ"
    compiled = subprocess.run(
        [
            str(_llrm_qb()),
            "--emit",
            "obj",
            "--dialect",
            "vbdos",
            "--runtime",
            "vbdos",
            "-o",
            str(object_file),
            str(source),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert object_file.is_file()

    run = launch(
        tmp_path,
        CFG.mount,
        [
            f"{CFG.link} {stem}.OBJ, {stem}.EXE,, {CFG.runtime}; > LINK.OUT",
            f"{stem}.EXE > ACTUAL.TXT",
        ],
        timeout=20,
        env={"LIB": r"V:\LIB"},
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    assert _normalized_dos(read_dos(tmp_path, "ACTUAL.TXT")) == _normalized_dos(golden.read_text())


def test_rust_qb_scalar_matches_its_program_level_oracle(tmp_path: Path) -> None:
    """The emitted scalar program must print the checked-in 1789/DONE oracle.

    This is the end-to-end gate for the paired BASIC program: VBDOS's LINK
    resolves the Rust-produced OMF first, its own runtime executes it on a
    real DOS 386, and the program-level answer is compared to the established
    golden output rather than to an object-internal representation.
    """
    _assert_program_matches_oracle(tmp_path, SOURCE, GOLDEN, "SCALAR")


def test_rust_qb_aggregate_parity_matches_its_program_level_oracle(tmp_path: Path) -> None:
    """The ported aggregate path must preserve Python PARITY's 1789 result.

    PARITYKERNEL owns a dynamic local UDT array, erases it before returning,
    and reloads its LONG result afterwards. This catches the established exit
    lifetime through Rust OMF, Microsoft LINK, and the VBDOS runtime rather
    than accepting an object that only passes host-side validation.
    """
    _assert_program_matches_oracle(tmp_path, PARITY_SOURCE, PARITY_GOLDEN, "PARITY")


def test_rust_qb_algebra_matches_its_program_level_oracle(tmp_path: Path) -> None:
    """Spill-capable Rust CodeGen must preserve algebra's 702774 result.

    The two LONG-returning procedure calls create the first paired-program
    pressure case that the old no-spill Rust allocator refused.  Compile it
    through fresh OMF and compare the DOS execution with the established
    Python-era program oracle rather than accepting allocation in isolation.
    """
    _assert_program_matches_oracle(tmp_path, ALGEBRA_SOURCE, ALGEBRA_GOLDEN, "ALGEBRA")


def test_rust_qb_branch_matches_its_program_level_oracle(tmp_path: Path) -> None:
    """The IF/ELSE paired program must print branch's -87904/DONE oracle.

    This keeps the signed INTEGER comparison, both LONG-returning function
    arms, and their join on the real VBDOS object/runtime path.
    """
    _assert_program_matches_oracle(tmp_path, BRANCH_SOURCE, BRANCH_GOLDEN, "BRANCH")


def test_rust_qb_memory_matches_its_program_level_oracle(tmp_path: Path) -> None:
    """The by-reference INTEGER update must print memory's 361001/DONE oracle."""
    _assert_program_matches_oracle(tmp_path, MEMORY_SOURCE, MEMORY_GOLDEN, "MEMORY")


def test_rust_qb_loop_matches_its_program_level_oracle(tmp_path: Path) -> None:
    """The loop-carried LONG total must print loop's 130991/DONE oracle."""
    _assert_program_matches_oracle(tmp_path, LOOP_SOURCE, LOOP_GOLDEN, "LOOP")

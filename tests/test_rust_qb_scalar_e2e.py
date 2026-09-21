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


def test_rust_qb_scalar_matches_its_program_level_oracle(tmp_path: Path) -> None:
    """The emitted scalar program must print the checked-in 1789/DONE oracle.

    This is the end-to-end gate for the paired BASIC program: VBDOS's LINK
    resolves the Rust-produced OMF first, its own runtime executes it on a
    real DOS 386, and the program-level answer is compared to the established
    golden output rather than to an object-internal representation.
    """
    object_file = tmp_path / "SCALAR.OBJ"
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
            str(SOURCE),
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
            f"{CFG.link} SCALAR.OBJ, SCALAR.EXE,, {CFG.runtime}; > LINK.OUT",
            "SCALAR.EXE > ACTUAL.TXT",
        ],
        timeout=20,
        env={"LIB": r"V:\LIB"},
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    assert _normalized_dos(read_dos(tmp_path, "ACTUAL.TXT")) == _normalized_dos(GOLDEN.read_text())

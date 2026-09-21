"""Rust C objects must survive Microsoft LINK and a real DOS 386."""

import shutil
import subprocess
from pathlib import Path

import pytest
from dosbox import launch
from configs import CONFIGS
from dosbox import dos_file
from dosbox import read_dos
from dosbox import dosbox_bin

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures" / "c" / "parity" / "scalar.cgs"
HARNESS = ROOT / "fixtures" / "c" / "parity" / "scalar-start.asm"
PARITY_SOURCE = ROOT / "fixtures" / "c" / "parity" / "parity.cgs"
PARITY_HARNESS = ROOT / "fixtures" / "c" / "parity" / "parity-start.asm"
CELLS_SOURCE = ROOT / "fixtures" / "c" / "cells.cgs"
CELLS_HARNESS = ROOT / "fixtures" / "c" / "cells-start.asm"
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")
CFG = CONFIGS["v-g3"]

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.skipif(shutil.which("cargo") is None, reason="cargo is not installed"),
    pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x"),
    pytest.mark.skipif(not Path(JWASM).is_file(), reason="jwasm is not installed"),
    pytest.mark.skipif(not CFG.available, reason="no v-g3 DOS linker toolchain"),
]


def _llrm_c() -> Path:
    binary = ROOT / "target" / "debug" / "llrm-c"
    build = subprocess.run(
        ["cargo", "build", "--quiet", "--bin", "llrm-c"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert build.returncode == 0, build.stdout + build.stderr
    assert binary.is_file()
    return binary


def _compile_and_run(
    source: Path,
    harness: Path,
    tmp_path: Path,
    *,
    timeout: int = 10,
) -> int:
    object_file = tmp_path / "PROGRAM.OBJ"
    compiled = subprocess.run(
        [str(_llrm_c()), "--emit", "obj", "-o", str(object_file), str(source)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert object_file.is_file()

    harness_file = tmp_path / "START.ASM"
    harness_file.write_bytes(harness.read_bytes())
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(harness_file)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr

    run = launch(
        tmp_path,
        CFG.mount,
        [f"{CFG.link} START.OBJ+PROGRAM.OBJ, PROGRAM.EXE,,; > LINK.OUT", "PROGRAM.EXE"],
        timeout=timeout,
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    value = dos_file(tmp_path, "VALUE.BIN")
    assert value is not None
    return int.from_bytes(value.read_bytes(), "little", signed=True)


def test_rust_c_scalar_returns_the_independent_parity_answer(tmp_path: Path) -> None:
    """The first Rust C paired program must link and return scalar's 1789.

    This is the runtime milestone for the real WCC capture path: an OMF object
    emitted by ``llrm-c`` must survive JWASM's external caller, Microsoft's
    v-g3 LINK, and a DOS 386 instead of merely looking valid to host parsing.
    """
    assert _compile_and_run(SOURCE, HARNESS, tmp_path, timeout=20) == 1789


def test_rust_c_local_short_cells_returns_the_independent_argument(tmp_path: Path) -> None:
    """Python's WCC capture of local ``short cells[4]`` must return 1234.

    This exercises the newly ported local-aggregate path as a real program:
    the WCC-derived stream is compiled by ``llrm-c``, linked with a far-cdecl
    caller, and run on DOS without relying on any optimizer.
    """
    assert _compile_and_run(CELLS_SOURCE, CELLS_HARNESS, tmp_path) == 1234


def test_rust_c_aggregate_parity_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """The Rust port of Python's aggregate C path must still return 1789.

    ``tests/test_frontend_parity.py`` established this independently for the
    working Python compiler.  This is the same real WCC capture, far-cdecl
    caller, Microsoft linker, and DOS execution oracle for ``llrm-c``.
    """
    assert _compile_and_run(PARITY_SOURCE, PARITY_HARNESS, tmp_path) == 1789

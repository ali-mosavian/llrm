"""Rust QB objects must survive Microsoft LINK and a real DOS 386."""

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
SOURCE = ROOT / "frontends" / "qb" / "fixtures" / "emission.bas"
HARNESS = ROOT / "fixtures" / "qb" / "emission-start.asm"
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")
CFG = CONFIGS["q-O"]

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.skipif(shutil.which("cargo") is None, reason="cargo is not installed"),
    pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x"),
    pytest.mark.skipif(not Path(JWASM).is_file(), reason="jwasm is not installed"),
    pytest.mark.skipif(not CFG.available, reason="no q-O DOS linker toolchain"),
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


def test_rust_qb_emission_returns_add_one_through_the_qb45_runtime(tmp_path: Path) -> None:
    """``emission.bas`` must link and return ADDONE's independent answer, 42.

    The Rust QB frontend emits an OMF object.  JWASM's external caller and
    QB45's own Microsoft LINK/runtime then establish that its far Pascal call
    returns the signed long result the DOS program writes to ``VALUE.BIN``.
    """
    object_file = tmp_path / "EMISSION.OBJ"
    compiled = subprocess.run(
        [
            str(_llrm_qb()),
            "--emit",
            "obj",
            "--dialect",
            "qb45",
            "--runtime",
            "qb45",
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

    harness_file = tmp_path / "START.ASM"
    harness_file.write_bytes(HARNESS.read_bytes())
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(harness_file)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr

    run = launch(
        tmp_path,
        CFG.mount,
        [
            f"{CFG.link} START.OBJ+EMISSION.OBJ, EMISSION.EXE,, {CFG.runtime}; > LINK.OUT",
            "EMISSION.EXE",
        ],
        timeout=20,
        env={"LIB": r"V:\LIB"},
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    value = dos_file(tmp_path, "VALUE.BIN")
    assert value is not None
    payload = value.read_bytes()
    assert len(payload) == 4, payload.hex()
    assert int.from_bytes(payload, "little", signed=True) == 42

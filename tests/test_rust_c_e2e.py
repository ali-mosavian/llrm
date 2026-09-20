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


def test_rust_c_scalar_returns_the_independent_parity_answer(tmp_path: Path) -> None:
    """The first Rust C paired program must link and return scalar's 1789.

    This is the runtime milestone for the real WCC capture path: an OMF object
    emitted by ``llrm-c`` must survive JWASM's external caller, Microsoft's
    v-g3 LINK, and a DOS 386 instead of merely looking valid to host parsing.
    """
    object_file = tmp_path / "SCALAR.OBJ"
    compiled = subprocess.run(
        [str(_llrm_c()), "--emit", "obj", "-o", str(object_file), str(SOURCE)],
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
        [f"{CFG.link} START.OBJ+SCALAR.OBJ, SCALAR.EXE,,; > LINK.OUT", "SCALAR.EXE"],
        timeout=20,
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    value = dos_file(tmp_path, "VALUE.BIN")
    assert value is not None
    assert int.from_bytes(value.read_bytes(), "little", signed=True) == 1789

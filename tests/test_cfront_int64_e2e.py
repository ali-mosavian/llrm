"""C int64 programs, through qbopt's object writer, LINK and a real 386."""

import shutil
import subprocess
from pathlib import Path

import pytest
from dosbox import launch
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin

from qbopt.backend import omfwrite
from qbopt.cfront import compile as cfront

ROOT = Path(__file__).resolve().parents[1]
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")
CFG = CONFIGS["v-g3"]

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x"),
    pytest.mark.skipif(not Path(JWASM).is_file(), reason="jwasm is not installed"),
    pytest.mark.skipif(not CFG.available, reason="no DOS linker toolchain"),
]


START = """\
.model medium
.386
.stack 512
extrn _main:far
.code
start:
    call far ptr _main
    mov ah, 4ch
    int 21h
end start
"""


def test_int64_number_crunching_programs_return_success(tmp_path: Path) -> None:
    """mix64, euclid64 and fib64 all returned nonzero before int64 lowering.

    This is the semantic gate: compile the recorded Watcom streams with
    qbopt, write OMF directly, link each module to a minimal DOS entry point,
    and require its self-checking ``main`` to return zero.  fib64 specifically
    returned 1 when liveness treated a parallel phi-copy group as sequential.
    """
    start = tmp_path / "START.ASM"
    start.write_text(START)
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(start)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr

    names = ("MIX64", "EUCLID64", "FIB64")
    for name in names:
        source = ROOT / "fixtures" / "c" / "mir" / f"{name.lower()}.cgs"
        module = cfront.assembled(source.read_text(), name.lower(), optimise=True)
        (tmp_path / f"{name}.OBJ").write_bytes(omfwrite.written(module, f"{name.lower()}.c"))

    steps = []
    for name in names:
        steps += [
            f"{CFG.link} START.OBJ+{name}.OBJ, {name}.EXE,,; >> LINK.OUT",
            f"{name}.EXE",
            f"if errorlevel 1 goto {name}FAIL",
            f"echo PASS > {name}.TXT",
            f"goto {name}DONE",
            f":{name}FAIL",
            f"echo FAIL > {name}.TXT",
            f":{name}DONE",
        ]
    run = launch(tmp_path, CFG.mount, steps, timeout=30)
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    assert {name: read_dos(tmp_path, f"{name}.TXT").strip() for name in names} == {name: "PASS" for name in names}

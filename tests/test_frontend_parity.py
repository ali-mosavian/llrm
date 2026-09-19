"""Paired source programs must survive either frontend and share one oracle."""

import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

from qbopt import wholeseg
from qbopt.backend import omfwrite
from qbopt.cfront import compile as cfront
from qbopt.objectfile import module
from qbopt.objectfile import omf


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "bench" / "parity"
FIXTURE = ROOT / "fixtures" / "parity" / "parity-v-g3.obj"
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")


def _expected() -> int:
    return json.loads((SOURCE / "expected.json").read_text())["parity"]


def test_both_frontends_reach_the_shared_optimizer_and_fresh_omf_writer() -> None:
    """BC aggregate code once refused allocation while equivalent C emitted.

    This is the bounded host-side parity gate: both independently raised
    programs must finish the shared optimization, allocation, layout, and
    fresh OMF path.  Runtime tests below bind both objects to the same answer.
    """
    basic = wholeseg.emitted(FIXTURE.read_bytes())
    assert basic.outcome is wholeseg.Emission.LIR, basic.reason
    assert module.of(omf.parse(basic.data)).code

    source = SOURCE / "parity.c"
    c_module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    c_object = omfwrite.written(c_module, source.name)
    assert module.of(omf.parse(c_object)).code


def _c_start() -> str:
    return """\
.model medium
.386
.stack 1024
extrn _parity_kernel:far
.data
value dd ?
filename db 'VALUE.BIN', 0
.code
start:
    mov ax, @data
    mov ds, ax
    call far ptr _parity_kernel
    mov word ptr value, ax
    mov word ptr value+2, dx
    mov ah, 3ch
    xor cx, cx
    lea dx, filename
    int 21h
    jc failed
    mov bx, ax
    mov ah, 40h
    mov cx, 4
    lea dx, value
    int 21h
    jc failed
    xor al, al
    jmp finished
failed:
    mov al, 1
finished:
    mov ah, 4ch
    int 21h
end start
"""


def _runtime_available() -> bool:
    sys.path.insert(0, str(ROOT / "tools"))
    from configs import CONFIGS
    from dosbox import dosbox_bin

    return dosbox_bin() is not None and Path(JWASM).is_file() and CONFIGS["v-g3"].available


@pytest.mark.e2e
@pytest.mark.skipif(not _runtime_available(), reason="DOSBox or the DOS toolchains are unavailable")
def test_basic_frontend_returns_the_independent_parity_answer(tmp_path: Path) -> None:
    """The optimized BC object must print the same independent result as C."""
    from tools import e2e

    result = e2e.run(
        "v-g3",
        names=["parity"],
        source_dir=SOURCE,
        golden_dir=SOURCE / "golden",
        work=tmp_path,
        timeout=30,
    )
    assert result.ok, result.verdicts
    assert _expected() == 1789


@pytest.mark.e2e
@pytest.mark.skipif(not _runtime_available(), reason="DOSBox or the DOS toolchains are unavailable")
def test_c_frontend_returns_the_independent_parity_answer(tmp_path: Path) -> None:
    """The C frontend's fresh object must return the BASIC corpus oracle."""
    from configs import CONFIGS
    from dosbox import launch

    source = SOURCE / "parity.c"
    c_module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    (tmp_path / "PARITY.OBJ").write_bytes(omfwrite.written(c_module, source.name))
    start = tmp_path / "START.ASM"
    start.write_text(_c_start())
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(start)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr

    cfg = CONFIGS["v-g3"]
    run = launch(
        tmp_path,
        cfg.mount,
        [f"{cfg.link} START.OBJ+PARITY.OBJ, CPARITY.EXE,,; > LINK.OUT", "CPARITY.EXE"],
        timeout=20,
    )
    assert run.finished and not run.timed_out, run
    link = (tmp_path / "LINK.OUT").read_text(errors="replace").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    value = next((tmp_path / name for name in ("VALUE.BIN", "value.bin") if (tmp_path / name).is_file()), None)
    assert value is not None
    assert int.from_bytes(value.read_bytes(), "little", signed=True) == _expected()

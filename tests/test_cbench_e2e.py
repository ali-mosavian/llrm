"""Known-answer C benchmark checks through OMF, LINK, and a real 386."""

import shutil
import subprocess
from pathlib import Path

import pytest
from configs import CONFIGS
from dosbox import dosbox_bin
from dosbox import launch
from dosbox import read_dos

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


CRC_START = """\
.model medium
.386
.stack 512
extrn _bench_crc:far
.data
value dd ?
filename db 'VALUE.BIN', 0
.code
start:
    mov ax, @data
    mov ds, ax
    xor ax, ax
    push ax
    push ax
    call far ptr _bench_crc
    add sp, 4
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


def test_optimized_crc_returns_the_canonical_check_value(tmp_path: Path) -> None:
    """CRC returned FFFFFFFF: unrolling orphaned the inner loop's live-out."""
    source = ROOT / "bench" / "c" / "crc.c"
    stream = cfront.recorded(source, [])
    module = cfront.assembled(stream, "crc", optimise=True)
    (tmp_path / "CRC.OBJ").write_bytes(omfwrite.written(module, source.name))
    start = tmp_path / "START.ASM"
    start.write_text(CRC_START)
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(start)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr

    run = launch(
        tmp_path,
        CFG.mount,
        [
            f"{CFG.link} START.OBJ+CRC.OBJ, CRC.EXE,,; > LINK.OUT",
            "CRC.EXE",
        ],
        timeout=20,
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    result = next((tmp_path / name for name in ("VALUE.BIN", "value.bin") if (tmp_path / name).is_file()), None)
    assert result is not None
    assert int.from_bytes(result.read_bytes(), "little") == 0xCBF43926

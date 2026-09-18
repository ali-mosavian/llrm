"""Known-answer C benchmark checks through OMF, LINK, and a real 386."""

import json
import shutil
import subprocess
from pathlib import Path

import pytest
from configs import CONFIGS
from dosbox import dosbox_bin
from dosbox import dos_file
from dosbox import launch
from dosbox import read_dos

from qbopt.backend import omfwrite
from qbopt.cfront import compile as cfront


ROOT = Path(__file__).resolve().parents[1]
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")
CFG = CONFIGS["v-g3"]
BENCHMARKS = (
    ("sieve", "SIEVE", 1024, 2),
    ("crc", "CRC", 0, 4),
    ("matmul", "MATMUL", 0, 2),
    ("mandel", "MANDEL", 0, 2),
    ("shellsort", "SHELL", 0, 2),
    ("floats", "FLOATS", 1000, 2),
    ("nbody", "NBODY", 200, 2),
)

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x"),
    pytest.mark.skipif(not Path(JWASM).is_file(), reason="jwasm is not installed"),
    pytest.mark.skipif(not CFG.available, reason="no DOS linker toolchain"),
]


def _start(benchmarks=BENCHMARKS) -> str:
    declarations = "\n".join(f"extrn _bench_{name}:far" for name, _file, _argument, _width in benchmarks)
    calls = []
    for index, (name, _file, argument, width) in enumerate(benchmarks):
        if width == 4:
            calls.extend((f"    mov ax, {argument >> 16}", "    push ax"))
        calls.extend((f"    mov ax, {argument & 0xffff}", "    push ax", f"    call far ptr _bench_{name}"))
        calls.extend(
            (
                f"    add sp, {width}",
                f"    mov word ptr values+{index * 4}, ax",
                f"    mov word ptr values+{index * 4 + 2}, dx",
                "    mov ah, 3ch",
                "    xor cx, cx",
                f"    lea dx, markerName{index}",
                "    int 21h",
                "    jc failed",
                "    mov bx, ax",
                "    mov ah, 3eh",
                "    int 21h",
            )
        )
    return f"""\
.model medium
.386
.stack 4096
{declarations}
.data
values db {len(benchmarks) * 4} dup (?)
filename db 'VALUE.BIN', 0
{chr(10).join(f"markerName{index} db 'P{index}.DAT', 0" for index in range(len(benchmarks)))}
.code
start:
    mov ax, @data
    mov ds, ax
    cld
{chr(10).join(calls)}
    mov ah, 3ch
    xor cx, cx
    lea dx, filename
    int 21h
    jc failed
    mov bx, ax
    mov ah, 40h
    mov cx, {len(benchmarks) * 4}
    lea dx, values
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


def _answers(tmp_path: Path, benchmarks) -> dict[str, int]:
    expected = json.loads((ROOT / "bench" / "c" / "expected.json").read_text())
    for name, filename, _argument, _width in benchmarks:
        source = ROOT / "bench" / "c" / f"{name}.c"
        stream = cfront.recorded(source, [])
        module = cfront.assembled(stream, name, optimise=True)
        (tmp_path / f"{filename}.OBJ").write_bytes(omfwrite.written(module, source.name))
    start = tmp_path / "START.ASM"
    start.write_text(_start(benchmarks))
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
            f"{CFG.link} START.OBJ+" + "+".join(f"{filename}.OBJ" for _name, filename, _argument, _width in benchmarks)
            + ", CBENCH.EXE,,; > LINK.OUT",
            "CBENCH.EXE",
        ],
        timeout=20,
    )
    completed = [index for index in range(len(benchmarks)) if dos_file(tmp_path, f"P{index}.DAT") is not None]
    assert run.finished and not run.timed_out, (run, completed)
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    result = next((tmp_path / name for name in ("VALUE.BIN", "value.bin") if (tmp_path / name).is_file()), None)
    assert result is not None
    raw = result.read_bytes()
    observed = {
        name: int.from_bytes(raw[index * 4 : index * 4 + 4], "little")
        for index, (name, _filename, _argument, _width) in enumerate(benchmarks)
    }
    assert observed == {name: expected[name]["result"] & 0xffffffff for name, *_rest in benchmarks}
    return observed


def test_optimized_c_benchmarks_return_their_independent_answers(tmp_path: Path) -> None:
    """CRC returned FFFFFFFF when unrolling orphaned its inner-loop live-out."""
    _answers(tmp_path, BENCHMARKS)


def test_cloned_c_nbody_returns_its_independent_answer(tmp_path: Path) -> None:
    """Nbody must match its recorded 4774160 oracle through fresh OMF,
    LINK, and a real DOS 386, rather than merely resemble GCC's listing.
    """
    _answers(tmp_path, (BENCHMARKS[-1],))


def test_duplicated_c_return_tail_preserves_both_answers(tmp_path: Path) -> None:
    """Duplicating choose's terminal tail must keep inputs 0/1 returning 10/8."""
    source = ROOT / "fixtures" / "c" / "choose.c"
    module = cfront.assembled(cfront.recorded(source, []), "choose", optimise=True)
    (tmp_path / "CHOOSE.OBJ").write_bytes(omfwrite.written(module, source.name))
    start = tmp_path / "START.ASM"
    start.write_text(
        """\
.model medium
.386
.stack 1024
extrn _choose:far
.data
values dw 2 dup (?)
filename db 'VALUE.BIN', 0
.code
start:
    mov ax, @data
    mov ds, ax
    push 0
    call far ptr _choose
    add sp, 2
    mov values, ax
    push 1
    call far ptr _choose
    add sp, 2
    mov values+2, ax
    mov ah, 3ch
    xor cx, cx
    lea dx, filename
    int 21h
    jc failed
    mov bx, ax
    mov ah, 40h
    mov cx, 4
    lea dx, values
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
    )
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(start)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr
    run = launch(
        tmp_path,
        CFG.mount,
        [f"{CFG.link} START.OBJ+CHOOSE.OBJ, CHOOSE.EXE,,; > LINK.OUT", "CHOOSE.EXE"],
        timeout=10,
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    result = next((tmp_path / name for name in ("VALUE.BIN", "value.bin") if (tmp_path / name).is_file()), None)
    assert result is not None
    assert result.read_bytes() == bytes((10, 0, 8, 0))

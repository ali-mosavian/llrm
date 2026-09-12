import shutil
import subprocess
from pathlib import Path

import pytest

from tools.references import fpcsex_check


@pytest.mark.e2e
def test_reference_observes_invalid_division_before_writing_q(tmp_path: Path) -> None:
    # The candidate stored NaN to q before #MF; BC retained its preceding value.
    qemu = shutil.which("qemu-system-i386")
    if qemu is None:
        raise FileNotFoundError("QEMU is required for qualified x87 trap validation")
    subprocess.run(
        ["uv", "run", "python", "tools/references/reference.py", str(tmp_path), "--program", "fpcsex"], check=True
    )
    subprocess.run(
        ["uv", "run", "python", "tools/references/fpcsex_check.py", str(tmp_path), "--qemu", "--traps"], check=True
    )
    result = subprocess.run(
        [
            qemu,
            "-accel",
            "tcg",
            "-cpu",
            "pentium3",
            "-m",
            "16",
            "-drive",
            f"file={tmp_path / 'qemu.img'},format=raw,if=floppy",
            "-boot",
            "a",
            "-display",
            "none",
            "-serial",
            "none",
            "-monitor",
            "none",
            "-debugcon",
            f"file:{tmp_path / 'TRAPS.BIN'}",
            "-global",
            "isa-debugcon.iobase=0xe9",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-no-reboot",
        ],
        timeout=15,
        check=False,
    )
    assert result.returncode == 33
    assert fpcsex_check.checked((tmp_path / "TRAPS.BIN").read_bytes(), traps=True) == 216

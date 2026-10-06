"""A long double program (tests/fixtures/c/longdouble.c) run in DOSBox prints
what the host compiler's does: the 80-bit format is the same on both.

Needs the release binaries, DOSBox-X and a host cc; skipped, loudly, without
them (LLRM_REQUIRE_BINARIES=1 fails).
"""

import os
import shutil
import warnings
import subprocess
import sys
from pathlib import Path

import pytest

from tools.e2e import dosbox
from tools.e2e.configs import QB45

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "dosbatch"))
import dosbatch  # noqa: E402
BIN = ROOT / "target" / "release"
SOURCE = ROOT / "tests" / "fixtures" / "c" / "longdouble.c"


def _unusable() -> str | None:
    for tool in ("llrm-c", "jwasm", "jwlink"):
        if not (BIN / tool).exists():
            return f"{BIN / tool} is missing"
    if not QB45.is_dir() or dosbox.dosbox_bin() is None:
        return "no DOSBox-X or no QB 4.5 tools (for DOS's mount)"
    return None if shutil.which("cc") else "no host cc"


def test_a_long_double_program_prints_on_dos_what_it_prints_on_the_host(tmp_path):
    why = _unusable()
    if why:
        message = f"NOT RUN: {why}"
        if os.environ.get("LLRM_REQUIRE_BINARIES"):
            pytest.fail(message)
        warnings.warn(message, stacklevel=1)
        pytest.skip(message)

    def run(*command):
        done = subprocess.run([str(one) for one in command], cwd=tmp_path, capture_output=True, text=True)
        assert done.returncode == 0, done.stdout + done.stderr
        return done.stdout

    run(BIN / "llrm-c", SOURCE, "-O2", "-o", "p.obj")
    dosbatch.link_c(tmp_path / "p.obj", tmp_path / "P.EXE", tmp_path)
    assert dosbox.launch(tmp_path, QB45, ["P.EXE > P.TXT"], timeout=60).finished
    on_dos = dosbox.read_dos(tmp_path, "P.TXT").replace("\r", "")

    (tmp_path / "host.c").write_text('#include <stdio.h>\nvoid report(long v) { printf("%ld\\n", v); }\n')
    source = SOURCE.read_text().replace("void main(void)", "int main(void)").rstrip().removesuffix("}") + "return 0;\n}\n"
    (tmp_path / "program.c").write_text(source)
    run("cc", "-O0", "-o", "host", "host.c", "program.c")
    assert on_dos == run("./host")
    assert on_dos.split() == ["11", "23", "11500", "15", "-7", "11", "1", "100000000", "100000000"]

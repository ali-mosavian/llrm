"""B$CHKEXP, B$CHKEX2 and B$CHKLOG (runtime/qb/dos/m16/fpu.asm) at their range edges, hand-called under DOSBox."""

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]


def test_the_m16_range_checks_raise_on_the_right_side_of_their_bounds(tmp_path):
    """A `ja` where the bound is `jae` let EXP(22712.700238998008) through as INF instead of ERROR 6: tests/dos/chk_edges.asm printed
    00006600600005550 FAIL where 00066600600005550 is right."""
    source = ROOT / "tests/dos/chk_edges.asm"
    fpu = ROOT / "runtime/qb/dos/m16/fpu.asm"
    for one in (source, fpu):
        subprocess.run(["jwasm", "-q", f"-Fo{tmp_path / one.stem}.obj", f"-I{fpu.parent}", str(one)], check=True)
    exe = tmp_path / "chk_edges.exe"
    subprocess.run(
        ["jwlink", "format", "dos", "file", "chk_edges.obj", "file", "fpu.obj", "name", "chk_edges.exe"],
        check=True,
        capture_output=True,
        cwd=tmp_path,
    )
    run = tmp_path / "run"
    run.mkdir()
    result = dosbatch.run([dosbatch.Job("PROG", "exe", exe, files=())], run)["PROG"]
    assert result.text == "00066600600005550 ok\n"

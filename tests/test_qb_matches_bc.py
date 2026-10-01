"""llrm-qb against BC 4.5, both run in DOSBox: tests/qb-bc/NAME.bas prints what
tests/qb-bc/NAME.txt records (BC's own output, or for a documented divergence
llrm's) under BC's switches and llrm-qb's.

For what the matrix (tools/e2e, BC against its own rewrite) cannot hold: BC
/D's overflow checks, and the front end's own code. Needs DOSBox-X and the QB
4.5 tools; skipped, loudly, without them (LLRM_REQUIRE_BINARIES=1 fails).
"""

import os
import shutil
import warnings
import subprocess
from pathlib import Path

import pytest

from tools.e2e import dosbox
from tools.e2e.configs import QB45

ROOT = Path(__file__).resolve().parents[1]
QB = Path(os.environ.get("LLRM_QB", ROOT / "target" / "release" / "llrm-qb"))
CASES = {
    # BC's switches, llrm-qb's flags
    "overflow-ops": ("/O /E /X /D", ["-ftrapv"], True),
    "single-constants": ("/O /E /X", [], True),
    # llrm's own behaviour where BC's is its instruction selection's (docs/frontends/qb/divergences.md)
    "overflow-target-unchanged": ("/O /E /X /D", ["-ftrapv"], False),
    "sin-cos": ("/O /E /X", [], False),
}


def _unusable() -> str | None:
    if not QB.exists():
        return f"{QB} is missing"
    if not QB45.is_dir():
        return f"{QB45} (QB 4.5's tools) is missing"
    if dosbox.dosbox_bin() is None:
        return "no dosbox-x"
    return None


@pytest.mark.parametrize("name", CASES)
def test_llrm_qb_prints_what_bc_does(name, tmp_path):
    why = _unusable()
    if why:
        message = f"NOT RUN: {why}"
        if os.environ.get("LLRM_REQUIRE_BINARIES"):
            pytest.fail(message)
        warnings.warn(message, stacklevel=1)
        pytest.skip(message)
    switches, flags, same_as_bc = CASES[name]
    source = ROOT / "tests" / "qb-bc" / f"{name}.bas"
    shutil.copy(source, tmp_path / "P.BAS")
    made = subprocess.run([QB, "P.BAS", "--dialect", "qb45", "--runtime", "qb45", *flags, "-o", "Q.OBJ"], cwd=tmp_path, capture_output=True, text=True)
    assert (tmp_path / "Q.OBJ").exists(), made.stderr
    link = r"V:\LINK.EXE {0}.OBJ, {0}.EXE,, V:\LIB\BCOM45.LIB; > {0}.LNK"
    lines = [rf"V:\BC.EXE {switches} P.BAS, P.OBJ; > P.BC"]
    for one in ("P", "Q"):
        lines += [link.format(one), f"{one}.EXE > {one}.TXT"]
    assert dosbox.launch(tmp_path, QB45, lines, timeout=180).finished
    bc, llrm = (dosbox.read_dos(tmp_path, f"{one}.TXT").replace("\r", "") for one in ("P", "Q"))
    golden = (ROOT / "tests" / "qb-bc" / f"{name}.txt").read_text()
    if same_as_bc:
        assert bc == golden, "BC's output is not the golden"
    else:
        assert bc != golden, "BC now agrees: this is no divergence"
    assert llrm == golden

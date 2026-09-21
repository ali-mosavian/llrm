"""Coverage for modern-language compiler stage observation."""

import json
import runpy
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NBODY = ROOT / "frontends" / "modern" / "fixtures" / "nbody.mod"


def test_nbody_stage_dumps_cover_every_implemented_boundary(tmp_path: Path) -> None:
    """nbody used to expose HIR and MIR only through separate ad-hoc commands."""
    stages = runpy.run_path("tools/modernstages.py")
    output = stages["dumped"](NBODY, tmp_path / "nbody")

    assert [one.name for one in sorted(output.iterdir())] == [
        "00-input.mod",
        "01-tokens.txt",
        "02-syntax.txt",
        "03-hir.json",
        "04-nbody-nbody-mir.txt",
        "05-nbody-main-mir.txt",
        "README.txt",
    ]
    assert output.joinpath("00-input.mod").read_bytes() == NBODY.read_bytes()
    assert "Fixed" in output.joinpath("01-tokens.txt").read_text()
    syntax = output.joinpath("02-syntax.txt").read_text()
    assert 'Struct {\n            name: "body"' in syntax
    assert "ForRange {" in syntax
    assert json.loads(output.joinpath("03-hir.json").read_text())["schema"] == 1
    mir = output.joinpath("04-nbody-nbody-mir.txt").read_text()
    assert "function nbody.nbody" in mir
    assert "call __print_fixed_i32" in mir
    assert "mul" in mir

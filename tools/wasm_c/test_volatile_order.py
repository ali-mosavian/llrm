"""A volatile-order probe that cannot fail proves nothing: a source that reads g3, g1, g2 must be flagged, and the left-to-right
source must pass at every level,; the objects here are the pin clang's (volatile_order.py checks any object, e.g. llrm-cc's)."""
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

import pytest

import volatile_order as vo

HERE = Path(__file__).resolve().parent
CLANG = os.environ.get("LLRM_CLANG_WASM", str(Path.home() / "scratch/llvm-build-clang-wasm/bin/clang"))
pytestmark = pytest.mark.skipif(not (Path(CLANG).exists() and shutil.which("wasm-objdump")), reason="needs the pin wasm clang and wabt")


def build(source: str, level: str, out: Path) -> str:
    c = out / "v.c"
    c.write_text(source)
    obj = out / f"v{level}.o"
    subprocess.run([CLANG, "--target=wasm32", "-ffreestanding", f"-{level}", "-c", str(c), "-o", str(obj)], check=True)
    return str(obj)


def test_a_reordered_source_is_flagged():
    source = (HERE / "volatile_order.c").read_text().replace("use3(g1, g2, g3)", "use3(g3, g1, g2)")
    with tempfile.TemporaryDirectory() as out:
        assert any("f3" in line for line in vo.wrong(build(source, "O2", Path(out))))


def test_the_source_order_holds_at_every_level():
    source = (HERE / "volatile_order.c").read_text()
    with tempfile.TemporaryDirectory() as out:
        for level in ("O0", "O1", "O2", "O3", "Os", "Oz"):
            assert vo.wrong(build(source, level, Path(out))) == [], level

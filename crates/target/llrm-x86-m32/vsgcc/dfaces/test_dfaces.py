"""The scoreboard's own checks: a regression test says what went wrong and what it produced."""
import sys
from pathlib import Path

import os

import pytest

REQUIRES = ["qcport"]  # d_faces.c and its headers are QCport's, which the repository does not hold
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
import harness  # noqa: E402
import run  # noqa: E402


@pytest.fixture(scope="module")
def work(tmp_path_factory):
    """A work directory of its own, so that the products of a real run are neither used nor overwritten."""
    path = tmp_path_factory.mktemp("dfaces")
    before = (harness.OUT, run.OUT, run.SOURCE)
    harness.OUT = run.OUT = path
    run.SOURCE = path / "dfaces" / "d_faces_flat.c"
    yield path
    harness.OUT, run.OUT, run.SOURCE = before


def test_the_scene_reaches_every_path_it_was_written_to(work):
    """A scene that skips a branch scores a compiler on the rest: the reference run counts what each stand-in saw."""
    reference = run.run("gccO0")
    assert [name for name, count in reference["coverage"].items() if not count and name != "flat"] == []


def test_an_optimised_build_reports_the_hash_of_the_reference(work):
    reference = run.run("gccO0")
    for variant in ("gccO2", "clangO1"):
        assert run.agree(run.run(variant)["reports"], reference["reports"]), variant


def test_a_build_that_computes_something_else_is_caught(work, tmp_path):
    """The hash must see the difference: d_faces.c with the second mip threshold moved reports another hash than the original
    at the same level, so the scoreboard cannot score a miscompile (or a changed file) as a faster build."""
    original = run.D_FACES
    text = original.read_text().replace("zl >=  560.0", "zl >=  500.0")
    assert text != original.read_text()
    changed = tmp_path / "d_faces.c"
    changed.write_text(text)
    run.D_FACES = changed
    try:
        different = run.run("gccO1")["reports"]
    finally:
        run.D_FACES = original
    assert not run.agree(different, run.run("gccO1")["reports"])


@pytest.mark.skipif("LLRM_CLANG_M16" not in os.environ, reason="needs LLRM_CLANG_M16")
def test_the_16_bit_build_does_what_the_reference_does(work):
    """llrm -m16 is held to what gcc and clang do in 32 bits, so its answer is judged against the flat reference. Its first run
    differed from every flat build, and the cause was the scene: a face whose `side >> 1` named a fourth brush model that the
    scene had not made, so the offset read was whatever followed the array, and that differs between a flat and a 16-bit layout."""
    pytest.importorskip("unicorn")
    reference = run.run("gccO0")
    small = run.run_m16("O2")
    assert run.agree(small["reports"], reference["reports"]), (small["reports"], reference["reports"])
    assert small["ins"] > 0

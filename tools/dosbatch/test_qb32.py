"""The 25 bench programs on the flat 32-bit target (DOS/32A) with the llrm QB runtime: each prints what bench/NAME/NAME.out says."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import corpus  # noqa: E402
import qb32  # noqa: E402
import qbruntime  # noqa: E402


@pytest.fixture(scope="module")
def found(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    return qb32.run(list(qbruntime.MILESTONE_ONE), tmp_path_factory.mktemp("qb32"))


@pytest.mark.parametrize("name", qbruntime.MILESTONE_ONE)
def test_bench_program_prints_what_it_should_on_dos32(found, name: str):
    status, same = found[name]
    if name == "grep":
        try:
            corpus.path("dickens")
        except corpus.Unavailable as error:
            pytest.skip(str(error))
    assert status == "ok", status
    assert same


# tests/qbrt probes whose redirected bytes are BCOM45's on dos32. Each name once broke: STRING$ and LEFT$ printed
# nothing (descriptor pointer cut to 4 bits), a bare PRINT after a loop killed the program (descriptor address 0),
# fixed-length string fields read the wrong registers (parameter order of B$ASSN, B$LDFS, B$GET4, B$PUT4).
PROBES = [
    "arrays_dynamic", "divide_by_zero", "func_string", "on_error", "on_error_label", "on_error_resume",
    "print_items", "print_terminators", "read_data", "str_slices", "string_convert", "trim_and_str",
    "using_numbers",
]


@pytest.fixture(scope="module")
def probed(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    return qb32.probes(PROBES, tmp_path_factory.mktemp("qb32probes"))


@pytest.mark.parametrize("name", PROBES)
def test_probe_prints_what_bcom45_prints_on_dos32(probed, name: str):
    assert probed[name] == ""


# NIBBLES and GORILLA on dos32, played with the keys of test_qbdemos and compared with BCOM45's screens.
@pytest.fixture(scope="module")
def demo_pairs(tmp_path_factory):
    found, reason = qbruntime.demo_sources()
    if not found or not qbruntime.dosbatch.QB45.is_dir():
        pytest.skip(reason or "QB45_DIR is unavailable")
    work = tmp_path_factory.mktemp("qb32demos")
    sources = {path.stem.upper(): path for path in found.values()}
    import qbplay  # noqa: E402

    (work / "ref").mkdir()
    reference = qbplay.built_pairs(sources, work / "ref")
    runtime = qb32.build(work / "runtime")
    pairs = {}
    for name, source in sources.items():
        obj = work / f"{name}.obj"
        assert (reason := qb32.compile_basic(source, obj)) is None, reason
        exe = work / f"{name}.exe"
        loaders = qb32.link(obj, runtime, exe, work)
        pairs[name] = (reference[name][0], exe, loaders)
    return pairs


@pytest.mark.parametrize("name", ["NIBBLES", "GORILLA"])
def test_demo_shows_what_bcom45_shows_on_dos32(demo_pairs, tmp_path, name: str):
    import shutil

    import test_qbdemos  # noqa: E402

    reference, candidate, loaders = demo_pairs[name]
    want = test_qbdemos.play(reference, tmp_path / "ref", test_qbdemos.SCRIPTS[name])
    (tmp_path / "cand").mkdir()
    for loader in loaders:
        shutil.copy(loader, tmp_path / "cand" / loader.name)
    got = test_qbdemos.play(candidate, tmp_path / "cand", test_qbdemos.SCRIPTS[name])
    import qbplay  # noqa: E402

    for checkpoint in want:
        assert qbplay.differing_pixels(want[checkpoint], got[checkpoint]) <= test_qbdemos.CURSOR_PIXELS, checkpoint

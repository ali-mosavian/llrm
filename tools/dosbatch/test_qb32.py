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
    "arrays_dynamic", "divide_by_zero", "func_string", "gfx_sun", "on_error", "on_error_label", "on_error_resume",
    "array_bounds", "print_items", "print_terminators", "varptr_peek", "read_data", "str_slices", "string_convert", "timer_sane", "trim_and_str",
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


# Played on: NIBBLES's snake runs into the wall (the delay loop is calibrated on TIMER, which once ran backwards on
# dos32 and sent the snake across the arena in two seconds), and GORILLA's players throw bananas (the font of
# the graphics screen once came back from a protected-mode interrupt as noise). The skyline, the sparkles and
# where the number lies are random, so a screen is compared where nothing is: the death message, and the
# top rows' prompts of GORILLA.
PLAYED = {
    "NIBBLES": (
        [(["space"], 10, "dies", None)],
        {"dies": (0, 0, 720, 400)},
    ),
    "GORILLA": (
        [
            (["p"], 5, "prompt1", None),
            (["4", "5", "enter"], 1, "prompt2", None),
            (["5", "0", "enter"], 8, "prompt3", None),
        ],
        {"prompt1": (0, 0, 150, 30), "prompt2": (400, 0, 640, 30), "prompt3": (0, 0, 150, 30)},
    ),
}


def region_difference(want: Path, got: Path, box: tuple[int, int, int, int]) -> int:
    import qbplay  # noqa: E402

    (width, _, left), (_, _, right) = qbplay.png_pixels(want), qbplay.png_pixels(got)
    scale = width // 640
    x0, y0, x1, y1 = (v * scale for v in box)
    step = 3 if len(left) == width * _ * 3 else 4
    return sum(
        left[(y * width + x) * step : (y * width + x) * step + 3] != right[(y * width + x) * step : (y * width + x) * step + 3]
        for y in range(y0, y1)
        for x in range(x0, x1)
    )


@pytest.mark.parametrize("name", ["NIBBLES", "GORILLA"])
def test_demo_played_looks_as_bcom45_does_on_dos32(demo_pairs, tmp_path, name: str):
    import shutil

    import test_qbdemos  # noqa: E402

    reference, candidate, loaders = demo_pairs[name]
    script = test_qbdemos.SCRIPTS[name] + PLAYED[name][0]
    want = test_qbdemos.play(reference, tmp_path / "ref", script)
    (tmp_path / "cand").mkdir()
    for loader in loaders:
        shutil.copy(loader, tmp_path / "cand" / loader.name)
    got = test_qbdemos.play(candidate, tmp_path / "cand", script)
    for checkpoint, box in PLAYED[name][1].items():
        assert region_difference(want[checkpoint], got[checkpoint], box) <= test_qbdemos.CURSOR_PIXELS, checkpoint


FLAT_PROGRAMS = ["long_strings"]


@pytest.fixture(scope="module")
def flat_found(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    return qb32.flat_programs(FLAT_PROGRAMS, tmp_path_factory.mktemp("qb32flat"))


@pytest.mark.parametrize("name", FLAT_PROGRAMS)
def test_flat_only_program_prints_what_it_should(flat_found, name: str):
    assert flat_found[name] == ""

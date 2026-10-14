"""Nib on the flat 32-bit target (DOS/32A): each bench program with a Nib source that has a flat build prints what bench/NAME/NAME.out
says, files it reads supplied. The programs whose header leaves the flat target out are named with the reason."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import corpus  # noqa: E402
import dosbatch  # noqa: E402
import nib32  # noqa: E402

# One of each kind: integer loops, recursion, an array, floats (x87), a fixed-point kernel, and the two that read a file.
SAMPLE = ["sieve", "fib", "queens", "shellsort", "nbody_single", "stft_fixed", "jpeg_single"]


@pytest.fixture(scope="module")
def found(tmp_path_factory):
    if not dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    return nib32.run(SAMPLE, tmp_path_factory.mktemp("nib32"))


@pytest.mark.parametrize("name", SAMPLE)
def test_a_bench_program_prints_what_it_should_on_the_flat_target(found, name: str):
    status, same = found[name]
    assert status == "ok", status
    assert same, f"{name} printed something else than bench/{name}/{name}.out"


def test_a_program_whose_header_leaves_the_flat_target_out_says_why():
    sources = nib32.programs()
    left_out = {name for name, source in sources.items() if nib32.flat(source)}
    assert left_out == {"grep", "huge", "lru"}, left_out
    assert "x86-m16" in nib32.flat(sources["grep"])
    assert nib32.flat(sources["sieve"]) is None


def test_the_flat_flags_are_the_second_set_of_the_header():
    sources = nib32.programs()
    assert nib32.flags(sources["jpeg_single"]) == ["-O2", "-march=i486", "-m32"]
    assert nib32.flags(sources["sieve"]) == ["-O2"], "a header with no flags: -O2"


def test_the_files_a_program_reads_are_found():
    sources = nib32.programs()
    assert [one.name for one in nib32.data_files(sources["jpeg_single"])] == ["photo.jpg"]
    assert nib32.data_files(sources["sieve"]) == ()
    try:
        signal = nib32.data_files(sources["stft_single"])
    except corpus.Unavailable:
        pytest.skip("the signal corpus is unavailable")
    assert [one.name for one in signal] == ["signal"]


def test_a_program_that_prints_something_else_is_reported_as_different(tmp_path, monkeypatch):
    """The comparison is not a pass for anything that ran: sieve against another program's .out."""
    if not dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    sources = nib32.programs()
    wrong = tmp_path / "sieve" / "sieve.nib"
    wrong.parent.mkdir()
    wrong.write_text(sources["sieve"].read_text().replace("print(", "print(1 + ", 1))
    (wrong.parent / "sieve.out").write_text((sources["sieve"].parent / "sieve.out").read_text())
    monkeypatch.setattr(nib32, "programs", lambda: {"sieve": wrong})
    status, same = nib32.run(["sieve"], tmp_path / "work")["sieve"]
    assert not same, status

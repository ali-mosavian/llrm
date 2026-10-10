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

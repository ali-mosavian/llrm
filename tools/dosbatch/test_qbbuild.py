"""qbbuild: a BASIC program on our runtime, linked by jwlink for both targets.

MID$ as a statement is B$SMID, assembled from smid.asm: its call of a C routine was written in the pascal language and so
upper-cased (`_STR_TMP_FREE@3`), which Microsoft's LINK takes and jwlink, which is case-sensitive, does not: no real-mode
program that assigns to MID$ linked."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402
import qbbuild  # noqa: E402

pytestmark = pytest.mark.slow_dos


def test_a_program_that_assigns_to_mid_links_and_runs_on_both_targets(tmp_path):
    if not dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    source = tmp_path / "MIDS.BAS"
    source.write_text('a$ = "hello"\nMID$(a$, 2, 1) = "E"\nPRINT a$; LEN(SPACE$(3))\nEND\n')
    built = qbbuild.build(source, tmp_path / "b", qbbuild.runtimes(tmp_path))
    for target in qbbuild.TARGETS:
        one = built[target]
        result = dosbatch.run([dosbatch.Job("J", "exe", one.exe, files=one.files)], tmp_path / f"run{target}", budget_ms=3000)["J"]
        assert result.status == "ok", f"{target}: {result.detail}"
        assert result.text.split() == ["hEllo", "3"], f"{target}: {result.text!r}"

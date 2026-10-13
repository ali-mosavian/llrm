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


def test_a_program_with_files_but_no_bload_does_not_link_bload_or_def_seg(tmp_path):
    """BLOAD in file.c made every program that opens a file link DEF SEG's object too (b$seg): a kilo-instruction start-up cost in
    every bq32 row that has an array (matmul +1,043).  BLOAD and BSAVE are an object of their own."""
    source = tmp_path / "FILES.BAS"
    source.write_text('OPEN "F.TXT" FOR OUTPUT AS #1\nPRINT #1, "x"\nCLOSE #1\nEND\n')
    objects = qbbuild.runtimes(tmp_path)
    for target in qbbuild.TARGETS:
        (tmp_path / f"b{target}").mkdir()
        built = qbbuild.one_target(source, tmp_path / f"b{target}", objects, ("-O2",), target)
        obj = tmp_path / f"b{target}" / f"FILES.{target}.obj"
        linked = {one.stem for one in qbbuild.needed((obj,), objects[target])}
        assert "file" in linked and not {"bload", "defseg"} & linked, f"{target}: {sorted(linked)}"
        assert built.exe.exists()


def test_a_basic_program_linking_a_nib_library_is_refused_at_the_flat_target_by_saying_so(tmp_path):
    """The library's build stopped in the compiler on "this target defines no regparm3 calling convention", out of the BASIC adapters;
    llrm-nib now refuses a library for BASIC at a flat target (#1542)."""
    source = dosbatch.ROOT / "examples" / "basic" / "scores.bas"
    objects = qbbuild.runtimes(tmp_path)
    (tmp_path / "b").mkdir()
    with pytest.raises(dosbatch.BuildError, match="a library for BASIC is not supported at a flat target"):
        qbbuild.one_target(source, tmp_path / "b", objects, ("-O2",), qbbuild.FLAT)

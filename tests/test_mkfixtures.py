"""
tools/mkfixtures.py's manifest, which is the provenance of the corpus.
"""

import sys
from pathlib import Path

sys.path.insert(0, "tools")

import mkfixtures


def test_a_partial_rebuild_keeps_the_rows_it_did_not_build(tmp_path: Path) -> None:
    """`--prog X` builds a slice and used to write only that slice.

    The objects stayed on disk and the record of which compiler made them,
    on which switches, did not. Adding suite/fpdeep.bas silently dropped all
    fifteen fpemu rows that way, and a manifest is the only thing that says
    where a fixture came from.
    """
    kept = mkfixtures.MANIFEST.read_text()
    rows = [line for line in kept.splitlines()[1:] if line.strip()]
    assert rows, "the manifest is empty, so this proves nothing"

    names = {line.split("\t")[0] for line in rows}
    on_disk = {p.name for p in mkfixtures.FIXTURES.glob("*.obj")}
    assert names <= on_disk, "the manifest names an object that is not there"

    # every program with objects in the corpus has rows describing them
    from collections import Counter

    by_source = Counter(line.split("\t")[2] for line in rows)
    assert len(by_source) > 2, f"only {len(by_source)} sources recorded -- a partial run has overwritten the rest"


def test_the_manifest_merge_is_keyed_on_the_file_name() -> None:
    """A rebuilt object replaces its own row and nothing else's."""
    import inspect

    source = inspect.getsource(mkfixtures.write_manifest)
    assert "_existing()" in source, "a partial run must read what is already recorded"
    assert "kept.update" in source, "and merge rather than replace"

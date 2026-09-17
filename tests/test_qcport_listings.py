"""The QCport listing harness must reject evidence from changed source."""

from pathlib import Path

import pytest

from tools import qcport_listings


def test_clean_revision_refuses_dirty_qcport_source(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """The old loop audit compared executables with a later-edited source tree.

    A changed header or C file can move every loop, so a listing manifest must
    fail before it records a compiler result whose input cannot be reproduced.
    """
    monkeypatch.setattr(qcport_listings, "git", lambda _root, *_args: " M src/render/r_walk.c\n")

    with pytest.raises(RuntimeError, match="source tree is dirty"):
        qcport_listings.clean_revision(tmp_path)


def test_clean_revision_names_a_dirty_qbopt_checkout(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """A listing made from uncommitted lowering named the previous commit.

    That made the RMW experiment look reproducible at a revision that did not
    contain it.  The harness must refuse that evidence rather than record a
    false qbopt revision in its manifest.
    """
    monkeypatch.setattr(qcport_listings, "git", lambda _root, *_args: " M qbopt/backend/lower.py\n")

    with pytest.raises(RuntimeError, match="qbopt checkout is dirty"):
        qcport_listings.clean_revision(tmp_path, "qbopt checkout")


def test_source_path_rejects_a_path_outside_the_clean_worktree(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="escapes QCport root"):
        qcport_listings.source_path(tmp_path, "../r_walk.c")

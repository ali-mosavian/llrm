import sys
import subprocess
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import clangpin  # noqa: E402


def fake(path: Path, line: str) -> str:
    path.write_text(f'#!/bin/sh\necho "{line}"\n')
    path.chmod(0o755)
    return str(path)


def test_a_clang_other_than_the_pinned_one_is_refused_unless_named(tmp_path, monkeypatch):
    """The per-pass table's clang column came from the distro's clang 20.1.8 while the plan's reference is the pinned 24.0.0git: nothing said so."""
    pinned = fake(tmp_path / "pinned", "clang version 24.0.0git (d1106deb71cc)")
    other = fake(tmp_path / "other", "Ubuntu clang version 20.1.8")
    monkeypatch.setattr(clangpin, "PINNED", Path(pinned))
    with pytest.raises(SystemExit, match="20.1.8.*not the pinned.*24.0.0git"):
        clangpin.require_pinned(other)
    clangpin.require_pinned(other, allow_other=True)
    clangpin.require_pinned(pinned)


def test_the_header_names_the_clang_and_its_version_and_the_default_is_the_pinned_one(tmp_path, monkeypatch):
    pinned = fake(tmp_path / "pinned", "clang version 24.0.0git (d1106deb71cc)")
    monkeypatch.setattr(clangpin, "PINNED", Path(pinned))
    assert clangpin.default() == pinned
    assert clangpin.header(pinned) == f"clang: {pinned} (clang version 24.0.0git (d1106deb71cc))"


def test_the_tools_that_set_llrm_beside_clang_take_the_pinned_one(monkeypatch):
    """compile-gap and compile_map ran `clang` from PATH: the distro's 20.1.8 (555 M on d_faces -O2) where the pinned 24.0.0git costs 415 M."""
    import importlib.util

    import compile_map

    assert compile_map.CLANG_BIN == clangpin.default()
    spec = importlib.util.spec_from_file_location("compile_gap", Path(__file__).parent / "compile-gap.py")
    source = Path(spec.origin).read_text()
    assert "default=clangpin.default()" in source and "clangpin.require_pinned(args.clang" in source

"""The OS layer's directory is this tree's, whichever tree built the compiler."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402

TARGET = dosbatch.linkrecipe.named(32)


def test_the_directory_a_compiler_built_in_another_tree_names_is_read_from_this_tree(monkeypatch):
    """A gate of main read a compiler the tip's gate had built in a shared target: its directory named train-tree, and `.relative_to(ROOT)`
    failed in 14 test modules at collection."""
    said = {"directory": "/elsewhere/train-tree/runtime/shared/dos/m32", "start": "start.asm"}
    monkeypatch.setattr(dosbatch.subprocess, "run", lambda cmd, **kw: subprocess.CompletedProcess(cmd, 0, said[cmd[-1]] + "\n", ""))
    assert dosbatch.os_layer(TARGET, "directory", "c") == str(dosbatch.ROOT / "runtime/shared/dos/m32")
    assert dosbatch.os_layer(TARGET, "start", "c") == "start.asm", "the other fields are the compiler's own"
    assert Path(dosbatch.os_layer(TARGET, "directory", "c")).relative_to(dosbatch.ROOT)

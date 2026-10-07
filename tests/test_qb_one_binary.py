"""llrm-qb is one compiler: with its qbfront linked in, a copy of the binary
compiles BASIC with no cargo and no source tree at hand (#175: it ran `cargo
run` on the tree it was built in, so a copy compiled with whatever that tree
held now, or failed when the tree's HIR schema had moved).

Needs target/release/llrm-qb; skipped, loudly, without it (LLRM_REQUIRE_BINARIES=1
fails).
"""

import os
import shutil
import warnings
import subprocess
from pathlib import Path

import pytest

from tools import llrmbin

BIN = Path(os.environ["LLRM_QB"]) if os.environ.get("LLRM_QB") else llrmbin.bin_dir() / "llrm-qb"


def test_a_copied_llrm_qb_compiles_with_no_cargo_and_no_tree(tmp_path):
    if not BIN.exists():
        message = f"NOT RUN: {BIN} is missing"
        if os.environ.get("LLRM_REQUIRE_BINARIES"):
            pytest.fail(message)
        warnings.warn(message, stacklevel=1)
        pytest.skip(message)
    copy = tmp_path / "llrm-qb"
    shutil.copy(BIN, copy)
    (tmp_path / "p.bas").write_text('PRINT "hi"\n')
    # No PATH: nothing to start cargo with; cwd is not the repo.
    done = subprocess.run([str(copy), "p.bas", "-o", "p.obj"], cwd=tmp_path, env={"PATH": ""}, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    assert (tmp_path / "p.obj").stat().st_size > 0

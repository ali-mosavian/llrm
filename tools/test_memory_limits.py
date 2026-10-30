"""Every script that compiles or runs programs in bulk caps its address space."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent


def test_the_bulk_compile_scripts_set_a_memory_limit():
    """A compile under the translated JumpThreading grew to 50 GB before anything stopped it. identity.sh and the rows scripts are shell: they carry the limit themselves."""
    for name in ("identity.sh", "rows/run.sh", "rows/row.sh"):
        assert "ulimit -v" in (ROOT / name).read_text(), name

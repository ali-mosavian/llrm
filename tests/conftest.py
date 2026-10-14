"""The tests' scratch space is on disk, not pytest's default under /tmp (a tmpfs). The compilers refuse a stage dump on tmpfs
(#1641), and the loop harness dumps the stages of every case it compiles."""

import sys
import shutil
import tempfile
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import llrmbin  # noqa: E402


@pytest.fixture
def tmp_path(request):
    base = llrmbin.target_dir() / "pytest-tmp"
    base.mkdir(parents=True, exist_ok=True)
    path = Path(tempfile.mkdtemp(prefix=f"{request.node.name[:40]}-", dir=base))
    yield path
    shutil.rmtree(path, ignore_errors=True)

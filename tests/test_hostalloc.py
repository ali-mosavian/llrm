"""The string allocator (runtime/qb/nhstutil.c) under random operations on the host: live strings keep their bytes, the
entries tile string space, and the heap trade and compaction leave both true."""

import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent


@pytest.mark.skipif(shutil.which("gcc") is None, reason="needs a host C compiler")
def test_string_allocator_survives_random_operations(tmp_path):
    exe = tmp_path / "alloc_test"
    build = subprocess.run(
        ["gcc", "-O1", "-g", "-I", str(ROOT / "tests/hostalloc/shim"), "-I", str(ROOT / "runtime/qb"), str(ROOT / "tests/hostalloc/alloc_test.c"), "-o", str(exe)],
        capture_output=True,
        text=True,
    )
    assert build.returncode == 0, build.stderr[-600:]
    for seed in range(1, 6):
        done = subprocess.run([str(exe), str(seed)], capture_output=True, text=True)
        assert done.returncode == 0 and done.stdout.strip() == "ok", f"seed {seed}: {done.stderr[-300:]}"

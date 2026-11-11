"""m16os called `llrm-cc`, dropped 10-26: a dist with only llrm-cc crashed the header (FileNotFoundError) and its listings ran llrm-cc without its environment."""
from __future__ import annotations

import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def shim(path: Path, body: str) -> Path:
    path.write_text(f"#!/bin/bash\n{body}\n")
    path.chmod(0o755)
    return path


def test_a_dist_with_only_llrm_cc_is_named(tmp_path):
    dist = tmp_path / "sbp-0123456789abcdef0123456789abcdef01234567" / "dist"
    dist.mkdir(parents=True)
    shim(dist / "llrm-cc", "echo 'llrm-cc 89abcdef89abcdef89abcdef89abcdef89abcdef'")
    shim(dist / "llrm-qb", "true")
    header = load("m16os").identify(dist)
    assert "89abcdef89abcdef89abcdef89abcdef89abcdef" in header, header
    assert "llrm-cc " in header, header


def test_a_dist_without_a_build_stamp_is_named_by_its_directory(tmp_path):
    dist = tmp_path / "sbp-0123456789abcdef0123456789abcdef01234567" / "dist"
    dist.mkdir(parents=True)
    shim(dist / "llrm-cc", "exit 1")
    assert "0123456789abcdef0123456789abcdef01234567 (from the directory name" in load("m16os").identify(dist)


def test_a_listing_runs_the_compiler_in_its_environment(tmp_path):
    cc = shim(tmp_path / "llrm-cc", 'test "$PROBE" = 1 || exit 3; while [ "$1" != -o ]; do shift; done; printf "k proc far\\n    ret\\nk endp\\n" > "$2"')
    assert load("m16os").listing([str(cc), "x.c"], "-Os", {"PROBE": "1"}) == {"k": 1}

"""A count from an incremental or a release build moves by more than the changes measured: the tools that count refuse such a binary
(`--allow-dev` measures anyway; the gate's measure warns and records it)."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402

DIST = b"llrm-build:profile=dist;incremental=0"
RELEASE = b"llrm-build:profile=release;incremental=1"


def binary(tmp_path: Path, mark: bytes) -> Path:
    path = tmp_path / "llrm-c"
    path.write_bytes(b"\x7fELF" + b"\0" * 64 + mark + b"\0" + b"more" * 64)
    return path


def test_a_binary_says_its_profile_and_whether_it_is_incremental(tmp_path):
    assert llrmbin.build_info(binary(tmp_path, DIST)) == {"profile": "dist", "incremental": False}
    assert llrmbin.build_info(binary(tmp_path, RELEASE)) == {"profile": "release", "incremental": True}
    assert llrmbin.build_info(binary(tmp_path, b"nothing about it")) is None


def test_only_the_dist_profile_without_incremental_is_accepted(tmp_path):
    assert llrmbin.require_clean(binary(tmp_path, DIST), env={}) == {"profile": "dist", "incremental": False}
    for mark in (RELEASE, b"llrm-build:profile=release;incremental=0", b"llrm-build:profile=dist;incremental=1", b"nothing about it"):
        with pytest.raises(SystemExit, match="refusing to measure"):
            llrmbin.require_clean(binary(tmp_path, mark), env={})


def test_the_override_measures_with_a_warning_and_the_gate_warns_and_records(tmp_path, capsys):
    path = binary(tmp_path, RELEASE)
    assert llrmbin.require_clean(path, env={}, allow_dev=True)["profile"] == "release"
    assert "WARNING" in capsys.readouterr().err
    assert llrmbin.require_clean(path, env={"LLRM_ALLOW_DEV": "1"})["incremental"] is True
    assert "WARNING" in capsys.readouterr().err
    assert llrmbin.require_clean(path, env={}, strict=False)["profile"] == "release"
    assert "WARNING" in capsys.readouterr().err


@pytest.mark.parametrize("tool", ["pass_budgets.py", "trace_corpus.py"])
def test_the_counting_tools_refuse_a_release_binary_unless_told_not_to(tmp_path, tool):
    """They accepted any binary: a delta of 0.1% between two incremental builds was inlining, not the change."""
    fake = binary(tmp_path, RELEASE)
    args = [sys.executable, str(HERE / tool)] + (["--llrm", str(fake)] if tool == "pass_budgets.py" else [str(tmp_path / "t.jsonl"), "--llrm", str(fake)])
    done = subprocess.run(args, capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
    assert done.returncode != 0 and "refusing to measure" in (done.stderr + done.stdout), done.stderr
    # With the override it gets as far as running the (fake) compiler, which is no program.
    again = subprocess.run([*args, "--allow-dev"], capture_output=True, text=True, env={"PATH": "/usr/bin:/bin"})
    assert "refusing to measure" not in again.stderr, again.stderr


def test_the_compiler_in_this_tree_says_how_it_was_built():
    """`llrm-c --build-info` prints the mark the tools read."""
    done = subprocess.run([str(llrmbin.bin_dir() / "llrm-c"), "--build-info"], capture_output=True, text=True)
    assert llrmbin.BUILD_MARK.search(done.stdout.encode()), done.stdout + done.stderr

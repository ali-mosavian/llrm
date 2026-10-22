"""A binary built from sha A while the command names sha B is refused (stub binaries, no build)."""
import os
import stat
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
import stamp  # noqa: E402


def stub(tmp_path, name, line, code=0):
    path = tmp_path / name
    path.write_text(f"#!/bin/sh\necho '{line}'\nexit {code}\n")
    path.chmod(path.stat().st_mode | stat.S_IEXEC)
    return path


def test_a_binary_built_from_another_sha_is_refused(tmp_path):
    """The 65eed2ad7 dist that was another tree's: its stamp said so and nothing looked."""
    binary = stub(tmp_path, "llrm-c", "llrm-c aaaa1111")
    assert stamp.check(binary, "aaaa1111") == "aaaa1111"
    assert stamp.check(binary, "aaaa") == "aaaa1111"
    with pytest.raises(SystemExit, match="built from aaaa1111, not bbbb2222"):
        stamp.check(binary, "bbbb2222")


def test_a_dirty_tree_is_refused_unless_allowed(tmp_path):
    binary = stub(tmp_path, "llrm-c", "llrm-c aaaa1111+dirty")
    with pytest.raises(SystemExit, match="with changes"):
        stamp.check(binary, "aaaa1111")
    assert stamp.check(binary, "aaaa1111", allow_dirty=True) == "aaaa1111"


def test_a_binary_without_a_stamp_is_refused(tmp_path):
    with pytest.raises(SystemExit, match="no build stamp"):
        stamp.check(stub(tmp_path, "llrm-c", "usage: llrm-c [-h]", code=2))


def test_the_options_are_taken_from_the_line_and_the_environment(monkeypatch):
    for name in ("LLRM_EXPECT_SHA", "LLRM_EXPECT_BASE_SHA", "LLRM_ALLOW_DIRTY"):
        monkeypatch.delenv(name, raising=False)
    rest, named, allow = stamp.options(["-O2", "--expect-sha", "abc", "--expect-base-sha=def", "--allow-dirty", "x"])
    assert (rest, named, allow) == (["-O2", "x"], {"sha": "abc", "base_sha": "def"}, True)
    monkeypatch.setenv("LLRM_EXPECT_SHA", "ghi")
    assert stamp.options([]) == ([], {"sha": "ghi", "base_sha": None}, False)

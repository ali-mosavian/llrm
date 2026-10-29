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
    binary = stub(tmp_path, "llrm-llc", "llrm-llc aaaa1111")
    assert stamp.check(binary, "aaaa1111") == "aaaa1111"
    assert stamp.check(binary, "aaaa") == "aaaa1111"
    with pytest.raises(SystemExit, match="built from aaaa1111, not bbbb2222"):
        stamp.check(binary, "bbbb2222")


def test_a_dirty_tree_is_refused_unless_allowed(tmp_path):
    binary = stub(tmp_path, "llrm-llc", "llrm-llc aaaa1111+dirty")
    with pytest.raises(SystemExit, match="with changes"):
        stamp.check(binary, "aaaa1111")
    assert stamp.check(binary, "aaaa1111", allow_dirty=True) == "aaaa1111"


def test_a_binary_without_a_stamp_is_refused(tmp_path):
    with pytest.raises(SystemExit, match="no build stamp"):
        stamp.check(stub(tmp_path, "llrm-llc", "usage: llrm-llc [-h]", code=2))


def test_the_options_are_taken_from_the_line_and_the_environment(monkeypatch):
    for name in ("LLRM_EXPECT_SHA", "LLRM_EXPECT_BASE_SHA", "LLRM_ALLOW_DIRTY"):
        monkeypatch.delenv(name, raising=False)
    rest, named, allow = stamp.options(["-O2", "--expect-sha", "abc", "--expect-base-sha=def", "--allow-dirty", "x"])
    assert (rest, named, allow) == (["-O2", "x"], {"sha": "abc", "base_sha": "def"}, True)
    monkeypatch.setenv("LLRM_EXPECT_SHA", "ghi")
    assert stamp.options([]) == ([], {"sha": "ghi", "base_sha": None}, False)


def test_a_binary_overwritten_after_its_ok_is_refused(tmp_path):
    """w-1's sbp-65eed2ad7b dist: another session copied a different llrm-c over it after the build; md5 01fdf3625b2a became ecbc4f3de389."""
    import hashlib

    dist = tmp_path / "sbp-aaaa1111" / "dist"
    dist.mkdir(parents=True)
    binary = stub(dist, "llrm-llc", "llrm-llc aaaa1111")
    (tmp_path / "sbp-aaaa1111" / "ok").write_text(f"sha: aaaa1111\n{hashlib.md5(binary.read_bytes()).hexdigest()}  llrm-llc\n")
    assert stamp.check(binary, "aaaa1111") == "aaaa1111"
    binary.write_text("#!/bin/sh\necho 'llrm-llc aaaa1111'\n# another build\n")
    with pytest.raises(SystemExit, match="recorded when it was built"):
        stamp.check(binary, "aaaa1111")


def test_an_llrm_cc_is_stamped_by_the_llrm_llc_beside_it(tmp_path):
    """`llrm-cc --version` is the pinned clang's, so objsweep refused every llrm-cc ("no build stamp") and the m64 sweep of the object writers could not start."""
    cc = stub(tmp_path, "llrm-cc", "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc)")
    stub(tmp_path, "llrm-llc", "llrm-llc aaaa1111")
    assert stamp.check(cc, "aaaa1111") == "aaaa1111"


def test_an_incremental_build_is_refused_for_a_count_unless_allowed(tmp_path, monkeypatch):
    """equal `object write` read 10.942 Minstr on an incremental build of a commit and 9.471 on the cost gate's CARGO_INCREMENTAL=0 build of the same one (-13%):
    budgets.toml rows measured on a dev tree's build were baselined on the wrong binary and #5744 was refused three times for it."""
    monkeypatch.delenv("LLRM_ALLOW_INCREMENTAL", raising=False)
    cold = stub(tmp_path, "llrm-llc", "llrm-llc aaaa1111")
    warm = stub(tmp_path, "llrm-qb", "llrm-qb aaaa1111+dirty+incremental")
    stamp.refuse_incremental(cold)
    assert stamp.read(warm) == ("aaaa1111", True)
    with pytest.raises(SystemExit, match="built from aaaa1111 incrementally"):
        stamp.refuse_incremental(warm)
    monkeypatch.setenv("LLRM_ALLOW_INCREMENTAL", "1")
    stamp.refuse_incremental(warm)

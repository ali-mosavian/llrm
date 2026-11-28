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
    binary = stub(tmp_path, "llrm-qb", "llrm-qb aaaa1111")
    assert stamp.check(binary, "aaaa1111") == "aaaa1111"
    assert stamp.check(binary, "aaaa") == "aaaa1111"
    with pytest.raises(SystemExit, match="built from aaaa1111, not bbbb2222"):
        stamp.check(binary, "bbbb2222")


def test_a_dirty_tree_is_refused_unless_allowed(tmp_path):
    binary = stub(tmp_path, "llrm-qb", "llrm-qb aaaa1111+dirty")
    with pytest.raises(SystemExit, match="with changes"):
        stamp.check(binary, "aaaa1111")
    assert stamp.check(binary, "aaaa1111", allow_dirty=True) == "aaaa1111"


def test_a_binary_without_a_stamp_is_refused(tmp_path):
    with pytest.raises(SystemExit, match="no build stamp"):
        stamp.check(stub(tmp_path, "llrm-qb", "usage: llrm-qb [-h]", code=2))


def test_the_options_are_taken_from_the_line_and_the_environment(monkeypatch):
    for name in ("LLRM_EXPECT_SHA", "LLRM_EXPECT_BASE_SHA", "LLRM_ALLOW_DIRTY"):
        monkeypatch.delenv(name, raising=False)
    rest, named, allow = stamp.options(["-O2", "--expect-sha", "abc", "--expect-base-sha=def", "--allow-dirty", "x"])
    assert (rest, named, allow) == (["-O2", "x"], {"sha": "abc", "base_sha": "def"}, True)
    monkeypatch.setenv("LLRM_EXPECT_SHA", "ghi")
    assert stamp.options([]) == ([], {"sha": "ghi", "base_sha": None}, False)


def test_a_binary_overwritten_after_its_ok_is_refused(tmp_path):
    """w-1's sbp-65eed2ad7b dist: another session copied a different llrm-cc over it after the build; md5 01fdf3625b2a became ecbc4f3de389."""
    import hashlib

    dist = tmp_path / "sbp-aaaa1111" / "dist"
    dist.mkdir(parents=True)
    binary = stub(dist, "llrm-qb", "llrm-qb aaaa1111")
    (tmp_path / "sbp-aaaa1111" / "ok").write_text(f"sha: aaaa1111\n{hashlib.md5(binary.read_bytes()).hexdigest()}  llrm-qb\n")
    assert stamp.check(binary, "aaaa1111") == "aaaa1111"
    binary.write_text("#!/bin/sh\necho 'llrm-qb aaaa1111'\n# another build\n")
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


def dist(tmp_path, name, **stamps):
    """A dist directory of stub compilers, each printing its stamp."""
    folder = tmp_path / name
    folder.mkdir()
    for binary, line in stamps.items():
        stub(folder, binary.replace("_", "-"), f"{binary.replace('_', '-')} {line}")
    return folder


def test_a_dist_is_refused_unless_every_compiler_is_the_sha_clean(tmp_path):
    """ab-rs7 stood for b6541d3901 in every b6541 A/B and was e83149f+dirty: a dist is the sha only when each compiler in it says so."""
    assert stamp.require_clean(dist(tmp_path, "ok", llrm_llc="aaaa1111", llrm_nib="aaaa1111"), "aaaa1111") == "aaaa1111"
    with pytest.raises(SystemExit, match="llrm-nib.*with changes"):
        stamp.require_clean(dist(tmp_path, "dirty", llrm_llc="aaaa1111", llrm_nib="aaaa1111+dirty"), "aaaa1111")
    with pytest.raises(SystemExit, match="llrm-qb was built from bbbb2222, not aaaa1111"):
        stamp.require_clean(dist(tmp_path, "other", llrm_llc="aaaa1111", llrm_qb="bbbb2222"), "aaaa1111")
    with pytest.raises(SystemExit, match="llrm-m68k-llc was built from bbbb2222, not aaaa1111"):
        stamp.require_clean(dist(tmp_path, "other_llc", llrm_llc="aaaa1111", llrm_m68k_llc="bbbb2222"), "aaaa1111")
    with pytest.raises(SystemExit, match="no stamped compiler"):
        stamp.require_clean(dist(tmp_path, "empty"), "aaaa1111")


def recorded_dist(tmp_path, name, **stamps):
    """A pair_build.sh layout: NAME/dist of stub compilers and NAME/ok recording their md5s."""
    (tmp_path / name).mkdir()
    folder = dist(tmp_path / name, "dist", **stamps)
    return folder


def record(folder):
    import hashlib

    (folder.parent / "ok").write_text("".join(f"{hashlib.md5(one.read_bytes()).hexdigest()}  {one.name}\n" for one in sorted(folder.iterdir())))


def test_a_backend_without_a_stamp_counts_only_as_part_of_llrm_llcs_recorded_build(tmp_path, capsys):
    """A tree from before a backend printed a stamp (llrm-6502-llc) was refused outright, so no pre-stamp base could be measured;
    let through on its own, an old unstamped backend copied into a fresh dist would stand for the sha. It is the sha only when the
    dist's build record lists it with llrm-llc, and every figure names it. Without a record it is refused: a copy's mtime is fresh too."""
    fresh = recorded_dist(tmp_path, "fresh", llrm_llc="aaaa1111")
    stub(fresh, "llrm-6502-llc", "")
    record(fresh)
    assert stamp.require_clean(fresh, "aaaa1111") == "aaaa1111"
    assert "unstamped (pre-stamp tree): llrm-6502-llc" in capsys.readouterr().err
    copied = recorded_dist(tmp_path, "copied", llrm_llc="aaaa1111")
    record(copied)
    stub(copied, "llrm-6502-llc", "")
    with pytest.raises(SystemExit, match="llrm-6502-llc prints no stamp and is not from llrm-llc's build"):
        stamp.require_clean(copied, "aaaa1111")
    unrecorded = dist(tmp_path, "unrecorded", llrm_llc="aaaa1111")
    stub(unrecorded, "llrm-6502-llc", "")
    with pytest.raises(SystemExit, match="no build record"):
        stamp.require_clean(unrecorded, "aaaa1111")

def test_identity_refuses_a_dist_that_is_not_the_named_sha(tmp_path):
    """The identity gate names the commits it compares (LLRM_EXPECT_BASE_SHA, LLRM_EXPECT_SHA) and refuses a dirty build before compiling."""
    base = dist(tmp_path, "base", llrm_llc="aaaa1111")
    new = dist(tmp_path, "new", llrm_llc="bbbb2222+dirty")
    for folder in (base, new):
        stub(folder, "llrm-cc", f"cc {folder.name}")
    script = Path(__file__).parent / "identity.sh"
    env = {**os.environ, "LLRM_EXPECT_BASE_SHA": "aaaa1111", "LLRM_EXPECT_SHA": "bbbb2222", "TMPDIR": str(tmp_path)}
    done = subprocess.run([str(script), "c", str(base / "llrm-cc"), str(new / "llrm-cc")], env=env, capture_output=True, text=True)
    assert done.returncode == 2 and "with changes" in done.stderr, done.stdout + done.stderr
    env.pop("LLRM_EXPECT_SHA")
    done = subprocess.run([str(script), "c", str(base / "llrm-cc"), str(new / "llrm-cc")], env=env, capture_output=True, text=True)
    assert done.returncode == 2 and "LLRM_EXPECT_SHA names no commit" in done.stderr, done.stdout + done.stderr

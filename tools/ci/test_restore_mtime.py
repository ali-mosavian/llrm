"""A fresh checkout stamped every source 'now': a restored target dir was older than all of it, and cargo rebuilt the workspace."""

import os
import sys
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import restore_mtime  # noqa: E402


def git(root, *args, when=None):
    env = {**os.environ, "GIT_AUTHOR_DATE": f"{when} +0000", "GIT_COMMITTER_DATE": f"{when} +0000", "GIT_CONFIG_GLOBAL": "/dev/null"} if when else None
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=root, check=True, capture_output=True, env=env)


def test_a_file_takes_the_time_of_the_last_commit_that_changed_it(tmp_path):
    git(tmp_path, "init", "-q")
    (tmp_path / "old.rs").write_text("a")
    (tmp_path / "changed.rs").write_text("a")
    git(tmp_path, "add", "-A")
    git(tmp_path, "commit", "-qm", "one", when=1_000_000_000)
    (tmp_path / "changed.rs").write_text("b")
    git(tmp_path, "commit", "-qam", "two", when=1_100_000_000)
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=tmp_path, capture_output=True, text=True).stdout.strip()
    assert restore_mtime.restore(tmp_path, head) == 2
    assert int((tmp_path / "old.rs").stat().st_mtime) == 1_000_000_000
    assert int((tmp_path / "changed.rs").stat().st_mtime) == 1_100_000_000


def test_a_file_changed_since_the_cached_build_is_newer_than_it_whatever_its_commit_date(tmp_path):
    """A branch committed before the cache was made kept its old author date: cargo took the edited file for unchanged and built stale code."""
    git(tmp_path, "init", "-q")
    (tmp_path / "same.rs").write_text("a")
    (tmp_path / "edited.rs").write_text("a")
    git(tmp_path, "add", "-A")
    git(tmp_path, "commit", "-qm", "one", when=1_000_000_000)
    base = subprocess.run(["git", "rev-parse", "HEAD"], cwd=tmp_path, capture_output=True, text=True).stdout.strip()
    (tmp_path / "edited.rs").write_text("b")
    git(tmp_path, "commit", "-qam", "two", when=1_000_000_001)
    restore_mtime.restore(tmp_path, base)
    assert int((tmp_path / "same.rs").stat().st_mtime) == 1_000_000_000
    assert (tmp_path / "edited.rs").stat().st_mtime > 1_700_000_000
    restore_mtime.restore(tmp_path, "0" * 40)
    assert (tmp_path / "same.rs").stat().st_mtime > 1_700_000_000, "a base this clone lacks must touch everything"


def test_a_directory_is_as_new_as_the_newest_tracked_file_below_it(tmp_path):
    """llrm-c's build.rs watches ../../../crates/target as a directory; the checkout's fresh directory mtimes reran it and rebuilt the compiler."""
    git(tmp_path, "init", "-q")
    (tmp_path / "a/b").mkdir(parents=True)
    (tmp_path / "a/b/x.rs").write_text("a")
    (tmp_path / "a/y.rs").write_text("a")
    git(tmp_path, "add", "-A")
    git(tmp_path, "commit", "-qm", "one", when=1_000_000_000)
    (tmp_path / "a/y.rs").write_text("b")
    git(tmp_path, "commit", "-qam", "two", when=1_100_000_000)
    head = subprocess.run(["git", "rev-parse", "HEAD"], cwd=tmp_path, capture_output=True, text=True).stdout.strip()
    restore_mtime.restore(tmp_path, head)
    assert int((tmp_path / "a/b").stat().st_mtime) == 1_000_000_000
    assert int((tmp_path / "a").stat().st_mtime) == 1_100_000_000

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import llrmbin  # noqa: E402


class ResolverTests(unittest.TestCase):
    def test_a_tree_built_elsewhere_is_found_without_a_target_directory(self):
        """Tools and tests hard-coded <repo>/target/release: in the gate's tree (CARGO_TARGET_DIR set, no ./target) ten
        failed with FileNotFoundError."""
        with tempfile.TemporaryDirectory() as repo, tempfile.TemporaryDirectory() as built:
            self.assertEqual(llrmbin.bin_dir({"CARGO_TARGET_DIR": built}, Path(repo)), Path(built) / "release")
            self.assertEqual(llrmbin.bin_dir({"CARGO_TARGET_DIR": built, "LLRM_BIN": "/x"}, Path(repo)), Path("/x"))
            self.assertEqual(llrmbin.bin_dir({}, Path(repo)), Path(repo) / "target" / "release")

    def test_a_binary_older_than_its_source_is_refused(self):
        """`cargo build -p llrm-cc` builds only the library: bench and a repro ran a day-old llrm-cc and measured the old
        compiler. The resolver names the binaries older than a file cargo's dep-info lists, and refuses them."""
        import os

        with tempfile.TemporaryDirectory() as repo, tempfile.TemporaryDirectory() as built:
            repo, release = Path(repo), Path(built) / "release"
            release.mkdir()
            (repo / "Cargo.toml").write_text('[[bin]]\nname = "llrm-cc"\npath = "x.rs"\n')
            source = repo / "lib.rs"
            source.write_text("")
            (release / "llrm-cc").write_text("")
            (release / "llrm-cc.d").write_text(f"{release}/llrm-cc: {source}\n")
            (release / "leftover").write_text("")  # not declared: `cargo build --bins` does not rebuild it
            (release / "leftover.d").write_text(f"{release}/leftover: {source}\n")
            generated = release / "build" / "core-1" / "out" / "selectors.rs"  # a build script rewrites these when it reruns
            generated.parent.mkdir(parents=True)
            generated.write_text("")
            (release / "llrm-cc.d").write_text(f"{release}/llrm-cc: {source} {generated}\n")
            os.utime(release / "llrm-cc", (1000, 1000))
            os.utime(release / "leftover", (1000, 1000))
            os.utime(generated, (5000, 5000))
            os.utime(source, (2000, 2000))
            self.assertEqual(llrmbin.stale_binaries(release, repo), ["llrm-cc"])
            with self.assertRaises(SystemExit) as refused:
                llrmbin.bin_dir({"LLRM_BIN": str(release)}, repo)
            self.assertIn("cargo build --release --bins", str(refused.exception))
            os.utime(release / "llrm-cc", (3000, 3000))  # newer than the source, older than the generated file
            self.assertEqual(llrmbin.bin_dir({"LLRM_BIN": str(release)}, repo), release)

    def test_binaries_are_judged_against_the_tree_they_were_built_from(self):
        """The gate's measure step builds the base commit in a tree of its own and reads its binaries; gates 69 and 71 failed it with
        "stale binaries ... older than the source they are built from" because the check took the toolchain patches' times from the tree
        being gated (checked out a minute ago), not from the base's."""
        with tempfile.TemporaryDirectory() as gated, tempfile.TemporaryDirectory() as base:
            gated, base = Path(gated), Path(base)
            for root in (gated, base):
                (root / "crates").mkdir()
                (root / "Cargo.toml").write_text('[[bin]]\nname = "llrm-cc"\npath = "x.rs"\n')
                (root / "toolchain/owshim").mkdir(parents=True)
                (root / "toolchain/owshim/ow-commit").write_text("")
            release = base / "target" / "release"
            release.mkdir(parents=True)
            source = base / "crates" / "lib.rs"
            source.write_text("")
            (release / "llrm-cc").write_text("")
            (release / "llrm-cc.d").write_text(f"{release}/llrm-cc: {source}\n")
            os.utime(source, (2000, 2000))
            os.utime(base / "toolchain/owshim/ow-commit", (2000, 2000))
            os.utime(release / "llrm-cc", (3000, 3000))
            os.utime(gated / "toolchain/owshim/ow-commit", (9000, 9000))  # checked out after the base was built
            self.assertEqual(llrmbin.stale_binaries(release, gated), [])
            self.assertEqual(llrmbin.bin_dir({"LLRM_BIN": str(release)}, gated), release)
            os.utime(base / "toolchain/owshim/ow-commit", (9500, 9500))  # its own patch changed since: stale
            self.assertEqual(llrmbin.stale_binaries(release, gated), ["llrm-cc"])

    def test_the_shell_scripts_ask_the_resolver_not_a_copy_of_it(self):
        """Eleven scripts spelled `target/release` and `LLRM_BIN` themselves: the same fact, and wrong in a tree built elsewhere."""
        import os
        import subprocess

        out = lambda *what, **env: subprocess.run([sys.executable, str(Path(llrmbin.__file__)), *what], env={**os.environ, **env}, capture_output=True, text=True).stdout.strip()
        self.assertEqual(out("bin", LLRM_BIN="/a"), "/a")
        self.assertEqual(out("target", CARGO_TARGET_DIR="/b"), "/b")
        scripts = [*Path(llrmbin.__file__).parent.glob("*.sh"), *Path(llrmbin.__file__).parent.glob("*/*.sh"), *(llrmbin.REPO / "crates/target/llrm-x86-m32/vsgcc").glob("*.sh")]
        spelled = [one.name for one in scripts if not one.name.startswith("test-") and ("target/release" in one.read_text() or "LLRM_BIN:-" in one.read_text())]  # a test-*.sh builds fixture trees that spell the path (test-sweep-targets.sh turned this red on parity)
        self.assertEqual(spelled, [])


def test_a_pinned_binary_has_a_path_of_one_length_whatever_directory_it_is_in(tmp_path):
    """The compiler's argv[0] moved an instruction count by up to 5e-4 with its length: a gate's tree and a base's differ in it."""
    short, long = tmp_path / "a", tmp_path / ("a-much-longer-directory-name-" * 3)
    for folder in (short, long):
        folder.mkdir()
        (folder / "llrm-cc").write_text("")
    first, second = llrmbin.pinned(short / "llrm-cc"), llrmbin.pinned(long / "llrm-cc")
    assert len(str(first)) == len(str(second)) == llrmbin.PINNED_BYTES
    assert os.readlink(first) == str(short / "llrm-cc") and os.readlink(second) == str(long / "llrm-cc")


def test_two_copies_of_one_binary_are_refused_and_two_different_ones_print_both_md5(tmp_path):
    """`cargo build -p llrm-cc` did not relink the binary: two commits' "builds" were one file, and the identity pair read 0 differ."""
    import pytest

    a, b, c = tmp_path / "a", tmp_path / "b", tmp_path / "c"
    a.write_bytes(b"one")
    b.write_bytes(b"one")
    c.write_bytes(b"two")
    with pytest.raises(SystemExit, match="same binary"):
        llrmbin.assert_distinct(a, b)
    header = llrmbin.assert_distinct(a, c)
    assert llrmbin.md5(a)[:12] in header and llrmbin.md5(c)[:12] in header
    assert "same source" in llrmbin.assert_distinct(a, b, same_source=True)


def test_a_source_with_a_future_mtime_is_named_as_the_reason_a_binary_stays_stale(tmp_path):
    """A private tree copied with its timestamps kept a source an hour in the future: `cargo build` could not clear the check, and the refusal said only 'stale binaries'."""
    import time

    (tmp_path / "crates").mkdir()
    (tmp_path / "Cargo.toml").write_text('[package]\nname = "llrm"\nversion = "0.0.0"\n[[bin]]\nname = "llrm-cc"\npath = "src/c.rs"\n')
    src = tmp_path / "crates/a.rs"
    src.write_text("x")
    release = tmp_path / "target/release"
    release.mkdir(parents=True)
    binary = release / "llrm-cc"
    binary.write_text("b")
    (release / "llrm-cc.d").write_text(f"{binary}: {src}\n")
    future = time.time() + 3600
    os.utime(src, (future, future))
    assert llrmbin.stale_binaries(release, tmp_path) == ["llrm-cc"]
    detail = llrmbin.stale_detail(release, tmp_path)
    assert str(src) in detail and "FUTURE" in detail, detail


def _dist(tmp_path, content: bytes, recorded: bytes):
    import hashlib

    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / "Cargo.toml").write_text('[[bin]]\nname = "llrm-cc"\npath = "x.rs"\n')
    source = repo / "lib.rs"
    source.write_text("")
    dist = tmp_path / "sbp-abc" / "dist"
    dist.mkdir(parents=True)
    (dist / "llrm-cc").write_bytes(content)
    (dist / "llrm-cc.d").write_text(f"{dist}/llrm-cc: {source}\n")
    (tmp_path / "sbp-abc" / "ok").write_text(f"sha: abc\n{hashlib.md5(recorded).hexdigest()}  llrm-cc\n")
    return repo, dist, source


def test_a_dist_binary_matching_its_recorded_md5_passes_with_an_old_mtime(tmp_path):
    """A checkout made every source newer than the dist built from the same sha; the guard then wanted the binaries touched, which hides a real stale one."""
    repo, dist, source = _dist(tmp_path, b"built", b"built")
    os.utime(dist / "llrm-cc", (1, 1))
    assert llrmbin.stale_binaries(dist, repo) == []


def test_a_dist_binary_not_matching_its_recorded_md5_is_stale_with_a_fresh_mtime(tmp_path):
    """A fresh mtime says nothing of what the binary is: one that differs from what the dist built must be refused."""
    repo, dist, source = _dist(tmp_path, b"other", b"built")
    os.utime(source, (1, 1))
    assert llrmbin.stale_binaries(dist, repo) == ["llrm-cc"]


def test_the_compiler_is_llrm_cc(tmp_path):
    """Every tool names the C compiler through `compiler`; a directory that still holds an llrm-c is not read as having it."""
    (tmp_path / "llrm-c").write_text("")
    assert llrmbin.compiler(tmp_path) == tmp_path / "llrm-cc"


def test_a_pinned_llrm_cc_has_the_path_length_llrm_c_had(tmp_path):
    """The name was counted as `llrm-c`: llrm-cc's path was one byte longer, which moves an instruction count."""
    (tmp_path / "llrm-cc").write_text("")
    assert len(str(llrmbin.pinned(tmp_path / "llrm-cc"))) == llrmbin.PINNED_BYTES


def test_the_stamp_is_read_from_llrm_llc_when_there_is_no_llrm_c(tmp_path, monkeypatch):
    """llrm-cc prints clang's --version, no stamp; the backend beside it prints the sha. Without llrm-c nothing was checked."""
    import stamp

    seen = []
    monkeypatch.setattr(stamp, "check", lambda binary, *args, **kwargs: seen.append(binary.name))
    for name in ("llrm-cc", "llrm-llc"):
        (tmp_path / name).write_text("")
    llrmbin.stamped(tmp_path)
    assert seen == ["llrm-llc"]

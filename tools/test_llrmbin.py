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

    def test_cc_environment_names_the_cache_dir_without_home(self):
        """tools/bench test_widths scrubs HOME: llrm-cc then had no way to the Open Watcom tree and both tests failed."""
        saved = {name: os.environ.pop(name, None) for name in ("HOME", "XDG_CACHE_HOME")}
        try:
            expected = str(llrmbin.ow_tree().parent.parent)
            self.assertEqual(llrmbin.cc_environment().get("XDG_CACHE_HOME"), expected)
        finally:
            for name, value in saved.items():
                if value is not None:
                    os.environ[name] = value

    def test_an_open_watcom_tree_is_present_only_once_it_is_complete(self):
        """toolchain/cache.sh writes `.complete` last: a directory without it is an interrupted fetch, and the compile then
        failed on a missing library instead of naming tools/ow-fetch.sh."""
        with tempfile.TemporaryDirectory() as cache:
            old = os.environ.get("XDG_CACHE_HOME")
            os.environ["XDG_CACHE_HOME"] = cache
            try:
                tree = llrmbin.ow_tree()
                tree.mkdir(parents=True)
                self.assertFalse(llrmbin.ow_present())
                (tree / ".complete").write_text("")
                self.assertTrue(llrmbin.ow_present())
            finally:
                os.environ.pop("XDG_CACHE_HOME") if old is None else os.environ.__setitem__("XDG_CACHE_HOME", old)

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
    """Every tool names the C compiler through `compiler`; a directory that still holds an llrm-cc is not read as having it."""
    (tmp_path / "llrm-cc").write_text("")
    assert llrmbin.compiler(tmp_path) == tmp_path / "llrm-cc"


def test_a_pinned_llrm_cc_has_the_path_length_llrm_c_had(tmp_path):
    """The name was counted as `llrm-cc`: llrm-cc's path was one byte longer, which moves an instruction count."""
    (tmp_path / "llrm-cc").write_text("")
    assert len(str(llrmbin.pinned(tmp_path / "llrm-cc"))) == llrmbin.PINNED_BYTES


def test_the_stamp_is_read_from_llrm_llc_when_there_is_no_llrm_c(tmp_path, monkeypatch):
    """llrm-cc prints clang's --version, no stamp; the backend beside it prints the sha. Without llrm-cc nothing was checked."""
    import stamp

    seen = []
    monkeypatch.setattr(stamp, "check", lambda binary, *args, **kwargs: seen.append(binary.name))
    for name in ("llrm-cc", "llrm-llc"):
        (tmp_path / name).write_text("")
    llrmbin.stamped(tmp_path)
    assert seen == ["llrm-llc"]


def test_a_binary_that_needs_a_feature_the_rebuild_lacks_is_not_judged_stale(tmp_path):
    """The runner rebuilds with `cargo build --release --bins`, which skips llrm-cc (required-features c-legacy): a llrm-cc left from an earlier build was older than a source, and every PR was REFUSED
    "binaries still older than the merged tree after a rebuild: llrm-cc" (#6302, 01:00). A binary the rebuild does build is still judged."""
    repo, release = tmp_path / "repo", tmp_path / "t" / "release"
    release.mkdir(parents=True)
    repo.mkdir()
    (repo / "Cargo.toml").write_text('[features]\ndefault = ["toolchain"]\ntoolchain = []\nc-legacy = []\n[[bin]]\nname = "llrm-cc"\npath = "x.rs"\nrequired-features = ["c-legacy"]\n[[bin]]\nname = "llrm-cc"\npath = "y.rs"\n')
    source = repo / "lib.rs"
    source.write_text("")
    for name in ("llrm-cc", "llrm-cc"):
        (release / name).write_text("")
        (release / f"{name}.d").write_text(f"{release}/{name}: {source}\n")
        os.utime(release / name, (1000, 1000))
    assert llrmbin.stale_binaries(release, repo) == ["llrm-cc"]


def test_two_dists_with_one_llrm_llc_are_refused_whatever_their_llrm_cc_is(tmp_path):
    """llrm-cc only drives; the mid-end and the backends are in the llrm-llc beside it. A base and a head sharing one llrm-llc read SAME on every row (the wasm identity run of
    #6710 read 0 DIFF where nbody -O2 moves 939 -> 910 B); the cause of that run is unproven, this is the check that would have stopped it."""
    import pytest

    base, head = tmp_path / "base", tmp_path / "head"
    for folder, cc in ((base, b"cc one"), (head, b"cc two")):
        folder.mkdir()
        (folder / "llrm-cc").write_bytes(cc)
        (folder / "llrm-llc").write_bytes(b"llc one")
    with pytest.raises(SystemExit, match="same llrm-llc"):
        llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc")
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc", same_llc=True)
    (head / "llrm-llc").write_bytes(b"llc two")
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc")
    (base / "llrm-llc").unlink()  # no llrm-llc to compare: nothing to refuse
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc")


def test_two_dists_with_one_llrm_wasm_llc_are_refused_though_their_llrm_llc_differ(tmp_path):
    """llrm-cc runs the sibling llrm-wasm-llc for wasm32: /tmp/rm/fp_m10_base and fp_m10 held byte-identical ones, and the wasm identity run read 0 DIFF for a fold that
    moves nbody -O2 939 -> 910 B. The llrm-llc check alone passed that pair."""
    import pytest

    base, head = tmp_path / "base", tmp_path / "head"
    for folder, llc in ((base, b"llc one"), (head, b"llc two")):
        folder.mkdir()
        (folder / "llrm-cc").write_bytes(folder.name.encode())
        (folder / "llrm-llc").write_bytes(llc)
        (folder / "llrm-wasm-llc").write_bytes(b"wasm one")
    names = ("llrm-llc", "llrm-wasm-llc")
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc")  # the default names pass it, as before
    with pytest.raises(SystemExit, match="same llrm-wasm-llc"):
        llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc", names=names)
    (head / "llrm-wasm-llc").write_bytes(b"wasm two")
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc", names=names)
    (base / "llrm-wasm-llc").unlink()
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc", names=names)
    with pytest.raises(SystemExit, match="missing llrm-wasm-llc"):
        llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc", names=names, required=True)
class CleanEnvTests(unittest.TestCase):
    def test_a_pipeline_switch_is_refused_and_a_tool_location_is_printed(self):
        """LLRM_WASM_BACKEND_ONLY on one side skipped the mid-end; a false second cause cost 40 minutes."""
        with self.assertRaises(SystemExit) as caught:
            llrmbin.assert_clean_env({"LLRM_WASM_BACKEND_ONLY": "1"})
        self.assertIn("LLRM_WASM_BACKEND_ONLY", str(caught.exception))
        self.assertEqual(llrmbin.assert_clean_env({"LLRM_PIN_CLANG": "/c", "PATH": "/bin"}), ["LLRM_PIN_CLANG=/c"])
        self.assertEqual(llrmbin.assert_clean_env({"LLRM_NO_STACKS": "1", "AB_ALLOW_ENV": "LLRM_NO_STACKS"}), ["LLRM_NO_STACKS=1"])




def test_a_pair_sharing_the_targets_own_backend_is_refused_for_every_row(tmp_path):
    """The 9-target AVG identity run (2026-11-09) compared dists whose llrm-llc differed; arm64, m68k, z80 and 6502 run their own llrm-<family>-llc, and a pair sharing it
    reads SAME on every row of that target. `backends_of` reads each row's `llc_program` from targets.toml (a hard-coded list missed arm64-glibc, 65c02-sim65, m68k-68040 ...).
    Not caught by md5: two worktrees built into one CARGO_TARGET_DIR reuse each other's workspace crates (hashed by relative path) when the outputs are newer than the sources;
    a stale crate that still compiles gives binaries with differing md5 and the wrong code. Build each tree in its own target dir."""
    import pytest
    import tomllib

    rows = tomllib.loads((Path(llrmbin.__file__).resolve().parents[1] / "crates/driver/llrm-cc/targets.toml").read_text())
    own = {name: row["llc_program"] for name, row in rows.items() if "llc_program" in row and row["llc_program"] != "llrm-llc"}
    assert own, "no row names its own backend: the table was not read"
    for name in rows:
        assert llrmbin.backends_of(name) == (("llrm-llc", own[name]) if name in own else ("llrm-llc",)), name
    base, head = tmp_path / "base", tmp_path / "head"
    for name, program in own.items():
        for folder in (base, head):
            folder.mkdir(exist_ok=True)
            (folder / "llrm-cc").write_bytes(folder.name.encode())
            (folder / "llrm-llc").write_bytes(folder.name.encode())
            (folder / program).write_bytes(b"one backend")
        with pytest.raises(SystemExit, match=f"same {program}"):
            llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc", names=llrmbin.backends_of(name), required=True)


def test_a_misspelled_target_name_raises_instead_of_reading_as_no_own_backend():
    """An unknown name fell back to ("llrm-llc",): `arm4-linux` for `arm64-linux` checked only llrm-llc and a pair sharing llrm-arm64-llc passed the identity run."""
    import pytest

    for typo in ("arm4-linux", "", "ARM64-LINUX", "m68k"):
        with pytest.raises(SystemExit, match="unknown target"):
            llrmbin.backends_of(typo)
    assert llrmbin.backends_of("arm64-linux") == ("llrm-llc", "llrm-arm64-llc")


def test_llrm_llc_in_the_environment_makes_two_sides_one_backend_and_is_refused(tmp_path, monkeypatch, capsys):
    """z80 identity #8527 compared one llrm-llc with itself: LLRM_LLC names the default backend of both sides, so the llrm-llc beside each llrm-cc was never run and every row read SAME.
    The guard resolves the backend as llrm-cc does, prints cc and llc md5 per side, and refuses two sides that resolve to one llc unless same_llc."""
    import pytest

    base, head = tmp_path / "base", tmp_path / "head"
    for folder in (base, head):
        folder.mkdir()
        (folder / "llrm-cc").write_bytes(folder.name.encode())
        (folder / "llrm-llc").write_bytes(b"llc " + folder.name.encode())
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc")
    out = capsys.readouterr().err
    assert "base:" in out and "head:" in out and "llc" in out, out
    monkeypatch.setenv("LLRM_LLC", str(base / "llrm-llc"))
    monkeypatch.setenv("AB_ALLOW_ENV", "")
    with pytest.raises(SystemExit, match="same llrm-llc"):
        llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc")
    llrmbin.assert_distinct_llc(base / "llrm-cc", head / "llrm-cc", same_llc=True)


def test_the_sides_command_exits_2_on_one_llc_and_0_with_same_ok(tmp_path):
    import subprocess, sys

    base, head = tmp_path / "base", tmp_path / "head"
    for folder in (base, head):
        folder.mkdir()
        (folder / "llrm-cc").write_bytes(folder.name.encode())
        (folder / "llrm-llc").write_bytes(b"one llc")
    run = lambda *extra: subprocess.run([sys.executable, str(Path(llrmbin.__file__)), "sides", str(base / "llrm-cc"), str(head / "llrm-cc"), *extra], capture_output=True, text=True)
    refused = run()
    assert refused.returncode == 2 and "same llrm-llc" in refused.stderr, refused
    allowed = run("--same-ok")
    assert allowed.returncode == 0 and "md5" in allowed.stderr, allowed


def test_the_compiler_environment_keeps_the_cache_dir_that_names_the_open_watcom_tree(monkeypatch):
    """bench/widths ran llrm-cc without HOME: `{ow}` resolved to a relative `.cache/llrm/ow-src-*`, "the Open Watcom tree is missing", and both test_widths tests failed."""
    monkeypatch.setenv("HOME", "/home/someone")
    monkeypatch.setenv("XDG_CACHE_HOME", "/cache")
    env = llrmbin.cc_environment()
    assert env["HOME"] == "/home/someone" and env["XDG_CACHE_HOME"] == "/cache"

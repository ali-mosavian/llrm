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
        """`cargo build -p llrm-c` builds only the library: bench and a repro ran a day-old llrm-c and measured the old
        compiler. The resolver names the binaries older than a file cargo's dep-info lists, and refuses them."""
        import os

        with tempfile.TemporaryDirectory() as repo, tempfile.TemporaryDirectory() as built:
            repo, release = Path(repo), Path(built) / "release"
            release.mkdir()
            (repo / "Cargo.toml").write_text('[[bin]]\nname = "llrm-c"\npath = "x.rs"\n')
            source = repo / "lib.rs"
            source.write_text("")
            (release / "llrm-c").write_text("")
            (release / "llrm-c.d").write_text(f"{release}/llrm-c: {source}\n")
            (release / "leftover").write_text("")  # not declared: `cargo build --bins` does not rebuild it
            (release / "leftover.d").write_text(f"{release}/leftover: {source}\n")
            os.utime(release / "llrm-c", (1000, 1000))
            os.utime(release / "leftover", (1000, 1000))
            os.utime(source, (2000, 2000))
            self.assertEqual(llrmbin.stale_binaries(release, repo), ["llrm-c"])
            with self.assertRaises(SystemExit) as refused:
                llrmbin.bin_dir({"LLRM_BIN": str(release)}, repo)
            self.assertIn("cargo build --release --bins", str(refused.exception))
            os.utime(release / "llrm-c", (3000, 3000))
            self.assertEqual(llrmbin.bin_dir({"LLRM_BIN": str(release)}, repo), release)

    def test_the_shell_scripts_ask_the_resolver_not_a_copy_of_it(self):
        """Eleven scripts spelled `target/release` and `LLRM_BIN` themselves: the same fact, and wrong in a tree built elsewhere."""
        import os
        import subprocess

        out = lambda *what, **env: subprocess.run([sys.executable, str(Path(llrmbin.__file__)), *what], env={**os.environ, **env}, capture_output=True, text=True).stdout.strip()
        self.assertEqual(out("bin", LLRM_BIN="/a"), "/a")
        self.assertEqual(out("target", CARGO_TARGET_DIR="/b"), "/b")
        scripts = [*Path(llrmbin.__file__).parent.glob("*.sh"), *Path(llrmbin.__file__).parent.glob("*/*.sh"), *(llrmbin.REPO / "crates/target/llrm-x86-m32/vsgcc").glob("*.sh")]
        spelled = [one.name for one in scripts if "target/release" in one.read_text() or "LLRM_BIN:-" in one.read_text()]
        self.assertEqual(spelled, [])

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

"""wat_link.py takes the llrm-cc it builds floatm with from LLRM_CC, LLRM_BIN or the tree, debug as well as release."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import wat_link


class Resolves(unittest.TestCase):
    def test_a_debug_only_tree_resolves_its_debug_binary(self):
        """A tree built with `cargo build` has no target/release/llrm-cc: the link stopped with a traceback on every program."""
        with tempfile.TemporaryDirectory() as root:
            debug = Path(root) / "target/debug/llrm-cc"
            debug.parent.mkdir(parents=True)
            debug.write_text("")
            self.assertEqual(wat_link.llrm_cc({}, Path(root)), debug)

    def test_the_environment_names_it(self):
        self.assertEqual(wat_link.llrm_cc({"LLRM_CC": "/x/llrm-cc"}, Path("/r")), Path("/x/llrm-cc"))
        self.assertEqual(wat_link.llrm_cc({"LLRM_BIN": "/b"}, Path("/r")), Path("/b/llrm-cc"))


class Tools(unittest.TestCase):
    def tree(self, root, stubs=True):
        ow = Path(root) / "ow"
        for name in ("build/binbuild/bwlink", "bld/fpuemu/i86/stubi86/noemu87.lib", "bld/fpuemu/386/stub386/noemu387.lib") if stubs else ("build/binbuild/bwlink",):
            (ow / name).parent.mkdir(parents=True, exist_ok=True)
            (ow / name).write_text("")
        return ow

    def test_the_tree_the_library_was_built_in_links_without_an_installed_open_watcom(self):
        """wat_link.py read wlink and the noemu libs from ~/dos/devtools/dev/c/watcom: CI has no installed Open Watcom, and the tree's own bwlink and bld/fpuemu stubs link the same."""
        with tempfile.TemporaryDirectory() as root:
            ow = self.tree(root)
            linker, libs16, libs32 = wat_link.toolset(ow, {}, Path(root) / "no-home")
            self.assertEqual(linker, ow / "build/binbuild/bwlink")
            self.assertEqual((libs16, libs32), ([ow / "bld/fpuemu/i86/stubi86"], [ow / "bld/fpuemu/386/stub386"]))
            self.assertEqual(wat_link.toolset(ow, {"WATCOM": "/installed"}, Path("/h"))[0], ow / "build/binbuild/bwlink")

    def test_a_tree_without_the_stubs_falls_back_to_the_installed_open_watcom(self):
        with tempfile.TemporaryDirectory() as root:
            ow = self.tree(root, stubs=False)
            linker, libs16, libs32 = wat_link.toolset(ow, {"WATCOM": "/installed"}, Path("/h"))
            self.assertEqual((linker, libs32), (Path("/installed/binl64/wlink"), [Path("/installed/lib386")]))
            self.assertEqual(wat_link.toolset(ow, {}, Path("/h"))[0], Path("/h/dos/devtools/dev/c/watcom/binl64/wlink"))


if __name__ == "__main__":
    unittest.main()

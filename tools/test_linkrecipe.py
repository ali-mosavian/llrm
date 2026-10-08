"""A tool reads the link recipe from the target's object.toml: a change to it must reach the tool."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import linkrecipe  # noqa: E402


class LinkRecipeTests(unittest.TestCase):
    def tree(self, text: str) -> Path:
        root = Path(tempfile.mkdtemp())
        machines = root / "crates" / "target" / "llrm-x86-test" / "src" / "machines"
        machines.mkdir(parents=True)
        (machines / "object.toml").write_text(text)
        return root

    def test_a_change_to_object_toml_reaches_the_recipe(self):
        """The tools wrote `format dos` and `-omf` themselves: a target linked as an LE executable was still linked for DOS."""
        saved = linkrecipe.ROOT
        try:
            linkrecipe.ROOT = self.tree('formats = ["omf"]\ndefault = "omf"\nbitness = 32\nheader = []\n[link]\nformat = ["format", "os2", "le"]\nlast = ["x.asm"]\n')
            self.assertEqual(linkrecipe.link("x86-test", "format"), ["format", "os2", "le"])
            self.assertEqual(linkrecipe.link("x86-test", "last"), ["x.asm"])
            self.assertEqual(linkrecipe.assembler("x86-test"), "-omf")
            linkrecipe.ROOT = self.tree('formats = ["omf"]\ndefault = "coff"\nbitness = 32\nheader = []\n[link]\nformat = []\n')
            with self.assertRaises(KeyError):
                linkrecipe.assembler("x86-test")
        finally:
            linkrecipe.ROOT = saved

    def test_a_target_is_found_by_the_mode_its_datalayout_declares(self):
        """run_tests named its flat target `x86-m32` itself: a target is found by the number its description declares."""
        saved = linkrecipe.ROOT
        try:
            linkrecipe.ROOT = self.tree('writer = "omf"\nbitness = 32\nheader = []\n[link]\nformat = []\n')
            machines = linkrecipe.ROOT / "crates" / "target" / "llrm-x86-test" / "src" / "machines"
            (linkrecipe.ROOT / "crates" / "target" / "llrm-x86-m7").mkdir()
            (linkrecipe.ROOT / "crates" / "target" / "llrm-x86-m7" / "src").mkdir()
            (linkrecipe.ROOT / "crates" / "target" / "llrm-x86-m7" / "src" / "machines").mkdir()
            (linkrecipe.ROOT / "crates" / "target" / "llrm-x86-m7" / "src" / "machines" / "datalayout.toml").write_text("mode = 7\n")
            self.assertEqual(linkrecipe.named(7), "x86-m7")
            self.assertEqual(linkrecipe.modes(), {"x86-m7": 7})
        finally:
            linkrecipe.ROOT = saved

    def test_the_real_targets_link_as_they_say(self):
        self.assertEqual((linkrecipe.named(16), linkrecipe.named(32)), ("x86-m16", "x86-m32"))
        self.assertEqual(linkrecipe.link("x86-m16", "format"), ["format", "dos"])
        self.assertEqual(linkrecipe.link("x86-m32", "format"), ["format", "os2", "le"])
        self.assertEqual(linkrecipe.ld_emulation("x86-m32"), "elf_i386")
        self.assertEqual(linkrecipe.coff_machine("x86-m32"), "x86")


if __name__ == "__main__":
    unittest.main()

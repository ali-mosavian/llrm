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


if __name__ == "__main__":
    unittest.main()

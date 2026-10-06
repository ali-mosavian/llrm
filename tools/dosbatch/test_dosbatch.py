"""`uv run --project tools python -m unittest discover -s tools/dosbatch`"""

import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402


class SourceTests(unittest.TestCase):
    def test_a_source_for_bc_ends_its_lines_with_cr_lf(self):
        """BC reads a LF-only file as one line, so a leading comment eats the program:
        every BC run printed nothing, and the two programs expected to differ from BC passed."""
        self.assertEqual(dosbatch.crlf(b"' c\nPRINT 1\r\nEND\n"), b"' c\r\nPRINT 1\r\nEND\r\n")


class RuntimeObjectTests(unittest.TestCase):
    def test_concurrent_builds_assemble_a_runtime_object_once_and_never_under_a_reader(self):
        """Builds run in threads and each re-made the start-up object of the description's defines in the
        shared directory while another's linker read it: jwlink E2146 "invalid object file" on three programs."""
        made = []

        def assembled(source, path, *defines):
            made.append(path)
            Path(path).write_bytes(b"\x80" + b"x" * 64)

        with tempfile.TemporaryDirectory() as work, mock.patch.object(dosbatch, "assemble", assembled), mock.patch.object(dosbatch, "_ASSEMBLED", set()):
            path = Path(work) / "START.OBJ"
            asked = [threading.Thread(target=dosbatch.runtime_object, args=("crates/target/llrm-x86-code32/runtime/nib/start.asm", path, ("STACK_BYTES=16384",))) for _ in range(16)]
            for one in asked:
                one.start()
            for one in asked:
                one.join()
            self.assertEqual(len(made), 1)
            self.assertNotEqual(made[0], path, "an object is made beside its name and renamed into place")
            self.assertEqual(path.read_bytes()[:1], b"\x80")


class LinkTests(unittest.TestCase):
    def test_c_runtime_is_each_targets_own_file_and_a_name_is_beside_the_source(self):
        """`link: @c-runtime` reached the compiler as a file name in bench (llrm-c: wccq failed on .../@c-runtime):
        only run_tests resolved it. Every reader resolves it here."""
        source = Path("bench/parity/parity/parity.nib")
        self.assertEqual(dosbatch.link_files(source, ["@c-runtime"], "x86-code16"), [dosbatch.ROOT / "tools/loops/runtime/ext.asm"])
        self.assertEqual(dosbatch.link_files(source, ["@c-runtime"], "x86-code32"), [dosbatch.ROOT / "crates/target/llrm-x86-code32/runtime/ext.asm"])
        self.assertEqual(dosbatch.link_files(source, ["geometry.c"], "x86-code32"), [Path("bench/parity/parity/geometry.c")])
        for target in ("x86-code16", "x86-code32"):
            self.assertTrue(all(one.exists() for one in dosbatch.link_files(source, ["@c-runtime"], target)))


if __name__ == "__main__":
    unittest.main()

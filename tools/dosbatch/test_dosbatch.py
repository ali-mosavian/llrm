"""`uv run --project tools python -m unittest discover -s tools/dosbatch`"""

import sys
import tempfile
import time
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
            asked = [threading.Thread(target=dosbatch.runtime_object, args=("runtime/shared/dos/m32/start.asm", path, ("STACK_BYTES=16384",))) for _ in range(16)]
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
        self.assertEqual(dosbatch.link_files(source, ["@c-runtime"], "x86-m16"), [dosbatch.ROOT / "runtime/c/x86-m16/ext.asm"])
        self.assertEqual(dosbatch.link_files(source, ["@c-runtime"], "x86-m32"), [dosbatch.ROOT / "runtime/c/x86-m32/ext.asm"])
        self.assertEqual(dosbatch.link_files(source, ["geometry.c"], "x86-m32"), [Path("bench/parity/parity/geometry.c")])
        for target in ("x86-m16", "x86-m32"):
            self.assertTrue(all(one.exists() for one in dosbatch.link_files(source, ["@c-runtime"], target)))


if __name__ == "__main__":
    unittest.main()


class NibCutTests(unittest.TestCase):
    def test_the_runtime_is_cut_to_what_the_program_and_the_whole_os_layer_name(self):
        """link_nib named the program and its foreign objects only, so a runtime routine the OS layer's hook calls (N$EDIV) was
        cut and the link failed (jwlink E2028); nib-build.sh named start, implementation and hook. Both ask os_objects."""
        with tempfile.TemporaryDirectory() as work:
            layer = {"start": Path(work) / "start.obj", "implementation": Path(work) / "implementation.obj", "hook": Path(work) / "hook.obj"}
            commands = []
            with (
                mock.patch.object(dosbatch, "os_objects", return_value=layer),
                mock.patch.object(dosbatch, "_host", side_effect=commands.append),
                mock.patch.object(dosbatch, "link_target", return_value=()),
                mock.patch.object(dosbatch, "m_flag", return_value="-m16"),
                mock.patch.object(dosbatch, "os_start", return_value=[]),
                mock.patch.object(dosbatch, "os_defines", return_value=()),
            ):
                dosbatch.link_nib("x86-m16", Path("p.nib"), Path("p.obj"), Path("p.exe"), Path(work), "-O2", (Path("f.obj"),))
            command = commands[0]
            named = [command[i + 1] for i, word in enumerate(command) if word == "--used-by"]
            self.assertEqual(named, ["p.obj", "f.obj", *map(str, layer.values())])


class OutputCapTests(unittest.TestCase):
    """A program that printed without end made T292 and T294 write 931 MB each and the event log 22 GB: a gate hung for hours."""

    def test_a_launch_that_fills_a_file_past_the_cap_is_stopped(self):
        import sys

        with tempfile.TemporaryDirectory() as where:
            writer = "import time\nwith open('T001.TXT', 'wb') as f:\n    while True:\n        f.write(b'x' * 65536); f.flush(); time.sleep(0.001)\n"
            began = time.monotonic()
            over = dosbatch._launch([sys.executable, "-c", writer], Path(where), 60, 1 << 20, cwd=where)
            self.assertEqual(over, "T001.TXT")
            self.assertLess(time.monotonic() - began, 30)

    def test_a_launch_within_the_cap_runs_to_its_end(self):
        import sys

        with tempfile.TemporaryDirectory() as where:
            self.assertIsNone(dosbatch._launch([sys.executable, "-c", "open('T001.TXT', 'w').write('ok')"], Path(where), 60, 1 << 20, cwd=where))

    def test_the_program_that_overflowed_fails_and_the_others_are_stopped(self):
        jobs = [dosbatch.Job("T001", "obj", Path("a.obj")), dosbatch.Job("T002", "obj", Path("b.obj"))]
        results = dosbatch.stopped_for_output(jobs, Path("."), "T002.TXT", 100)
        self.assertEqual((results["T002"].status, results["T001"].status), ("over the output cap", "stopped"))

"""`uv run --project tools python -m unittest discover -s tools/dosbatch`"""

import os
import shutil
import sys
import tempfile
import time
import threading
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).parent))

import dosbatch  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import envflag  # noqa: E402


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
    def test_two_start_ups_with_one_stem_both_reach_the_link(self):
        """The QB m32 link names a platform start.asm and the OS layer's: both were START.OBJ, the second overwrote the first and the link lost a start-up."""
        platform, shared = "runtime/qb/dos/m32/start.asm", "runtime/shared/dos/m32/start.asm"
        said = []
        link = {"first": [], "last": [], "final": [], "format": []}
        with tempfile.TemporaryDirectory() as work, mock.patch.object(dosbatch, "target_link", return_value=link), mock.patch.object(
            dosbatch, "runtime_object", side_effect=lambda name, path, defines=(): path
        ), mock.patch.object(dosbatch, "_host", side_effect=said.append), mock.patch.object(dosbatch, "link_loaders", return_value=()):
            dosbatch.link_target("m32", Path(work) / "P.OBJ", Path(work) / "P.EXE", Path(work), runtime=([platform, shared], []), defines=())
        files = [word for word in said[0] if word.endswith(".OBJ") and word != str(Path(work) / "P.OBJ")]
        self.assertEqual(len(files), 2)
        self.assertEqual(len(set(files)), 2, files)

    def test_a_candidate_runtime_is_copied_and_never_falls_back_to_the_toolchain_library(self):
        """The first candidate link silently used BCOM45 after its replacement archive was not mounted."""
        with tempfile.TemporaryDirectory() as work:
            work = Path(work)
            candidate = work / "LLRMQB.LIB"
            candidate.write_bytes(b"library")
            job = dosbatch.Job("T001", "obj", work / "P.OBJ", runtime="llrmqb", runtime_file=candidate)
            self.assertEqual(dosbatch.runtime_library(job, dosbatch.QB45_TOOLS, work), r"C:\LLRMQB.LIB")
            self.assertEqual((work / "LLRMQB.LIB").read_bytes(), b"library")
        with self.assertRaisesRegex(dosbatch.BuildError, "runtime file"):
            dosbatch.runtime_library(dosbatch.Job("T001", "obj", Path("P.OBJ"), runtime="empty"), dosbatch.QB45_TOOLS, Path("."))

    def test_c_runtime_is_each_targets_own_file_and_a_name_is_beside_the_source(self):
        """`link: @c-runtime` reached the compiler as a file name in bench (the compiler failed on .../@c-runtime):
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

    def test_a_program_that_divides_nothing_links_with_the_hooks_division_fault_handler(self):
        """init.asm's divide-fault handler calls N$EDIV, which the cut left out when only the program was named:
        jwlink E2028 on `fn main() -> i16: print("hook")`. LLRM_REQUIRE_JWLINK=1 (the gate's) makes a missing tool a failure,
        not a skip."""
        missing = [name for name in ("jwlink", "llrm-nib") if not (dosbatch.BIN / name).exists()]
        if missing:
            self.assertFalse(envflag.flag("LLRM_REQUIRE_JWLINK"), f"LLRM_REQUIRE_JWLINK is set and {missing} is missing")
            print(f"SKIPPED: needs {missing}", file=sys.stderr)
            self.skipTest(f"needs {missing}")
        with tempfile.TemporaryDirectory() as work:
            work = Path(work)
            source = work / "h.nib"
            source.write_text('fn main() -> i16:\n    print("hook")\n    return 0\n')
            dosbatch._host([str(dosbatch.BIN / "llrm-nib"), str(source), "-m16", "-O2", "--procedure-segments", "-o", str(work / "P.OBJ")])
            dosbatch.link_nib(dosbatch.REAL_MODE, source, work / "P.OBJ", work / "P.EXE", work, "-O2", ())
            self.assertTrue((work / "P.EXE").stat().st_size > 0)


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


class LoadedHostTests(unittest.TestCase):
    """The torture step stopped programs that fit their budget whenever the machine was busy: with `cycles=max` an emulated
    millisecond was what the host managed in a millisecond, so a loaded host ran fewer instructions in the same budget."""

    SPIN = bytes.fromhex("66b980841e00" "6649" "75fc" "b8004c" "cd21")  # mov ecx,2000000; dec ecx / jnz; exit 0: 4 M instructions

    def run_spin(self, work: Path, budget_ms: int):
        import dataclasses

        source = work / "spin.bin"
        source.write_bytes(self.SPIN)
        tools = dataclasses.replace(dosbatch.QB45_TOOLS, mount=work)
        return dosbatch.run([dosbatch.Job("SPIN", "exe", source)], work / "run", timeout=120, budget_ms=budget_ms, build_ms=5000, tools=tools)["SPIN"]

    def test_a_program_that_fits_its_budget_finishes_on_a_busy_host(self):
        import subprocess
        import sys

        if not dosbatch.DOSBOX.exists():
            self.skipTest("no dosbox-x")
        burners = [subprocess.Popen([sys.executable, "-c", "while True: pass"]) for _ in range(4 * (os.cpu_count() or 4))]
        try:
            with tempfile.TemporaryDirectory() as where:
                # 4 M instructions in 300 ms of emulated time is 13 k per millisecond: under the configured rate
                result = self.run_spin(Path(where), 300)
        finally:
            for one in burners:
                one.kill()
                one.wait()
        self.assertEqual(result.status, "ok", result.detail)

    def test_a_binary_without_virtual_time_is_refused_and_not_run_with_a_flaky_budget(self):
        """The pinned dosbox-x predated virtual time, so the setting was ignored and the busy-host test failed in every gate with
        nothing said about why. A binary that cannot give it is named instead."""
        import dataclasses
        import stat

        with tempfile.TemporaryDirectory() as where:
            fake = Path(where) / "dosbox-x"
            # a runner that ends the job having run 12345 instructions, whatever it was asked
            fake.write_text('#!/bin/sh\necho \'{"ev":"end","reason":"limit","instructions":12345}\' >&"$DOSRUN_FD"\n')
            fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
            self.assertIn("no `virtual time`", dosbatch.virtual_time_problem(fake) or "")
            with mock.patch.object(dosbatch, "DOSBOX", fake), mock.patch.object(dosbatch, "_VIRTUAL", {}):
                with self.assertRaises(dosbatch.BinaryError):
                    dosbatch.run([dosbatch.Job("SPIN", "exe", Path(where) / "x")], Path(where) / "run")

    def test_the_binary_the_tools_use_has_virtual_time(self):
        if not dosbatch.DOSBOX.exists():
            self.skipTest("no dosbox-x")
        self.assertIsNone(dosbatch.virtual_time_problem(dosbatch.DOSBOX))


class StallTests(unittest.TestCase):
    """A launch's wall time is the host's load: a fixed limit killed a torture step that was still making progress."""

    def test_a_launch_that_keeps_reporting_is_never_cut_off_by_how_long_it_takes(self):
        import sys

        writer = "import time\nfor i in range(10):\n    open('events.txt', 'a').write('{}\\n'); time.sleep(0.5)\n"
        with tempfile.TemporaryDirectory() as where:
            self.assertIsNone(dosbatch._launch([sys.executable, "-c", writer], Path(where), None, 1 << 20, stall=2, cwd=where))

    def test_a_launch_that_stops_reporting_is_stopped(self):
        import subprocess
        import sys

        # Stuck is spinning without a report: stall counts the emulator's CPU seconds (test_hostload.py has the one that
        # only waits).
        with tempfile.TemporaryDirectory() as where:
            began = time.monotonic()
            with self.assertRaises(subprocess.TimeoutExpired):
                dosbatch._launch([sys.executable, "-c", "while True: pass"], Path(where), None, 1 << 20, stall=2, cwd=where)
            self.assertLess(time.monotonic() - began, 300)  # 2 CPU seconds, which a host at load 200 gives over minutes


def test_a_program_run_on_its_own_is_not_given_the_microsoft_install_as_v(tmp_path):
    """V: was mounted for every launch, so the same flat program counted 458341 or 458306 instructions by whether the host had
    QB45: its count is the benchmark, and the path that needs no QB45 must not depend on it."""
    exe = tmp_path / "P.EXE"
    exe.write_bytes(b"MZ")

    def script(kind: str) -> str:
        work = tmp_path / kind
        with mock.patch.object(dosbatch, "_launch", return_value=None), mock.patch.object(dosbatch, "collect", return_value={}):
            dosbatch.run([dosbatch.Job("J", kind, exe)], work)
        return (work / "jobs.txt").read_text()

    assert "mount v" not in script("exe")
    assert "mount v" in script("obj")


def test_a_program_whose_screen_is_compared_keeps_the_mount_so_its_rows_line_up_with_the_reference(tmp_path):
    """Dropping V: for every exe job emptied the rows the mount messages had filled: 71 test_qbrt programs failed
    "row 3: want 80 cells got 0" against BCOM45's screen."""
    exe = tmp_path / "P.EXE"
    exe.write_bytes(b"MZ")
    with mock.patch.object(dosbatch, "_launch", return_value=None), mock.patch.object(dosbatch, "collect", return_value={}):
        dosbatch.run([dosbatch.Job("J", "exe", exe, screen=True)], tmp_path / "w")
    assert "mount v" in (tmp_path / "w" / "jobs.txt").read_text()


def test_a_failed_assembly_leaves_no_err_file_in_the_working_directory(tmp_path, monkeypatch):
    """jwasm writes `<source>.err` into the current directory when it fails: tools/dosbatch/init.err landed untracked in the source tree."""
    import pytest

    if not (dosbatch.BIN / "jwasm").is_file():
        pytest.skip("jwasm is not built")
    cwd = tmp_path / "cwd"
    cwd.mkdir()
    monkeypatch.chdir(cwd)
    with pytest.raises(Exception):  # noqa: B017
        dosbatch.assemble(tmp_path / "missing" / "init.asm", tmp_path / "x.obj")
    assert list(cwd.iterdir()) == []

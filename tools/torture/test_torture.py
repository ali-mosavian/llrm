"""`uv run --project tools python -m unittest discover -s tools/torture`"""

import shutil
import tempfile
import sys
import unittest
from unittest import mock
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import torture  # noqa: E402


class CompilerTests(unittest.TestCase):
    def test_the_programs_and_the_support_library_are_built_by_llrm_cc_on_one_row(self):
        """libc.c was built by llrm-c while the programs' calling convention came from the other front end's rows: on parity it failed (E1034 memcpy "already defined")
        and the harness stopped before the first program."""
        seen = []
        with mock.patch.object(torture.subprocess, "run", lambda command, **kw: seen.append((command, kw.get("env", {}))) or mock.Mock(returncode=0)):
            torture.build_support("x86-m32", Path("/w/libc.obj"))
        (command, env), = seen
        self.assertEqual(Path(command[0]).name, "llrm-cc")
        self.assertEqual(env["LLRM_CC_TARGET"], "x86_32-wat")
        with tempfile.TemporaryDirectory() as tmp, mock.patch.object(torture, "run_within_cpu", lambda command, seconds, env=None: seen.append((command, env)) or None):
            (Path(tmp) / "a.c").write_text("int main(void) { return 0; }\n")
            torture.build(Path(tmp) / "a.c", "O2", "x86-m32", Path(tmp), Path(tmp) / "libc.obj", "T0", [])
        command, env = seen[-1]
        self.assertEqual((Path(command[0]).name, env["LLRM_CC_TARGET"]), ("llrm-cc", "x86_32-wat"))


class CauseTests(unittest.TestCase):
    def test_a_front_end_error_is_counted_by_what_it_says_not_where_or_of_what(self):
        text = "llrm-c: wccq failed on /x/y.c:\n/x/y.c(10): Warning! W131: No prototype found for function 'f'\n/x/y.c(12): Error! E1054: Missing operand\n"
        self.assertEqual(torture.cause(text), "wccq: Missing operand")
        self.assertEqual(torture.cause("llrm-c: foo_: place n45 has incomplete extent"), "foo_: place nN has incomplete extent")

    def test_a_clang_error_is_the_cause_not_the_warning_before_it(self):
        """A warning on the first line was every finding's cause: 920411-1 and 930718-1 counted by their warnings under llrm-cc."""
        text = "/x/y.c:3:1: warning: a function definition without a prototype is deprecated [-Wdeprecated-non-prototype]\n/x/y.c:9:5: error: call to undeclared function 'f'\n"
        self.assertEqual(torture.cause(text), "error: call to undeclared function '_'")

    def test_a_refusal_is_matched_on_the_cause_and_names_its_reason(self):
        rules = torture.expected()
        self.assertIn("front end", torture.refusal("wccq: Expecting '_' but found '_'", rules))
        self.assertIsNone(torture.refusal("_main: .X is neither defined nor imported", rules))


    def test_the_front_ends_gnu_and_c99_limits_are_named_not_counted_as_findings(self):
        """Five more wccq errors (15 builds) were listed as compile findings; each is syntax Open Watcom lacks."""
        rules = torture.expected()
        for cause in ("wccq: Cannot use typedef '_' as a variable", "wccq: Incomplete enum declaration", "wccq: Type cast must be a scalar type",
                      "wccq: Assembler error: '_'", "wccq: Expression for '_' must be a '_' or '_'"):
            self.assertIsNotNone(torture.refusal(cause, rules), cause)


class OptionTests(unittest.TestCase):
    def test_a_program_that_needs_fwrapv_is_built_with_it(self):
        """950704-1 checks for signed overflow after the add and says so with dg-additional-options "-fwrapv": built without
        it the optimiser dropped the check and the program aborted (wrong, at -O2 and -Os)."""
        self.assertEqual(torture.program_options('/* { dg-additional-options "-fwrapv" } */\nint x;'), ["-fwrapv"])
        self.assertEqual(torture.program_options('/* { dg-additional-options "-O3 -fwrapv -fno-tree-ccp" } */'), ["-fwrapv"])
        self.assertEqual(torture.program_options("int x;"), [])

    def test_a_program_that_asks_for_fwrapv_with_dg_options_is_built_with_it(self):
        """920711-1 says `{ dg-options "-fwrapv" }`, which the runner did not read: `--a > 0` of the smallest int was folded to
        `a > 1` on the no-wrap promise of a build that had asked for wrapping, and the program aborted (-O2 and -Os)."""
        self.assertEqual(torture.program_options('/* { dg-options "-fwrapv" } */\nint x;'), ["-fwrapv"])

    def test_the_braced_dg_options_form_and_std_gnu89_are_read(self):
        """930529-1 says `{ dg-options { "-fwrapv" } }`, which the pattern missed: built without it the loop past INT_MAX ran forever (stopped
        at the 15 s limit). loop-2d asks for -std=gnu89, without which clang refuses its implicit int."""
        self.assertEqual(torture.program_options('/* { dg-options { "-fwrapv" } } */\nint x;'), ["-fwrapv"])
        self.assertEqual(torture.program_options('/* { dg-additional-options "-std=gnu89" } */\nint x;'), ["-std=gnu89"])


class SymbolTests(unittest.TestCase):
    def test_an_undefined_symbol_is_refused_whichever_way_the_target_decorates_it(self):
        """m16 (cdecl, a leading underscore) reported `___builtin_prefetch` and `_sprintf`: the refusals name `__builtin_` and
        `sprintf`, so 114 builds of programs that need a builtin or libc routine the runner lacks counted as link findings."""
        rules = torture.expected()
        for symbol in ("__builtin_prefetch_", "___builtin_prefetch", "_sprintf", "sprintf_", "___builtin_ffs@3", "_sprintf@3"):
            found = [torture.refusal(f"undefined symbol {one}", rules) for one in torture.spellings(symbol)]
            self.assertTrue(any(found), symbol)
        self.assertFalse(any(torture.refusal(f"undefined symbol {one}", rules) for one in torture.spellings("_frobnicate")))


class WorkTests(unittest.TestCase):
    def test_two_runs_without_a_work_option_do_not_share_a_directory(self):
        """The default was one fixed directory: a gate running beside a full run deleted its files (`dosbatch.run` clears its
        work directory), the full run crashed on FileNotFoundError and the gate called dozens of programs wrong."""
        made = []
        real = Path.mkdir

        def recording(self, *args, **kwargs):
            made.append(self)
            return real(self, *args, **kwargs)

        for _ in range(2):
            with mock.patch.object(torture, "programs", return_value=[]), mock.patch.object(sys, "argv", ["torture.py"]), mock.patch.object(Path, "mkdir", recording):
                self.assertEqual(torture.main(), 2)
        self.assertEqual(len(made), 2)
        self.assertNotEqual(made[0], made[1])
        for one in made:
            shutil.rmtree(one, ignore_errors=True)


class StackTests(unittest.TestCase):
    def test_a_required_stack_size_is_the_expression_the_test_states(self):
        for text, bytes in (('/* { dg-require-stack-size "0x10000" } */', 65536), ('/* { dg-require-stack-size "128 * 128 * 4 + 1024" } */', 66560), ('{ dg-require-stack-size "8*100*100" }', 80000), ("int x;", None)):
            self.assertEqual(torture.stack_needed(text), bytes, text)

    def test_a_program_gets_the_stack_it_asks_for_and_the_size_it_may_use(self):
        """pr20621-1 needs 64 KB of stack and ran on the 16 KB the start-up gave every program (exit 255 at the guard); the programs
        that size themselves from STACK_SIZE were named differences, one of them 24 KB deep against 16."""
        self.assertEqual(torture.stack_plan("int x;", 16384, 32), (16384, None))
        self.assertEqual(torture.stack_plan('{ dg-require-stack-size "0x10000" }', 16384, 32), (65536 + torture.STACK_SLACK, None))
        self.assertEqual(torture.stack_plan("/* { dg-add-options stack_size } */", 16384, 32), (16384, 16384 - torture.STACK_SLACK))
        # A real-mode segment holds 64 KB with the data: the stack is asked for as far as it goes.
        self.assertEqual(torture.stack_plan('{ dg-require-stack-size "524544" }', 16384, 16)[0], 65536 - 2 * torture.STACK_SLACK)


class DifferencesTests(unittest.TestCase):
    def test_a_program_that_differs_by_design_names_its_reason(self):
        self.assertIn("ISO C11", torture.differences()["pr32244-1"])
        self.assertIn("pr34971", torture.differences())


if __name__ == "__main__":
    unittest.main()


class CompileLimitTests(unittest.TestCase):
    """`COMPILE_SECONDS` was wall clock: at load 40 a compile that takes a few seconds did not finish in 60 s, at -O0 too."""

    def test_a_compile_the_host_is_slow_to_run_is_not_cut_off_by_the_wall_clock(self):
        import sys

        done = torture.run_within_cpu([sys.executable, "-c", "import time; time.sleep(3)"], 1)
        self.assertIsNotNone(done)
        self.assertEqual(done.returncode, 0)

    def test_a_compile_that_uses_more_cpu_than_its_limit_is_stopped(self):
        import sys

        self.assertIsNone(torture.run_within_cpu([sys.executable, "-c", "while True: pass"], 1))

    def test_a_compile_that_fails_reports_its_output(self):
        import sys

        done = torture.run_within_cpu([sys.executable, "-c", "import sys; sys.stderr.write('boom'); sys.exit(3)"], 5)
        self.assertEqual((done.returncode, done.stderr), (3, "boom"))


def test_batches_are_run_at_once_each_in_a_work_directory_of_its_own(tmp_path):
    """489 runs went through DOSBox in serial batches of 150 (279 s on one core); a launch clears the directory it is given."""
    import threading

    order = list(range(489))
    groups = torture.batches(order)
    assert len(groups) <= torture.LAUNCHES and sum(groups, []) == order and max(map(len, groups)) <= torture.BATCH
    seen, gate = [], threading.Barrier(len(groups), timeout=10)

    def launch(group, work):
        seen.append(work)
        gate.wait()  # every launch is in flight together, or this times out
        return {"n": len(group)}

    done = torture.run_batches(groups, launch, tmp_path)
    assert [d["n"] for d in done] == list(map(len, groups))
    assert len(set(seen)) == len(groups)


def test_a_launch_that_raises_fails_the_run_instead_of_vanishing(tmp_path):
    def launch(group, work):
        if group == [2]:
            raise RuntimeError("lost")
        return {}

    import pytest

    with pytest.raises(RuntimeError):
        torture.run_batches([[1], [2]], launch, tmp_path)


def test_int_max_follows_the_compilers_int_not_a_target_macro():
    """llrm-cc's 32-bit row does not define `__386__`: limits.h gave INT_MAX 32767 there and 20040409-1 (wrong-xor) aborted at m32 at every level."""
    import subprocess

    include = Path(__file__).parent / "include"
    for width in ("32767", "2147483647"):
        done = subprocess.run(
            ["cc", "-E", "-P", "-U__INT_MAX__", f"-D__INT_MAX__={width}", "-I", str(include), "-"],
            input="#include <limits.h>\nINT_MAX\n", capture_output=True, text=True, check=True,
        )
        assert done.stdout.strip() == width

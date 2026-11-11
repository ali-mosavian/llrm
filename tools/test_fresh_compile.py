"""A compile that fails or writes nothing must never leave an object to be read as this cell's."""

import os
import subprocess
import tempfile
import textwrap
import unittest

TOOLS = os.path.dirname(os.path.abspath(__file__))
HELPER = os.path.join(TOOLS, "fresh_compile.sh")


def sh(script, **kw):
    return subprocess.run(["bash", "-c", script], capture_output=True, text=True, **kw)


class FreshCompile(unittest.TestCase):
    def test_a_failed_compile_removes_the_old_object(self):
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, "a.o")
            open(out, "w").write("old")
            done = sh(f'. {HELPER}; fresh_compile {out} false; echo $?; test -e {out} && echo STALE')
            self.assertEqual(done.stdout.split(), ["1"], done.stdout)

    def test_a_silent_success_is_a_failure(self):
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, "a.o")
            open(out, "w").write("old")
            done = sh(f'. {HELPER}; fresh_compile {out} true; echo $?')
            self.assertEqual(done.stdout.strip(), "97")

    def test_a_compile_that_writes_the_object_returns_zero(self):
        with tempfile.TemporaryDirectory() as d:
            out = os.path.join(d, "a.o")
            done = sh(f'. {HELPER}; fresh_compile {out} sh -c "echo x > {out}"; echo $?')
            self.assertEqual(done.stdout.strip(), "0")


def fake_cc(path, tag):
    """A compiler: `bad` in the source name -> exit 0 writing nothing; else an object and, for -S, an instruction line."""
    open(path, "w").write(textwrap.dedent(f"""\
        #!/bin/bash
        # {tag}
        src=; out=; mode=c
        while [ $# -gt 0 ]; do case $1 in -o) out=$2; shift;; -S) mode=S;; -c) ;; -*) ;; *) src=$1;; esac; shift; done
        case $src in *bad*) exit 0;; esac
        if [ $mode = S ]; then printf '\\tmov r0, r1\\n' > "$out"; else printf 'OBJ%s' "$src" > "$out"; fi
        """))
    os.chmod(path, 0o755)


class SweepsRefuseSilentCompiles(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.d = self.dir.name
        for side in ("base", "new"):
            os.makedirs(os.path.join(self.d, side))
            for b in ("llrm-cc", "llrm-llc"):
                fake_cc(os.path.join(self.d, side, b), f"{side}-{b}")

    def tearDown(self):
        self.dir.cleanup()

    def test_cells_reports_a_silent_compile_as_empty_not_as_the_previous_cell(self):
        """A compile exiting 0 without writing left the previous cell's .o/.s; cells.sh printed its numbers for the next cell."""
        lst = os.path.join(self.d, "list")
        open(lst, "w").write("x x86_32-linux -O2 good.c\nx x86_32-linux -O2 bad.c\n")
        done = subprocess.run([os.path.join(TOOLS, "ab", "cells.sh"), self.d, lst], capture_output=True, text=True)
        self.assertIn("EMPTY x86_32-linux -O2 bad.c", done.stdout, done.stdout + done.stderr)
        self.assertEqual(done.returncode, 1)

    def test_identity_does_not_read_two_empty_outputs_as_the_same(self):
        """Both compilers exiting 0 with an empty object read SAME (cmp of two empty files)."""
        for side in ("base", "new"):
            cc = os.path.join(self.d, side, "llrm-cc")
            open(cc, "w").write(f"#!/bin/bash\n# {side} silent\nwhile [ $# -gt 0 ]; do [ \"$1\" = -o ] && : > \"$2\"; shift; done\nexit 0\n")
            os.chmod(cc, 0o755)
        done = subprocess.run(
            [os.path.join(TOOLS, "identity.sh"), "c", os.path.join(self.d, "base", "llrm-cc"), os.path.join(self.d, "new", "llrm-cc")],
            capture_output=True, text=True, env={**os.environ, "IDENTITY_SAME_LLC": "1"}, cwd=self.d,
        )
        self.assertNotIn("SAME", done.stdout, done.stdout[:300])


if __name__ == "__main__":
    unittest.main()

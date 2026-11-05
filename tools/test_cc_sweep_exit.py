"""cc-sweep.sh exited 0 whatever llrm-cc did: a missing or refusing compiler printed 'ok 0 fail N' and was green."""

import os
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "tools", "cc-sweep.sh")


def run(stub):
    with tempfile.TemporaryDirectory() as d:
        bindir = os.path.join(d, "bin")
        os.makedirs(bindir)
        if stub is not None:
            path = os.path.join(bindir, "llrm-cc")
            with open(path, "w") as f:
                f.write("#!/bin/sh\n" + stub)
            os.chmod(path, 0o755)
        env = dict(os.environ, LLRM_BIN=bindir, CC_SWEEP_OUT=os.path.join(d, "out"))
        return subprocess.run(["sh", SCRIPT, "x86_64-linux"], cwd=ROOT, env=env, capture_output=True, text=True)


class CcSweepExit(unittest.TestCase):
    def test_missing_compiler_is_not_green(self):
        r = run(None)
        self.assertNotEqual(r.returncode, 0, r.stdout)

    def test_refusing_compiler_is_not_green(self):
        r = run("exit 1\n")
        self.assertNotEqual(r.returncode, 0, r.stdout)

    def test_compiling_is_green(self):
        r = run('while [ $# -gt 1 ]; do [ "$1" = -o ] && : > "$2"; shift; done\nexit 0\n')
        self.assertEqual(r.returncode, 0, r.stdout)


if __name__ == "__main__":
    unittest.main()

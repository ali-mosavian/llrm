"""g-identical.sh exits 0 only when every compared pair is identical: a refused compiler or a -g difference used to exit 0 ("identical 0, differing 0")."""

import os
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "tools", "g-identical.sh")
FAKE = {
    "refuses": "exit 1\n",
    "same": 'while [ $# -gt 1 ]; do [ "$1" = -o ] && o=$2; shift; done; echo asm > "$o"\n',
    "g_differs": 'g=0; while [ $# -gt 0 ]; do [ "$1" = -g ] && g=1; [ "$1" = -o ] && o=$2; shift; done; echo asm$g > "$o"\n',
}


def run(kind):
    with tempfile.TemporaryDirectory() as d:
        fake = os.path.join(d, "llrm-c")
        with open(fake, "w") as f:
            f.write("#!/bin/sh\n" + FAKE[kind])
        os.chmod(fake, 0o755)
        return subprocess.run(["bash", SCRIPT, fake, "m32:elf"], cwd=ROOT, capture_output=True, text=True)


class GIdenticalExit(unittest.TestCase):
    def test_refused_compiler_is_not_green(self):
        r = run("refuses")
        self.assertNotEqual(r.returncode, 0, r.stdout)

    def test_g_difference_is_not_green(self):
        r = run("g_differs")
        self.assertNotEqual(r.returncode, 0, r.stdout[-300:])

    def test_identical_is_green(self):
        r = run("same")
        self.assertEqual(r.returncode, 0, r.stdout[-300:])


if __name__ == "__main__":
    unittest.main()

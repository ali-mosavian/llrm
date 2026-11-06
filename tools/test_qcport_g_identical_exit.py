"""qcport-g-identical.sh exits 0 only when every compared module is identical: a -g difference or a compiler refusing everything used to exit 0."""

import os
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "tools", "qcport-g-identical.sh")
FAKE = {
    "refuses": "exit 1\n",
    "same": 'while [ $# -gt 1 ]; do [ "$1" = -o ] && o=$2; shift; done; echo asm > "$o"\n',
    "g_differs": 'g=0; while [ $# -gt 0 ]; do [ "$1" = -g ] && g=1; [ "$1" = -o ] && o=$2; shift; done; echo asm$g > "$o"\n',
}


def run(kind):
    with tempfile.TemporaryDirectory() as d:
        os.makedirs(os.path.join(d, "src", "host"))
        open(os.path.join(d, "src", "host", "m.c"), "w").close()
        fake = os.path.join(d, "llrm-c")
        with open(fake, "w") as f:
            f.write("#!/bin/sh\n" + FAKE[kind])
        os.chmod(fake, 0o755)
        env = dict(os.environ, QCPORT=os.path.join(d, "src"), QCPORT_INC=d)
        return subprocess.run(["bash", SCRIPT, fake], cwd=ROOT, env=env, capture_output=True, text=True)


class QcportGIdenticalExit(unittest.TestCase):
    def test_refused_compiler_is_not_green(self):
        r = run("refuses")
        self.assertNotEqual(r.returncode, 0, r.stdout)

    def test_g_difference_is_not_green(self):
        r = run("g_differs")
        self.assertNotEqual(r.returncode, 0, r.stdout)

    def test_identical_is_green(self):
        r = run("same")
        self.assertEqual(r.returncode, 0, r.stdout)


if __name__ == "__main__":
    unittest.main()

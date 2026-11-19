"""qcport-compile.sh with no modules compiled nothing and exited 0 ("0 modules, 0 refusals")."""

import os
import subprocess
import tempfile
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCRIPT = os.path.join(ROOT, "tools", "qcport-compile.sh")


def run(modules, compiler_ok=True):
    with tempfile.TemporaryDirectory() as d:
        src = os.path.join(d, "src")
        for sub in ("host", "render", "model", "game", "sound", "ui"):
            os.makedirs(os.path.join(src, sub))
        for m in range(modules):
            open(os.path.join(src, "host", f"m{m}.c"), "w").close()
        fake = os.path.join(d, "llrm-cc")
        with open(fake, "w") as f:
            # llrm-cc defaults to x86_64-linux, where QCport does not parse: the stand-in refuses a compile that names no m16 row or no `-c`
            host_guard = "case \"$LLRM_CC_TARGET $*\" in x86_16*\" -c \"*) ;; *) echo 'llrm-cc: host target'; exit 1;; esac\n"
            f.write("#!/bin/sh\n" + host_guard + ("exit 0\n" if compiler_ok else "echo 'llrm-cc: refused'; exit 1\n"))
        os.chmod(fake, 0o755)
        env = dict(os.environ, QCPORT=src, QCPORT_INC=d)
        return subprocess.run(["bash", SCRIPT, fake], cwd=ROOT, env=env, capture_output=True, text=True)


class QcportCompileExit(unittest.TestCase):
    def test_no_modules_is_not_green(self):
        r = run(0)
        self.assertNotEqual(r.returncode, 0, r.stdout)

    def test_refusal_is_not_green(self):
        self.assertNotEqual(run(1, compiler_ok=False).returncode, 0)

    def test_the_compile_names_the_m16_target_not_the_host_default(self):
        """After #8011 (llrm-c -> llrm-cc) the script passed no target: every module was refused as x86_64 and the output said nothing about the compiler."""
        r = run(1)
        self.assertNotIn("host target", r.stdout)

    def test_compiled_is_green(self):
        r = run(1)
        self.assertEqual(r.returncode, 0, r.stdout)


if __name__ == "__main__":
    unittest.main()

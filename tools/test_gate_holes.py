"""Gate scripts that exit 0 when they compared nothing: each test runs one to the rc it must have."""

import os
import stat
import subprocess
import tempfile
import unittest

TOOLS = os.path.dirname(os.path.abspath(__file__))


def script(path, body):
    with open(path, "w") as f:
        f.write("#!/bin/sh\n" + body)
    os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
    return path


def run(args, **env):
    return subprocess.run(args, capture_output=True, text=True, env={**os.environ, **env}, timeout=120)


class GateHoles(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.d = self.dir.name

    def tearDown(self):
        self.dir.cleanup()

    def test_identity_where_both_compilers_refuse_everything_fails(self):
        """Two builds that each exit 3 for every program printed only REFUSED rows and exited 0."""
        a = script(os.path.join(self.d, "a"), "exit 3\n")
        b = script(os.path.join(self.d, "b"), "# distinct\nexit 3\n")
        r = run(["bash", f"{TOOLS}/identity.sh", "nib", a, b])
        self.assertNotEqual(r.returncode, 0, r.stdout[-300:])

    def _cc_check_env(self, **extra):
        cc = script(os.path.join(self.d, "cc"), 'for a; do [ "$p" = -o ] && : > "$a"; p=$a; done\nexit 0\n')
        clang = script(os.path.join(self.d, "clang"), 'for a; do [ "$p" = -o ] && : > "$a"; p=$a; done\nexit 0\n')
        link = script(os.path.join(self.d, "link"), ': > "$1"\n')
        src = os.path.join(self.d, "src")
        os.makedirs(src, exist_ok=True)
        return {"LLRM_CC": cc, "LLRM_LLC": cc, "LLRM_PIN_CLANG": clang, "LLRM_CC_REF_CLANG": clang,
                "LLRM_CC_REF_TRIPLE": "x86_64-linux-gnu", "LLRM_CC_REF_LINK": link, "LLRM_CC_DIR": src, **extra}

    def test_cc_check_with_no_programs_fails(self):
        """An empty program directory compared nothing and exited 0."""
        r = run(["sh", f"{TOOLS}/cc_check.sh"], **self._cc_check_env())
        self.assertNotEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_cc_check_with_a_missing_runner_is_not_ok(self):
        """LLRM_CC_RUN=missingbin ran neither side (rc 127, empty output on both) and printed `ok`."""
        env = self._cc_check_env(LLRM_CC_RUN="/nonexistent/runner")
        open(os.path.join(env["LLRM_CC_DIR"], "hello.c"), "w").write("int main(void){return 0;}\n")
        r = run(["sh", f"{TOOLS}/cc_check.sh", "hello"], **env)
        self.assertNotEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertNotIn("ok    hello", r.stdout)

    def _llc_check_root(self, build_ok):
        root = os.path.join(self.d, "root")
        os.makedirs(f"{root}/tools")
        os.makedirs(f"{root}/tests/llc")
        open(f"{root}/tests/llc/hello.c", "w").write("int main(void){return 0;}\n")
        for name in ("llc_check.sh", "ab_env.sh"):  # ab_env.sh: llc_check.sh sources it (#7506); without it every case exits 2
            with open(f"{TOOLS}/{name}") as f, open(f"{root}/tools/{name}", "w") as g:
                g.write(f.read())
        bin_dir = os.path.join(self.d, "bin")
        os.makedirs(bin_dir)
        script(f"{bin_dir}/buildslot", 'echo "error: build broke"\nexit 101\n' if not build_ok else "exit 0\n")
        # a clang that is gcc for the reference and writes a stub .ll; an llc that "compiles" to a ready object
        subprocess.run(["gcc", "-c", f"{root}/tests/llc/hello.c", "-o", f"{bin_dir}/ready.o"], check=True)
        clang = script(
            f"{bin_dir}/clang",
            'if [ "$1" = --version ]; then echo "clang version 24 d1106deb71cc"; exit 0; fi\n'
            'case "$*" in *-emit-llvm*) for a; do o=$a; done; : > "$o"; exit 0;; esac\n'
            'args=; skip=0; for a; do if [ $skip = 1 ]; then skip=0; continue; fi; '
            'case $a in -target) skip=1;; *) args="$args $a";; esac; done\n'
            f"exec gcc $args\n",
        )
        llc = script(f"{bin_dir}/llc", f'for a; do o=$a; done; cp {bin_dir}/ready.o "$o"\n')
        return root, bin_dir, clang, llc

    def test_llc_check_where_the_build_failed_does_not_run_the_stale_llc(self):
        """A failed `cargo build` was ignored (no pipefail): the stale llrm-llc ran and the table read `ok`."""
        root, bin_dir, clang, llc = self._llc_check_root(build_ok=False)
        r = run(["sh", f"{root}/tools/llc_check.sh", "hello"], PATH=f"{bin_dir}:{os.environ['PATH']}",
                LLRM_PIN_CLANG=clang, LLRM_LLC=llc)
        self.assertNotEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("build broke", r.stdout + r.stderr)  # it failed for its own reason, not for a missing sourced file

    def test_llc_check_with_a_good_build_reads_ok(self):
        """Control: the stubs run a program end to end, so the failing cases above fail for their own reason."""
        root, bin_dir, clang, llc = self._llc_check_root(build_ok=True)
        r = run(["sh", f"{root}/tools/llc_check.sh", "hello"], PATH=f"{bin_dir}:{os.environ['PATH']}",
                LLRM_PIN_CLANG=clang, LLRM_LLC=llc)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn(" ok ", r.stdout)

    def test_llc_check_with_no_programs_fails(self):
        root, bin_dir, clang, llc = self._llc_check_root(build_ok=True)
        os.remove(f"{root}/tests/llc/hello.c")
        r = run(["sh", f"{root}/tools/llc_check.sh"], PATH=f"{bin_dir}:{os.environ['PATH']}",
                LLRM_PIN_CLANG=clang, LLRM_LLC=llc, LLRM_NO_BUILD="1")
        self.assertNotEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_cells_where_every_cell_is_empty_fails(self):
        """Dists that fail every compile printed `EMPTY` for each cell and exited 0."""
        for side in ("base", "new"):
            os.makedirs(f"{self.d}/{side}")
            for name in ("llrm-cc", "llrm-llc"):
                script(f"{self.d}/{side}/{name}", f"# {side} {name}\nexit 1\n")
        cell = os.path.join(self.d, "list")
        open(cell, "w").write(f"x x86_32-linux -O2 {self.d}/hello.c\n")
        r = run(["bash", f"{TOOLS}/ab/cells.sh", self.d, cell])
        self.assertIn("EMPTY", r.stdout)
        self.assertNotEqual(r.returncode, 0, r.stdout)


if __name__ == "__main__":
    unittest.main()

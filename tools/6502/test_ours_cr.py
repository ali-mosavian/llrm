#!/usr/bin/env python3
"""tools/6502: the 6502 clock harness (needs OURS_RT, sim65, ca65/cl65; see the env defaults at the top of ours_cr.py).
A link order that followed set iteration (PYTHONHASHSEED) moved code across pages: float_in_place_update at -Os, same compiler,
same runtime, 201888 clocks under seed 0 and 201946 under seed 3 (2026-11-06), a +58 clock floor on every "worse cell" claim.
ours_cr.py without OURS_RT used a stale default runtime (/tmp/nv/w6/mos_rt_28a5.s): 8 programs failed to link at -O1..-O3
(the folded sweep, 2026-11-05) and an A/B could have compared one side against the wrong runtime. It must refuse and say why."""
import os, subprocess, sys, tempfile, unittest
from pathlib import Path
HERE = os.path.dirname(os.path.abspath(__file__))

def run(env_rt, script="ours_cr.py"):
    env = {k: v for k, v in os.environ.items() if k != "OURS_RT"}
    if env_rt is not None: env["OURS_RT"] = env_rt
    return subprocess.run([sys.executable, os.path.join(HERE, script), "/nonexistent/llrm-cc", "O2", "/dev/null", "__empty__"],
                          capture_output=True, text=True, env=env, timeout=120)

class OursCr(unittest.TestCase):
    def test_refuses_without_ours_rt(self):
        r = run(None)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("OURS_RT", r.stderr, r.stderr[-300:])
        self.assertNotIn("Traceback", r.stderr)

    def test_refuses_a_missing_runtime_file(self):
        r = run("/nonexistent/mos_rt.s")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("OURS_RT", r.stderr, r.stderr[-300:])
        self.assertNotIn("Traceback", r.stderr)

    def test_the_sibling_harnesses_refuse_too(self):
        for script in ("ours.py", "ours_keep.py", "ours.multi.py"):
            r = run(None, script)
            self.assertNotEqual(r.returncode, 0, script)
            self.assertIn("OURS_RT is not set", r.stderr, f"{script}: {r.stderr[-200:]}")

ROOT = os.path.join(HERE, "..", "..")
CORPUS = os.environ.get("OURS_CORPUS", os.path.join(ROOT, "tests", "run", "c"))
OVERLAY = os.path.join(HERE, "overlay")

class Corpus(unittest.TestCase):
    """The 82-program table is measured on tests/run/c; a second copy drifted from it in 4 files (2026-11-06) and a number on it is
    not a number on the repo corpus. Only what must differ lives in overlay/."""
    def test_every_program_is_in_the_corpus_or_the_overlay(self):
        for name in open(os.path.join(HERE, "pass82.txt")).read().split():
            home = OVERLAY if os.path.exists(os.path.join(OVERLAY, name + ".c")) else CORPUS
            for ext in ("c", "out"):
                self.assertTrue(os.path.exists(os.path.join(home, f"{name}.{ext}")), f"{name}.{ext} is in neither {CORPUS} nor the overlay")

    def test_an_overlay_file_differs_from_its_corpus_twin(self):
        for f in sorted(os.listdir(OVERLAY)):
            twin = os.path.join(CORPUS, f)
            if os.path.exists(twin):
                self.assertNotEqual(open(os.path.join(OVERLAY, f), "rb").read(), open(twin, "rb").read(), f"overlay/{f} copies its tests/run/c twin: drift trap")

CC = os.environ.get("OURS_TEST_CC", os.path.expanduser("~/scratch/s4/frozen-setcc/lchead/llrm-cc"))
RT = os.environ.get("OURS_TEST_RT", os.path.expanduser("~/scratch/s4/fa/lanecopy/rt_head.s"))

@unittest.skipUnless(os.path.exists(CC) and os.path.exists(RT), "needs a llrm-cc and a runtime (OURS_TEST_CC, OURS_TEST_RT)")
class Deterministic(unittest.TestCase):
    def test_clocks_do_not_depend_on_the_hash_seed(self):
        import tempfile
        got = []
        for seed in ("0", "3"):
            out = tempfile.mktemp(suffix=".tsv")
            env = dict(os.environ, PYTHONHASHSEED=seed, OURS_RT=RT, OURS_PADS="0", OURS_FLAGS="honor")
            r = subprocess.run([sys.executable, os.path.join(HERE, "ours_cr.py"), CC, "Os", out, "float_in_place_update"],
                               capture_output=True, text=True, env=env, timeout=300)
            self.assertEqual(r.returncode, 0, r.stderr[-300:])
            got.append(open(out).read())
        self.assertEqual(got[0], got[1], "the link order followed the hash seed")

@unittest.skipUnless(os.path.exists(CC) and os.path.exists(RT), "needs a llrm-cc and a runtime (OURS_TEST_CC, OURS_TEST_RT)")
class Members(unittest.TestCase):
    """The harness linked whole library modules: shift_int64_variable pulled 9676 B of i64 mul/div/mod for 1529 B of shifts (11300 B against
    gcc's 6393, 2026-11-06). A program that uses shifts links the shift members, and each member defines its own routine only."""
    def test_a_shift_program_links_only_shifts(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "o.tsv")
            r = subprocess.run([sys.executable, os.path.join(HERE, "ours_cr.py"), CC, "Os", out, "shift_int64_variable"],
                               capture_output=True, text=True, env={**os.environ, "OURS_RT": RT, "OURS_PADS": "0"}, timeout=600)
            self.assertEqual(r.returncode, 0, r.stderr[-300:])
            size = int(open(out).read().split("\t")[2])
        self.assertLess(size, 4000, f"{size} B: the link carried routines the program does not call")

    def test_a_member_defines_one_routine(self):
        sys.path.insert(0, HERE)
        import crlib
        rt = Path(HERE).resolve().parents[1] / "runtime" / "compiler-rt"
        self.assertEqual(crlib.index(rt)["__ashldi3"], "lib/int/ashldi3.c")
        text = crlib.member(rt, CC, "lib/int/ashldi3.c", ["-Os"])
        self.assertIn("__ashldi3:", text)
        for other in ("__udivdi3", "__muldi3", "__lshrdi3"):
            self.assertNotIn(other + ":", text)

if __name__ == "__main__": unittest.main()

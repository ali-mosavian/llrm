#!/usr/bin/env python3
"""tools/6502: the 6502 clock harness (needs OURS_RT, sim65, ca65/cl65; see the env defaults at the top of ours_cr.py).
A link order that followed set iteration (PYTHONHASHSEED) moved code across pages: float_in_place_update at -Os, same compiler,
same runtime, 201888 clocks under seed 0 and 201946 under seed 3 (2026-11-06), a +58 clock floor on every "worse cell" claim.
ours_cr.py without OURS_RT used a stale default runtime (/tmp/nv/w6/mos_rt_28a5.s): 8 programs failed to link at -O1..-O3
(the folded sweep, 2026-11-05) and an A/B could have compared one side against the wrong runtime. It must refuse and say why."""
import os, re, subprocess, sys, tempfile, unittest
from pathlib import Path
HERE = os.path.dirname(os.path.abspath(__file__))

def run(env_rt, script="ours_cr.py"):
    env = {k: v for k, v in os.environ.items() if k != "OURS_RT"}
    if env_rt is not None: env["OURS_RT"] = env_rt
    return subprocess.run([sys.executable, os.path.join(HERE, script), "/nonexistent/llrm-cc", "O2", "/dev/null", "__empty__"],
                          capture_output=True, text=True, env=env, timeout=120)

def header_flags_fn():
    src = open(os.path.join(HERE, "ours_cr.py")).read()
    ns = {}
    exec("import re\n" + src[src.index("X86_ONLY"):src.index("def one(job):")], ns)
    return ns["header_flags"]

class HeaderFlags(unittest.TestCase):
    """ours_cr compiled every program without its `// flags:` -f options: 24 of 212 programs (12 with
    -fno-inline-functions, tail_calls among them) were measured inlined when the test says they are not (2026-11-08).
    Their rows in the tip table, the shift-RMW and trial-unroll A/Bs were withdrawn."""
    def write(self, header):
        f = tempfile.NamedTemporaryFile("w", suffix=".c", delete=False); f.write(header + "\nint main(){return 0;}\n"); f.close()
        self.addCleanup(os.unlink, f.name); return f.name
    def test_f_options_pass_x86_only_options_drop(self):
        p = self.write("// flags: -O2 -march=i486 -fno-inline-functions | -O2 -m32 -mabi=sysv -fno-inline-functions -fwrapv")
        self.assertEqual(header_flags_fn()(p), ["-fno-inline-functions", "-fwrapv"])
    def test_unknown_option_is_an_error(self):
        with self.assertRaises(SystemExit): header_flags_fn()(self.write("// flags: -O2 -mfoo=bar"))
    def test_no_header_no_flags(self):
        self.assertEqual(header_flags_fn()(self.write("int x;")), [])

class PadSpan(unittest.TestCase):
    """The A/B scan ran with OURS_PADS=0,1,2,3: four copies of one layout. wide_shift_by_one read +3,377 clocks for -44 B,
    a page-crossing cliff (607,141 at pads 0..16, 619,097 at 200) that four neighbouring pads cannot see (2026-11-08)."""
    def pads(self, value):
        env = {k: v for k, v in os.environ.items()}
        env.update(OURS_RT="/nonexistent", OURS_PADS=value)
        return subprocess.run([sys.executable, os.path.join(HERE, "ours_cr.py"), "/nonexistent/llrm-cc", "O2", "/dev/null", "__empty__"],
                              capture_output=True, text=True, env=env, timeout=120)
    def test_neighbouring_pads_are_refused(self):
        r = self.pads("0,1,2,3")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("span", r.stderr)
    def test_spread_pads_are_accepted(self):
        self.assertNotIn("span", self.pads("0,37,101,200").stderr)

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

@unittest.skipUnless(os.path.exists(CC) and os.path.exists(RT), "needs a llrm-cc and a runtime (OURS_TEST_CC, OURS_TEST_RT)")
class Archive(unittest.TestCase):
    """The shipped runtime is an archive of per-routine objects and ld65 pulls members by undefined symbol: a program of three shifts links the
    three shifts (and the runtime routines they call), not the divides (11135 B when the i64 routines were one object; 2026-11-06)."""
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp()
        subprocess.run([sys.executable, os.path.join(HERE, "mkrt.py"), CC, os.path.join(cls.tmp, "rt.lib"), os.path.join(cls.tmp, "w")],
                       check=True, capture_output=True, timeout=1200)

    def link(self, program):
        sys.path.insert(0, HERE)
        import mkrt
        src = Path(HERE).resolve().parents[1] / "tests" / "run" / "c" / f"{program}.c"
        env = {**os.environ, "OURS_RT": RT, "OURS_PADS": "0"}
        driver = subprocess.run([sys.executable, "-c", "import ours_cr; print(ours_cr.driver())"], cwd=HERE, env=env, capture_output=True, text=True, check=True).stdout
        tmp = self.tmp
        Path(tmp, "d.s").write_text(driver)
        subprocess.run([CC, "--target=6502-sim65", "-Os", "-S", "-w", "-fsigned-char", "-I", os.path.join(HERE, "inc"), str(src), "-o", os.path.join(tmp, "o.s")], check=True)
        Path(tmp, "f.s").write_text(".autoimport +\n" + Path(tmp, "o.s").read_text())
        r = subprocess.run(["cl65", "-t", "sim6502", "-m", "x.map", "-o", "x.bin", "d.s", "f.s", "rt.lib"], cwd=tmp, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr[-300:])
        return mkrt.pulled(Path(tmp, "x.map").read_text())

    def test_a_shift_program_links_the_shifts_and_no_divide(self):
        got = self.link("shift_int64_variable")
        names = " ".join(got)
        for shift in ("ashldi3", "lshrdi3", "ashrdi3"):
            self.assertIn(shift, names)
        for divide in ("udivdi3", "divdi3", "muldi3"):
            self.assertNotIn(divide, names)
        self.assertLess(sum(got.values()), 4000, got)

    def test_the_text_level_runtime_reaches_what_the_archive_pulls(self):
        """Rust tests (and ours_cr) still paste `reachable_runtime`'s text where they price the assembly; it must stay the same set of
        routines as the link pulls. Comments are stripped first: `__ashrsi3_fast` reached memset, memcpy and memmove through the comment
        that introduces them (+ their bytes in every program that shifts a long by a variable)."""
        sys.path.insert(0, HERE)
        os.environ["OURS_RT"] = RT
        os.environ["OURS_PADS"] = "0"
        import mkrt, ours_cr
        text = mkrt.MOS_RT.read_text()
        ours_cr.RT = re.sub(r";.*", "", text)
        labels = {n: [x.strip() for x in re.search(r"^\.export (.*)$", t, re.M).group(1).split(",")] for n, t in mkrt.split_rt(text).items()}
        cells = {"__sa", "__ra", "__sb", "__st", "__sr", "__sg"}
        Path(self.tmp, "start.s").write_text(mkrt.startup())
        for routine in re.findall(r"^\.export (__\w+)", text, re.M):
            wanted = set(re.findall(r"^([A-Za-z_]\w*):", ours_cr.reachable_runtime(f"  jsr {routine}\n"), re.M)) - cells
            Path(self.tmp, "q.s").write_text(f".autoimport +\n.export _main\n_main:\n  jsr {routine}\n  rts\n")
            r = subprocess.run(["cl65", "-t", "sim6502", "-m", "q.map", "-o", "q.bin", "start.s", "q.s", "rt.lib"], cwd=self.tmp, capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr[-300:])
            got = set()
            for member in mkrt.pulled(Path(self.tmp, "q.map").read_text()):
                if member.startswith("mos_") and member != "mos___rt_cells.o":
                    got |= {x.removeprefix("__rt_") if x.startswith("__rt_") else x for x in labels[member[4:-2]]}
            self.assertEqual(wanted, got - cells, routine)

    def test_a_helper_two_routines_share_is_one_member(self):
        got = self.link("recdeep")
        self.assertEqual(sorted(n for n in got if "udivmod32" in n), ["mos___rt_udivmod32.o"], got)

LINK_CC = os.environ.get("OURS_TEST_LINK_CC", os.path.expanduser("~/scratch/s4/frozen-setcc/lnk/llrm-cc"))

@unittest.skipUnless(os.path.exists(LINK_CC), "needs a llrm-cc with the 6502-cc65 link recipe (OURS_TEST_LINK_CC)")
class DriverLink(unittest.TestCase):
    """`llrm-cc --target=6502-cc65 prog.c -o prog.bin` links through tools/targets/6502-link.sh: the runtime is an archive of per-routine
    objects and ld65 pulls the members a program calls. Before, the 6502 rows stopped at -S and the harnesses pasted the runtime's text."""
    def build(self, source):
        tmp = tempfile.mkdtemp()
        Path(tmp, "p.c").write_text(source)
        env = {**os.environ, "LLRM_6502_MAP": os.path.join(tmp, "p.map")}
        r = subprocess.run([LINK_CC, "--target=6502-cc65", "-Os", os.path.join(tmp, "p.c"), "-o", os.path.join(tmp, "p.bin")],
                           capture_output=True, text=True, env=env, timeout=1500)
        self.assertEqual(r.returncode, 0, r.stderr[-400:])
        sys.path.insert(0, HERE)
        import mkrt
        members = mkrt.pulled(Path(tmp, "p.map").read_text())
        code = subprocess.run(["sim65", os.path.join(tmp, "p.bin")], capture_output=True, timeout=60).returncode
        return code, members

    def test_a_multiply_program_links_the_multiply_and_no_divide_or_shift(self):
        code, members = self.build("unsigned long a = 70000, b = 3;\nint main(void) { return (int)((a * b) >> 8) & 0x7f; }\n")
        self.assertEqual(code, ((70000 * 3) >> 8) & 0x7F)
        self.assertIn("mos___mulsi3.o", members)
        self.assertEqual([m for m in members if "div" in m or "mod" in m or "di3" in m], [], members)

    def test_a_shift_program_links_the_three_shifts_and_no_other_i64_routine(self):
        code, members = self.build("unsigned long long v = 0x0123456789ABCDEFULL;\nvolatile unsigned c = 13;\n"
                                   "int main(void) { return (int)(((v << c) >> 56) ^ (v >> c) ^ (unsigned long long)((long long)v >> c)) & 0x7f; }\n")
        v, c, m64 = 0x0123456789ABCDEF, 13, (1 << 64) - 1
        self.assertEqual(code, ((((v << c) & m64) >> 56) ^ (v >> c) ^ (v >> c)) & 0x7F)
        i64 = sorted(m for m in members if m.startswith("lib_int_"))
        self.assertEqual(i64, ["lib_int_ashldi3.o", "lib_int_ashrdi3.o", "lib_int_lshrdi3.o"], members)

@unittest.skipUnless(os.path.exists(CC) and os.path.exists(RT), "needs a llrm-cc and a runtime (OURS_TEST_CC, OURS_TEST_RT)")
class RtLib(unittest.TestCase):
    """OURS_RTLIB makes ours_cr link the shipped archive, so the byte column is the product's. idiv_loop_os calls udivmod16, whose branch to
    udivmod16c is a jmp across objects there: +3 B against the harness's pasted text (574 against 571, 2026-11-06). The harness figure
    was reported as the product's until then."""
    def size(self, rtlib):
        with tempfile.TemporaryDirectory() as tmp:
            out = os.path.join(tmp, "o.tsv")
            env = {**os.environ, "OURS_RT": RT, "OURS_PADS": "0"}
            env.pop("OURS_RTLIB", None)
            if rtlib:
                env["OURS_RTLIB"] = rtlib
            r = subprocess.run([sys.executable, os.path.join(HERE, "ours_cr.py"), CC, "Os", out, "idiv_loop_os"], capture_output=True, text=True, env=env, timeout=900)
            self.assertEqual(r.returncode, 0, r.stderr[-300:])
            return int(open(out).read().split("\t")[2])

    def test_the_archive_link_is_the_product_figure(self):
        with tempfile.TemporaryDirectory() as tmp:
            lib = os.path.join(tmp, "rt.lib")
            subprocess.run([sys.executable, os.path.join(HERE, "mkrt.py"), CC, lib, os.path.join(tmp, "w")], check=True, capture_output=True, timeout=1200)
            self.assertEqual(self.size(lib) - self.size(None), 3)

if __name__ == "__main__": unittest.main()

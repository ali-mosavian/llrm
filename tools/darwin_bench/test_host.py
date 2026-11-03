"""The DOS-flavoured bench programs (huge, lru, textfill, jpeg_*, stft_*) did not compile on arm64-macos at all (far/huge, <io.h>, missing
input files): 7 of 37 programs had no row. With host.py each builds with pin and gcc-13 and prints the .out value, equal on both
(stft_single excepted: its libm twiddles differ from the x86 expected, so only both-compilers-agree is checked there)."""
import os, re, subprocess, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import host
H = os.path.dirname(os.path.abspath(__file__)); REPO = os.environ.get("BENCH_REPO", os.path.abspath(H + "/../.."))
PIN = os.environ["LLRM_PIN_CLANG"]; GCC = "/opt/homebrew/bin/gcc-13"
SDK = subprocess.check_output(["xcrun", "--show-sdk-path"], text=True).strip()
SEVEN = ["huge", "lru", "textfill", "jpeg_fixed", "jpeg_single", "stft_fixed", "stft_single"]

def run(cc, prog, work, flags):
    d = f"{work}/{prog}.{os.path.basename(cc)}"; os.makedirs(d, exist_ok=True)
    objs = host.objects(PIN, SDK, work + "/host", REPO)
    drv = f"{work}/driver.o"; subprocess.run([PIN, "-isysroot", SDK, "-O2", "-c", H + "/driver.c", "-o", drv], check=True)
    r = subprocess.run([cc, "-isysroot", SDK, "-O2", "-w", *(["-mmacosx-version-min=14.0"] if cc == GCC else []), "-I", f"{REPO}/bench", *host.include_flags(REPO, prog), *flags,
                        "-Dmain=orig_main", "-c", f"{REPO}/bench/{prog}/{prog}.c", "-o", f"{d}/k.o"], capture_output=True, text=True)
    if r.returncode: return "compile: " + r.stderr[:200]
    subprocess.run([PIN, "-isysroot", SDK, f"{d}/k.o", drv, *objs, "-o", f"{d}/t"], check=True, capture_output=True)
    host.write_data(REPO, prog, d)
    return subprocess.run([f"{d}/t", "1", "1", "once"], capture_output=True, text=True, cwd=d).stdout.split()

class T(unittest.TestCase):
    def test_seven_build_and_agree(self):
        with tempfile.TemporaryDirectory() as w:
            for prog in SEVEN:
                got = {cc: run(cc, prog, w, host.FLAT) for cc in (PIN, GCC)}
                for cc, g in got.items(): self.assertTrue(g and all(x.lstrip("-").isdigit() for x in g), (prog, cc, g))
                self.assertEqual(got[PIN], got[GCC], prog)
                if prog != "stft_single":
                    want = [int(x) % 2**32 for x in open(f"{REPO}/bench/{prog}/{prog}.out").read().split()]
                    self.assertEqual([int(x) % 2**32 for x in got[PIN]], want, prog)
if __name__ == "__main__": unittest.main()

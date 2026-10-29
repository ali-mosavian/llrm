"""Clocks, taken branches and jumps of ours against the pin's llc, with and without a pass's switch, on the vsgcc programs at m32.

    uv run --project tools python tools/pairs/bench_pin.py LLRM_CC LEVEL(-O2|-O3) SWITCH [PROG...]

Our IR (the `switchconv` stage of the wrapped program, -mabi=sysv) goes to our back end (OMF) and to the pin's `llc -O2 -filetype=obj` with
and without SWITCH (ELF, linked as the gcc and clang variants are); crates/target/llrm-x86-m32/vsgcc/harness.py runs all three in its 486
model. Prints per program and in total: clocks, instructions, taken branches, `jmp`s executed. VSGCC_WORK (default ~/scratch/w3-bench-work)
holds the products; the stub is made there as run.sh makes it.
"""
import glob, os, re, shutil, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
VS = REPO / "crates/target/llrm-x86-m32/vsgcc"
WORK = Path(os.environ.setdefault("VSGCC_WORK", str(Path.home() / "scratch/w3-bench-work")))
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(VS))
sys.path.insert(0, str(REPO / "tools"))
import pin  # noqa: E402

LLRM, LEVEL, SWITCH, *ONLY = sys.argv[1:]
os.environ["LLRM_CC_TARGET"] = "x86_32-wat"  # llrm-cc takes its target row from the environment
EMU = subprocess.run(["python3", str(REPO / "tools/linkrecipe.py"), "x86-m32", "ld-emulation"], capture_output=True, text=True).stdout.strip()
of = pin.target("m32")
LLC = str(pin.PINNED_BIN / "llc")


def stub():
    (WORK / "b").mkdir(parents=True, exist_ok=True)
    (WORK / "o").mkdir(parents=True, exist_ok=True)
    if not (WORK / "stub.elf").exists():
        subprocess.run(["gcc", "-m32", "-c", str(VS / "stub.s"), "-o", str(WORK / "stub.o")], check=True)
        subprocess.run(["ld", "-m", EMU, "-static", "-e", "0", "-Ttext=0x8000", "-o", str(WORK / "stub.elf"), str(WORK / "stub.o")], check=True)
        (WORK / "stub.elf.src").write_text(__import__("hashlib").sha256((VS / "stub.s").read_bytes()).hexdigest())


def build(prog):
    env = dict(os.environ, LLRM=LLRM, VSGCC_ONLY="llrm")
    subprocess.run([str(VS / "build.sh"), prog], env=env, check=True, capture_output=True)
    src = WORK / "b" / f"{prog}.c"
    w = tempfile.mkdtemp(dir=str(WORK))
    try:
        r = subprocess.run([LLRM, "-m32", "-mabi=sysv", LEVEL, "-march=i486", "-S", "--dump", w, "-o", w + "/o.s", str(src)], capture_output=True, text=True)
        st = glob.glob(w + "/*-switchconv.ll")
        if r.returncode or not st:
            return False
        ir = re.sub(r" tre-deferred| threeway", "", open(st[0]).read())
        text = pin.retarget(ir, of)
        rt = subprocess.run(["gcc", "-m32", "-print-libgcc-file-name"], capture_output=True, text=True).stdout.strip()
        for name, extra in (("pinwith", []), ("pinwithout", [SWITCH])):
            obj = WORK / "b" / f"{prog}.{name}.o"
            d = subprocess.run([LLC, f"-mtriple={of.triple}", f"-mcpu={of.cpu}", f"-mattr={of.features}", "-O2", "-filetype=obj", *extra, "-o", str(obj), "-"], input=text, capture_output=True, text=True)
            if d.returncode:
                return False
            link = subprocess.run(["ld", "-m", EMU, "-static", "-e", "0", f"-Ttext={pin.LINK_TEXT_BASE:#x}", f"--just-symbols={WORK / 'stub.elf'}", "-o", str(WORK / "b" / f"{prog}.{name}.elf"), str(obj), rt], capture_output=True, text=True)
            if link.returncode:
                return False
        return True
    finally:
        shutil.rmtree(w, ignore_errors=True)


stub()
import harness  # noqa: E402
from iced_x86 import Mnemonic  # noqa: E402

ours_variant = "llrm" if LEVEL == "-O2" else "llrmO3"
progs = ONLY or subprocess.run(["python3", str(VS / "programs.py")], capture_output=True, text=True).stdout.split()
rows = []
for prog in progs:
    if not build(prog):
        print(prog, "not built", file=sys.stderr)
        continue
    row = [prog]
    for variant in (ours_variant, "pinwith", "pinwithout"):
        r = harness.run(prog, variant)
        taken = jmps = 0
        for (a, b), n in r["edges"].items():
            ins = r["cache"][a][0]
            if b != a + ins.len:
                taken += n
                if ins.mnemonic == Mnemonic.JMP:
                    jmps += n
        row.append(dict(clocks=r["clocks"], ins=r["ins"], taken=taken, jmp=jmps, ok=r["reports"]))
    if not (row[1]["ok"] == row[2]["ok"] == row[3]["ok"]):
        print(prog, "reports differ", file=sys.stderr)
        continue
    rows.append(row)
if os.environ.get("BENCH_PIN_JSON"):
    import json
    Path(os.environ["BENCH_PIN_JSON"]).write_text(json.dumps(rows))
tot = lambda i, k: sum(r[i][k] for r in rows)
print(f"{len(rows)}/{len(progs)} programs {LEVEL} m32, switch {SWITCH}")
for k in ("clocks", "ins", "taken", "jmp"):
    print(f"  {k:7} ours {tot(1, k)}  pin {tot(2, k)}  pin with {SWITCH} {tot(3, k)}")
for r in sorted(rows, key=lambda r: r[3]["clocks"] - r[2]["clocks"], reverse=True)[:8]:
    print("  ", r[0], {k: (r[1][k], r[2][k], r[3][k]) for k in ("clocks", "taken", "jmp")})

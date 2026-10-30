#!/usr/bin/env python3
"""The pinned opt's AArch64 cost model on scalar instructions, per cpu and cost kind: `python3 tti_oracle.py` writes
tti_oracle.txt (lines `cpu kind name cost`; names encode opcode, types, operand variant) and tti_intrinsics.txt (lines
`cpu kind callee cost`) beside it, after a header naming the opt. LLVM_BIN (default ~/scratch/llvm-build-aarch64/bin)
holds the opt; tti_tests.rs reads both files."""
import itertools, os, re, subprocess, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
OPT = os.path.join(os.path.expanduser(os.environ.get("LLVM_BIN", "~/scratch/llvm-build-aarch64/bin")), "opt")
INTS = ["i8", "i16", "i32", "i64", "i128"]
FLTS = ["float", "double"]
BIN_INT = ["add", "sub", "mul", "sdiv", "udiv", "srem", "urem", "shl", "lshr", "ashr", "and", "or", "xor"]
BIN_FLT = ["fadd", "fsub", "fmul", "fdiv", "frem"]
CPUS = ["generic", "apple-m1"]
KINDS = ["throughput", "latency", "code-size", "size-latency"]
VAR = {"reg": "%b", "c3": "3", "c7": "7", "c8": "8", "cm8": "-8", "c1": "1"}
DATALAYOUT = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
TRIPLE = "aarch64-unknown-linux-gnu"

# Every result is stored: an instruction nothing reads is priced as one that folds into its user (an `isExtFree` loop
# over no uses returns true), so an unread `zext` costs 0.
lines, order, types = [], [], {}


def result_type(text):
    words = text.split()
    if words[0] in ("icmp", "fcmp"):
        return "i1"
    if words[0] in ("trunc", "zext", "sext", "fpext", "fptrunc", "sitofp", "uitofp", "fptosi", "fptoui", "ptrtoint", "inttoptr", "bitcast"):
        return text.split(" to ")[1]
    if words[0] == "load":
        return words[1].rstrip(",")
    return words[3] if words[0] == "select" else words[1]


def add(name, text):
    lines.append(f"  %{name} = {text}")
    order.append(name)
    types[name] = result_type(text)


for ty in INTS:
    for op in BIN_INT:
        for v, val in VAR.items():
            if v != "reg" and op in ("or", "xor"):
                continue
            if op in ("shl", "lshr", "ashr") and v in ("cm8", "c7"):
                continue
            add(f"{op}.{ty}.{v}", f"{op} {ty} %a.{ty}, {val.replace('%b', '%b.' + ty)}")
for op in ("and", "or", "xor"):
    add(f"{op}.i1.reg", f"{op} i1 %a.i1, %b.i1")
for ty in FLTS:
    for op in BIN_FLT:
        add(f"{op}.{ty}.reg", f"{op} {ty} %a.{ty}, %b.{ty}")
    add(f"fneg.{ty}.reg", f"fneg {ty} %a.{ty}")
for a, b in itertools.permutations(INTS, 2):
    if int(a[1:]) > int(b[1:]):
        add(f"trunc.{a}.{b}", f"trunc {a} %a.{a} to {b}")
    else:
        add(f"zext.{a}.{b}", f"zext {a} %a.{a} to {b}")
        add(f"sext.{a}.{b}", f"sext {a} %a.{a} to {b}")
for i in INTS:
    for f in FLTS:
        for op in ("sitofp", "uitofp"):
            add(f"{op}.{i}.{f}", f"{op} {i} %a.{i} to {f}")
        for op in ("fptosi", "fptoui"):
            add(f"{op}.{f}.{i}", f"{op} {f} %a.{f} to {i}")
add("fpext.float.double", "fpext float %a.float to double")
add("fptrunc.double.float", "fptrunc double %a.double to float")
for ty in INTS:
    add(f"ptrtoint.{ty}", f"ptrtoint ptr %p to {ty}")
    add(f"inttoptr.{ty}", f"inttoptr {ty} %a.{ty} to ptr")
for s, d in (("i32", "float"), ("i64", "double"), ("float", "i32"), ("double", "i64")):
    add(f"bitcast.{s}.{d}", f"bitcast {s} %a.{s} to {d}")
for ty in INTS:
    for pred in ("eq", "ne", "slt", "ult", "sge"):
        add(f"icmp{pred}.{ty}", f"icmp {pred} {ty} %a.{ty}, %b.{ty}")
        add(f"icmp{pred}.{ty}.c0", f"icmp {pred} {ty} %a.{ty}, 0")
for ty in FLTS:
    for pred in ("oeq", "olt", "une", "uno", "ord", "ogt"):
        add(f"fcmp{pred}.{ty}", f"fcmp {pred} {ty} %a.{ty}, %b.{ty}")
for ty in INTS + FLTS + ["i1"]:
    add(f"select.{ty}", f"select i1 %c, {ty} %a.{ty}, {ty} %b.{ty}")
for ty in INTS + FLTS:
    add(f"load.{ty}", f"load {ty}, ptr %p")
add("load.ptr", "load ptr, ptr %p")
add("load.i256", "load i256, ptr %p")  # four parts: tells a fixed load latency from a scheduled one
ALL = INTS + FLTS + ["i1"]
args = ", ".join([f"{t} %a.{t}, {t} %b.{t}" for t in ALL] + ["i1 %c", "ptr %p"])
uses = "\n".join(f"  store {types[n]} %{n}, ptr %p" for n in order)
stores = uses + "\n" + "\n".join(f"  store {t} %a.{t}, ptr %p" for t in INTS + FLTS) + "\n  store ptr %p, ptr %p"
HEAD = f'target datalayout = "{DATALAYOUT}"\ntarget triple = "{TRIPLE}"\n'
scalar_ir = f"{HEAD}define void @f({args}) {{\n" + "\n".join(lines) + f"\n{stores}\n  ret void\n}}\n"

INTRINSICS = []


def intr(name, ret, a):
    INTRINSICS.append((name, ret, a))


for ty in INTS[:4]:
    for m in ["smax", "smin", "umax", "umin", "sadd.sat", "uadd.sat", "ssub.sat", "usub.sat"]:
        intr(f"llvm.{m}.{ty}", ty, [ty, ty])
    for m in ["sadd", "uadd", "ssub", "usub", "smul", "umul"]:
        intr(f"llvm.{m}.with.overflow.{ty}", "{ " + ty + ", i1 }", [ty, ty])
    for m in ["ctpop", "bitreverse"]:
        intr(f"llvm.{m}.{ty}", ty, [ty])
    if ty != "i8":
        intr(f"llvm.bswap.{ty}", ty, [ty])
    for m in ["ctlz", "cttz", "abs"]:
        intr(f"llvm.{m}.{ty}", ty, [ty, "i1"])
    intr(f"llvm.fshl.{ty}", ty, [ty, ty, ty])
    intr(f"llvm.fshr.{ty}", ty, [ty, ty, ty])
for f, t in (("f64", "double"), ("f32", "float")):
    for m in ["fabs", "sqrt", "sin", "cos", "log2", "exp2", "rint", "floor", "ceil", "trunc", "round", "nearbyint"]:
        intr(f"llvm.{m}.{f}", t, [t])
    for m in ["minnum", "maxnum", "minimum", "maximum", "copysign", "pow"]:
        intr(f"llvm.{m}.{f}", t, [t, t])
    intr(f"llvm.fma.{f}", t, [t, t, t])
    intr(f"llvm.fmuladd.{f}", t, [t, t, t])
    for r in ["i32", "i64"]:
        intr(f"llvm.lrint.{r}.{f}", r, [t])
intr("llvm.assume", "void", ["i1"])
intr("llvm.memset.p0.i64", "void", ["ptr", "i8", "i64", "i1"])
intr("llvm.memcpy.p0.p0.i64", "void", ["ptr", "ptr", "i64", "i1"])
intr("llvm.memmove.p0.p0.i64", "void", ["ptr", "ptr", "i64", "i1"])
vals = {"i8": "%b", "i16": "%a", "i32": "%w", "i64": "%q", "double": "%d", "float": "%f", "i1": "false", "ptr": "%p"}
body, decls = [], []
for n, (name, ret, a) in enumerate(INTRINSICS):
    cargs = ", ".join(f"{t} {vals[t]}" for t in a)
    body.append(f"  {'' if ret == 'void' else f'%v{n} = '}call {ret} @{name}({cargs})")
    decls.append(f"declare {ret} @{name}({', '.join(a)})")
intr_ir = (
    f"{HEAD}define void @g(i8 %b, i16 %a, i32 %w, i64 %q, double %d, float %f, ptr %p) {{\n"
    + "\n".join(body)
    + "\n  ret void\n}\n"
    + "\n".join(decls)
    + "\n"
)

tmp = tempfile.mkdtemp()
SCALAR = os.path.join(tmp, "scalar.ll")
INTR = os.path.join(tmp, "intr.ll")
open(SCALAR, "w").write(scalar_ir)
open(INTR, "w").write(intr_ir)

LINE = re.compile(r"Cost Model: Found an estimated cost of (-?\d+|Invalid) for instruction:\s+(?:%(\S+) = )?(.*)")


def run(cpu, kind, path):
    r = subprocess.run(
        [OPT, "-passes=print<cost-model>", "-disable-output", f"-mtriple={TRIPLE}", f"-mcpu={cpu}", f"-cost-kind={kind}", path],
        capture_output=True,
        text=True,
    )
    assert r.returncode == 0, r.stderr[:500]
    return [m.groups() for m in map(LINE.match, r.stderr.splitlines()) if m]


out, iout = [], []
for cpu in CPUS:
    for kind in KINDS:
        rows = run(cpu, kind, SCALAR)
        n = len(order)
        for (cost, _, _), name in zip(rows[:n], order):
            out.append(f"{cpu} {kind} {name} {cost}")
        for cost, name, text in rows[2 * n:]:
            if text.startswith("store"):
                out.append(f"{cpu} {kind} store.{text.split()[1]} {cost}")
        calls = [r for r in run(cpu, kind, INTR) if "call" in r[2]]
        assert len(calls) == len(INTRINSICS), (len(calls), len(INTRINSICS))
        for (cost, _, _), (name, _, _) in zip(calls, INTRINSICS):
            iout.append(f"{cpu} {kind} {name} {cost}")

version = subprocess.run([OPT, "--version"], capture_output=True, text=True).stdout.splitlines()[1].strip()
clang = subprocess.run([os.path.join(os.path.dirname(OPT), "clang"), "--version"], capture_output=True, text=True).stdout.split("\n")[0]
version += " " + (clang.split("llvm-project.git ")[1].rstrip(")")[:12] if "llvm-project.git " in clang else "")
open(os.path.join(HERE, "tti_oracle.txt"), "w").write(f"# {version}; python3 tti_oracle.py\n" + "\n".join(out) + "\n")
open(os.path.join(HERE, "tti_intrinsics.txt"), "w").write(f"# {version}; python3 tti_oracle.py\n" + "\n".join(iout) + "\n")
print(len(out), "rows", len(iout), "intrinsic rows", version)

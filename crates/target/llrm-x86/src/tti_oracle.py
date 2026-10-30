#!/usr/bin/env python3
"""The pinned opt's cost model on scalar instructions, per cpu and cost kind: `python3 tti_oracle.py` writes tti_oracle.txt
beside it, lines `cpu kind name cost` (names encode opcode, types, operand variant), after a header naming the opt.
LLVM_BIN (default ~/scratch/llvm-build/bin) holds the opt; the file is checked in and tti_tests.rs reads it."""
import re, subprocess, os, sys, itertools
HERE = os.path.dirname(os.path.abspath(__file__))
OPT = os.path.join(os.path.expanduser(os.environ.get("LLVM_BIN", "~/scratch/llvm-build/bin")), "opt")
INTS = ["i8", "i16", "i32", "i64"]
FLTS = ["float", "double", "x86_fp80"]
BIN_INT = ["add", "sub", "mul", "sdiv", "udiv", "srem", "urem", "shl", "lshr", "ashr", "and", "or", "xor"]
BIN_FLT = ["fadd", "fsub", "fmul", "fdiv", "frem"]
# (triple, datalayout, cpus): the 32-bit profiles, and the 64-bit one (x86-m64's `-march=x86-64`).
TARGETS = [
    ("i486-unknown-linux-gnu", "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128",
     ["i386", "i486", "pentium", "pentiumpro", "k6", "athlon", "core2"]),
    ("x86_64-unknown-linux-gnu", "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
     ["x86-64"]),
]
KINDS = ["throughput", "latency", "code-size", "size-latency"]
# operand variants of the second operand of an integer binary op
VAR = {"reg": "%b", "c3": "3", "c8": "8", "cm8": "-8", "c1": "1"}
lines = []; names = []
def add(name, text):
    lines.append(f"  %{name} = {text}"); names.append(name)
for ty in INTS:
    for op in BIN_INT:
        for v, val in VAR.items():
            if v != "reg" and op not in ("mul", "sdiv", "udiv", "srem", "urem", "shl", "lshr", "ashr", "add", "sub", "and"):
                continue
            if op in ("shl", "lshr", "ashr") and v in ("cm8",): continue
            add(f"{op}.{ty}.{v}", f"{op} {ty} %a.{ty}, {val.replace('%b', '%b.' + ty)}")
for ty in FLTS:
    for op in BIN_FLT:
        add(f"{op}.{ty}.reg", f"{op} {ty} %a.{ty}, %b.{ty}")
    add(f"fneg.{ty}.reg", f"fneg {ty} %a.{ty}")
CASTS = []
for a, b in itertools.permutations(INTS, 2):
    CASTS.append(("trunc" if int(a[1:]) > int(b[1:]) else None, a, b))
    CASTS.append(("zext" if int(a[1:]) < int(b[1:]) else None, a, b))
    CASTS.append(("sext" if int(a[1:]) < int(b[1:]) else None, a, b))
for op, a, b in [c for c in CASTS if c[0]]:
    add(f"{op}.{a}.{b}", f"{op} {a} %a.{a} to {b}")
for i in INTS:
    for f in FLTS:
        for op in ("sitofp", "uitofp"): add(f"{op}.{i}.{f}", f"{op} {i} %a.{i} to {f}")
        for op in ("fptosi", "fptoui"): add(f"{op}.{f}.{i}", f"{op} {f} %a.{f} to {i}")
for a, b in itertools.permutations(FLTS, 2):
    op = "fpext" if FLTS.index(a) < FLTS.index(b) else "fptrunc"
    add(f"{op}.{a}.{b}", f"{op} {a} %a.{a} to {b}")
for ty in INTS:
    add(f"ptrtoint.{ty}", f"ptrtoint ptr %p to {ty}"); add(f"inttoptr.{ty}", f"inttoptr {ty} %a.{ty} to ptr")
add("bitcast.i32.float", "bitcast i32 %a.i32 to float"); add("bitcast.i64.double", "bitcast i64 %a.i64 to double")
add("bitcast.float.i32", "bitcast float %a.float to i32"); add("bitcast.double.i64", "bitcast double %a.double to i64")
for ty in INTS:
    for pred in ("eq", "ne", "slt", "ult", "sge"):
        add(f"icmp{pred}.{ty}", f"icmp {pred} {ty} %a.{ty}, %b.{ty}")
        add(f"icmp{pred}.{ty}.c0", f"icmp {pred} {ty} %a.{ty}, 0")
for ty in FLTS:
    for pred in ("oeq", "olt", "une", "uno", "ord", "ogt"):
        add(f"fcmp{pred}.{ty}", f"fcmp {pred} {ty} %a.{ty}, %b.{ty}")
for ty in INTS + FLTS:
    add(f"select.{ty}", f"select i1 %c, {ty} %a.{ty}, {ty} %b.{ty}")
for ty in INTS + FLTS:
    add(f"load.{ty}", f"load {ty}, ptr %p")
args = ", ".join([f"{t} %a.{t}, {t} %b.{t}" for t in INTS + FLTS] + ["i1 %c", "ptr %p"])
body = "\n".join(lines)
stores = "\n".join(f"  store {t} %a.{t}, ptr %p" for t in INTS + FLTS)
import tempfile
out = []
for triple, datalayout, cpus in TARGETS:
  ir = f'target datalayout = "{datalayout}"\ntarget triple = "{triple}"\ndefine void @f({args}) {{\n{body}\n{stores}\n  ret void\n}}\n'
  SCALAR = os.path.join(tempfile.mkdtemp(), "scalar.ll")
  open(SCALAR, "w").write(ir)
  for cpu in cpus:
    for kind in KINDS:
        r = subprocess.run([OPT, "-passes=print<cost-model>", "-disable-output", f"-mtriple={triple}", f"-mcpu={cpu}", f"-cost-kind={kind}", SCALAR], capture_output=True, text=True)
        n = 0
        for l in r.stderr.splitlines():
            m = re.match(r"Cost Model: Found an estimated cost of (\d+|Invalid) for instruction:\s+(?:%(\S+) = )?(.*)", l)
            if not m: continue
            cost, name, text = m.groups()
            if name is None:
                name = "store." + text.split()[1]
            out.append(f"{cpu} {kind} {name} {cost}"); n += 1
        if kind == KINDS[0]: print(cpu, "instructions per run", n)
version = subprocess.run([OPT, "--version"], capture_output=True, text=True).stdout.splitlines()[1].strip()
clang = subprocess.run([os.path.join(os.path.dirname(OPT), "clang"), "--version"], capture_output=True, text=True).stdout.split("\n")[0]
version += " " + (clang.split("llvm-project.git ")[1].rstrip(")")[:12] if "llvm-project.git " in clang else "")
open(os.path.join(HERE, "tti_oracle.txt"), "w").write(f"# {version}; python3 tti_oracle.py\n" + "\n".join(out) + "\n")
print(len(out), "rows", version)

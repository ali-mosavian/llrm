#!/usr/bin/env python3
"""The pinned opt's `getInstructionCost` on whole instructions, per cpu and cost kind: `python3 instructioncost_oracle.py`
writes instructioncost_corpus.ll and instructioncost_oracle.txt beside it, lines `cpu kind function index cost` (index: the
instruction's place in the function), after a header naming the opt. LLVM_BIN (default ~/scratch/llvm-build/bin) holds the opt;
instructioncost_tests.rs reads both files."""
import os, re, subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
BIN = os.path.expanduser(os.environ.get("LLVM_BIN", "~/scratch/llvm-build/bin"))
OPT = os.path.join(BIN, "opt")
CPUS = ["i386", "i486", "pentium", "pentiumpro", "k6", "athlon", "core2"]
KINDS = ["throughput", "latency", "code-size", "size-latency"]
HEAD = 'target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"\ntarget triple = "i486-unknown-linux-gnu"\n'
G = "@g = global [8 x i16] zeroinitializer\n@s = global { i8, i32, [4 x i16] } zeroinitializer\n"

FUNCS = {}

def fn(name, args, body, extra=""):
    FUNCS[name] = f"define void @{name}({args}) {{\n{body}\n  ret void\n}}\n{extra}"

fn("arith", "i16 %a, i16 %b, i32 %w, i32 %x, double %d, float %f",
   """  %a1 = add i16 %a, %b
  %a2 = mul i16 %a, 8
  %a3 = sdiv i32 %w, 3
  %a4 = shl i32 %w, 4
  %a5 = and i16 %a, -8
  %a6 = fmul double %d, %d
  %a7 = fadd float %f, 1.0
  %a8 = fneg double %d
  %a9 = udiv i32 %w, %x""")
fn("casts", "i16 %a, i32 %w, double %d, float %f, ptr %p, ptr %q, ptr addrspace(2) %p2",
   """  %c1 = zext i16 %a to i32
  %c2 = sext i16 %a to i32
  %c3 = trunc i32 %w to i16
  %c4 = sitofp i32 %w to double
  %c5 = fptosi double %d to i32
  %c6 = fpext float %f to double
  %c7 = fptrunc double %d to float
  %c8 = ptrtoint ptr %p to i32
  %c9 = inttoptr i32 %w to ptr
  %c10 = bitcast float %f to i32
  %c11 = inttoptr i16 %a to ptr addrspace(2)
  %c12 = addrspacecast ptr addrspace(2) %p2 to ptr
  %c13 = ptrtoint ptr %q to i16
  %c14 = ptrtoint ptr %q to i64
  %k1 = load i16, ptr %q
  %c15 = sitofp i16 %k1 to float
  %k2 = load i8, ptr %q
  %c16 = uitofp i8 %k2 to double
  %k3 = load i32, ptr %q
  %c17 = sitofp i32 %k3 to double
  %l1 = load i16, ptr %q
  %c18 = sext i16 %l1 to i32
  %l2 = load i16, ptr %q
  %c19 = zext i16 %l2 to i32
  %l3 = load i8, ptr %q
  %c20 = sext i8 %l3 to i16
  %l4 = load i8, ptr %q
  %c21 = zext i8 %l4 to i32
  %l5 = load i32, ptr %q
  %c22 = zext i32 %l5 to i64
  %l6 = load i16, ptr %q
  %c23 = sext i16 %l6 to i64""")
fn("cmpsel", "i16 %a, i16 %b, i1 %c, i1 %e, double %d",
   """  %m1 = icmp slt i16 %a, %b
  %m2 = icmp eq i16 %a, 0
  %m3 = fcmp olt double %d, %d
  %s1 = select i1 %c, i16 %a, i16 %b
  %s2 = select i1 %c, i1 %e, i1 false
  %s3 = select i1 %c, i1 true, i1 %e
  %s4 = select i1 %m1, double %d, double %d""")
fn("memory", "ptr %p, i16 %a, i32 %w, double %d",
   """  %l1 = load i16, ptr %p
  %l2 = load i32, ptr %p
  %l3 = load double, ptr %p
  %l4 = load i32, ptr %p
  %l5 = trunc i32 %l4 to i16
  store i16 %a, ptr %p
  store i32 7, ptr %p
  store double %d, ptr %p
  %gi = getelementptr i32, ptr %p, i32 %w
  store i32 1, ptr %gi
  %gc = getelementptr i32, ptr %p, i32 2
  store i32 1, ptr %gc""")
fn("geps", "ptr %p, i16 %i, i32 %w, i32 %j",
   """  %g1 = getelementptr i16, ptr %p, i16 %i
  %g2 = getelementptr i16, ptr %p, i32 3
  %g3 = getelementptr i8, ptr %p, i32 %w
  %g4 = getelementptr [8 x i16], ptr %p, i32 %w, i32 %j
  %g5 = getelementptr i32, ptr %p
  %g6 = getelementptr i16, ptr @g, i16 %i
  %g7 = getelementptr i16, ptr @g, i32 3
  %g8 = getelementptr { i8, i32, [4 x i16] }, ptr %p, i32 0, i32 2, i32 %w
  %g9 = getelementptr { i8, i32, [4 x i16] }, ptr @s, i32 0, i32 1
  %g10 = getelementptr i32, ptr %p, i32 1000000
  %g11 = getelementptr i64, ptr %p, i32 %w
  %g12 = getelementptr i32, ptr %p, i32 %w
  %g13 = getelementptr i8, ptr %p, i32 -4
  %u1 = load i32, ptr %g12
  %g14 = getelementptr i16, ptr %p, i32 %j
  %u2 = load i32, ptr %g14""")
fn("flow", "i32 %w, i16 %a",
   """  switch i32 %w, label %d [ i32 1, label %x
                           i32 2, label %x ]
x:
  %p = phi i16 [ %a, %0 ], [ %a, %0 ]
  br label %e
d:
  br label %e
e:
  %q = phi i16 [ 1, %d ], [ %p, %x ]
  %al = alloca i32
  %an = alloca i32, i32 %w
  %fr = freeze i16 %q""".replace("%0", "%entry"))
FUNCS["flow"] = FUNCS["flow"].replace("define void @flow(i32 %w, i16 %a) {\n", "define void @flow(i32 %w, i16 %a) {\nentry:\n")
fn("calls", "ptr %p, i16 %a, i32 %w, double %d, ptr %q",
   """  %r1 = call i16 @ext(i16 %a, i16 %a)
  call void @ext0()
  %r2 = call double @sqrt(double %d)
  %r3 = call double @pow(double %d, double %d)
  %r4 = call i32 @in(i32 %w)
  %r5 = call i16 %q(i16 %a)
  %r6 = call i16 @llvm.smax.i16(i16 %a, i16 %a)
  call void @llvm.assume(i1 true)
  call void @llvm.memcpy.p0.p0.i32(ptr %p, ptr %p, i32 8, i1 false)
  call void @llvm.memset.p0.i32(ptr %p, i8 0, i32 %w, i1 false)""",
   """declare i16 @ext(i16, i16)
declare void @ext0()
declare double @sqrt(double)
declare double @pow(double, double)
define internal i32 @in(i32 %x) {
  ret i32 %x
}
""")

# Every intrinsic llrm knows, at the widths it meets.
INTRINSICS = []
def intr(name, ret, args):
    INTRINSICS.append((name, ret, args))
INTS = ["i8", "i16", "i32", "i64"]
FLOATS = [("f64", "double"), ("f32", "float")]
PTRS = {"p0": "ptr", "p1": "ptr addrspace(1)", "p2": "ptr addrspace(2)"}
for ty in INTS:
    for m in ["smax", "smin", "umax", "umin"]:
        intr(f"llvm.{m}.{ty}", ty, [ty, ty])
    for m in ["sadd", "uadd", "ssub", "usub", "smul", "umul"]:
        intr(f"llvm.{m}.with.overflow.{ty}", "{ " + ty + ", i1 }", [ty, ty])
    intr(f"llvm.ctpop.{ty}", ty, [ty]); intr(f"llvm.bswap.{ty}", ty, [ty]) if ty != "i8" else None
    intr(f"llvm.abs.{ty}", ty, [ty, "i1"])
    intr(f"llvm.cttz.{ty}", ty, [ty, "i1"])
    intr(f"llvm.smul.fix.{ty}", ty, [ty, ty, "i32"])
    intr(f"llvm.fshl.{ty}", ty, [ty, ty, ty]); intr(f"llvm.fshr.{ty}", ty, [ty, ty, ty])
for f, t in FLOATS:
    for m in ["fabs", "sqrt", "sin", "cos", "atan", "log2", "exp2", "rint"]:
        intr(f"llvm.{m}.{f}", t, [t])
    intr(f"llvm.fmuladd.{f}", t, [t, t, t])
    for r in ["i16", "i32"]:
        intr(f"llvm.lrint.{r}.{f}", r, [t])
# The cost model prints no price for `llvm.lifetime.*`: test_lifetime_and_assume_are_free covers them.
intr("llvm.assume", "void", ["i1"])
for dst, dty in PTRS.items():
    for ln in ["i16", "i32"]:
        intr(f"llvm.memset.{dst}.{ln}", "void", [dty, "i8", ln, "i1"])
        for src, sty in PTRS.items():
            intr(f"llvm.memcpy.{dst}.{src}.{ln}", "void", [dty, sty, ln, "i1"])
            intr(f"llvm.memmove.{dst}.{src}.{ln}", "void", [dty, sty, ln, "i1"])
body = []; decls = []
for n, (name, ret, args) in enumerate(INTRINSICS):
    vals = {"i8": "%b", "i16": "%a", "i32": "%w", "i64": "%q", "double": "%d", "float": "%f", "i1": "false",
            "ptr": "%p", "ptr addrspace(1)": "%p1", "ptr addrspace(2)": "%p2"}
    cargs = ", ".join(f"{t} {vals[t]}" for t in args)
    # SimplifyCFG's switch-of-powers-of-two prices `cttz(x, true)`.
    if name.startswith("llvm.cttz."): cargs = f"{args[0]} {vals[args[0]]}, i1 true"
    if ".fix." in name: cargs = f"{args[0]} {vals[args[0]]}, {args[1]} {vals[args[1]]}, i32 3"
    body.append(f"  {'' if ret == 'void' else f'%v{n} = '}call {ret} @{name}({cargs})")
    decls.append(f"declare {ret} @{name}({', '.join(args)})")
fn("intrinsics", "i8 %b, i16 %a, i32 %w, i64 %q, double %d, float %f, ptr %p, ptr addrspace(1) %p1, ptr addrspace(2) %p2", "\n".join(body), "\n".join(decls) + "\n")

corpus = HEAD + G + "".join(FUNCS.values())
open(os.path.join(HERE, "instructioncost_corpus.ll"), "w").write(corpus)
rows = []
for cpu in CPUS:
    for kind in KINDS:
        r = subprocess.run([OPT, "-passes=print<cost-model>", "-disable-output", "-mtriple=i486-unknown-linux-gnu",
                            f"-mcpu={cpu}", f"-cost-kind={kind}", os.path.join(HERE, "instructioncost_corpus.ll")],
                           capture_output=True, text=True)
        assert r.returncode == 0, r.stderr[:500]
        current, index = None, 0
        for line in r.stderr.splitlines():
            m = re.match(r"Printing analysis 'Cost Model Analysis' for function '(\S+)'", line)
            if m:
                current, index = m.group(1), 0; continue
            m = re.match(r"Cost Model: Found an estimated cost of (-?\d+|Invalid) for instruction:", line)
            if m and current:
                rows.append(f"{cpu} {kind} {current} {index} {m.group(1)}"); index += 1
# Intrinsics are priced from this table (T7): the same for every cpu without SSE.
table = {}
for row in rows:
    cpu, kind, function, index, cost = row.split()
    if function != "intrinsics" or int(index) >= len(INTRINSICS): continue
    name = INTRINSICS[int(index)][0]
    profile = "sse2" if cpu == "core2" else "nosse"
    key = (profile, kind, name)
    assert table.setdefault(key, cost) == cost, (key, cost, table[key])

# The matrix: every opcode at every type the targets meet, with variable and constant operands, loads as cast sources and
# stores of truncs, so that a price the corpus above does not hold cannot hide (`sext (load)` was 1 where the pin says 0).
MATRIX_CPUS = ["i486", "pentium"]
def matrix():
    ints = ["i8", "i16", "i32", "i64"]
    fls = ["float", "double", "x86_fp80"]
    args = "i1 %c, i8 %i8, i16 %i16, i32 %i32, i64 %i64, float %float, double %double, x86_fp80 %x86_fp80, ptr %p, ptr %q, i32 %n"
    fns = []
    def f(name, body, decls=""):
        fns.append(f"define void @{name}({args}) {{\n" + "\n".join("  " + l for l in body) + f"\n  ret void\n}}\n{decls}")
    n = [0]
    def v():
        n[0] += 1
        return f"%v{n[0]}"
    for t in ints:
        body = []
        for op in ["add", "sub", "mul", "sdiv", "udiv", "srem", "urem", "shl", "lshr", "ashr", "and", "or", "xor"]:
            for rhs in [f"%{t}", "8", "7", "-1"]:
                body.append(f"{v()} = {op} {t} %{t}, {rhs}")
        f(f"arith_{t}", body)
    for t in fls:
        body = []
        for op in ["fadd", "fsub", "fmul", "fdiv", "frem"]:
            for rhs in [f"%{t}", "2.0", "3.0"]:
                body.append(f"{v()} = {op} {t} %{t}, {rhs}")
        body.append(f"{v()} = fneg {t} %{t}")
        f(f"farith_{t}", body)
    for t in ints:
        body = []
        for pred in ["eq", "ne", "ugt", "uge", "ult", "ule", "sgt", "sge", "slt", "sle"]:
            for rhs in [f"%{t}", "0", "5"]:
                body.append(f"{v()} = icmp {pred} {t} %{t}, {rhs}")
        f(f"icmp_{t}", body)
    for t in fls:
        body = []
        for pred in ["oeq", "ogt", "oge", "olt", "ole", "one", "ord", "ueq", "ugt", "uge", "ult", "ule", "une", "uno"]:
            body.append(f"{v()} = fcmp {pred} {t} %{t}, %{t}")
        f(f"fcmp_{t}", body)
    body = []
    for t in ints + fls + ["ptr"]:
        a = "%p" if t == "ptr" else f"%{t}"
        body.append(f"{v()} = select i1 %c, {t} {a}, {t} {a}")
        cmp = v()
        body.append(f"{cmp} = icmp slt i32 %i32, %n")
        body.append(f"{v()} = select i1 {cmp}, {t} {a}, {t} {a}")
    f("selects", body)
    body = []
    allint = ["i1"] + ints
    for src in allint:
        for dst in allint:
            sv = "%c" if src == "i1" else f"%{src}"
            if dst == src: continue
            op = "trunc" if int(dst[1:]) < int(src[1:]) else None
            for o in (["trunc"] if op else ["zext", "sext"]):
                body.append(f"{v()} = {o} {src} {sv} to {dst}")
                if src != "i1":
                    l = v(); body.append(f"{l} = load {src}, ptr %p")
                    body.append(f"{v()} = {o} {src} {l} to {dst}")
        if src != "i1":
            for dst in [d for d in ints if int(d[1:]) < int(src[1:])]:
                t = v(); body.append(f"{t} = trunc {src} %{src} to {dst}")
                body.append(f"store {dst} {t}, ptr %q")
    f("intcasts", body)
    body = []
    for src in ints:
        for dst in fls:
            for o in ["sitofp", "uitofp"]:
                body.append(f"{v()} = {o} {src} %{src} to {dst}")
                l = v(); body.append(f"{l} = load {src}, ptr %p")
                body.append(f"{v()} = {o} {src} {l} to {dst}")
    for src in fls:
        for dst in ints:
            for o in ["fptosi", "fptoui"]:
                body.append(f"{v()} = {o} {src} %{src} to {dst}")
    body += [f"{v()} = fpext float %float to double", f"{v()} = fpext float %float to x86_fp80", f"{v()} = fpext double %double to x86_fp80",
             f"{v()} = fptrunc double %double to float", f"{v()} = fptrunc x86_fp80 %x86_fp80 to float", f"{v()} = fptrunc x86_fp80 %x86_fp80 to double",
             f"{v()} = bitcast i32 %i32 to float", f"{v()} = bitcast float %float to i32", f"{v()} = bitcast i64 %i64 to double", f"{v()} = bitcast double %double to i64"]
    for t in ["i16", "i32", "i64"]:
        body += [f"{v()} = ptrtoint ptr %p to {t}", f"{v()} = inttoptr {t} %{t} to ptr"]
    f("fpcasts", body)
    body = []
    for t in ints + fls + ["ptr"]:
        a = "%p" if t == "ptr" else f"%{t}"
        body.append(f"{v()} = load {t}, ptr %p")
        body.append(f"store {t} {a}, ptr %q")
        for idx in ["3", "%i32", "%i16", "%i64"]:
            ity = {"3": "i32", "%i32": "i32", "%i16": "i16", "%i64": "i64"}[idx]
            g = v(); body.append(f"{g} = getelementptr inbounds {t}, ptr %p, {ity} {idx}")
            body.append(f"{v()} = load {t}, ptr {g}")
            body.append(f"store {t} {a}, ptr {g}")
    for t in ["i8", "i16", "i32"]:
        body.append(f"store {t} 0, ptr %q")
        body.append(f"store {t} 1, ptr %q")
    body += [f"{v()} = getelementptr [8 x i32], ptr %p, i32 %i32, i32 %n", f"{v()} = getelementptr [8 x i32], ptr %p, i32 1, i32 2",
             f"{v()} = getelementptr {{ i8, i32, [4 x i16] }}, ptr %p, i32 %i32, i32 2, i32 %n",
             f"{v()} = getelementptr i8, ptr %p, i32 %i32"]
    f("memops", body)
    f("misc", [f"{v()} = freeze i32 %i32", f"{v()} = alloca i32", f"{v()} = alloca i32, i32 %n", f"{v()} = alloca [16 x i8]",
               f"{v()} = call i32 @ext(i32 %i32)", f"call void @ext0()", f"{v()} = call double @ext_d(double %double)",
               f"{v()} = call i32 @llvm.ctpop.i32(i32 %i32)", f"{v()} = call i16 @llvm.bswap.i16(i16 %i16)",
               f"{v()} = call float @llvm.fabs.f32(float %float)", f"{v()} = call double @llvm.sqrt.f64(double %double)",
               f"{v()} = call i32 @llvm.smax.i32(i32 %i32, i32 %n)", f"{v()} = call i64 @llvm.umin.i64(i64 %i64, i64 %i64)"],
      "declare i32 @ext(i32)\ndeclare void @ext0()\ndeclare double @ext_d(double)\ndeclare i32 @llvm.ctpop.i32(i32)\ndeclare i16 @llvm.bswap.i16(i16)\n"
      "declare float @llvm.fabs.f32(float)\ndeclare double @llvm.sqrt.f64(double)\ndeclare i32 @llvm.smax.i32(i32, i32)\ndeclare i64 @llvm.umin.i64(i64, i64)\n")
    return HEAD + "".join(fns)

MATRIX = matrix()
open(os.path.join(HERE, "instructioncost_matrix.ll"), "w").write(MATRIX)
mrows = []
for cpu in MATRIX_CPUS:
    for kind in KINDS:
        r = subprocess.run([OPT, "-passes=print<cost-model>", "-disable-output", "-mtriple=i486-unknown-linux-gnu",
                            f"-mcpu={cpu}", f"-cost-kind={kind}", os.path.join(HERE, "instructioncost_matrix.ll")],
                           capture_output=True, text=True)
        assert r.returncode == 0, r.stderr[:500]
        current, index = None, 0
        for line in r.stderr.splitlines():
            m = re.match(r"Printing analysis 'Cost Model Analysis' for function '(\S+)'", line)
            if m:
                current, index = m.group(1), 0; continue
            m = re.match(r"Cost Model: Found an estimated cost of (-?\d+|Invalid) for instruction:", line)
            if m and current:
                mrows.append(f"{cpu} {kind} {current} {index} {m.group(1)}"); index += 1
version = subprocess.run([OPT, "--version"], capture_output=True, text=True).stdout.splitlines()[1].strip()
open(os.path.join(HERE, "instructioncost_oracle.txt"), "w").write(f"# {version}; python3 instructioncost_oracle.py\n" + "\n".join(rows) + "\n")
open(os.path.join(HERE, "tti_intrinsics.txt"), "w").write(
    f"# {version}; python3 instructioncost_oracle.py\n" + "\n".join(f"{p} {k} {n} {c}" for (p, k, n), c in sorted(table.items())) + "\n")
open(os.path.join(HERE, "instructioncost_matrix.txt"), "w").write(f"# {version}; python3 instructioncost_oracle.py\n" + "\n".join(mrows) + "\n")
print(len(rows), "rows", len(mrows), "matrix rows", len(table), "intrinsic prices")

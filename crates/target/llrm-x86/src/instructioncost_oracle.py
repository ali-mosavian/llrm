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
fn("casts", "i16 %a, i32 %w, double %d, float %f, ptr %p",
   """  %c1 = zext i16 %a to i32
  %c2 = sext i16 %a to i32
  %c3 = trunc i32 %w to i16
  %c4 = sitofp i32 %w to double
  %c5 = fptosi double %d to i32
  %c6 = fpext float %f to double
  %c7 = fptrunc double %d to float
  %c8 = ptrtoint ptr %p to i32
  %c9 = inttoptr i32 %w to ptr
  %c10 = bitcast float %f to i32""")
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
  store double %d, ptr %p""")
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
def intr(name, ret, args, mangle=""):
    INTRINSICS.append((name, ret, args))
for ty in ["i16", "i32"]:
    for m in ["smax", "smin", "umax", "umin"]:
        intr(f"llvm.{m}.{ty}", ty, [ty, ty])
    for m in ["sadd", "uadd", "ssub", "usub", "smul", "umul"]:
        intr(f"llvm.{m}.with.overflow.{ty}", "{ " + ty + ", i1 }", [ty, ty])
for f, t in [("f64", "double"), ("f32", "float")]:
    for m in ["fabs", "sqrt", "sin", "cos", "log2", "exp2", "rint"]:
        intr(f"llvm.{m}.{f}", t, [t])
    intr(f"llvm.fmuladd.{f}", t, [t, t, t])
intr("llvm.lrint.i32.f64", "i32", ["double"])
intr("llvm.ctpop.i16", "i16", ["i16"]); intr("llvm.bswap.i32", "i32", ["i32"]); intr("llvm.abs.i16", "i16", ["i16", "i1"])
intr("llvm.smul.fix.i16", "i16", ["i16", "i16", "i32"])
# The cost model prints no price for `llvm.lifetime.*`: test_lifetime_and_assume_are_free covers them.
intr("llvm.assume", "void", ["i1"]); intr("llvm.memcpy.p0.p0.i32", "void", ["ptr", "ptr", "i32", "i1"]); intr("llvm.memmove.p0.p0.i32", "void", ["ptr", "ptr", "i32", "i1"])
intr("llvm.memset.p0.i32", "void", ["ptr", "i8", "i32", "i1"])
body = []; decls = []
for n, (name, ret, args) in enumerate(INTRINSICS):
    vals = {"i16": "%a", "i32": "%w", "double": "%d", "float": "%f", "i1": "false", "i64": "4", "ptr": "%p", "i8": "0"}
    cargs = ", ".join(f"{t} {vals[t]}" for t in args)
    if name.startswith("llvm.smul.fix"): cargs = "i16 %a, i16 %a, i32 3"
    body.append(f"  {'' if ret == 'void' else f'%v{n} = '}call {ret} @{name}({cargs})")
    decls.append(f"declare {ret} @{name}({', '.join(args)})")
fn("intrinsics", "i16 %a, i32 %w, double %d, float %f, ptr %p", "\n".join(body), "\n".join(decls) + "\n")

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
version = subprocess.run([OPT, "--version"], capture_output=True, text=True).stdout.splitlines()[1].strip()
open(os.path.join(HERE, "instructioncost_oracle.txt"), "w").write(f"# {version}; python3 instructioncost_oracle.py\n" + "\n".join(rows) + "\n")
open(os.path.join(HERE, "tti_intrinsics.txt"), "w").write(
    f"# {version}; python3 instructioncost_oracle.py\n" + "\n".join(f"{p} {k} {n} {c}" for (p, k, n), c in sorted(table.items())) + "\n")
print(len(rows), "rows", len(table), "intrinsic prices")

#!/usr/bin/env python3
"""The pinned opt's price of each scalar intrinsic for wasm32, per cost kind: `python3 tti_intrinsics_oracle.py` writes
tti_intrinsics.txt beside it, lines `kind name cost`. LLVM_BIN (default ~/scratch/llvm-build-clang-wasm/bin) holds an opt
built with the WebAssembly target."""
import os, re, subprocess, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
OPT = os.path.join(os.path.expanduser(os.environ.get("LLVM_BIN", "~/scratch/llvm-build-clang-wasm/bin")), "opt")
KINDS = ["throughput", "latency", "code-size", "size-latency"]
HEAD = 'target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"\ntarget triple = "wasm32-unknown-unknown"\n'

INTRINSICS = []
def intr(name, ret, args):
    INTRINSICS.append((name, ret, args))
for ty in ["i8", "i16", "i32", "i64"]:
    for m in ["smax", "smin", "umax", "umin"]:
        intr(f"llvm.{m}.{ty}", ty, [ty, ty])
    for m in ["sadd", "uadd", "ssub", "usub", "smul", "umul"]:
        intr(f"llvm.{m}.with.overflow.{ty}", "{ " + ty + ", i1 }", [ty, ty])
    for m in ["sadd", "uadd", "ssub", "usub", "sshl", "ushl"]:
        intr(f"llvm.{m}.sat.{ty}", ty, [ty, ty])
    intr(f"llvm.ctpop.{ty}", ty, [ty])
    if ty != "i8":
        intr(f"llvm.bswap.{ty}", ty, [ty])
    intr(f"llvm.abs.{ty}", ty, [ty, "i1"])
    intr(f"llvm.ctlz.{ty}", ty, [ty, "i1"])
    intr(f"llvm.cttz.{ty}", ty, [ty, "i1"])
    intr(f"llvm.fshl.{ty}", ty, [ty, ty, ty])
    intr(f"llvm.fshr.{ty}", ty, [ty, ty, ty])
    intr(f"llvm.bitreverse.{ty}", ty, [ty])
for w in ["i32", "i64"]:
    intr(f"llvm.memset.p0.{w}", "void", ["ptr", "i8", w, "i1"])
    intr(f"llvm.memcpy.p0.p0.{w}", "void", ["ptr", "ptr", w, "i1"])
    intr(f"llvm.memmove.p0.p0.{w}", "void", ["ptr", "ptr", w, "i1"])
for f, t in [("f64", "double"), ("f32", "float")]:
    for m in ["fabs", "sqrt", "sin", "cos", "log", "log2", "exp", "exp2", "pow", "floor", "ceil", "trunc", "rint",
              "nearbyint", "round", "roundeven", "minnum", "maxnum", "copysign", "fma", "fmuladd", "minimum", "maximum"]:
        n = {"pow": 2, "minnum": 2, "maxnum": 2, "copysign": 2, "minimum": 2, "maximum": 2, "fma": 3, "fmuladd": 3}.get(m, 1)
        intr(f"llvm.{m}.{f}", t, [t] * n)
    for r in ["i32", "i64"]:
        intr(f"llvm.lrint.{r}.{f}", r, [t])
        intr(f"llvm.lround.{r}.{f}", r, [t])
VALS = {"ptr": "null", "i8": "%b", "i16": "%a", "i32": "%w", "i64": "%q", "double": "%d", "float": "%f", "i1": "false"}
body, decls = [], []
for n, (name, ret, args) in enumerate(INTRINSICS):
    cargs = ", ".join(f"{t} {VALS[t]}" for t in args)
    body.append(f"  {'' if ret == 'void' else f'%v{n} = '}call {ret} @{name}({cargs})")
    decls.append(f"declare {ret} @{name}({', '.join(args)})")
ir = HEAD + "define void @intrinsics(i8 %b, i16 %a, i32 %w, i64 %q, double %d, float %f) {\n" + "\n".join(body) + "\n  ret void\n}\n" + "\n".join(decls) + "\n"
table = {}
with tempfile.NamedTemporaryFile("w", suffix=".ll") as file:
    file.write(ir); file.flush()
    for kind in KINDS:
        r = subprocess.run([OPT, "-passes=print<cost-model>", "-disable-output", f"-cost-kind={kind}", file.name],
                           capture_output=True, text=True)
        assert r.returncode == 0, r.stderr[:500]
        index = 0
        for line in r.stderr.splitlines():
            m = re.match(r"Cost Model: Found an estimated cost of (-?\d+|Invalid) for instruction:", line)
            if m and index < len(INTRINSICS):
                table[(kind, INTRINSICS[index][0])] = m.group(1); index += 1
        assert index == len(INTRINSICS), (kind, index)
version = subprocess.run([OPT, "--version"], capture_output=True, text=True).stdout.splitlines()[1].strip()
open(os.path.join(HERE, "tti_intrinsics.txt"), "w").write(
    f"# {version}; python3 tti_intrinsics_oracle.py\n" + "\n".join(f"{k} {n} {c}" for (k, n), c in sorted(table.items())) + "\n")
print(len(table), "intrinsic prices")

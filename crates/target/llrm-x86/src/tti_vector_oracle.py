#!/usr/bin/env python3
"""The pinned opt's cost of the vector instructions a float loop is made of at x86-64 (`-mcpu=x86-64`: SSE2), per cost kind:
`python3 tti_vector_oracle.py` writes tti_vector_oracle.txt beside it, lines `kind op type cost` (a cast: `kind op dst src cost`),
after a header naming the opt. tti_vector.rs includes the file: the rows are the pin's `X86TTIImpl::getCastInstrCost`,
`getArithmeticInstrCost` and `getMemoryOpCost`, read off. LLVM_BIN (required) is the directory of the pinned opt."""
import os, re, subprocess, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
if "LLVM_BIN" not in os.environ:
    raise SystemExit("LLVM_BIN is not set: it names the directory of the pinned opt (a build of the pin)")
OPT = os.path.join(os.path.expanduser(os.environ["LLVM_BIN"]), "opt")
INTS = {"i8": 8, "i16": 16, "i32": 32, "i64": 64}
FLTS = {"float": 32, "double": 64}
LANES = [2, 4, 8, 16, 32]
MAX_BITS = 1024  # the widest vector the planner forms: 16 lanes of an i32 widened to i64 doubles that
KINDS = ["throughput", "latency", "code-size", "size-latency"]
TRIPLE = "x86_64-unknown-linux-gnu"
LAYOUT = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
casts = []
for n in LANES:
    for dn, db in {**INTS, **FLTS}.items():
        for sn, sb in {**INTS, **FLTS}.items():
            if max(db, sb) * n > MAX_BITS:
                continue
            di, si = dn in INTS, sn in INTS
            if di and si and db < sb: op = "trunc"
            elif di and si and db > sb: op = None
            elif not di and not si and db > sb: op = "fpext"
            elif not di and not si and db < sb: op = "fptrunc"
            else: op = None
            ops = [op] if op else []
            if di and si and db > sb: ops = ["zext", "sext"]
            if not si and di: ops = ["fptosi", "fptoui"]
            if si and not di: ops = ["sitofp", "uitofp"]
            for o in ops:
                casts.append((o, n, dn, sn))
others = []  # (text, (opname, type)) for each fp arithmetic and memory row
for n in LANES:
    for tn, tb in FLTS.items():
        if tb * n > MAX_BITS:
            continue
        ty = f"<{n} x {tn}>"
        for o in ("fadd", "fsub", "fmul", "fdiv"):
            others.append((f"{o} {ty} %a{{i}}, %a{{i}}", ty, (o, f"{n}x{tn}"), ty))
        others.append((f"fneg {ty} %a{{i}}", ty, ("fneg", f"{n}x{tn}"), ty))
        others.append((f"load {ty}, ptr %p, align {tb // 8}", ty, ("load", f"{n}x{tn}"), ty))
        others.append((f"store {ty} %a{{i}}, ptr %p, align {tb // 8}", ty, ("store", f"{n}x{tn}"), ty))
# A mask extended to integer lanes (`zext <4 x i1>` of a float compare).
for n in LANES:
    for dn, db in INTS.items():
        if db * n > MAX_BITS:
            continue
        for o in ("zext", "sext"):
            casts.append((o, n, dn, "i1"))
# The lanes: an `insertelement` into a vector or into poison, an `extractelement`, at each constant index.
for n in LANES:
    for tn, tb in FLTS.items():
        if tb * n > MAX_BITS:
            continue
        ty = f"<{n} x {tn}>"
        for idx in range(n):
            others.append((f"insertelement {ty} %a{{i}}, {tn} 1.0, i64 {idx}", ty, ("insert", f"{n}x{tn} {idx} vector"), ty))
            others.append((f"insertelement {ty} poison, {tn} 1.0, i64 {idx}", ty, ("insert", f"{n}x{tn} {idx} poison"), ty))
            others.append((f"extractelement {ty} %a{{i}}, i64 {idx}", ty, ("extract", f"{n}x{tn} {idx}"), ty))
# A vector of pointers taken apart (the addresses of a scalarized access).
for n in LANES:
    if 64 * n > MAX_BITS:
        continue
    ty = f"<{n} x ptr>"
    for idx in range(n):
        others.append((f"extractelement {ty} %a{{i}}, i64 {idx}", ty, ("extract", f"{n}xptr {idx}"), ty))
lines, names = [], []
for i, (o, n, dn, sn) in enumerate(casts):
    lines.append(f"  %c{i} = {o} <{n} x {sn}> %a{i} to <{n} x {dn}>")
args = ", ".join([f"<{n} x {sn}> %a{i}" for i, (o, n, dn, sn) in enumerate(casts)] + [f"{t} %a{len(casts) + j}" for j, (_, t, _, _) in enumerate(others)] + ["ptr %p"])
for j, (text, t, key, _) in enumerate(others):
    k = len(casts) + j
    body = text.format(i=k)
    lines.append(("  %r" + str(k) + " = " + body) if not body.startswith("store") else "  " + body)
ir = f'target datalayout = "{LAYOUT}"\ntarget triple = "{TRIPLE}"\ndefine void @f({args}) {{\n' + "\n".join(lines) + "\n  ret void\n}\n"
path = os.path.join(tempfile.mkdtemp(), "vc.ll")
open(path, "w").write(ir)
out = []
for kind in KINDS:
    r = subprocess.run([OPT, "-passes=print<cost-model>", "-disable-output", f"-mtriple={TRIPLE}", "-mcpu=x86-64", f"-cost-kind={kind}", path], capture_output=True, text=True)
    costs = [m.group(1) for m in re.finditer(r"Cost Model: Found an estimated cost of (\d+|Invalid) for instruction:", r.stderr)]
    assert len(costs) == len(casts) + len(others) + 1, (kind, len(costs), len(casts), len(others), r.stderr[:300])  # + the ret
    for i, (o, n, dn, sn) in enumerate(casts):
        out.append(f"{kind} {o} {n}x{dn} {n}x{sn} {costs[i]}")
    for j, (_, _, (o, ty), _) in enumerate(others):
        out.append(f"{kind} {o} {ty} {costs[len(casts) + j]}")
version = subprocess.run([OPT, "--version"], capture_output=True, text=True).stdout.splitlines()[1].strip()
clang = subprocess.run([os.path.join(os.path.dirname(OPT), "clang"), "--version"], capture_output=True, text=True).stdout.split("\n")[0]
version += " " + (clang.split("llvm-project.git ")[1].rstrip(")")[:12] if "llvm-project.git " in clang else "")
open(os.path.join(HERE, "tti_vector_oracle.txt"), "w").write(f"# {version}; python3 tti_vector_oracle.py\n" + "\n".join(out) + "\n")
print(len(out), "rows", version)

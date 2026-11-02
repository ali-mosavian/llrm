#!/usr/bin/env python3
"""Regenerates tests/ivshare/shapes/<id>.<p16|p32|p64>.ll: the IR LSR sees for the
loop shapes of docs (LSR hybrid design, section 5). Run from the repo root."""
import os

OUT = "crates/opt/llrm-transforms/tests/ivshare/shapes"
LAYOUT = {
    16: "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32",
    32: "e-p:32:32-i64:32-f80:32-n8:16:32-S32",
    64: "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
}


def loop(w, streams, *, ity=None, n="arg", down=False, stride=None, acc=False, guard=True,
         start=None, extra_iv=None, value_exit=False):
    """streams: [(elem_ty, 'ld'|'st')]. Returns the function text."""
    ity = ity or f"i{w}"
    params = ", ".join(f"ptr %p{k}" for k in range(len(streams)))
    nparam = ", " + ity + " %n" if n == "arg" else ""
    lim = "%n" if n == "arg" else str(n)
    t = f"define {'i32' if acc else 'void'} @f({params}{nparam}) {{\n"
    t += "entry:\n"
    if guard:
        t += f"  %g = icmp sgt {ity} {lim}, 0\n  br i1 %g, label %loop, label %exit\n"
    else:
        t += "  br label %loop\n"
    t += "loop:\n"
    if down:
        t += f"  %i = phi {ity} [ {lim}, %entry ], [ %next, %loop ]\n"
    else:
        s0 = start if start is not None else 0
        t += f"  %i = phi {ity} [ {s0}, %entry ], [ %next, %loop ]\n"
    if acc:
        t += "  %acc = phi i32 [ 0, %entry ], [ %acc2, %loop ]\n"
    if extra_iv:
        t += f"  %j = phi {ity} [ 0, %entry ], [ %jnext, %loop ]\n"
    sums = []
    for k, (ty, role) in enumerate(streams):
        mul = (stride or {}).get(k, 1)
        idx = "%i"
        if mul != 1:
            t += f"  %x{k} = mul nuw nsw {ity} %i, {mul}\n"
            idx = f"%x{k}"
        if ity != f"i{w}":
            t += f"  %e{k} = zext {ity} {idx} to i{w}\n"
            idx = f"%e{k}"
        t += f"  %a{k} = getelementptr {ty}, ptr %p{k}, i{w} {idx}\n"
        if role == "ld":
            t += f"  %v{k} = load {ty}, ptr %a{k}\n"
            bits = int(ty[1:])
            if bits < 32:
                t += f"  %w{k} = sext {ty} %v{k} to i32\n"
                sums.append(f"%w{k}")
            elif bits == 32:
                sums.append(f"%v{k}")
            else:
                t += f"  %w{k} = trunc {ty} %v{k} to i32\n"
                sums.append(f"%w{k}")
    cur = "0"
    for k, s in enumerate(sums):
        t += f"  %s{k} = add i32 {cur}, {s}\n"
        cur = f"%s{k}"
    if acc:
        t += f"  %acc2 = add i32 %acc, {cur}\n"
    for k, (ty, role) in enumerate(streams):
        if role == "st":
            bits = int(ty[1:])
            val = cur if bits == 32 else None
            if val is None:
                t += f"  %t{k} = trunc i32 {cur} to {ty}\n" if bits < 32 else f"  %t{k} = zext i32 {cur} to {ty}\n"
                val = f"%t{k}"
            t += f"  store {ty} {val}, ptr %a{k}\n"
    if down:
        t += f"  %next = add nsw {ity} %i, -1\n  %c = icmp eq {ity} %next, 0\n"
    else:
        t += f"  %next = add nuw {ity} %i, 1\n  %c = icmp eq {ity} %next, {lim}\n"
    if extra_iv:
        t += f"  %jnext = add {ity} %j, 3\n"
    t += "  br i1 %c, label %exit, label %loop\nexit:\n"
    if acc:
        t += "  %r = phi i32 [ 0, %entry ], [ %acc2, %loop ]\n  ret i32 %r\n" if guard else "  ret i32 %acc2\n"
    else:
        t += "  ret void\n"
    return t + "}\n"


S3 = [("i32", "ld"), ("i32", "ld"), ("i32", "st")]
ROWS = {
    # end-form rows
    "B01": dict(streams=S3),
    "B02": dict(streams=S3, down=True),
    "B03": dict(streams=[("i8", "ld"), ("i32", "ld"), ("i64", "st")]),
    "B07": dict(streams=S3),
    "B09": dict(streams=S3, n=100),
    "B10": dict(streams=S3, guard=False),
    "B11": dict(streams=S3),
    "B12": dict(streams=[("i32", "ld"), ("i32", "ld"), ("i32", "st")], stride={0: 2}),
    # general rows
    "F01": dict(streams=[("i32", "ld"), ("i32", "ld")], acc=True),
    "F03": dict(streams=[("i8", "ld"), ("i8", "st")]),
    "F06": dict(streams=[("i16", "ld"), ("i16", "ld"), ("i16", "ld"), ("i16", "st")]),
    "F09": dict(streams=[("i64", "ld"), ("i16", "ld"), ("i8", "st")]),
    "F12": dict(streams=[("i32", "ld"), ("i32", "st")], down=True),
    "F21": dict(streams=[("i32", "ld"), ("i32", "st")], start=3),
    "F23": dict(streams=[("i32", "ld"), ("i32", "st")], extra_iv=True),
    "F34": dict(streams=[("i32", "ld"), ("i32", "st")], stride={0: 2, 1: 2}),
    # negative rows: the end form must not appear
    "N4": dict(streams=[("i8", "ld"), ("i8", "st")], ity="i8", n=-6, guard=False),
    "F32": dict(streams=[("i32", "ld"), ("i32", "st")], stride={0: 7}),
}

os.makedirs(OUT, exist_ok=True)
for rid, spec in ROWS.items():
    for w in (16, 32, 64):
        ity = spec.get("ity")
        if rid in ("N11", "N4"):
            pass
        body = loop(w, **spec)
        with open(f"{OUT}/{rid}.p{w}.ll", "w") as f:
            f.write(f'target datalayout = "{LAYOUT[w]}"\n\n{body}')
print(len(ROWS), "shapes")

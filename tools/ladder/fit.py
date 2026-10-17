#!/usr/bin/env python3
"""Fit ladder.py's JSON: cost = fixed + slope * IR-instructions, and the log-log exponent, per shape and phase.

    fit.py LADDER.json [--upto N]      (--upto: fit only rows with N <= this; default all)

Columns: fixed in M (the N->0 intercept), slope in k instructions per IR instruction before optimization, exponent
(1.0 linear, 2.0 quadratic) over the rows fitted.
"""
from __future__ import annotations

import argparse
import json
import math

COLUMNS = {
    "LLVM optimizer": lambda r: r["opt"] / 1e6,
    "LLVM backend": lambda r: r["llc"] / 1e6,
    "llrm optimizer": lambda r: r["llrm_phases"]["optimizer"],
    "llrm backend": lambda r: r["llrm_phases"]["backend"],
    "LLVM total": lambda r: r["clang"] / 1e6,
    "llrm total": lambda r: r["llrm"] / 1e6,
}


def line(xs: list[float], ys: list[float]) -> tuple[float, float]:
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sum((x - mx) ** 2 for x in xs)
    return my - slope * mx, slope


def exponent(xs: list[float], ys: list[float]) -> float:
    return line([math.log(x) for x in xs], [math.log(y) for y in ys])[1]


def fit(rows: list[dict], upto: int | None = None) -> dict[str, dict[str, tuple[float, float, float]]]:
    out: dict[str, dict[str, tuple[float, float, float]]] = {}
    for shape in dict.fromkeys(r["shape"] for r in rows):
        sel = [r for r in rows if r["shape"] == shape and (upto is None or r["n"] <= upto)]
        xs = [float(r["ir"]) for r in sel]
        out[shape] = {}
        for name, get in COLUMNS.items():
            ys = [get(r) for r in sel]
            fixed, slope = line(xs, ys)
            out[shape][name] = (fixed, slope * 1000, exponent(xs, ys))
    return out


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("ladder")
    ap.add_argument("--upto", type=int)
    a = ap.parse_args()
    print("| shape | phase | LLVM fixed M | LLVM k/IR | LLVM exp | llrm fixed M | llrm k/IR | llrm exp |\n|---|---|---:|---:|---:|---:|---:|---:|")
    for shape, cols in fit(json.load(open(a.ladder)), a.upto).items():
        for phase in ("optimizer", "backend", "total"):
            l, r = cols[f"LLVM {phase}"], cols[f"llrm {phase}"]
            print(f"| {shape} | {phase} | {l[0]:.1f} | {l[1]:.0f} | {l[2]:.2f} | {r[0]:.1f} | {r[1]:.0f} | {r[2]:.2f} |")

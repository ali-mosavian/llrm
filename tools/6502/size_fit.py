#!/usr/bin/env python3
"""Fit the 6502 byte table (crates/target/llrm-6502/src/opcosts.rs `calibrate`) to the bytes the backend writes.

usage: LLRM_SIZE_FIT=1 cargo test -p llrm-6502 --test size_calibration -- --nocapture | tools/6502/size_fit.py

Each `row PROGRAM FUNCTION MODEL WRITTEN class=count,...` is one function: the operations it has by class
(width-weighted) and the bytes it is written as. The fit is non-negative least squares on the relative error
(a function under 20 B weighs as 20), coordinate descent; the prices it prints are bytes per count.
"""
import statistics
import sys

rows = []
for line in sys.stdin:
    if not line.startswith("row "):
        continue
    _, program, name, model, written, counts = (line.split() + [""])[:6]
    features = dict(item.split("=") for item in counts.split(",") if item)
    rows.append((program, name, int(model), int(written), {k: int(v) for k, v in features.items()}))

classes = sorted({k for row in rows for k in row[4]})
x = [[row[4].get(c, 0) for c in classes] for row in rows]
y = [row[3] for row in rows]
w = [1 / max(v, 20) for v in y]
price = [0.0] * len(classes)
for _ in range(2000):
    for j in range(len(classes)):
        num = den = 0.0
        for i in range(len(rows)):
            if x[i][j]:
                rest = y[i] - sum(x[i][k] * price[k] for k in range(len(classes)) if x[i][k] and k != j)
                num += w[i] ** 2 * x[i][j] * rest
                den += w[i] ** 2 * x[i][j] ** 2
        price[j] = max(0.0, num / den) if den else 0.0
fit = [sum(x[i][k] * price[k] for k in range(len(classes))) for i in range(len(rows))]
print(f"{len(rows)} functions")
for c, p in zip(classes, price):
    print(f"{c:10} {p:6.2f}")


def report(label, estimate):
    ratios = [e / v for e, v in zip(estimate, y)]
    print(f"{label}: aggregate {sum(estimate) / sum(y):.2f} median {statistics.median(ratios):.2f} "
          f"within 25% {100 * sum(abs(r - 1) < 0.25 for r in ratios) / len(ratios):.0f}%")


report("fit  ", fit)
report("model", [row[2] for row in rows])

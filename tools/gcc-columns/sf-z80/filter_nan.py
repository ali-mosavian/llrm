"""usage: filter_nan.py NARGS : rewrite vec.inc without the records whose inputs are NaN; print how many were dropped."""
import re, sys
na = int(sys.argv[1])
isnan = lambda lo, hi: (((hi << 16 | lo) << 1) & 0xffffffff) > 0xff000000
out, drop = [], 0
for l in open("vec.inc").read().split("\n"):
    m = re.match(r"\s*\.word\s+(.*)", l)
    if not m:
        out.append(l)
        continue
    w = [int(x, 16) for x in m.group(1).split(",")]
    if isnan(w[0], w[1]) or (na == 2 and isnan(w[2], w[3])):
        drop += 1
        continue
    out.append(l)
n = sum(1 for l in out if l.strip().startswith(".word"))
out = [("NV = %d" % n) if l.startswith("NV =") else l for l in out]
open("vec.inc", "w").write("\n".join(out) + "\n")
print(drop)

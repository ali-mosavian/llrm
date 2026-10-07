import json, math, sys, os
W = os.environ.get("VSGCC_WORK")
base, llvm = sys.argv[1:3] if len(sys.argv) > 2 else ("llrmElf", "llvmElf")
rows = [json.loads(l) for l in open(f"{W}/results.jsonl") if l.startswith("{")]
P = {}
for r in rows: P.setdefault(r["prog"], {})[r["variant"]] = r
for lvl in ("O2", "Os"):
    print(f"## {lvl}")
    ratios = {k: {} for k in ("ins", "clocks", "code")}
    skipped = []
    for p, d in sorted(P.items()):
        a, b = d.get(base + lvl), d.get(llvm + lvl)
        if not a or not b: skipped.append((p, "missing")); continue
        if not (a["ok"] and b["ok"]): skipped.append((p, f"wrong result base={a['ok']} llvm={b['ok']} got={b['got']}")); continue
        for k in ratios: ratios[k][p] = b[k] / max(1, a[k])
    for k, rs in ratios.items():
        g = math.exp(sum(map(math.log, rs.values())) / len(rs))
        w = max(rs, key=rs.get); best = min(rs, key=rs.get)
        print(f"{k:7} n={len(rs)} geomean llvm/base {g:.3f}  worst {rs[w]:.2f} ({w})  best {rs[best]:.2f} ({best})")
    print("skipped:", skipped)
    if "-v" in sys.argv:
        for p in sorted(ratios["ins"]): print(f"  {p:12}", *(f"{k}={ratios[k][p]:.2f}" for k in ratios))

#!/usr/bin/env python3
"""Counts, compile time and code/data bytes of an llrm-c build against the reference build's.

    report.py OURS_DIR REF_DIR [--worst N]

Both are inventory.py --out directories. Objects both built are compared; the totals are of those.
"""
import argparse
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import compare  # noqa: E402


def report(ours, ref, worst=5):
    a, b = compare.load(ours), compare.load(ref)
    rows = [r for r in compare.compare(ours, ref) if a[(r[0], r[1])]["tool"] == "llrm-c"]
    c_ours = [v for v in a.values() if v["tool"] == "llrm-c"]
    c_ref = [v for v in b.values() if v["tool"] == "bwcc"]
    out = []
    out.append(f"C sources: {len(c_ours)}; llrm-c compiled {sum(v['status'] == 'ok' for v in c_ours)}, "
               f"the reference {sum(v['status'] == 'ok' for v in c_ref)}")
    asm = [v for v in a.values() if v["tool"] == "jwasm"]
    out.append(f"asm sources: {len(asm)}; JWasm assembled {sum(v['status'] == 'ok' for v in asm)}")
    ok = [v for v in c_ours if v["status"] == "ok"]
    rk = [v for v in c_ref if v["status"] == "ok"]

    def times(label, values):
        top = sorted(values, key=lambda v: v["seconds"])[-worst:][::-1]
        out.append(f"{label}: total {sum(v['seconds'] for v in values):.2f} s over {len(values)}; worst "
                   + ", ".join(f"{v['dir']}/{v['obj']} {v['seconds']:.3f} s" for v in top))
    times("llrm-c compile time", ok)
    times("reference compile time", rk)
    code_o, code_r = sum(r[3] for r in rows), sum(r[4] for r in rows)
    out.append(f"compared {len(rows)} objects: code {code_o} vs {code_r} bytes ({code_o / code_r:.3f}x), "
               f"data {sum(r[5] for r in rows)} vs {sum(r[6] for r in rows)} bytes")
    sized = [r for r in rows if r[4] > 0]
    verdict = Counter("smaller" if r[3] < r[4] else "equal" if r[3] == r[4] else "larger" for r in sized)
    out.append(f"code against the reference, per object: {dict(verdict)}")
    big = sorted(rows, key=lambda r: -r[3])[:worst]
    out.append("largest llrm-c objects: " + ", ".join(f"{r[0]}/{r[1]} {r[3]} B (ref {r[4]})" for r in big))
    ratio = sorted((r for r in sized if r[4] >= 40), key=lambda r: -r[3] / r[4])[:worst]
    out.append("worst against the reference (reference code >= 40 B): "
               + ", ".join(f"{r[0]}/{r[1]} {r[3]}/{r[4]} B {r[3] / r[4]:.2f}x" for r in ratio))
    over = sorted(sized, key=lambda r: -(r[3] - r[4]))[:worst]
    out.append("largest absolute excess: " + ", ".join(f"{r[0]}/{r[1]} +{r[3] - r[4]} B" for r in over))
    return "\n".join(out)


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("ours", type=Path)
    ap.add_argument("ref", type=Path)
    ap.add_argument("--worst", type=int, default=5)
    a = ap.parse_args()
    print(report(a.ours, a.ref, a.worst))

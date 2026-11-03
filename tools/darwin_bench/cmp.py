"""Cell comparator: metrics where cand is worse than ref. Cells are (bytes, insns, us)."""
TOL = (1, 1, 1.10)  # head-vs-base: bytes and insns exact, clocks 10% (noise)
METRICS = ("bytes", "insns", "us")


def regressions(cand, ref, tol=TOL):
    return [m for m, c, r, t in zip(METRICS, cand, ref, tol) if c > r * t]


def regressions_vs_best(cand, refs, tol=TOL):
    return regressions(cand, tuple(min(r[i] for r in refs) for i in range(3)), tol)

"""Cell comparator: metrics where cand is worse than ref. Cells are (bytes, insns, us)."""
TOL = (1, 1, 1.10)  # head-vs-base: bytes and insns exact, clocks 10% (noise)
METRICS = ("bytes", "insns", "us")


def regressions(cand, ref, tol=TOL):
    return [m for m, c, r, t in zip(METRICS, cand, ref, tol) if c > r * t]


def regressions_vs_best(cand, refs, tol=TOL):
    return regressions(cand, tuple(min(r[i] for r in refs) for i in range(3)), tol)

def clock_note(load, times, ok=True, noisy_over=0.10):
    """Matrix note for a clocks cell: load, spread (max-min over median), "noisy" above noisy_over, output flag."""
    med = sorted(times)[len(times) // 2]
    sp = (max(times) - min(times)) / med if med else 0
    return f"darwin us, load {load:.1f}, spread {sp:.0%}" + (", noisy" if sp > noisy_over else "") + ("" if ok else ", output differs from .out")

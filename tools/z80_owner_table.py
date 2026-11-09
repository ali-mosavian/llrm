"""z80_owner_table.py: ours vs the best SDCC 4.5.0 variant per program and level, one TSV (the z80 owner table).

    z80_owner_table.py OUT.tsv --ours DIR --sdcc DIR [--sdcc DIR ...] --programs SRC_DIR --header-line TEXT ...

DIR holds pair.<lv>.tsv / new.<lv>.tsv / full.<lv>.tsv files of `prog<TAB>variant<TAB>bytes<TAB>clocks` (variant `ours` or
`sdcc-*`), lv in 1 s z 2 3. Verdicts: O1/Os/Oz strict on both axes; O2/O3 at 1 B = 3 T. UNMEASURED and NO-SDCC are
counted, never ok. `virtual` marks a cell no single SDCC variant attains and ours is Pareto-optimal in (informational: the
pin has no Z80 target, so no pin-equivalent bar exists to turn it into a pass).
"""
import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import corpus_targets  # noqa: E402
import virtual_ok  # noqa: E402
import z80_flag_check  # noqa: E402

LEVELS = [("1", "O1"), ("s", "Os"), ("z", "Oz"), ("2", "O2"), ("3", "O3")]
STRICT = {"O1", "Os", "Oz"}
RATE = 3  # T-states per byte at O2/O3
UNRANKED = "UNRANKED (double: ours f64 vs SDCC f32)"


def read_pairs(path):
    """{program: {variant: (bytes, clocks)}} of one pair file."""
    out = {}
    for line in Path(path).read_text().splitlines():
        f = line.split("\t")
        if len(f) >= 4 and f[2].isdigit() and f[3].isdigit():
            out.setdefault(f[0], {})[f[1]] = (int(f[2]), int(f[3]))
    return out


def is_double(source):
    return re.search(r"\bdouble\b", source) is not None


def verdict(level, ours, sdcc, double=False):
    """The cell's verdict. `ours` is (B, T) or None; `sdcc` {variant: (B, T)}."""
    if ours is None:
        return "UNMEASURED"
    if not sdcc:
        return "NO-SDCC"
    if double:
        return UNRANKED
    best_b = min(b for b, _ in sdcc.values())
    best_t = min(t for _, t in sdcc.values())
    d_b, d_t = ours[0] - best_b, ours[1] - best_t
    if level in STRICT:
        return ((" B-worse" if d_b > 0 else "") + (" T-worse" if d_t > 0 else "")).strip() or "ok"
    return "worse" if d_t + RATE * d_b > 0 else "ok"


def is_virtual_cell(ours, sdcc):
    """No SDCC variant attains both bars, and none dominates ours (ours is on the Pareto front)."""
    refs = list(sdcc.values())
    if ours is None or not refs or not virtual_ok.is_virtual(refs):
        return False
    return not any(all(r[i] <= ours[i] for i in range(2)) and r != ours for r in refs)


def applicable(sources, target="z80-sdcc"):
    """The programs the corpus is for on this target (`// targets:` and the x86-only flag rules)."""
    return {name for name, text in sources.items() if corpus_targets.applies(target, text)}


REFUSED = []  # (program, level, variant, why): a variant whose codegen flags differ from ours never sets a bar


def rows(ours, sdcc, sources):
    """Table rows for each level: every applicable program; one without an ours row is UNMEASURED."""
    out = []
    for lv, name in LEVELS:
        programs = sorted(applicable(sources))
        for prog in programs:
            o = ours.get(lv, {}).get(prog, {}).get("ours")
            s = {k: v for k, v in sdcc.get(lv, {}).get(prog, {}).items() if k.startswith("sdcc")}
            for k in list(s):
                why = z80_flag_check.cell_mismatch(sources.get(prog, ""), lv, k)
                if why:
                    REFUSED.append((prog, name, k, why))
                    del s[k]
            double = is_double(sources.get(prog, ""))
            v = verdict(name, o, s, double)
            if o is None or not s:
                out.append((prog, name, o[0] if o else "", o[1] if o else "", "", "", "", "", "", "", v, "", len(s)))
                continue
            bb = min(s.items(), key=lambda kv: kv[1][0])
            bt = min(s.items(), key=lambda kv: kv[1][1])
            out.append((prog, name, o[0], o[1], bb[1][0], bb[0], bt[1][1], bt[0], o[0] - bb[1][0], o[1] - bt[1][1], v,
                        "Y" if is_virtual_cell(o, s) else "", len(s)))
    return out


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--ours", action="append", required=True)
    ap.add_argument("--sdcc", action="append", required=True)
    ap.add_argument("--programs", required=True)
    ap.add_argument("--header-line", action="append", default=[])
    a = ap.parse_args(argv)
    ours, sdcc = {}, {}
    for lv, _ in LEVELS:
        for d, into in ((a.ours, ours), (a.sdcc, sdcc)):
            for base in d:
                for pattern in (f"new.{lv}.tsv", f"full.{lv}.tsv", f"pair.{lv}.tsv", f"{lv}.tsv"):
                    p = Path(base) / pattern
                    if p.exists():
                        for prog, v in read_pairs(p).items():
                            into.setdefault(lv, {}).setdefault(prog, {}).update(
                                {k: x for k, x in v.items() if (k == "ours") == (into is ours)})
    sources = {p.stem: p.read_text() for p in Path(a.programs).glob("*.c")}
    table = rows(ours, sdcc, sources)
    with open(a.out, "w") as f:
        f.write(f"# flag parity: tools/z80_flag_check.py, {len(REFUSED)} SDCC cells refused for differing codegen flags\n")
        for line in a.header_line:
            f.write("# " + line + "\n")
        f.write("prog\tlevel\tours_B\tours_T\tbest_B\tbest_B_variant\tbest_T\tbest_T_variant\tdB\tdT\tverdict\tvirtual\tn_sdcc\n")
        for r in table:
            f.write("\t".join(str(x) for x in r) + "\n")
    counts = {}
    for r in table:
        counts[r[10]] = counts.get(r[10], 0) + 1
    print(len(table), "rows;", counts)


if __name__ == "__main__":
    main(sys.argv[1:])

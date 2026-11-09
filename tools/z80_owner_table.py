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


def stale_oz(ours):
    """Programs whose Oz bytes exceed their Os bytes in `ours` ({level: {program: {"ours": (B, T)}}}).

    llrm-z80 at -Oz keeps the -Os build where that is smaller (`Request::oz_best_of`), so from one binary Oz is never
    bigger than Os. A row that is was measured by another binary, or before the best-of: loop_fusion Oz read 953 B
    against Os 907 B in the 2026-11-09 table, when the binary gives 907 for both.

    The choice is made on the module's estimated bytes, the table measures linked bytes: where the estimate is wrong
    (memmove_overlap, switch_peeled_table: Z80-OZ-1/-2) Oz is bigger. That is an estimator gap, not a stale row. The
    caller names it with `--known-oz-gap PROG=ROW`; ROW must be a row of docs/working/above-main-rows.md with an owner
    and an exit. Remove the flag when `oz_best_of` is fixed. Any other program is stale.

    `oz_best_of` chooses per module (`compile::effective`), and a module is one corpus program, so the invariant holds
    per program, not per function.
    """
    return sorted(
        (prog, os_row["ours"][0], oz_row["ours"][0])
        for prog, os_row in ours.get("s", {}).items()
        if (oz_row := ours.get("z", {}).get(prog)) and "ours" in os_row and "ours" in oz_row
        and oz_row["ours"][0] > os_row["ours"][0]
    )


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


ROWS = Path(__file__).resolve().parent.parent / "docs" / "working" / "above-main-rows.md"


def open_rows(table):
    """Ids of the rows of the above-main table that have an owner and an exit."""
    found = set()
    for line in Path(table).read_text().splitlines():
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if line.startswith("| ") and len(cells) >= 10 and cells[8] and cells[9] and cells[0] != "id":
            found.add(cells[0])
    return found


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--ours", action="append", required=True)
    ap.add_argument("--sdcc", action="append", required=True)
    ap.add_argument("--programs", required=True)
    ap.add_argument("--header-line", action="append", default=[])
    ap.add_argument("--known-oz-gap", action="append", default=[], metavar="PROG=ROW",
                    help="a program whose Oz bytes exceed Os through an oz_best_of estimator gap, with its row in "
                         "above-main-rows.md (owner and exit); remove when oz_best_of is fixed")
    ap.add_argument("--rows", default=str(ROWS), help="the rows table that --known-oz-gap rows must be in")
    ap.add_argument("--allow-stale-oz", action="store_true", help="write the table although an Oz row is bigger than its Os row")
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
    known = dict(item.split("=", 1) for item in a.known_oz_gap)
    missing = sorted(row for row in known.values() if row not in open_rows(a.rows))
    if missing:
        print(f"--known-oz-gap rows not in {a.rows} with an owner and an exit: {' '.join(missing)}", file=sys.stderr)
        sys.exit(1)
    stale = [row for row in stale_oz(ours) if row[0] not in known]
    gaps = [row for row in stale_oz(ours) if row[0] in known]
    if gaps:
        a.header_line.append("OZ>OS ESTIMATOR GAP (oz_best_of): " + " ".join(f"{prog}={known[prog]}" for prog, _, _ in gaps))
    if stale and a.allow_stale_oz:
        a.header_line.append("STALE-OZ ALLOWED: " + " ".join(prog for prog, _, _ in stale))
    elif stale:
        for prog, os_b, oz_b in stale:
            print(f"stale Oz row: {prog} Oz {oz_b} B > Os {os_b} B", file=sys.stderr)
        print("regenerate the Oz column from the binary of the Os column (or --known-oz-gap PROG=ROW, --allow-stale-oz)", file=sys.stderr)
        sys.exit(1)
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

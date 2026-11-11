#!/usr/bin/env python3
"""m32_matrix_summary.py STAMP TSV...: per file (target x level) measured cells, NA cells and the cells where ours is larger than the pin llc / pin clang / gcc / best-of-clang-and-gcc column.
SIZE RULE (as m16, #8466): a cell is padding-free code + data (.data, .rodata*: constant pools, jump tables, strings), strict; static = data + bss is a second
axis, bss alone is never judged. A file without the data columns is refused (MISSING, exit 1): its sizes would be text only.
A missing ours, pin_clang or (on x86_32 with a gcc column) gcc cell is printed as MISSING and exits 1: no fallback to another column.
A file without a gcc column says "clang-only bar" in its header line. After the table, `flips` lists the cells whose verdict differs between the old
text-only rule and the size rule, read from the same run."""
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools"))
import corpus_targets  # noqa: E402

MISSING = []
COLUMNS = {  # side -> (code, data, bss) column names
    "ours": ("ours_code", "ours_data", "ours_bss"),
    "llc": ("pin_llc_code", "pin_llc_data", "pin_llc_bss"),
    "clang": ("pin_clang_code", "pin_clang_data", "pin_clang_bss"),
    "a4": ("pin_clang_a4_code", "pin_clang_a4_data", "pin_clang_a4_bss"),
    "gcc": ("gcc_code", "gcc_data", "gcc_bss"),
}


PADDED = {"ours": "ours_padded", "llc": "pin_llc_padded", "clang": "pin_clang_padded", "a4": "pin_clang_a4_padded", "gcc": "gcc_padded"}
OVERSIZE = []  # (file, program, side, code, emitted text): a padding-free code column must not exceed the text the object emitted


def source_of(name):
    """The corpus source of a matrix row's program name (`tests_run_c_NAME` is tests/run/c/NAME.c, `bench_DIR_FILE` is bench/DIR/FILE.c), or None."""
    if name.startswith("tests_run_c_"):
        found = REPO / "tests/run/c" / f"{name[len('tests_run_c_'):]}.c"
    elif name.startswith("bench_"):
        rest = name[len("bench_"):]
        half = len(rest) // 2
        directory, stem = (rest[:half], rest[half + 1:]) if rest[:half] == rest[half + 1:] and rest[half:half + 1] == "_" else (rest.split("_")[0], rest)
        found = REPO / "bench" / directory / f"{stem if stem != rest else directory}.c"
    else:
        return None
    return found if found.is_file() else None


def applies(target, name):
    """False for a program that is not for `target` (`// targets:` of corpus_targets, e.g. DOS-only): a matrix compiles every program at every level,
    declared variants or not, and such a row is no cell (rep_tails O1 +739 B and os_more O1 +196 B were counted worse at x86_32-linux)."""
    source = source_of(name)
    return source is None or corpus_targets.applies(target, source.read_text(), source)


def level_of(path):
    found = re.search(r"-(O[123s])-", path)
    return found[1] if found else path


def num(x):
    try:
        return int(x)
    except (TypeError, ValueError):
        return None


class Table:
    def __init__(self, path):
        rows = [l.rstrip("\n").split("\t") for l in open(path)]
        self.path, self.hdr, self.rows = path, rows[0], rows[1:]

    def has(self, side):
        return COLUMNS[side][0] in self.hdr

    def get(self, row, name):
        return num(row[self.hdr.index(name)]) if name in self.hdr and self.hdr.index(name) < len(row) else None

    def cell(self, row, side):
        """(code, size, static) of one side of one row; size = code + data and static = data + bss, None where a part is unreadable."""
        code, data, bss = (self.get(row, n) for n in COLUMNS[side])
        return code, None if code is None or data is None else code + data, None if data is None or bss is None else data + bss


def worse(pairs):
    return sum(a > b for a, b in pairs)


def main(argv):
    print(open(argv[0]).read().strip())
    print("size rule: code_bytes padding-free (padding is its own column) + data (.data, .rodata*); static = data + bss, judged apart; bss alone is not judged")
    print("file\ttarget\trows\tmeasured(llc)\tNA(llc)\tworse_vs_pin_llc\tworse_vs_pin_clang\tmeasured(clang)\tNA(clang)\tworse_vs_gcc\tworse_vs_best(gcc,clang)\tbar\tworse_vs_pin16_CF6\tstatic_worse_vs_best")
    flips = []
    for path in argv[1:]:
        t = Table(path)
        for target in sorted({r[0] for r in t.rows}):
            rs = [r for r in t.rows if r[0] == target and applies(target, r[1])]
            skipped = sorted(r[1] for r in t.rows if r[0] == target and not applies(target, r[1]))
            if skipped:
                print(f"n/a\t{level_of(path)}\t{target}\t{len(skipped)} programs, not cells: {' '.join(skipped)}")
            gated = t.has("gcc") and target == "x86_32-linux"
            clang_side = "a4" if gated and t.has("a4") else "clang"  # the a4 and gcc columns exist for x86_32 rows only
            sides = ["ours", clang_side] + (["gcc"] if gated else [])
            for r in rs:
                nosrc = len(r) > 14 and r[14].startswith("nosrc")
                for side in sides:
                    code, size, _ = t.cell(r, side)
                    if code is not None and size is None and not nosrc:
                        MISSING.append((path, r[1], COLUMNS[side][1]))
                    elif code is None and not nosrc:
                        MISSING.append((path, r[1], COLUMNS[side][0]))
                code, size, _ = t.cell(r, "llc")
                if code is not None and size is None:
                    MISSING.append((path, r[1], COLUMNS["llc"][1]))
            pad = {}
            for r in rs:
                for side, name in PADDED.items():
                    code, emitted = (t.get(r, COLUMNS[side][0]) if t.has(side) else None), t.get(r, name)
                    if code is None or emitted is None:
                        continue
                    if code > emitted:
                        OVERSIZE.append((path, r[1], side, code, emitted))
                    pad[side] = pad.get(side, 0) + emitted - code
            c = {side: [t.cell(r, side) for r in rs] for side in COLUMNS if t.has(side)}
            c16 = [t.cell(r, "clang") for r in rs]

            def pairs(side, k):
                return [(a[k], b[k]) for a, b in zip(c["ours"], c[side]) if a[k] is not None and b[k] is not None]

            def best(k):
                return [(a[k], min(b[k], g[k])) for a, b, g in zip(c["ours"], c[clang_side], c["gcc"]) if None not in (a[k], b[k], g[k])]

            ml, mc = pairs("llc", 1), pairs(clang_side, 1)
            if rs and not ml:  # a whole column unreadable is a broken instrument, not a result (pin_llc read NA on 349 of 349 cells for a call that did not fit pin.target)
                MISSING.append((path, f"all {len(rs)} {target} rows", COLUMNS["llc"][0]))
            n_llc_na = sum(x[1] is None for x in c["llc"])
            n_clang_na = sum(x[1] is None for x in c[clang_side])
            wg = worse(pairs("gcc", 1)) if t.has("gcc") else "-"
            wb = worse(best(1)) if gated else "-"
            ws = worse(best(2)) if gated else "-"
            bar = "best of gcc and clang (pin at stack alignment 4)" if gated and t.has("a4") else ("best of gcc and clang" if gated else "clang-only bar")
            w16 = worse([(a[1], b[1]) for a, b in zip(c["ours"], c16) if a[1] is not None and b[1] is not None])
            print(f"{level_of(path)}\t{target}\t{len(rs)}\t{len(ml)}\t{n_llc_na}\t{worse(ml)}\t{worse(mc)}\t{len(mc)}\t{n_clang_na}\t{wg}\t{wb}\t{bar}\t{w16}\t{ws}")
            print("pad\t" + "\t".join(f"{side} {n}" for side, n in pad.items()) + "\t(padded - code_bytes padding-free, own column, never added to a side)")
            # flips: the same run judged by code alone and by code + data
            if gated:
                for r, a, b, g in zip(rs, c["ours"], c[clang_side], c["gcc"]):
                    if None in (a[0], a[1], b[0], b[1], g[0], g[1]):
                        continue
                    old, new = a[0] > min(b[0], g[0]), a[1] > min(b[1], g[1])
                    if old != new:
                        flips.append((level_of(path), target, r[1], "worse" if new else "better", a[0], min(b[0], g[0]), a[1], min(b[1], g[1])))
    print("flips (best of gcc and clang): cell, direction under the size rule, code ours/bar -> size ours/bar")
    for level, target, program, direction, oc, bc, os_, bs in flips:
        print(f"flip\t{level}\t{target}\t{program}\t{direction}\tcode {oc}/{bc} -> size {os_}/{bs}")
    print(f"flips: {sum(f[3] == 'worse' for f in flips)} newly worse, {sum(f[3] == 'better' for f in flips)} newly better")
    for path, program, column in MISSING:
        print(f"MISSING reference: {path} {program} {column}", file=sys.stderr)
    for path, program, side, code, emitted in OVERSIZE:
        print(f"REFUSED code_bytes padding-free: {path} {program} {side} code {code} > emitted text {emitted}", file=sys.stderr)
    return 2 if OVERSIZE else (1 if MISSING else 0)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

"""The gate's row for the m32 C cells: llrm's cells of two vsgcc runs (main, tip) judged with the bench's rule.

  uv run --project tools python crates/target/llrm-x86-m32/vsgcc/compare.py MAIN.jsonl TIP.jsonl

Per program and level (llrmO1 -O1: reported, not gated; llrm -O2, llrmO3 -O3: priced at the 486 line fill; llrmOs -Os: bytes and clocks strict,
instructions and memory operands only on a tie). `code` is padding-free (harness.code_size). Refuses (exit 1) on rows without one compiler stamp per file (build.sh), main and tip built by the same compiler, rows that are not ok, kernels
that ran 0 instructions, or cell sets that differ; FAIL cells exit 1 too."""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[3] / "tools" / "bench"))
import bench  # noqa: E402

LEVELS = {"llrmO1": "O1", "llrm": "O2", "llrmO3": "O3", "llrmOs": "Os"}


def read(path):
    return {(r["prog"], r["variant"]): r for r in map(json.loads, Path(path).read_text().splitlines()) if r["variant"] in LEVELS}


def stamp(rows):
    """The one compiler stamp of a file's rows, or why there is none."""
    stamps = {r.get("cc") for r in rows.values()}
    if None in stamps:
        return None, "rows have no compiler stamp: rebuild with build.sh"
    if len(stamps) > 1:
        return None, f"rows of two compilers {sorted(stamps)} in one file"
    return stamps.pop() if stamps else None, None


def cell(row):
    return {"instructions": row["ins"], "memory_operands": row["mem"], "clocks": row["clocks"], "code_bytes": row["code"], "data_bytes": row.get("data", 0)}


def judge(main, tip):
    """(refusals, FAIL lines, cells judged, cells reported only)."""
    refusals = [f"{key[0]}/{key[1]}: row not ok or no instructions" for rows in (main, tip) for key, row in rows.items() if not row.get("ok") or not row.get("ins")]
    stamps = []
    for name, rows in (("main", main), ("tip", tip)):
        one, why = stamp(rows)
        stamps.append(one)
        if why:
            refusals.append(f"{name}: {why}")
    if stamps[0] is not None and stamps[0] == stamps[1]:
        refusals.append(f"main and tip were built by the same compiler {stamps[0]}: nothing to compare")
    if set(main) != set(tip):
        refusals.append(f"cell sets differ: only main {sorted(set(main) - set(tip))[:4]}, only tip {sorted(set(tip) - set(main))[:4]}")
    fails, judged, reported = [], 0, 0
    for key in sorted(set(main) & set(tip)):
        level = LEVELS[key[1]]
        if level == "O1":
            reported += 1
            continue
        judged += 1
        fails += bench.against(f"{key[0]} m32 -{level}", cell(tip[key]), cell(main[key]), rate=bench.rate_for("bq32", level))
    return refusals, fails, judged, reported


def main():
    refusals, fails, judged, reported = judge(read(sys.argv[1]), read(sys.argv[2]))
    for line in refusals:
        print("REFUSED:", line)
    for line in fails:
        print("FAIL:", line)
    print(f"vsgcc m32 C: {judged} cells judged ({reported} -O1 reported only): {len(fails)} FAIL, {len(refusals)} refused")
    return 1 if fails or refusals else 0


if __name__ == "__main__":
    sys.exit(main())

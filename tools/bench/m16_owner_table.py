"""m16_owner_table.py BENCH.json DIST OUT.tsv [HARNESS_COMMIT]: the m16 owner table, from a `bench.py --opt O1 O2 Os --references --no-judge --json` run.

One row per program, language and level of ours, judged per axis against the best external (C: ow, bcc, tc, tcpp; BASIC: bc, pds71, vbdos): -Os on total bytes
(code + data; BASIC code + static) and clocks; O1/O2/O3 on clocks only, the code bytes over the bar reported in the note (user ruling 2026-11-14). A Nib row's bar is the C compilers on the C twin that has the Nib semantics
(`bench.bar_program`: `NAME~init`, `NAME~bounds`), the compilers on the plain source stay beside it (`plain_ext_*`); a twin's own row is reference-only (verdict TWIN); a Nib program with a twin on disk and none measured is NO_TWIN, never judged on the plain source.
The header names the dist, the md5 of its compilers, the harness commit and `date -u`: a table without them is not evidence.
"""

import collections
import datetime
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import bench  # noqa: E402

C_REFS = ("ow", "bcc", "tc", "tcpp")
BAS_REFS = ("bc", "pds71", "vbdos")
OURS = ("c", "nib", "bas", "bq16")
# The CPU llrm measures at m16: no -march, the default (the 486, the table the judge counts); -march=i386 prices the 386 table. The gate passes no flag.
OURS_ISA = "i486"


# A reference that prints a value other than the host oracle's is no bar for that cell. Where the cause is open it is recorded here with its date, owner and exit.
WRONG_OUTPUT_CAUSES = {
    "stft_fixed~bounds": "2026-11-23: the Borland-family ~bounds build prints another value (bcc already on the full corpus); cause open, owner w12, exit 2026-12-13",
}


def counts(row: dict | None, program: str = "", name: str = "") -> dict | None:
    """The four axes of a measured row, None where the cell is missing or errored. `static` is data + bss + the arrays `name` allocates at
    run time for the BASIC `program` (bench.run_time_arrays): a reference that allocates what llrm holds in its data is not smaller for it."""
    if not row or "error" in row or "clocks" not in row or "code_bytes" not in row:
        return None
    static = bench.with_footprint({"data_bytes": row.get("data_bytes", 0), "bss_bytes": row.get("bss_bytes", 0)}, bench.run_time_arrays(program, name))["static_bytes"]
    # Total bytes at -Os: code and initialised data, and for BASIC (11-13 ruling) code and static: data, bss and the arrays allocated at run time, of both sides.
    total = row["code_bytes"] + (static if name in ("bas", "bq16", *BAS_REFS) else row.get("data_bytes", 0))
    return {"code": row["code_bytes"], "total": total, "clocks": row["clocks"], "data": row.get("data_bytes", 0), "static": static}


def limit_bound(row: dict | None) -> str:
    """">= 3e8" for a cell the counter stopped at its instruction limit (LLRM_ICOUNT_LIMIT): the program has not finished, so its clocks are at least that many and its
    bytes are unknown. A bound, never a measurement: it is printed, and is no bar and no verdict; "" for any other cell."""
    found = re.search(r"stopped at the (\d+)-instruction limit", (row or {}).get("error", ""))
    if not found:
        return ""
    count = found.group(1)
    return f">= {count[0]}e{len(count) - 1}" if set(count[1:]) <= {"0"} else f">= {count}"


def has_twin_source(program: str) -> bool:
    """A Nib program with a C twin on disk (`bench/NAME/twin/SEMANTIC.c`): its bar is that twin, whose object holds the same I/O functions
    (BENCH_IO_FUNCTIONS) and bounds checks as the Nib program's. The plain source has neither."""
    return any((bench.BENCH / program / "twin" / f"{semantic}.c").exists() for semantic in bench.TWINS)


def no_references(program: str) -> bool:
    """A program whose bench.toml says `references = false` (a source or library no reference compiler builds): no bar by design."""
    manifest = bench.BENCH / program / "bench.toml"
    return manifest.exists() and tomllib.loads(manifest.read_text()).get("references") is False


def findings(level: str, ours: dict, refs: dict) -> list[str]:
    """Where `ours` is worse than the best of `refs`, under the user's ruling of 2026-11-14 and oversight's reading of it (the same day): -Os matches the best
    on total bytes (code + initialised data; BASIC code + static: data, bss and run-time arrays) and on clocks; O1/O2/O3 match the best on clocks, the code bytes over the
    bar reported (`bytes_over`) and not required. The per-axis code and data stay in their columns."""
    found = []
    for axis in (("total", "clocks") if level == "Os" else ("clocks",)):
        best = min((r[axis], name) for name, r in refs.items())
        if ours[axis] > best[0]:
            found.append(f"{axis} {ours[axis]}>{best[0]}({best[1]})")
    return found


def bytes_over(level: str, ours: dict, refs: dict) -> str:
    """At O1/O2/O3 the code bytes over the best bar, for the note column: reported, not judged."""
    if level == "Os":
        return ""
    over = ours["code"] - min(r["code"] for r in refs.values())
    return f"bytes +{over} (reported)" if over > 0 else ""


CONVENTION_BYTES = 4096  # a zero array this big is a layout choice (stored in the image, or zeroed at start-up), not code quality


def convention(ours: dict, refs: dict) -> bool:
    """Ours keeps a big zero array in bss and a reference stores the same zeros as data: the bytes differ by the object format (`huge`: 80,400 B stored by
    bcc, tcpp and ow, none by ours), so the cell is shown and counted as neither a win nor a loss."""
    held = ours["static"] - ours["data"]
    return held >= CONVENTION_BYTES and any(r["data"] >= held for r in refs.values())


def net_code(ours: dict | None, language: str, program: str, level: str) -> int | str:
    """A Nib cell that does file I/O: its code less the std.io skeleton's fixed bytes (bench.net_code), the measure the plain-C bar is read against; "" for any other cell."""
    if not ours or language != "nib" or program not in bench.io_programs([]) or level not in bench.fixed_io_skeleton():
        return ""
    return bench.net_code(ours["code"], level)


def judged_by(program: str, language: str, level: str, present: set[str]) -> tuple[str, tuple[str, ...], str]:
    """(bar program, reference compilers, their level) of a cell of ours. C and Nib: the level of the cell, and the references' -O2 for -O3, which none of them has (oversight,
    2026-11-26). BASIC: the references' -O2."""
    if language in ("c", "nib"):
        return bench.bar_program(program, language, present), C_REFS, "O2" if level == "O3" else level
    return program, BAS_REFS, "O2"


def rows(table: dict) -> tuple[list[tuple], collections.Counter]:
    """(rows, counts per level and verdict) of a bench run's {"NAME/LANG/LEVEL": counts}."""
    present = {key.rsplit("/", 2)[0] for key in table}
    out, tally = [], collections.Counter()
    for key, value in sorted(table.items()):
        program, language, level = key.rsplit("/", 2)
        if language in C_REFS + BAS_REFS or language not in OURS:
            continue
        ours = counts(value, program, language)
        bar, names, at = judged_by(program, language, level, present)
        refs = {name: counts(table.get(f"{bar}/{name}/{at}"), bar, name) for name in names}
        have = {name: r for name, r in refs.items() if r}
        plain = {name: counts(table.get(f"{program}/{name}/{at}"), program, name) for name in names} if bar != program else {}
        plain = {name: r for name, r in plain.items() if r}
        if not ours:
            verdict, why = ("REFUSED" if "error" in (value or {}) else "UNMEASURED"), (value or {}).get("error", "")[:60]
            if limit_bound(value):
                verdict, why = "UNMEASURED", f"{limit_bound(value)} instructions: stopped at the limit"
            row = (program, language, level, "", "", "", "", "", "", "", verdict, why)
        elif bench.is_twin_name(program):
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], "", "", "", "TWIN", "reference only: the bar of its program's Nib row")
        elif language == "nib" and bar == program and has_twin_source(program):
            # The twin was not measured: the plain source's bar lacks the Nib semantics and the I/O, so it is no bar (2026-11-13: six Nib cells read WORSE so).
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], "", "", "", "NO_TWIN", f"twin of {program} not measured")
        elif not have:
            verdict = "NO_BAR_BY_CONSTRUCTION" if no_references(program) else "NO_BAR"
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], "", "", "", verdict, "")
        elif convention(ours, have):
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], min(r["code"] for r in have.values()), min(r["clocks"] for r in have.values()), len(names) - len(have), "CONVENTION",
                   "a reference stores the zeros ours holds in bss: an object-format convention, neither a win nor a loss")
        else:
            found = findings(level, ours, have)
            verdict = "OK" if not found else ("WORSE_LB" if len(names) - len(have) else "WORSE")
            note = ";".join([*found, *filter(None, [bytes_over(level, ours, have)])]) + (f" [bar {bar}]" if bar != program else "")
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], min(r["code"] for r in have.values()), min(r["clocks"] for r in have.values()), len(names) - len(have), verdict, note)
        bounds = [f"{name} {limit_bound(table.get(f'{bar}/{name}/{at}'))}" for name in names if limit_bound(table.get(f"{bar}/{name}/{at}"))]
        errors = {name: table[f"{bar}/{name}/{at}"]["error"].splitlines()[0] for name in names if "error" in table.get(f"{bar}/{name}/{at}", {}) and not limit_bound(table[f"{bar}/{name}/{at}"])}
        wrong = [f"{name} ({error})" for name, error in errors.items() if error.startswith("prints ")]
        broken = [f"{name} ({error[:40]})" for name, error in errors.items() if not error.startswith("prints ")]
        if (bounds or broken or wrong) and ours:
            notes = [row[11], "bound: " + ", ".join(bounds) + " instructions (stopped at the limit; no bar)" if bounds else "",
                     "wrong output, not a bar: " + ", ".join(wrong) + (f" [{WRONG_OUTPUT_CAUSES[bar]}]" if wrong and bar in WRONG_OUTPUT_CAUSES else "") if wrong else "",
                     "no bar from: " + ", ".join(broken) if broken else ""]
            row = (*row[:11], "; ".join(filter(None, notes)))
        tally[(level, row[10])] += 1
        plain_code = min((r["code"] for r in plain.values()), default="")
        net = net_code(ours, language, program, level)
        out.append((*row, plain_code, min((r["clocks"] for r in plain.values()), default=""), net, net - plain_code if net != "" and plain_code != "" else ""))
    return out, tally


def reference_coverage(table: dict) -> dict[str, tuple[int, int]]:
    """Per reference compiler, (cells measured, cells the table's own rows asked of it): a reference with none measured is a broken job, not a program
    it cannot build (the 18de C table had `ow` only: every bcc, tc and tcpp cell read "no object")."""
    present = {key.rsplit("/", 2)[0] for key in table}
    asked: collections.Counter = collections.Counter()
    got: collections.Counter = collections.Counter()
    for key in table:
        program, language, level = key.rsplit("/", 2)
        if language in C_REFS + BAS_REFS or language not in OURS or bench.is_twin_name(program):
            continue
        bar, names, at = judged_by(program, language, level, present)
        for name in names:
            asked[name] += 1
            got[name] += bool(counts(table.get(f"{bar}/{name}/{at}"), bar, name))
    return {name: (got[name], asked[name]) for name in asked}


def md5_lines(dist: Path) -> str:
    return " ".join(f"{hashlib.md5((dist / name).read_bytes()).hexdigest()}  {dist / name}" for name in ("llrm-llc", "llrm-cc", "llrm-nib") if (dist / name).is_file())


def render(table: dict, dist: Path, harness: str, now: datetime.datetime) -> str:
    body, tally = rows(table)
    header = [
        f"# x86-m16 owner table {now:%Y-%m-%dT%H:%MZ} (date -u)",
        f"# dist {dist}; md5: {md5_lines(dist)}",
        f"# harness commit {harness}; bench.py --opt {' '.join(sorted({key.rsplit('/', 1)[1] for key in table if key.rsplit('/', 2)[1] in OURS}))} --references --no-judge; no -march: the default CPU ({OURS_ISA})",
        "# net_code: a Nib cell with file I/O less the std.io skeleton's fixed bytes (io_skeleton/fixed.toml, not this build's measure); net_vs_plain_code: net_code less plain_ext_code (strict at O1/Os, priced at O2)",
        "# bytes: code (CODE), data (initialised), static (data+bss); judged (ruling 2026-11-14): Os on total bytes (code+data; BASIC code+static) and clocks; O1/O2/O3 on clocks only, bytes reported in findings; C and Nib O3 cells against the references' O2 build (they have no O3);"
        " best external per language: C = ow/bcc/tc/tcpp, Nib = the C compilers on the C twin with the Nib semantics (NAME~init, NAME~bounds; the plain source's best in plain_ext_*), BASIC = bc/pds71/vbdos (O2 key)",
        "# verdicts: OK, WORSE, CONVENTION (a reference stores the zeros ours holds in bss: shown, not counted a win or a loss), WORSE_LB (a reference lacks a cell: lower bound), NO_BAR (no reference), NO_BAR_BY_CONSTRUCTION (bench.toml `references = false`: listed for regression tracking, never counted in the bar), NO_TWIN (a Nib program whose twin is not measured: unproven, never OK), TWIN (a reference twin, not judged), UNMEASURED/REFUSED (counted, never OK)",
        "# reference cells measured/asked: " + " ".join(f"{name}={got}/{asked}" for name, (got, asked) in reference_coverage(table).items()) + "; a cell without a bar reads NO_BAR or WORSE_LB",
        "# counts: " + ", ".join(f"{level}:{verdict}={n}" for (level, verdict), n in sorted(tally.items())),
    ]
    columns = "prog\tlang\tlevel\tcode_B\tdata_B\tstatic_B\tclk\tbest_ext_code\tbest_ext_clk\trefs_missing\tverdict\tfindings\tplain_ext_code\tplain_ext_clk\tnet_code\tnet_vs_plain_code\tours_isa\trefs_isa"
    lines = ["\t".join(str(x) for x in (*row, OURS_ISA, "ow486/bcc386/tc186/tcpp286" if row[1] in ("c", "nib") else "bc86/pds86/vbdos86")) for row in body]
    return "\n".join(header) + "\n" + columns + "\n" + "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    if len(argv) not in (3, 4):
        print(__doc__)
        return 2
    table = json.loads(Path(argv[0]).read_text())
    empty = [name for name, (got, asked) in reference_coverage(table).items() if asked and not got]
    if empty:
        print(f"refused: no cell measured for {', '.join(empty)}: a broken reference job, not a table", file=sys.stderr)
        return 3
    harness = argv[3] if len(argv) == 4 else subprocess.run(["git", "-C", str(bench.ROOT), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    Path(argv[2]).write_text(render(table, Path(argv[1]), harness, datetime.datetime.now(datetime.UTC)))
    print(f"{len(rows(table)[0])} rows")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

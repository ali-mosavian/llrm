"""m16_owner_table.py BENCH.json DIST OUT.tsv [HARNESS_COMMIT]: the m16 owner table, from a `bench.py --opt O1 O2 Os --references --no-judge --json` run.

One row per program, language and level of ours, judged per axis against the best external (C: ow, bcc, tc, tcpp; BASIC: bc, pds71, vbdos): O1 and Os strict on code,
clocks, data and static; O2 priced (1 clk = 2 B) plus strict data and static. A Nib row's bar is the C compilers on the C twin that has the Nib semantics
(`bench.bar_program`: `NAME~init`, `NAME~bounds`), the compilers on the plain source stay beside it (`plain_ext_*`); a twin's own row is reference-only (verdict TWIN); a Nib program with a twin on disk and none measured is NO_TWIN, never judged on the plain source.
The header names the dist, the md5 of its compilers, the harness commit and `date -u`: a table without them is not evidence.
"""

import collections
import datetime
import hashlib
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import bench  # noqa: E402

C_REFS = ("ow", "bcc", "tc", "tcpp")
BAS_REFS = ("bc", "pds71", "vbdos")
OURS = ("c", "nib", "bas", "bq16")


def counts(row: dict | None, program: str = "", name: str = "") -> dict | None:
    """The four axes of a measured row, None where the cell is missing or errored. `static` is data + bss + the arrays `name` allocates at
    run time for the BASIC `program` (bench.run_time_arrays): a reference that allocates what llrm holds in its data is not smaller for it."""
    if not row or "error" in row or "clocks" not in row or "code_bytes" not in row:
        return None
    return {"code": row["code_bytes"], "clocks": row["clocks"], "data": row.get("data_bytes", 0), "static": bench.with_footprint({"data_bytes": row.get("data_bytes", 0), "bss_bytes": row.get("bss_bytes", 0)}, bench.run_time_arrays(program, name))["static_bytes"]}


def has_twin_source(program: str) -> bool:
    """A Nib program with a C twin on disk (`bench/NAME/twin/SEMANTIC.c`): its bar is that twin, whose object holds the same I/O functions
    (BENCH_IO_FUNCTIONS) and bounds checks as the Nib program's. The plain source has neither."""
    return any((bench.BENCH / program / "twin" / f"{semantic}.c").exists() for semantic in bench.TWINS)


def findings(level: str, ours: dict, refs: dict) -> list[str]:
    """Where `ours` is worse than the best of `refs`, per axis."""
    found = []

    def strict(axes):
        for axis in axes:
            best = min((r[axis], name) for name, r in refs.items())
            if ours[axis] > best[0]:
                found.append(f"{axis} {ours[axis]}>{best[0]}({best[1]})")

    if level in ("O1", "Os"):
        strict(("code", "clocks", "data", "static"))
    else:
        strict(("data", "static"))
        if max(1000 * (ours["clocks"] - r["clocks"]) + 500 * (ours["code"] - r["code"]) for r in refs.values()) > 0:
            found.append("priced")
    return found


def net_code(ours: dict | None, language: str, program: str, level: str) -> int | str:
    """A Nib cell that does file I/O: its code less the std.io skeleton's fixed bytes (bench.net_code), the measure the plain-C bar is read against; "" for any other cell."""
    if not ours or language != "nib" or program not in bench.io_programs([]) or level not in bench.fixed_io_skeleton():
        return ""
    return bench.net_code(ours["code"], level)


def rows(table: dict) -> tuple[list[tuple], collections.Counter]:
    """(rows, counts per level and verdict) of a bench run's {"NAME/LANG/LEVEL": counts}."""
    present = {key.rsplit("/", 2)[0] for key in table}
    out, tally = [], collections.Counter()
    for key, value in sorted(table.items()):
        program, language, level = key.rsplit("/", 2)
        if language in C_REFS + BAS_REFS or language not in OURS:
            continue
        ours = counts(value, program, language)
        if language in ("c", "nib"):
            bar, names, at = bench.bar_program(program, language, present), C_REFS, level
        else:
            bar, names, at = program, BAS_REFS, "O2"
        refs = {name: counts(table.get(f"{bar}/{name}/{at}"), bar, name) for name in names}
        have = {name: r for name, r in refs.items() if r}
        plain = {name: counts(table.get(f"{program}/{name}/{at}"), program, name) for name in names} if bar != program else {}
        plain = {name: r for name, r in plain.items() if r}
        if not ours:
            verdict, why = ("REFUSED" if "error" in (value or {}) else "UNMEASURED"), (value or {}).get("error", "")[:60]
            row = (program, language, level, "", "", "", "", "", "", "", verdict, why)
        elif bench.is_twin_name(program):
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], "", "", "", "TWIN", "reference only: the bar of its program's Nib row")
        elif language == "nib" and bar == program and has_twin_source(program):
            # The twin was not measured: the plain source's bar lacks the Nib semantics and the I/O, so it is no bar (2026-11-13: six Nib cells read WORSE so).
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], "", "", "", "NO_TWIN", f"twin of {program} not measured")
        elif not have:
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], "", "", "", "NO_BAR", "")
        else:
            found = findings(level, ours, have)
            verdict = "OK" if not found else ("WORSE_LB" if len(names) - len(have) else "WORSE")
            note = ";".join(found) + (f" [bar {bar}]" if bar != program else "")
            row = (program, language, level, ours["code"], ours["data"], ours["static"], ours["clocks"], min(r["code"] for r in have.values()), min(r["clocks"] for r in have.values()), len(names) - len(have), verdict, note)
        tally[(level, row[10])] += 1
        plain_code = min((r["code"] for r in plain.values()), default="")
        net = net_code(ours, language, program, level)
        out.append((*row, plain_code, min((r["clocks"] for r in plain.values()), default=""), net, net - plain_code if net != "" and plain_code != "" else ""))
    return out, tally


def md5_lines(dist: Path) -> str:
    return " ".join(f"{hashlib.md5((dist / name).read_bytes()).hexdigest()}  {dist / name}" for name in ("llrm-llc", "llrm-cc", "llrm-nib") if (dist / name).is_file())


def render(table: dict, dist: Path, harness: str, now: datetime.datetime) -> str:
    body, tally = rows(table)
    header = [
        f"# x86-m16 owner table {now:%Y-%m-%dT%H:%MZ} (date -u)",
        f"# dist {dist}; md5: {md5_lines(dist)}",
        f"# harness commit {harness}; bench.py --opt O1 O2 Os --references --no-judge; flags -march=i386",
        "# net_code: a Nib cell with file I/O less the std.io skeleton's fixed bytes (io_skeleton/fixed.toml, not this build's measure); net_vs_plain_code: net_code less plain_ext_code (strict at O1/Os, priced at O2)",
        "# bytes: code (CODE), data (initialised), static (data+bss); judged per axis: O1/Os strict on code/clocks/data/static; O2 priced (1 clk = 2 B) plus strict data/static;"
        " best external per language: C = ow/bcc/tc/tcpp, Nib = the C compilers on the C twin with the Nib semantics (NAME~init, NAME~bounds; the plain source's best in plain_ext_*), BASIC = bc/pds71/vbdos (O2 key)",
        "# verdicts: OK, WORSE, WORSE_LB (a reference lacks a cell: lower bound), NO_BAR (no reference), NO_TWIN (a Nib program whose twin is not measured: unproven, never OK), TWIN (a reference twin, not judged), UNMEASURED/REFUSED (counted, never OK)",
        "# counts: " + ", ".join(f"{level}:{verdict}={n}" for (level, verdict), n in sorted(tally.items())),
    ]
    columns = "prog\tlang\tlevel\tcode_B\tdata_B\tstatic_B\tclk\tbest_ext_code\tbest_ext_clk\trefs_missing\tverdict\tfindings\tplain_ext_code\tplain_ext_clk\tnet_code\tnet_vs_plain_code\tours_isa\trefs_isa"
    lines = ["\t".join(str(x) for x in (*row, "i386", "ow486/bcc386/tc186/tcpp286" if row[1] in ("c", "nib") else "bc86/pds86/vbdos86")) for row in body]
    return "\n".join(header) + "\n" + columns + "\n" + "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    if len(argv) not in (3, 4):
        print(__doc__)
        return 2
    table = json.loads(Path(argv[0]).read_text())
    harness = argv[3] if len(argv) == 4 else subprocess.run(["git", "-C", str(bench.ROOT), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    Path(argv[2]).write_text(render(table, Path(argv[1]), harness, datetime.datetime.now(datetime.UTC)))
    print(f"{len(rows(table)[0])} rows")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

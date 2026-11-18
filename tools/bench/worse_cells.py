"""Cells where ours is worse than the best stored reference compiler, per axis.

    python3 tools/bench/worse_cells.py RESULTS.json... [--bench bench]

RESULTS are bench.py --json files (keys `program/language/level`, a program may be `parity/name`). A reference is a section
`[compiler.LEVEL]` of the program's expected.toml. Bytes are code, data and static (data + bss; bss is judged only inside static),
the three bench.py judges: a constant kept out of .text counts for every column. At -O2 a cell is worse than a reference when 1000 * delta clocks + price * delta bytes > 0 (bench.py `against`; the price is
bench.rate_for, milliclocks per byte, 500 on m16: one clock is two bytes); at -O1 and -Os each axis is strict. A program with `references = false` is listed as "no ref"; a program
with neither references nor that mark raises: a lookup that finds nothing must not read as "nothing worse".
"""

import sys
import tomllib
from pathlib import Path

REFERENCES = {"c": ("ow", "bcc", "tc", "tcpp"), "bq16": ("bc", "pds71", "vbdos")}


class NoReferenceLookup(Exception):
    pass


class NoHelperBytes(Exception):
    """A stored C reference cell with no `helper_bytes`: measured before the library routines a program links were counted, so its
    `code_bytes` may read smaller than the program is (tc's recpow -O1: 211 B, 399 with its long divide and multiply)."""


def rate() -> int:
    """The price of a byte at -O2 in milliclocks: bench.py's own (`rate_for`), read from the m16 timing sheet, never a copy."""
    import bench

    return bench.rate_for("c", "O2")


def with_footprint(row: dict, program: str = "", name: str = "") -> dict:
    import bench

    return bench.with_footprint(row, bench.run_time_arrays(program, name))


def program_of(key: str) -> str:
    """`parity/algebra/c/O2` -> `parity/algebra`."""
    return key.rsplit("/", 2)[0]


def references(bench: Path, program: str, language: str, level: str) -> dict[str, dict] | None:  # noqa: C901
    """{compiler: its numbers} for the program at the level; None if the program says `references = false`."""
    folder = bench / program
    marked = tomllib.loads((folder / "bench.toml").read_text()) if (folder / "bench.toml").exists() else {}
    if marked.get("references") is False:
        return None
    path = folder / "expected.toml"
    if not path.exists():
        raise NoReferenceLookup(f"{program}: no expected.toml and not marked `references = false`")
    table = tomllib.loads(path.read_text())
    cells = {name: table[name][level] for name in REFERENCES[language] if level in table.get(name, {})}
    if language == "c":
        for name, cell in cells.items():
            if "helper_bytes" not in cell:
                raise NoHelperBytes(f"{program} {name}.{level}: no helper_bytes: re-measure with bench.py --references --bless")
    return cells


def worse(results: dict, bench: Path, price: int | None = None) -> tuple[list[tuple], list[str], list[str]]:
    """([(key, [axis findings])], [programs without references], [`key: reference compilers with no stored cell`]). A missing column
    is reported, never skipped: the next-best column would stand in for the best one and understate the gap."""
    price = rate() if price is None else price
    rows, none, missing = [], [], []
    for key, ours in results.items():
        program, language, level = program_of(key), key.split("/")[-2], key.split("/")[-1]
        if language not in REFERENCES or "code_bytes" not in ours:
            continue
        ours = with_footprint(ours, program, language)
        found = references(bench, program, language, level)
        found = None if found is None else {name: with_footprint(numbers, program, name) for name, numbers in found.items()}
        if found is None:
            none.append(program)
            continue
        absent = [name for name in REFERENCES[language] if name not in found]
        if absent:
            missing.append(f"{key}: no stored {', '.join(absent)}")
        findings = []
        if level == "O2":
            for name, numbers in sorted(found.items()):
                if all(axis in numbers for axis in ("clocks", "code_bytes")):
                    clocks, size = ours["clocks"] - numbers["clocks"], ours["code_bytes"] - numbers["code_bytes"]
                    if 1000 * clocks + price * size > 0:
                        findings.append(f"{clocks:+d} clk {size:+d} B vs {name}")
            findings += strict(ours, found, DATA_AXES)
        else:
            findings += strict(ours, found, AXES)
        if findings:
            rows.append((key, findings))
    return rows, sorted(set(none)), missing


AXES = (("code_bytes", "B"), ("clocks", "clk"), ("data_bytes", "data B"), ("static_bytes", "static B"))
DATA_AXES = AXES[2:]  # at -O2 code bytes and clocks are traded; data and static are not


def strict(ours: dict, found: dict, axes) -> list[str]:
    findings = []
    for axis, label in axes:
        best = min(((numbers[axis], name) for name, numbers in found.items() if axis in numbers), default=None)
        if best and ours.get(axis, 0) > best[0]:
            findings.append(f"{label} {ours[axis]} vs {best[0]}({best[1]}) {100 * (ours[axis] - best[0]) / best[0]:+.1f}%" if best[0] else f"{label} {ours[axis]} vs 0({best[1]})")
    return findings


if __name__ == "__main__":
    import json

    bench = Path("bench")
    files = [argument for argument in sys.argv[1:] if not argument.startswith("--")]
    if "--bench" in sys.argv:
        bench = Path(sys.argv[sys.argv.index("--bench") + 1])
        files = [file for file in files if file != str(bench)]
    failed = False
    for file in files:
        rows, none, missing = worse(json.loads(Path(file).read_text()), bench)
        for key, findings in rows:
            print(key, "; ".join(findings))
        print("no ref:", ", ".join(none) or "-")
        for line in missing:
            print("MISSING", line)
        failed = failed or bool(missing)
    sys.exit(2 if failed else 0)

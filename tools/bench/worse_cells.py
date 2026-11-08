"""Cells where ours is worse than the best stored reference compiler, per axis.

    python3 tools/bench/worse_cells.py RESULTS.json... [--bench bench]

RESULTS are bench.py --json files (keys `program/language/level`, a program may be `parity/name`). A reference is a section
`[compiler.LEVEL]` of the program's expected.toml. At -O2 a cell is worse than a reference when 1000 * delta clocks + RATE * delta bytes > 0 (bench.py `against`; RATE
milliclocks per byte, 500 on m16: one clock is two bytes); at -O1 and -Os each axis is strict. A program with `references = false` is listed as "no ref"; a program
with neither references nor that mark raises: a lookup that finds nothing must not read as "nothing worse".
"""

import sys
import tomllib
from pathlib import Path

RATE = 500  # bench.rate_for("c", "O2") on m16
REFERENCES = {"c": ("ow", "bcc", "tc", "tcpp"), "bq16": ("bc", "pds71", "vbdos")}


class NoReferenceLookup(Exception):
    pass


def program_of(key: str) -> str:
    """`parity/algebra/c/O2` -> `parity/algebra`."""
    return key.rsplit("/", 2)[0]


def references(bench: Path, program: str, language: str, level: str) -> dict[str, dict] | None:
    """{compiler: its numbers} for the program at the level; None if the program says `references = false`."""
    folder = bench / program
    marked = tomllib.loads((folder / "bench.toml").read_text()) if (folder / "bench.toml").exists() else {}
    if marked.get("references") is False:
        return None
    path = folder / "expected.toml"
    if not path.exists():
        raise NoReferenceLookup(f"{program}: no expected.toml and not marked `references = false`")
    table = tomllib.loads(path.read_text())
    return {name: table[name][level] for name in REFERENCES[language] if level in table.get(name, {})}


def worse(results: dict, bench: Path) -> tuple[list[tuple], list[str]]:
    """([(key, [axis findings])], [programs without references])."""
    rows, none = [], []
    for key, ours in results.items():
        program, language, level = program_of(key), key.split("/")[-2], key.split("/")[-1]
        if language not in REFERENCES or "code_bytes" not in ours:
            continue
        found = references(bench, program, language, level)
        if found is None:
            none.append(program)
            continue
        findings = []
        if level == "O2":
            for name, numbers in sorted(found.items()):
                if all(axis in numbers for axis in ("clocks", "code_bytes")):
                    clocks, size = ours["clocks"] - numbers["clocks"], ours["code_bytes"] - numbers["code_bytes"]
                    if 1000 * clocks + RATE * size > 0:
                        findings.append(f"{clocks:+d} clk {size:+d} B vs {name}")
        else:
            for axis, label in (("code_bytes", "B"), ("clocks", "clk")):
                best = min(((numbers[axis], name) for name, numbers in found.items() if axis in numbers), default=None)
                if best and ours.get(axis, 0) > best[0]:
                    findings.append(f"{label} {ours[axis]} vs {best[0]}({best[1]}) {100 * (ours[axis] - best[0]) / best[0]:+.1f}%")
        if findings:
            rows.append((key, findings))
    return rows, sorted(set(none))


if __name__ == "__main__":
    import json

    bench = Path("bench")
    files = [argument for argument in sys.argv[1:] if not argument.startswith("--")]
    if "--bench" in sys.argv:
        bench = Path(sys.argv[sys.argv.index("--bench") + 1])
        files = [file for file in files if file != str(bench)]
    for file in files:
        rows, none = worse(json.loads(Path(file).read_text()), bench)
        for key, findings in rows:
            print(key, "; ".join(findings))
        print("no ref:", ", ".join(none) or "-")

#!/usr/bin/env python3
"""Per function, what the old and the DAG instruction selector leave: instructions, copies, executed instructions, bytes.

    tools/pairs/isel_shadow.py LLRM_CC [--new LLRM_CC2] [--old-isel old] [--new-isel dag] [--src DIR] [--levels O1,O2] [--targets m32,m16] [--programs a,b]

Each cell (program, target, level) is compiled by `LLRM_CC --isel=OLD` and `LLRM_CC2 (default LLRM_CC) --isel=NEW` (default `shadow`: the DAG path where it can select, else the old selector, each refusal reported) with `LLRM_DEBUG=isel`; the rows
(`[isel] NAME selector=.. instructions=.. copies=.. executed=.. memory=..`, see backend/shadow.rs) and, at m32, the bytes of each function
(ELF symbol sizes) and at m16 (jwasm's listing of `-S`, offsets from `proc` to `endp`; calibrated against the OMF code bytes by
~/scratch/w3-qo/bytes.py) are compared by function. jwasm is $JWASM, else beside LLRM_CC, else on PATH. Prints one summary line, then each function that differs and each cell one side refused. A compile that
fails is a refusal and is never counted as equal. The sources are objsweep's. Exit 1 on any difference or refusal.
"""
from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from objsweep import FLAGS  # noqa: E402

ROW = re.compile(r"^\[isel\] (\S+) selector=(\S+) instructions=(\d+) copies=(\d+) executed=(\S+) memory=(\S+)$")
FIELDS = ("instructions", "copies", "executed", "memory")
SYMBOL = re.compile(r"^\s*\d+: [0-9a-f]+\s+(\d+) FUNC\s+\S+\s+\S+\s+\S+ (\S+)$")


REFUSED = re.compile(r"^\[isel\] (\S+) refused: (.*)$")


def parse_rows(stderr: str) -> dict[str, dict[str, str]]:
    """{function: {field: value}} from the `isel` channel's rows; a function written twice keeps its last row. A function the DAG path
    refused (`--isel=shadow`, which falls back to the old selector there) carries `refused: why`."""
    rows, refused = {}, {}
    for line in stderr.splitlines():
        if found := ROW.match(line):
            rows[found[1]] = dict(zip(FIELDS, found.groups()[2:]))
        elif found := REFUSED.match(line):
            refused[found[1]] = found[2]
    for name, why in refused.items():
        rows.setdefault(name, {})["refused"] = why
    return rows


def parse_sizes(readelf: str) -> dict[str, str]:
    """{function: bytes} from `readelf -sW`."""
    return {plain(found[2]): found[1] for line in readelf.splitlines() if (found := SYMBOL.match(line))}


def parse_listing(listing: str) -> dict[str, str]:
    """{function: bytes} from a jwasm listing: the offset of `NAME endp` less that of `NAME proc`; the decoration (`_f@3`) is dropped."""
    starts, sizes = {}, {}
    for line in listing.splitlines():
        if found := re.match(r"([0-9A-F]{8})\s+(\S+) (proc|endp)\b", line):
            at, name, what = int(found[1], 16), plain(found[2]), found[3]
            if what == "proc":
                starts[name] = at
            elif name in starts:
                sizes[name] = str(at - starts[name])
    return sizes


def plain(name: str) -> str:
    return re.sub(r"@\d+$", "", name).lstrip("_")


def jwasm_for(binary: str) -> str:
    found = os.environ.get("JWASM") or (Path(binary).parent / "jwasm").is_file() and str(Path(binary).parent / "jwasm") or shutil.which("jwasm")
    if not found:
        raise SystemExit("jwasm not found: set JWASM or put it beside the compiler (m16 bytes come from its listing)")
    return found


def differences(old: dict, new: dict) -> list[str]:
    """One line per function whose rows differ or that one side lacks, in name order."""
    lines = []
    for name in sorted(old.keys() | new.keys()):
        a, b = old.get(name), new.get(name)
        if a is None or b is None:
            lines.append(f"{name}: only in {'old' if b is None else 'new'}")
        elif "refused" in b:
            lines.append(f"{name}: REFUSED {b['refused']}")
        elif a != b:
            lines.append(f"{name}: " + ", ".join(f"{k} {a[k]} -> {b[k]}" for k in a if a[k] != b.get(k)))
    return lines


def compile_cell(binary: str, isel: str, source: Path, target: str, level: str, out: Path) -> tuple[dict | None, str]:
    """(rows, '') or (None, the first line of the error)."""
    env = {**os.environ, "LLRM_DEBUG": "isel", "LLRM_CC_TARGET": {"m32": "x86_32-wat", "m16": "x86_16-wat"}[target]}
    done = subprocess.run([binary, *FLAGS[target], *(["-fobject-format=elf"] if target == "m32" else []), f"-{level}", f"--isel={isel}", "-c", "-o", str(out), str(source)], capture_output=True, text=True, env=env, timeout=600)
    if done.returncode != 0:
        return None, (done.stderr.strip().splitlines() or [f"exit {done.returncode}"])[-1][:140]
    rows = {plain(name): row for name, row in parse_rows(done.stderr).items()}
    if target == "m32":
        sizes = parse_sizes(subprocess.run(["readelf", "-sW", str(out)], capture_output=True, text=True).stdout)
        for name, row in rows.items():
            row["bytes"] = sizes.get(name, "-")
    else:
        asm = out.with_suffix(".asm")
        made = subprocess.run([binary, *FLAGS[target], f"-{level}", f"--isel={isel}", "-S", "-o", str(asm), str(source)], capture_output=True, text=True, env=env)
        if made.returncode != 0:
            return None, (made.stderr.strip().splitlines() or ["-S failed"])[-1][:140]
        lst = out.with_suffix(".lst")
        subprocess.run([jwasm_for(binary), "-nologo", f"-Fl={lst}", f"-Fo={out.with_suffix('.jo')}", str(asm)], capture_output=True)
        sizes = parse_listing(lst.read_text(errors="replace")) if lst.exists() else {}
        for name, row in rows.items():
            row["bytes"] = sizes.get(name, "-")
    return rows, ""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("binary")
    parser.add_argument("--new", default="")
    parser.add_argument("--old-isel", default="old")
    parser.add_argument("--new-isel", default="shadow")
    parser.add_argument("--src", default=str(Path(os.environ.get("ROWS_WORK", Path.home() / "scratch/rows")) / "base/b"))
    parser.add_argument("--levels", default="O2")
    parser.add_argument("--targets", default="m32,m16")
    parser.add_argument("--programs", default="")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "6")))
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    import stamp

    rest, named, allow = stamp.options(sys.argv[1:])
    args = parser.parse_args(rest)
    new = args.new or args.binary
    stamp.check(args.binary, named["base_sha"], allow, "old")
    stamp.check(new, named["sha"], allow, "new")
    sources = sorted(Path(args.src).glob("*.c"))
    if args.programs:
        sources = [s for s in sources if s.stem in set(args.programs.split(","))]
    if not sources:
        print(f"no sources under {args.src}")
        return 2
    cells = [(s, t, lv) for s in sources for t in args.targets.split(",") for lv in args.levels.split(",")]
    scratch = Path(tempfile.mkdtemp(prefix="isel-shadow-"))

    def run(cell):
        source, target, level = cell
        tag = f"{source.stem}-{target}-{level}"
        return (cell, compile_cell(args.binary, args.old_isel, source, target, level, scratch / f"{tag}.old"),
                compile_cell(new, args.new_isel, source, target, level, scratch / f"{tag}.new"))

    report, refused, compared, same_cells = [], [], 0, 0
    with ThreadPoolExecutor(args.jobs) as pool:
        for (source, target, level), (a, ea), (b, eb) in pool.map(run, cells):
            name = f"{source.stem} {target} -{level}"
            if a is None or b is None:
                refused.append(f"{name}: old: {ea or 'ok'}; new: {eb or 'ok'}")
                continue
            compared += len(a.keys() | b.keys())
            if lines := differences(a, b):
                report.extend(f"  {name} {line}" for line in lines)
            else:
                same_cells += 1
    print(f"isel_shadow old={args.old_isel} new={args.new_isel}: {len(report)} functions differ of {compared}; {same_cells} of {len(cells)} cells identical, {len(refused)} refused")
    print("\n".join(report))
    for line in refused:
        print(f"  REFUSED {line}")
    return 1 if report or refused else 0


if __name__ == "__main__":
    sys.exit(main())

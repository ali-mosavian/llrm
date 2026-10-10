#!/usr/bin/env python3
"""The -g matrix: for each frontend, bit width, object format and debug format, whether llrm emits the information and
whether an independent reader accepts it with the names, lines, parameters and locals of a small known program.

    debug-matrix.py [--llrm DIR] [--only fe,bits]      prints the matrix (markdown) and the failing checks

Readers (none is llrm's own): llvm-dwarfdump --verify (DWARF), lld-link + llvm-pdbutil (CodeView in COFF), jwlink
`debug codeview` + Open Watcom cvpack + wdump (CodeView in OMF, linked to an MZ or LE image). Turbo Debugger records are
read by TLINK/TDUMP in DOSBox: tests/turbo.rs.
"""

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))
import llrmbin  # noqa: E402

WATCOM = Path.home() / "dos/devtools/dev/c/watcom/binl"
JWLINK = Path.home() / ".local/bin/jwlink"

# What each program is, and what a reader must find in what it reads.
PROGRAMS = {
    "c": {"file": "known.c", "functions": ["add", "main"], "variables": ["a", "b", "sum", "p", "counter"], "lines": [7, 8, 9, 14, 15, 16, 17], "types": ["pt"]},
    "nib": {"file": "known.nib", "functions": ["add", "main"], "variables": ["a", "b", "sum", "total"], "lines": [2, 3, 6, 7], "types": []},
    "qb": {"file": "known.bas", "functions": ["add"], "variables": ["a", "b", "sum", "total"], "lines": [3, 4, 7, 8, 9], "types": []},
}
OBJECTS = {16: ["omf"], 32: ["omf", "coff", "elf", "macho"]}
FORMATS = {"codeview": "-gcodeview", "td": "-gtd", "dwarf": "-gdwarf"}


def run(argv, **kw):
    return subprocess.run([str(a) for a in argv], capture_output=True, text=True, errors="replace", **kw)


def missing(wanted, text, fold=True):
    hay = text.lower() if fold else text
    return [w for w in wanted if (w.lower() if fold else w) not in hay]


def compile_cell(llrm, fe, bits, obj, fmt, out):
    program = PROGRAMS[fe]
    res = run([llrm / f"llrm-{fe}", f"-m{bits}", "-O0", FORMATS[fmt], f"-fobject-format={obj}", ROOT / "tests/fixtures/matrix" / program["file"], "-o", out])
    if res.returncode != 0:
        return False, (res.stderr.strip().splitlines() or ["failed"])[-1]
    return True, ""


def present(obj, path):
    """The debug formats an object carries, by the sections or records that hold them (not by reading them)."""
    if obj in ("elf", "coff", "macho"):
        sections = run(["llvm-readobj-20", "-S", path]).stdout
        found = set()
        if ".debug_info" in sections or "__debug_info" in sections:
            found.add("dwarf")
        if ".debug$S" in sections:
            found.add("codeview")
        return found
    classes, data, at = set(), path.read_bytes(), 0
    while at + 3 <= len(data):
        kind, size = data[at], int.from_bytes(data[at + 1 : at + 3], "little")
        if kind == 0x88 and size > 2:
            classes.add(data[at + 4])
        at += 3 + size
    return ({"codeview"} if 0xA1 in classes else set()) | ({"td"} if classes & {0xE3, 0xE5, 0xE0, 0xE1, 0xE2} else set())


def check_dwarf(fe, obj_path):
    program = PROGRAMS[fe]
    verify = run(["llvm-dwarfdump-20", "--verify", obj_path])
    if verify.returncode != 0:
        return [f"llvm-dwarfdump --verify: {verify.stdout.strip().splitlines()[-1] if verify.stdout.strip() else verify.stderr.strip()}"]
    info = run(["llvm-dwarfdump-20", "--debug-info", "--debug-line", obj_path]).stdout
    problems = []
    named = set()
    for die in re.split(r"(?m)^0x[0-9a-f]+:\s+", info)[1:]:
        tag, name = re.match(r"DW_TAG_(\w+)", die), re.search(r'DW_AT_name\s+\("([^"]*)"\)', die)
        if tag and name:
            named.add((tag.group(1), name.group(1)))
    kinds = {"functions": {"subprogram"}, "variables": {"variable", "formal_parameter"}, "types": {"structure_type"}}
    for kind, tags in kinds.items():
        for name in program[kind]:
            if not any((tag, name) in named for tag in tags):
                problems.append(f"no {kind[:-1]} {name}")
    located = len(re.findall(r"DW_AT_location", info))
    if located == 0:
        problems.append("no variable has a location")
    rows = {int(m) for m in re.findall(r"^0x[0-9a-f]+\s+(\d+)\s", info, re.M)}
    for line in program["lines"]:
        if line not in rows:
            problems.append(f"no line {line}")
    return problems


def check_coff_codeview(fe, obj_path, work):
    program = PROGRAMS[fe]
    entry = "main"
    pdb = work / "x.pdb"
    linked = run(["lld-link", "/machine:x86", "/subsystem:console", f"/entry:{entry}", "/nodefaultlib", "/debug", f"/pdb:{pdb}", f"/out:{work / 'x.exe'}", obj_path])
    if linked.returncode != 0:
        return [f"lld-link: {(linked.stdout + linked.stderr).strip().splitlines()[-1]}"]
    dump = run(["llvm-pdbutil-20", "dump", "-l", "--symbols", "--types", "--globals", pdb]).stdout
    symbols = {"functions": set(re.findall(r"S_[GL]PROC32 \[[^\]]*\] `([^`]*)`", dump)), "types": set(re.findall(r"LF_(?:STRUCTURE|UNION|CLASS) [^`\n]*`([^`]*)`", dump)),
               "variables": set(re.findall(r"S_(?:LOCAL|[GL]DATA32) \[[^\]]*\] `([^`]*)`", dump))}
    problems = [f"no {kind[:-1]} {name}" for kind, found in symbols.items() for name in program[kind] if name not in found]
    rows = {int(m) for m in re.findall(r"(?<![\w:])(\d+) [0-9A-F]{8}\b", dump)}
    problems += [f"no line {line}" for line in program["lines"] if line not in rows]
    return problems


def check_omf_codeview(fe, bits, obj_path, work):
    program = PROGRAMS[fe]
    image = work / "x.exe"
    publics = re.findall(r"- '([^']+)' Type", run([WATCOM / "dmpobj", "-q", obj_path]).stdout)
    start = next((name for name in publics if "main" in name.lower()), publics[0])
    fmt = ["format", "dos"] if bits == 16 else ["format", "os2", "le"]
    linked = run([JWLINK, *fmt, "debug", "codeview", "file", obj_path, "name", image, "option", f"quiet,undefsok,start={start},nodefaultlibs"])
    if not image.exists():
        return [f"jwlink: {(linked.stdout + linked.stderr).strip().splitlines()[-1]}"]
    # cvpack renames a temporary over the image: both must be on one device.
    packed = run([WATCOM / "cvpack", image], cwd=work, env={**os.environ, "TMPDIR": str(work), "TMP": str(work), "TEMP": str(work)})
    if packed.returncode != 0:
        return [f"cvpack: {packed.stderr.strip() or packed.stdout.strip().splitlines()[-1]}"]
    dump = run([WATCOM / "wdump", "-q", "-d", "-Dx", image]).stdout
    if "NB09" not in dump:
        return ["wdump: no CodeView in the packed image"]
    text = image.read_bytes().decode("latin-1")
    problems = [f"no {kind[:-1]} {name}" for kind in ("functions", "variables", "types") for name in missing(program[kind], text)]
    rows = {int(m, 16) for m in re.findall(r"^\s+[0-9A-F]{8}\s+([0-9A-F]{4})/\d+", dump, re.M)}
    problems += [f"no line {line}" for line in program["lines"] if line not in rows]
    return problems


def cell(llrm, fe, bits, obj, fmt, work):
    out = work / f"{fe}-{bits}-{obj}-{fmt}.o"
    emitted, why = compile_cell(llrm, fe, bits, obj, fmt, out)
    if not emitted:
        return "not emitted", why
    has = present(obj, out)
    if fmt not in has:
        return "not emitted", f"-g{fmt} wrote {sorted(has) or 'no debug information'}"
    if obj == "elf" or (fmt == "dwarf" and obj in ("coff", "macho")):
        problems = check_dwarf(fe, out)
    elif obj == "coff":
        problems = check_coff_codeview(fe, out, work)
    elif obj == "omf" and fmt == "codeview":
        problems = check_omf_codeview(fe, bits, out, work)
    else:
        return "emitted", "no reader here: tests/turbo.rs (TLINK/TDUMP in DOSBox)"
    return ("valid", "") if not problems else ("BROKEN", "; ".join(problems))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--llrm")
    parser.add_argument("--only")
    args = parser.parse_args()
    llrm = Path(args.llrm) if args.llrm else llrmbin.bin_dir()
    only = set(args.only.split(",")) if args.only else None
    rows, details = [], []
    with tempfile.TemporaryDirectory() as tmp:
        for fe in PROGRAMS:
            for bits in (16, 32):
                if only and fe not in only and f"{fe},{bits}" not in only:
                    continue
                for obj in OBJECTS[bits]:
                    for fmt in FORMATS:
                        work = Path(tmp) / f"{fe}{bits}{obj}{fmt}"
                        work.mkdir()
                        state, why = cell(llrm, fe, bits, obj, fmt, work)
                        rows.append((fe, bits, obj, fmt, state))
                        if why:
                            details.append(f"{fe} -m{bits} {obj} {fmt}: {state}: {why}")
    print("| frontend | -m | object | CodeView | Turbo Debugger | DWARF |")
    print("|---|---|---|---|---|---|")
    seen = {}
    for fe, bits, obj, fmt, state in rows:
        seen.setdefault((fe, bits, obj), {})[fmt] = state
    for (fe, bits, obj), states in seen.items():
        print(f"| {fe} | {bits} | {obj} | " + " | ".join(states.get(f, "") for f in FORMATS) + " |")
    print()
    print("\n".join(details))


if __name__ == "__main__":
    main()

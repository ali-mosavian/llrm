#!/usr/bin/env python3
"""Frame rules (`.eh_frame`), ours against the pin's, on one IR.

Each C file goes through the pin clang and `opt -O2` once; the pin `llc` and `llrm-llc` then make an x86-64 ELF object of that same `.ll`. A function
whose code is the same instruction for instruction is compared: the rows of its unwind table (offset into the function, canonical frame address,
saved registers) must match. A function whose code differs is refused, never counted as equal.
"""
import argparse
import os
import sys
import tempfile
from pathlib import Path

from dwarf_loc_diff import code_of, run


# triple, objdump, the flags of our llc after the pin's
TARGETS = {
    "x86_64": ("x86_64-unknown-linux-gnu", "objdump", []),
    "aarch64": ("aarch64-unknown-linux-gnu", "aarch64-linux-gnu-objdump", ["--isel=dag"]),
}


def tables_of(path: str) -> dict[str, tuple]:
    """function -> the rows of its FDE: (offset, cfa register, cfa offset, saved registers), read with pyelftools."""
    from elftools.dwarf.callframe import FDE  # noqa: PLC0415
    from elftools.elf.elffile import ELFFile  # noqa: PLC0415

    with open(path, "rb") as handle:
        elf = ELFFile(handle)
        # No unwind table at all: every function is a hole, not a crash.
        if elf.get_section_by_name(".eh_frame") is None:
            return {}
        starts = {}
        for symbol in elf.get_section_by_name(".symtab").iter_symbols():
            if symbol["st_info"]["type"] == "STT_FUNC":
                starts[symbol["st_value"]] = symbol.name
        dwarf = elf.get_dwarf_info()
        out: dict[str, tuple] = {}
        for entry in dwarf.EH_CFI_entries() or []:
            if not isinstance(entry, FDE):
                continue
            begin = entry["initial_location"]
            name = starts.get(begin)
            if name is None:
                continue
            rows = []
            for row in entry.get_decoded().table:
                cfa = row["cfa"]
                saved = tuple(sorted((reg, rule.type, rule.arg) for reg, rule in row.items() if isinstance(reg, int)))
                rows.append((row["pc"] - begin, cfa.reg, cfa.offset, saved))
            out[name] = tuple(rows)
    return out


def shape(table: tuple | None) -> tuple | None:
    """The rows without their offsets."""
    return None if table is None else tuple(row[1:] for row in table)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="+")
    parser.add_argument("--pin", default=os.environ.get("LLRM_PIN_BIN"), help="the pin LLVM bin directory (default: $LLRM_PIN_BIN)")
    parser.add_argument("--llc", required=True, help="llrm-llc (llrm-arm64-llc with --target aarch64)")
    parser.add_argument("--target", choices=sorted(TARGETS), default="x86_64")
    args = parser.parse_args()
    if not args.pin:
        parser.error("--pin or LLRM_PIN_BIN names the pin LLVM bin directory")
    triple, objdump, extra = TARGETS[args.target]
    work = Path(tempfile.mkdtemp(prefix="cfidiff-"))
    same = different = refused = files = rich = shape_same = shape_different = 0
    for source in map(Path, args.sources):
        try:
            ir, opt = work / f"{source.stem}.ll", work / f"{source.stem}.opt.ll"
            run([f"{args.pin}/clang", f"--target={triple}", "-fno-pic", "-O2", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", str(source), "-o", str(ir)])
            run([f"{args.pin}/opt", "-O2", "-vectorize-loops=false", "-vectorize-slp=false", str(ir), "-S", "-o", str(opt)])
            pin_o, our_o = work / f"{source.stem}.pin.o", work / f"{source.stem}.our.o"
            run([f"{args.pin}/llc", "-O2", "-relocation-model=static", "-filetype=obj", str(opt), "-o", str(pin_o)])
            run([args.llc, "-O2", *extra, "-o", str(our_o), str(opt)])
            pin_code, our_code = (code_of(run([objdump, "-d", "--no-show-raw-insn", str(o)])) for o in (pin_o, our_o))
            pin_tables, our_tables = tables_of(str(pin_o)), tables_of(str(our_o))
        except (RuntimeError, KeyError) as why:
            print(f"  refused {source.name}: {why}")
            files += 1
            continue
        for name, table in sorted(pin_tables.items()):
            if our_code.get(name) != pin_code.get(name):
                refused += 1
                # Different code puts the rows at other offsets: the rules alone (frame register, offset, saved registers, in order) must still match.
                if shape(our_tables.get(name)) == shape(table):
                    shape_same += 1
                else:
                    shape_different += 1
                    print(f"  shape different {source.name} {name}:\n    pin  {table}\n    ours {our_tables.get(name)}")
            elif our_tables.get(name) == table:
                same += 1
                rich += len(table) > 1
            else:
                different += 1
                print(f"  different {source.name} {name}:\n    pin  {table}\n    ours {our_tables.get(name)}")
    print(
        f"cfi_diff: {same + different} functions compared, {refused} with different code (rules only: {shape_same} same, {shape_different} different), "
        f"{files} files refused: {same} same ({rich} with more than the entry row), {different} different"
    )
    return 2 if same + different == 0 else (1 if different else 0)


if __name__ == "__main__":
    sys.exit(main())

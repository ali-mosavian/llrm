#!/usr/bin/env python3
"""x86-64 Mach-O `__eh_frame` rows, ours against the pin's, on one IR.

The pin's x86-64 Mach-O object has no `__compact_unwind`, only `__eh_frame`. Each C file goes through the pin clang and `opt -O2` once; the pin
`llc` and our `llc` make an object of that `.ll`; `llvm-objdump --dwarf=frames` gives each FDE's call frame instructions. The functions are paired
by position. An object of ours with no FDEs gives every function as different (`--strip`, the control).
"""
import argparse
import os
import re
import sys
import tempfile
from pathlib import Path

from dwarf_loc_diff import run


def frames_of(text: str) -> list[list[str]]:
    """The instructions of each FDE in `llvm-objdump --dwarf=frames` text, in order; the CIE's are not repeated. Padding (`DW_CFA_nop`) is not a
    row, and an advance is its delta: the address it reaches (`to 0x84`) is where the code put it, and the code is the object's own."""
    out: list[list[str]] = []
    current: list[str] | None = None
    for line in text.splitlines():
        if re.match(r"^[0-9a-f]+ [0-9a-f]+ [0-9a-f]+ FDE ", line):
            current = []
            out.append(current)
        elif re.match(r"^[0-9a-f]+ [0-9a-f]+ [0-9a-f]+ CIE", line):
            current = None
        elif current is not None and (op := re.match(r"^\s+(DW_CFA_\w+:.*)$", line)):
            if not op.group(1).startswith("DW_CFA_nop"):
                current.append(re.sub(r" to 0x[0-9a-f]+$", "", op.group(1).strip()))
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="+")
    parser.add_argument("--pin", default=os.environ.get("LLRM_PIN_BIN"), help="the pin LLVM bin directory (clang, opt, llc)")
    parser.add_argument("--objdump", required=True, help="an llvm-objdump that reads x86-64 Mach-O")
    parser.add_argument("--llc", required=True, help="our llc")
    parser.add_argument("--strip", action="store_true", help="control: ours has no FDEs")
    args = parser.parse_args()
    if not args.pin:
        parser.error("--pin or LLRM_PIN_BIN names the pin LLVM bin directory")
    work = Path(tempfile.mkdtemp(prefix="cfimacho-"))
    same = different = refused = 0
    for source in map(Path, args.sources):
        try:
            ir, opt = work / f"{source.stem}.ll", work / f"{source.stem}.opt.ll"
            run([f"{args.pin}/clang", "--target=x86_64-apple-macosx", "-O2", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", str(source), "-o", str(ir)])
            run([f"{args.pin}/opt", "-O2", "-vectorize-loops=false", "-vectorize-slp=false", str(ir), "-S", "-o", str(opt)])
            pin_o, our_o = work / f"{source.stem}.pin.o", work / f"{source.stem}.our.o"
            run([f"{args.pin}/llc", "-O2", "-filetype=obj", str(opt), "-o", str(pin_o)])
            run([args.llc, "-O2", "--isel=dag", "-o", str(our_o), str(opt)])
            pin_frames, our_frames = (frames_of(run([args.objdump, "--dwarf=frames", str(o)])) for o in (pin_o, our_o))
        except (RuntimeError, KeyError) as why:
            print(f"  refused {source.name}: {why}")
            refused += 1
            continue
        if args.strip:
            our_frames = []
        if our_frames and len(our_frames) != len(pin_frames):
            print(f"  refused {source.name}: {len(pin_frames)} functions in the pin's, {len(our_frames)} in ours")
            refused += 1
            continue
        for index, rows in enumerate(pin_frames):
            ours = our_frames[index] if our_frames else None
            if ours == rows:
                same += 1
            else:
                different += 1
                print(f"  different {source.name} #{index}: pin {rows} ours {ours}")
    print(f"cfi_macho_diff: {same + different} functions compared, {refused} files refused: {same} same, {different} different")
    return 2 if same + different == 0 else (1 if different else 0)


if __name__ == "__main__":
    sys.exit(main())

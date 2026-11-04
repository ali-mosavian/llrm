#!/usr/bin/env python3
"""Mach-O compact unwind encodings, ours against the pin's, on one IR.

Each C file goes through the pin clang (`--target`'s Apple triple) and `opt -O2` once; the pin `llc` and `llrm-arm64-llc` then make a Mach-O object of that
`.ll`. For each function (by symbol) `llvm-objdump --macho --unwind-info` gives a compact encoding: the pin's and ours must match. A function the
pin's `llc` did not emit as ours is not compared. An object of ours with no `__compact_unwind` gives every function as different (`--strip`, the control).
"""
import argparse
import os
import re
import sys
import tempfile
from pathlib import Path

from dwarf_loc_diff import run


# macOS 11 or any 10.6 or later: the pin writes no `__compact_unwind` for the 10.4 that clang's bare `x86_64-apple-macosx` means.
TRIPLES = {"aarch64": "arm64-apple-macosx11.0.0", "x86_64": "x86_64-apple-macos11"}


def encodings_of(text: str) -> list[tuple[str, str]]:
    """(function start address, compact encoding word) in the order of the section, from `llvm-objdump --macho --unwind-info`. The functions of the
    two objects are the same in the same order but not at the same addresses (other code, other lengths), so they are paired by position."""
    out: list[tuple[str, str]] = []
    name = None
    for line in text.splitlines():
        if (start := re.match(r"\s*start:\s+(0x[0-9a-f]+)", line)) is not None:
            name = start.group(1)
        elif name is not None and (word := re.match(r"\s*compact encoding:\s+(0x[0-9a-f]+)", line)) is not None:
            out.append((name, word.group(1)))
            name = None
    return out


def same_word(ours: str, pin: str) -> bool:
    """The words are equal; in DWARF mode (0x03..) the low 24 bits are the offset of the function's FDE in the object, which is the object's own."""
    if pin.startswith("0x03") and ours.startswith("0x03"):
        return True
    return ours == pin


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="+")
    parser.add_argument("--pin", default=os.environ.get("LLRM_PIN_BIN"), help="the aarch64 pin LLVM bin directory (clang, opt, llc, llvm-objdump)")
    parser.add_argument("--target", choices=sorted(TRIPLES), default="aarch64")
    parser.add_argument("--objdump", help="the llvm-objdump to read both objects with (default: the pin's)")
    parser.add_argument("--llc", required=True, help="our llc for the target")
    parser.add_argument("--strip", action="store_true", help="control: ours has no encodings")
    args = parser.parse_args()
    if not args.pin:
        parser.error("--pin or LLRM_PIN_BIN names the pin LLVM bin directory")
    work = Path(tempfile.mkdtemp(prefix="machodiff-"))
    same = different = files = 0
    kinds: dict[tuple[str, str], int] = {}
    for source in map(Path, args.sources):
        try:
            ir, opt = work / f"{source.stem}.ll", work / f"{source.stem}.opt.ll"
            run([f"{args.pin}/clang", f"--target={TRIPLES[args.target]}", "-O2", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", str(source), "-o", str(ir)])
            run([f"{args.pin}/opt", "-O2", "-vectorize-loops=false", "-vectorize-slp=false", str(ir), "-S", "-o", str(opt)])
            pin_o, our_o = work / f"{source.stem}.pin.o", work / f"{source.stem}.our.o"
            run([f"{args.pin}/llc", "-O2", "-filetype=obj", str(opt), "-o", str(pin_o)])
            run([args.llc, "-O2", "--isel=dag", "-o", str(our_o), str(opt)])
            pin_words, our_words = (encodings_of(run([args.objdump or f"{args.pin}/llvm-objdump", "--macho", "--unwind-info", str(o)])) for o in (pin_o, our_o))
        except (RuntimeError, KeyError) as why:
            print(f"  refused {source.name}: {why}")
            files += 1
            continue
        if args.strip:
            our_words = []
        if our_words and len(our_words) != len(pin_words):
            print(f"  refused {source.name}: {len(pin_words)} functions in the pin's, {len(our_words)} in ours")
            files += 1
            continue
        for index, (name, word) in enumerate(pin_words):
            ours = our_words[index][1] if our_words else None
            if ours is not None and same_word(ours, word):
                same += 1
            else:
                different += 1
                key = (word[:4], (ours or "none")[:4])
                kinds[key] = kinds.get(key, 0) + 1
                print(f"  different {source.name} #{index} (pin {name}): pin {word} ours {ours}")
    by_kind = ", ".join(f"pin {a} ours {b}: {n}" for (a, b), n in sorted(kinds.items()))
    print(f"unwind_macho_diff: {same + different} functions compared, {files} files refused: {same} same, {different} different ({by_kind})")
    return 2 if same + different == 0 else (1 if different else 0)


if __name__ == "__main__":
    sys.exit(main())

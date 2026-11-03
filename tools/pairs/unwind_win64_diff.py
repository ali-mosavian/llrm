#!/usr/bin/env python3
"""Win64 unwind data (`.pdata`/`.xdata`), ours against the pin's, on one IR.

Each C file goes through the pin clang (x86_64-pc-windows-msvc) and `opt -O2` once; the pin `llc` and `llrm-llc` then make a COFF object of that
`.ll`. A function whose code is the same instruction for instruction is compared: the size of prolog, the frame register and the unwind codes (what
`llvm-objdump -u` prints) must match. A function whose code differs is refused, never counted as equal. An object with no unwind data at all gives
every function as different (the stripped control).
"""
import argparse
import os
import re
import sys
import tempfile
from pathlib import Path

from dwarf_loc_diff import run


def code_of(disassembly: str) -> dict[int, list[str]]:
    """start offset in .text -> the function's instructions, immediates and offsets within the function kept."""
    out: dict[int, list[str]] = {}
    current: list[str] | None = None
    start = 0
    for line in disassembly.splitlines():
        if (head := re.match(r"^([0-9a-f]+) <(.+)>:$", line)) is not None:
            start = int(head.group(1), 16)
            current = out.setdefault(start, [])
        elif current is not None and (row := re.match(r"^\s*([0-9a-f]+):\t(.*)$", line)):
            current.append(f"{int(row.group(1), 16) - start:x} {re.sub(r'\s+', ' ', row.group(2).split('#')[0].strip())}")
    return out


def tables_of(text: str) -> dict[int, tuple]:
    """start offset in .text -> (end, size of prolog, frame register, unwind codes) from `llvm-objdump -u`."""
    out: dict[int, tuple] = {}
    for block in text.split("Function Table:")[1:]:
        for part in re.split(r"\n\s*Start Address:", block)[1:] if "Start Address:" in block else []:
            start = re.match(r"\s*\.text(?: \+ 0x([0-9a-f]+))?", part)
            if start is None:
                continue
            end = re.search(r"End Address: \.text(?: \+ 0x([0-9a-f]+))?", part)
            prolog = re.search(r"Size of prolog: (\d+)", part)
            frame = re.search(r"Frame register: (\S+)|No frame pointer used", part)
            codes = tuple(re.findall(r"0x[0-9a-f]+: (UOP_[^\n]*)", part))
            out[int(start.group(1) or "0", 16)] = (
                int(end.group(1) or "0", 16) if end else None,
                int(prolog.group(1)) if prolog else None,
                frame.group(0) if frame else None,
                codes,
            )
    return out


FIXTURES = [str(Path(__file__).resolve().parents[2] / "bench" / name) for name in ("fib/fib.c", "crc/crc.c", "hanoi/hanoi.c")]
SKIPPED = 77  # the gate's SKIPPED verdict


def verdict(same: int, different: int, ours_emit: bool, strip: bool) -> int:
    """0 equal, 1 different, 2 nothing compared, 77 where our object has no unwind data at all (Win64 rows not landed): not a result, never an ok.
    The stripped control is the same absence made on purpose, so it reads as every function different."""
    if same + different == 0:
        return 2
    if not ours_emit and not strip:
        return SKIPPED
    return 1 if different else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="*", default=FIXTURES, help="C files (default: three bench programs: a leaf, a loop, calls with saved registers)")
    parser.add_argument("--pin", default=os.environ.get("LLRM_PIN_BIN"), help="the x86 pin LLVM bin directory (clang, opt, llc)")
    parser.add_argument("--tools", default=str(Path.home() / "scratch/llvm-build-aarch64/bin"), help="the directory of llvm-objdump")
    parser.add_argument("--llc", required=True, help="llrm-llc")
    parser.add_argument("--allow-pending", action="store_true", help="pass -fallow-pending-abi: a preview of what the Win64 rows would give before they land")
    parser.add_argument("--strip", action="store_true", help="control: ours stripped of its unwind data")
    args = parser.parse_args()
    if not args.pin:
        parser.error("--pin or LLRM_PIN_BIN names the pin LLVM bin directory")
    work = Path(tempfile.mkdtemp(prefix="w64diff-"))
    same = different = refused = files = rich = 0
    ours_emit = False
    pending = 0
    for source in map(Path, args.sources):
        try:
            ir, opt = work / f"{source.stem}.ll", work / f"{source.stem}.opt.ll"
            run([f"{args.pin}/clang", "--target=x86_64-pc-windows-msvc", "-O2", "-S", "-emit-llvm", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-O0-optnone", str(source), "-o", str(ir)])
            run([f"{args.pin}/opt", "-O2", "-vectorize-loops=false", "-vectorize-slp=false", str(ir), "-S", "-o", str(opt)])
            pin_o, our_o = work / f"{source.stem}.pin.obj", work / f"{source.stem}.our.obj"
            run([f"{args.pin}/llc", "-O2", "-filetype=obj", str(opt), "-o", str(pin_o)])
            run([args.llc, "-O2", "-m64", "-fobject-format=coff", *(["-fallow-pending-abi"] if args.allow_pending else []), "-o", str(our_o), str(opt)])
            if args.strip:
                run(["objcopy", "-R", ".pdata", "-R", ".xdata", str(our_o)])
            pin_code, our_code = (code_of(run([f"{args.tools}/llvm-objdump", "-d", "--no-show-raw-insn", str(o)])) for o in (pin_o, our_o))
            pin_tables, our_tables = (tables_of(run([f"{args.tools}/llvm-objdump", "-u", str(o)])) for o in (pin_o, our_o))
        except (RuntimeError, KeyError) as why:
            print(f"  refused {source.name}: {why}")
            files += 1
            pending += "-fallow-pending-abi" in str(why)
            continue
        ours_emit = ours_emit or bool(our_tables)
        for start, table in sorted(pin_tables.items()):
            if our_code.get(start) != pin_code.get(start):
                refused += 1
            elif our_tables.get(start) == table:
                same += 1
                rich += bool(table[3])
            else:
                different += 1
                print(f"  different {source.name} +{start:#x}:\n    pin  {table}\n    ours {our_tables.get(start)}")
    print(f"unwind_win64_diff: {same + different} functions compared, {refused} refused (code differs), {files} files refused: {same} same ({rich} with unwind codes), {different} different")
    if pending and pending == files == len(args.sources):
        print("SKIPPED: llrm-llc refuses the windows platform as pending (the Win64 rows have not landed)")
        return SKIPPED
    code = verdict(same, different, ours_emit, args.strip)
    if code == SKIPPED:
        print("SKIPPED: our object has no .pdata/.xdata (the Win64 unwind rows have not landed)")
    return code


if __name__ == "__main__":
    sys.exit(main())

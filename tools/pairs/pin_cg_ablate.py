#!/usr/bin/env python3
"""pin_cg_ablate.py: what each pass of the pin's llc pipeline does on real code, and what the pin's own switches cost.

    LLRM_CG_SETS="m32:DIR m16:DIR" tools/pairs/pin_cg_ablate.py [changed|switches]

Each IR (`DIR/*/01-globalopt.ll`: the bench C programs and the QCport modules, unoptimized, from the pin's clang) goes through the pin's
`default<O2>` (`default<Os>` is the same list on optsize IR), then `llc -O2 -print-after-all`.

`changed`: (a pass that prints nothing under -print-after-all, such as a loop pass, has its change counted on the next pass that does: LSR's on `Lower Garbage Collection Instructions`.) A pass changed an IR when its dump of a function differs from the last pass's dump of the same function. Per pass: the IRs
(and functions) it changed. Zero for every IR is the evidence for an N/A row; the controls (isel, the register allocator) must not be zero.

`switches`: for each pin switch of the slot table's rows, llc with and without it: instructions (asm lines) and object bytes (.text of
the ELF object), summed over the IRs and the IRs that differ. Targets are the explicit ones of pin.py. Bytes are the sum of the function symbols' sizes (no padding between functions) with `-align-loops=1`.
"""
from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pin  # noqa: E402
import pin_ablate  # noqa: E402

HEAD = re.compile(r"^(?:# )?\*\*\* IR Dump After (.*?) \*\*\*:?$", re.M)
CHUNK = re.compile(r'^(?:define [^\n]*@(?:"([^"]+)"|([\w.$]+))\(|# Machine code for function ([^\n]+?):)', re.M)
# no loop padding: llrm aligns nothing at m16 and the bytes are the code's own
ALIGN = ["-align-loops=1"]

# the slot table's switches that are ablations (a pass off), by row
SWITCHES = {
    "CodeGen Prepare": ["-disable-cgp"],
    "CodeGen Prepare: branch opts": ["-disable-cgp-branch-opts"],
    "CodeGen Prepare: select to branch": ["-disable-cgp-select2branch"],
    "CodeGen Prepare: extending-load promotion": ["-disable-cgp-ext-ld-promotion"],
    "CodeGen Prepare: store-extract": ["-disable-cgp-store-extract"],
    "CodeGen Prepare: delete phis": ["-disable-cgp-delete-phis"],
    "CodeGen Prepare: complex address modes": ["-disable-complex-addr-modes"],
    "Loop Strength Reduction": ["-disable-lsr"],
    "Constant Hoisting": ["-disable-constant-hoisting"],
    "Partially inline calls to library functions": ["-disable-partial-libcall-inlining"],
    "Early Tail Duplication": ["-disable-early-taildup"],
    "Remove dead machine instructions": ["-disable-machine-dce"],
    "Early If-Conversion": ["-disable-early-ifcvt"],
    "Early Machine Loop Invariant Code Motion": ["-disable-machine-licm"],
    "Machine Common Subexpression Elimination": ["-disable-machine-cse"],
    "Machine code sinking": ["-disable-machine-sink"],
    "Peephole Optimizations": ["-disable-peephole"],
    "X86 LEA Optimize": ["-disable-x86-lea-opt"],
    "Machine Instruction Scheduler": ["-enable-misched=0"],
    "Stack Slot Coloring": ["-disable-ssc"],
    "Machine Copy Propagation Pass": ["-disable-copyprop"],
    "Machine Loop Invariant Code Motion": ["-disable-postra-machine-licm"],
    "PostRA Machine Sink": ["-disable-postra-machine-sink"],
    "Shrink Wrapping analysis": ["-enable-shrink-wrap=0"],
    "Control Flow Optimizer": ["-disable-branch-fold"],
    "Tail Duplication": ["-disable-tail-duplicate"],
    "Post RA top-down list latency scheduler": ["-disable-post-ra"],
    "Branch Probability Basic Block Placement": ["-disable-block-placement"],
    "X86 Domain Reassignment Pass": ["-disable-x86-domain-reassignment"],
    "X86 cmov Conversion": ["-x86-cmov-converter=0"],
}


def sets():
    out = []
    for spec in os.environ.get("LLRM_CG_SETS", f"m32:{Path.home()}/scratch/fa/pinir").split():
        mode, directory = spec.split(":", 1)
        out.append((mode, sorted(Path(directory).expanduser().glob("*/01-globalopt.ll"))))
    return out


def optimized(text: str, of: pin.Target) -> str | None:
    fixture = pin_ablate.FIXTURE.with_name("O2.txt")
    line = [l for l in fixture.read_text().splitlines() if not l.startswith(";")][-1]
    done = pin.opt(line, pin_ablate.pin_text_of(text), of)
    return done.stdout.split("\n", 1)[1] if done.returncode == 0 else None


def changed_passes(ir: str, of: pin.Target) -> dict[str, int] | None:
    """The passes that changed a function of `ir`, with how many functions."""
    done = pin.llc(["-O2", "-print-after-all", "-filetype=asm", "-o", "/dev/null"], ir, of)
    if done.returncode:
        return None
    parts = HEAD.split(done.stderr)
    last: dict[str, str] = {}
    out: dict[str, int] = {}
    for name, body in zip(parts[1::2], parts[2::2]):
        pass_name = re.sub(r"\s*\([^()]*\)$", "", name)
        marks = list(CHUNK.finditer(body))
        changed = False
        for at, mark in enumerate(marks):
            key = next(g for g in mark.groups() if g)
            text = body[mark.start() : marks[at + 1].start() if at + 1 < len(marks) else len(body)]
            text = text.split("\n}\n")[0] if text.startswith("define") else text.split("# End machine code")[0]
            # attribute groups are numbered anew in each dump: `#3` here is `#4` there
            norm = re.sub(r" #\d+", " #N", "\n".join(l for l in text.splitlines() if not l.startswith("#") or l.startswith("# Machine")))
            if key in last and last[key] != norm:
                changed = True
                out[pass_name] = out.get(pass_name, 0) + 1
            last[key] = norm
    return out


def measure(ir: str, of: pin.Target, extra: list[str]) -> tuple[int, int] | None:
    """(instructions, .text bytes) of `llc -O2 extra` over `ir`."""
    asm = pin.llc(["-O2", *ALIGN, *extra, "-filetype=asm", "-o", "-"], ir, of)
    if asm.returncode:
        return None
    instructions = sum(1 for l in asm.stdout.splitlines() if l.startswith("\t") and not l.startswith("\t.") and l.strip())
    with tempfile.NamedTemporaryFile(suffix=".o") as obj:
        done = pin.llc(["-O2", *ALIGN, *extra, "-filetype=obj", "-o", obj.name], ir, of)
        if done.returncode:
            return None
        symbols = subprocess.run(["nm", "-S", obj.name], capture_output=True, text=True).stdout
    # the functions' own sizes: the padding between them is the linker's
    sizes = [int(l.split()[1], 16) for l in symbols.splitlines() if len(l.split()) == 4 and l.split()[2] in "Tt"]
    return instructions, sum(sizes)


OURS = os.environ.get("LLRM_C", str(Path.home() / "scratch/fa/tgtT/release/llrm-c"))
SOURCES = Path(os.environ.get("LLRM_CG_SOURCES", Path.home() / "scratch/rows/cc1d-new/b"))


def text_bytes(path: str) -> int:
    sizes = subprocess.run(["objdump", "-h", path], capture_output=True, text=True).stdout
    found = re.search(r"\.text\s+([0-9a-f]+)", sizes)
    return int(found.group(1), 16) if found else 0


def instructions_of(asm: str) -> int:
    """Instructions in llrm-c's masm output (indented lines that are neither labels nor data)."""
    return sum(
        1
        for l in asm.splitlines()
        if re.match(r"^\s{4}[a-z]", l) and not l.rstrip().endswith(":") and not re.match(r"^\s{4}(db|dw|dd|dq|align|public|extern)\b", l)
    )


def ours_stage(source: Path, mode: str, level: str, includes: list[str]) -> tuple[str, int, int] | None:
    """(the MIR llrm-c hands its backend, its instructions, its .text bytes): the same IR goes to the pin's llc, so the two backends are compared alone
    (`switchconv`, the last stage the pin reads: the one tools of w-4's leacmp.py take). The dump goes to disk, not tmpfs."""
    flags = {"m32": ["-m32", "-mabi=sysv", "-march=i486"], "m16": ["-m16", "-march=i386"]}[mode]
    work = tempfile.mkdtemp(dir=str(Path.home() / "scratch/fa"))
    try:
        asm = subprocess.run([OURS, *flags, f"-{level}", *includes, "--dump", work, "-S", str(source), "-o", f"{work}/o.s"], capture_output=True, text=True)
        obj = subprocess.run([OURS, *flags, f"-{level}", *includes, "-fobject-format=elf", str(source), "-o", f"{work}/o.o"], capture_output=True, text=True)
        stage = sorted(Path(work).glob("*-switchconv.ll"))
        if asm.returncode or obj.returncode or not stage:
            return None
        ir = re.sub(r" tre-deferred| threeway", "", stage[0].read_text())
        return ir, instructions_of(Path(f"{work}/o.s").read_text()), text_bytes(f"{work}/o.o")
    finally:
        import shutil

        shutil.rmtree(work, ignore_errors=True)


def pin_text(ir: str, of: pin.Target, extra: list[str]) -> tuple[int, int] | None:
    """(instructions, .text bytes incl. padding) of `llc -O2 extra`, the pin's own alignment kept (ours aligns as the pin does at m32)."""
    asm = pin.llc(["-O2", *extra, "-filetype=asm", "-o", "-"], ir, of)
    if asm.returncode:
        return None
    count = sum(1 for l in asm.stdout.splitlines() if l.startswith("\t") and not l.startswith("\t.") and l.strip())
    with tempfile.NamedTemporaryFile(suffix=".o") as obj:
        if pin.llc(["-O2", *extra, "-filetype=obj", "-o", obj.name], ir, of).returncode:
            return None
        return count, text_bytes(obj.name)


def premise() -> None:
    """Per switch of SWITCHES: over the programs it changes, ours against the pin with and without the pass (instructions, .text bytes), on the one IR llrm-c hands its backend."""
    for level, optsize in (("O2", False), ("Os", True)):
        rows: dict[str, list[int]] = {}
        n = 0
        for mode, irs in sets():
            of = pin.target(mode, optsize=optsize)
            for path in irs:
                source = SOURCES / f"{path.parent.name}.c"
                if not source.exists():
                    continue
                made = ours_stage(source, mode, level, [])
                if made is None:
                    continue
                ir, *ours = made
                with_pass = pin_text(ir, of, [])
                if with_pass is None:
                    continue
                n += 1
                for name, switches in SWITCHES.items():
                    for switch in switches:
                        without = pin_text(ir, of, [switch])
                        if without is None or without == with_pass:
                            continue
                        row = rows.setdefault(f"{name} [{switch}]", [0] * 8)
                        for at, value in enumerate((*ours, *with_pass, *without, 1, 0)):
                            row[at] += value
        print(f"# {level}: {n} programs, ours {OURS}")
        print("pass [switch] | ours instr | ours B | pin-with instr | pin-with B | pin-without instr | pin-without B | IRs it changes")
        for name, row in rows.items():
            print(f"{name} | {row[0]} | {row[1]} | {row[2]} | {row[3]} | {row[4]} | {row[5]} | {row[6]}")
        sys.stdout.flush()


def main(argv: list[str]) -> int:
    if argv and argv[0] == "premise":
        premise()
        return 0
    what = argv[0] if argv else "changed"
    only = argv[1] if len(argv) > 1 else ""
    for level, optsize in (("O2", False), ("Os", True)):
        totals: dict[str, list[int]] = {}
        n = 0
        for mode, irs in sets():
            of = pin.target(mode, optsize=optsize)
            for path in irs:
                ir = optimized(path.read_text(), of)
                if ir is None:
                    continue
                n += 1
                if what == "changed":
                    got = changed_passes(ir, of)
                    if got is None:
                        n -= 1
                        continue
                    for name, count in got.items():
                        row = totals.setdefault(name, [0, 0])
                        row[0] += 1
                        row[1] += count
                else:
                    base = measure(ir, of, [])
                    if base is None:
                        n -= 1
                        continue
                    for row_name, switches in SWITCHES.items():
                        if only and only not in row_name:
                            continue
                        for switch in switches:
                            off = measure(ir, of, [switch])
                            row = totals.setdefault(f"{row_name} [{switch}]", [0, 0, 0, 0, 0, 0])
                            if off is None:
                                row[5] += 1
                                continue
                            row[0] += off[0] - base[0]
                            row[1] += off[1] - base[1]
                            row[2] += (off != base)
                            row[3] += base[0]
                            row[4] += base[1]
        print(f"# {level}{' (optsize IR)' if optsize else ''}: {n} IRs; pin d1106deb71cc")
        if what == "changed":
            print("pass | IRs it changed | functions")
            for name, (irs_changed, functions) in sorted(totals.items(), key=lambda kv: -kv[1][0]):
                print(f"{name} | {irs_changed} | {functions}")
        else:
            print("pass [switch] | d instructions | d bytes | IRs differing | base instructions | base bytes | llc failed")
            for name, row in totals.items():
                print(f"{name} | {row[0]:+d} | {row[1]:+d} | {row[2]} | {row[3]} | {row[4]} | {row[5]}")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

#!/usr/bin/env python3
"""Which targets a tests/run/c program is for: its `// targets:` line (in the first lines, after `// flags:` and `// link:`).

A program with no line is for every target.

Flags for one target family only go on `// flags-FAMILY: ...` (`flags_for`; m68k first), never on the shared `// flags:` line, which keeps
its meaning for every other target (the x86 rows read `-mcmodel` there; clang rejects `-mcmodel=medium` on i386 and on m68k). A flag
only a backend takes, not clang, is listed on the target's row in `crates/driver/llrm-cc/targets.toml` (`llc_mcmodel` for the code
model): llrm-cc then gives it to the backend alone, and a backend that does not know it fails the build (never a silent drop). Tokens: `dos` (x86_16 and x86_32 DOS/Watcom targets), `linux` (the sysv `-linux` targets), `!TOKEN` (every target but those; alone it means all others), `m68k` (every m68k target), `lp64` (targets whose `long` is 64 bits: x86_64, arm64; for an `.out` written with a 64-bit `long`), `ptr16` (a 16-bit address space: data over 64 KiB does not fit), `int16` (a 16-bit `int`: a bit-field wider than 16 bits is an error), `freestanding` (no libc headers: `#include <stdio.h>` fails), `byte-stack` (frames keep one byte and nothing realigns SP: a local aligned past 8 is refused), a family (`x86-m16`,
`x86-m32`, `x86-m64`) or a full target name. A harness asks `applies`, and prints `NA` for a program that does not, never
FAIL: `tools/corpus_targets.py check TARGET FILE.c` exits 3 and prints "n/a, DOS-only" (or "n/a, targets: ...").
"""
import re
import sys
from pathlib import Path

HEADER_LINES = 6
DOS = re.compile(r"^x86_(16|32)-(dos|wat)")
FAMILY = re.compile(r"^x86_(16|32|64)-")
M68K = re.compile(r"^m68k-")
LP64 = re.compile(r"^(x86_64|arm64)-")
SMALL = re.compile(r"^(x86_16|z80|6502)-")  # 16-bit pointers and int
EIGHT_BIT = re.compile(r"^(z80|6502)(-|$)")  # a 16-bit int and no x86 mode to give `-m32` a meaning
FREESTANDING = re.compile(r"^(wasm32|z80|6502)(-|$)")
BYTE_STACK = re.compile(r"^z80(-|$)")  # `Machine::kept_stack_align` 1: llrm-z80


def tags(source: str) -> list[str] | None:
    """The tokens of the program's `// targets:` line, or None where it has none."""
    for line in source.splitlines()[:HEADER_LINES]:
        if line.startswith("// targets:"):
            return line.removeprefix("// targets:").split()
    return None


def flags_for(family: str, source: str) -> list[str]:
    """The extra flags of a `// flags-FAMILY:` line (first lines): flags only one target family takes. The shared `// flags:`
    line is every harness's; `-mcmodel=medium` there fails the x86 -m32 rows (clang: unsupported argument)."""
    for line in source.splitlines()[:HEADER_LINES]:
        if line.startswith(f"// flags-{family}:"):
            return line.removeprefix(f"// flags-{family}:").split()
    return []


def flag_configurations(source: str) -> list[list[str]]:
    """The `|`-separated configurations of the program's `// flags:` line (first lines), each a token list."""
    for line in source.splitlines()[:HEADER_LINES]:
        if line.startswith("// flags:"):
            return [one.split() for one in line.removeprefix("// flags:").split("|")]
    return []


def x86_32_only(source: str) -> bool:
    """Every configuration of the program's flags forces a 32-bit x86 mode (`-m32`, `-mabi=sysv`): its .out was written for 32-bit int and pointers."""
    configurations = flag_configurations(source)
    return bool(configurations) and all(any(t in ("-m32", "-mabi=sysv") for t in one) for one in configurations)


def applies(target: str, source: str) -> bool:
    if EIGHT_BIT.match(target) and x86_32_only(source):
        return False
    listed = tags(source)
    if listed is None:
        return True
    family = FAMILY.match(target)
    names = {target, f"x86-m{family.group(1)}" if family else target}

    def matches(token: str) -> bool:
        return token in names or (token == "dos" and bool(DOS.match(target))) or (token == "m68k" and bool(M68K.match(target))) or (token == "lp64" and bool(LP64.match(target))) or (token in ("ptr16", "int16") and bool(SMALL.match(target))) or (token == "freestanding" and bool(FREESTANDING.match(target))) or (token == "byte-stack" and bool(BYTE_STACK.match(target))) or (token == "linux" and target.endswith("-linux"))

    positive = [t for t in listed if not t.startswith("!")]
    return not any(matches(t[1:]) for t in listed if t.startswith("!")) and (not positive or any(matches(t) for t in positive))


def reason(source: str) -> str:
    listed = tags(source) or []
    return "n/a, DOS-only" if listed == ["dos"] else "n/a, targets: " + " ".join(listed)


def expected_output(source: Path, target: str) -> Path:
    """The file a program's output is compared with on TARGET: `NAME.FAMILY.out` (FAMILY: the target's first dash-field, `m68k`
    for `m68k-68020`) where the target's ABI differs from the one `NAME.out` was written for, else `NAME.out`. Each override has a
    one-line `NAME.FAMILY.why` saying which ABI fact differs; a byte-order difference is fixed in the source, not here."""
    own = source.with_suffix(f".{target.split('-')[0]}.out")
    return own if own.exists() else source.with_suffix(".out")


def main(argv: list[str]) -> int:
    if len(argv) != 4 or argv[1] != "check":
        print("usage: corpus_targets.py check TARGET FILE.c", file=sys.stderr)
        return 2
    source = Path(argv[3]).read_text()
    if applies(argv[2], source):
        return 0
    print(reason(source))
    return 3


if __name__ == "__main__":
    sys.exit(main(sys.argv))

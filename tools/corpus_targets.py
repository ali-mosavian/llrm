#!/usr/bin/env python3
"""Which targets a tests/run/c program is for: its `// targets:` line (in the first lines, after `// flags:` and `// link:`).

A program with no line is for every target.

Flags for one target family only go on `// flags-FAMILY: ...` (`flags_for`; m68k first), never on the shared `// flags:` line, which keeps
its meaning for every other target (the x86 rows read `-mcmodel` there; clang rejects `-mcmodel=medium` on i386 and on m68k). A flag
only a backend takes, not clang, is listed on the target's row in `crates/driver/llrm-cc/targets.toml` (`llc_mcmodel` for the code
model): llrm-cc then gives it to the backend alone, and a backend that does not know it fails the build (never a silent drop). Tokens: `dos` (x86_16 and x86_32 DOS/Watcom targets), a family (`x86-m16`,
`x86-m32`, `x86-m64`) or a full target name. A harness asks `applies`, and prints `NA` for a program that does not, never
FAIL: `tools/corpus_targets.py check TARGET FILE.c` exits 3 and prints "n/a, DOS-only" (or "n/a, targets: ...").
"""
import re
import sys
from pathlib import Path

HEADER_LINES = 6
DOS = re.compile(r"^x86_(16|32)-(dos|wat)")
FAMILY = re.compile(r"^x86_(16|32|64)-")


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


def applies(target: str, source: str) -> bool:
    listed = tags(source)
    if listed is None:
        return True
    family = FAMILY.match(target)
    names = {target, f"x86-m{family.group(1)}" if family else target}
    return any(token in names or (token == "dos" and DOS.match(target)) for token in listed)


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

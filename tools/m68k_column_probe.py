#!/usr/bin/env python3
"""The m68k matrix's external columns carry no distro hardening and one relocation model.

Static: the flags `examples/matrix.rs` passes to gcc and to the pin's clang/llc (distro defaults reach a column only
through flags it does not name: x87 measured -fno-omit-frame-pointer, PIE and stack protector on x86). Dynamic: the
relocation kinds of one object per column, from `objdump -r`; the static model has no GOT or PLT kind.
Usage: m68k_column_probe.py [matrix.rs] [obj ...]
"""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import external_flags  # noqa: E402

PIC = re.compile(r"-f(pic|PIC|pie|PIE)\b")
HARDENING = re.compile(r"-f(stack-protector|stack-clash-protection|cf-protection(?!=none)|hardened)\b|-D_FORTIFY_SOURCE")
FORBIDDEN_KINDS = re.compile(r"GOT|PLT|TLS|COPY|JMP_SLOT|RELATIVE")


def column_flags(source: str) -> dict[str, list[str]]:
    """The string literals of every `.args([..])` and `.arg("..")` inside gcc_object and clang_object."""
    flags: dict[str, list[str]] = {}
    for name in ("gcc_object", "clang_object"):
        start = source.index(f"fn {name}(")
        end = source.find("\nfn ", start + 1)
        body = source[start : end if end != -1 else len(source)]
        flags[name] = [lit for lit in re.findall(r'"([^"{}]*-[^"{}]*)"', body) if lit.startswith("-")]
    return flags


def shared_flags(source: str) -> list[str]:
    """The literals of `const EXTERNAL_FLAGS` plus `GCC_ZERO_INIT_PADDING`, and whether each column's code uses its constant."""
    block = re.search(r"const EXTERNAL_FLAGS[^=]*=\s*\[(.*?)\];", source, re.S)
    pad = re.search(r'const GCC_ZERO_INIT_PADDING[^=]*=\s*"([^"]*)"', source)
    return (re.findall(r'"([^"]*)"', block.group(1)) if block else []) + ([pad.group(1)] if pad else [])


def shared_violations(source: str) -> list[str]:
    """The matrix's list must be the shared config's for m68k (target `none`, gcc 15, no relocation flag), used by both columns."""
    want = sorted(external_flags.flags("none", (), 15))
    have = sorted(shared_flags(source))
    found = [] if have == want else [f"matrix.rs shared flags {have} differ from external_flags.py {want}"]
    for name, constants in (("gcc_object", ("EXTERNAL_FLAGS", "GCC_ZERO_INIT_PADDING")), ("clang_object", ("EXTERNAL_FLAGS",))):
        start = source.index(f"fn {name}(")
        end = source.find("\nfn ", start + 1)
        body = source[start : end if end != -1 else len(source)]
        found += [f"{name}: does not pass {c}" for c in constants if c not in body]
    return found


def violations(flags: dict[str, list[str]]) -> list[str]:
    found = []
    gcc, clang = flags["gcc_object"], flags["clang_object"]
    for name, args in flags.items():
        found += [f"{name}: {arg} (a distro default is named only by being absent)" for arg in args if HARDENING.search(arg)]
    found += [f"gcc_object: {arg}" for arg in gcc if PIC.search(arg)]
    if "-fno-pic" not in clang:
        found.append("clang_object: no -fno-pic")
    if "-relocation-model=static" not in clang:
        found.append("clang_object: no -relocation-model=static")
    if "-ffreestanding" not in gcc:
        found.append("gcc_object: no -ffreestanding")
    return found


def reloc_kinds(listing: str) -> set[str]:
    """The relocation kinds of `objdump -r` output."""
    return set(re.findall(r"\bR_68K_\w+", listing))


def object_violations(kinds: set[str]) -> list[str]:
    return sorted(kind for kind in kinds if FORBIDDEN_KINDS.search(kind))


# A union initialised through its smaller member: -fzero-init-padding-bits=all zeroes the rest, standard does not.
PADDING_SOURCE = "union u { char c; long l[4]; };\nextern void use(void *);\nvoid f(void) { union u a = { 1 }; use(&a); }\n"


def padding_flag_reaches_gcc(gcc: str) -> bool:
    """Whether `-fzero-init-padding-bits=all` and `=standard` give different assembly: the flag is live in this gcc."""
    with tempfile.TemporaryDirectory() as tmp:
        source = os.path.join(tmp, "p.c")
        open(source, "w").write(PADDING_SOURCE)
        asm = {}
        for mode in ("all", "standard"):
            out = os.path.join(tmp, mode + ".s")
            subprocess.run([gcc, "-m68000", "-O2", "-ffreestanding", f"-fzero-init-padding-bits={mode}", "-S", "-o", out, source], check=True)
            asm[mode] = open(out).read()
    return asm["all"] != asm["standard"]


if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else "crates/target/llrm-m68k/examples/matrix.rs"
    text = open(path).read()
    problems = violations(column_flags(text)) + shared_violations(text)
    for obj in sys.argv[2:]:
        listing = subprocess.run(["objdump", "-r", obj], capture_output=True, text=True).stdout
        problems += [f"{obj}: {kind}" for kind in object_violations(reloc_kinds(listing))]
    print("\n".join(problems) or "columns: static flags and relocation kinds are neutral")
    sys.exit(1 if problems else 0)

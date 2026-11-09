#!/usr/bin/env python3
"""Whole-program bytes of a tests/run/c program on m68k: the program object, and the library members it pulls in.

m68k_program_size.py [--cpu 68020] [--level Os] [--compiler ours|gcc|both] NAME...

The program is compiled by llrm-cc (`ours`) or by m68k-linux-gnu-gcc (`gcc`, the call-the-library column), assembled, and linked with the
crt stub and the compiler-rt archive llrm-cc builds at the same level (both columns use it: libgcc's m68k routines are not in the gcc
root). Only the members the program references link, as in a real link. Every row prints both numbers, `program` and `library`, and their sum
`whole`; a column that shows one of them hides the other (a libcall column linked with every routine, or none, was off by hundreds of bytes).
Environment: LLRM_BIN (llrm-cc), GCC_M68K, LLRM_PIN_CLANG, M68K_EXEC_CACHE.
"""
import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402
import m68k_exec_sweep as sweep  # noqa: E402
import o0_sweep  # noqa: E402

ENTRY = re.compile(r"^ (\.text\S*)\s*(?:\n\s+)?0x[0-9a-f]+\s+0x([0-9a-f]+)\s+(\S+)$", re.M)


def text_bytes(map_text: str) -> dict[str, int]:
    """The .text bytes each input file of a link contributes, by file (`lib.a(member.o)` for an archive member), from an ld map."""
    sizes: dict[str, int] = {}
    for _, size, path in ENTRY.findall(map_text):
        sizes[path] = sizes.get(path, 0) + int(size, 16)
    return sizes


def members_of(sizes: dict[str, int]) -> dict[str, int]:
    """The archive members of a link, by member name."""
    return {re.search(r"\(([^)]+)\)", path).group(1): size for path, size in sizes.items() if "(" in path}


def short(member: str) -> str:
    """`rt_68020_Os_int_ctzsi2.o` (build_support's object name) as `ctzsi2`."""
    return re.sub(r"^rt_[^_]+_[^_]+_[^_]+_", "", member).removesuffix(".o")


def globalize(text: str) -> str:
    """Every label global, so the linker can name the functions (the compilers' output makes none)."""
    out = [".text"]
    for line in text.splitlines():
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*:", line):
            out.append(f".globl {line[:-1]}")
        out.append(line)
    return "\n".join(out) + "\n"


def measure(source: Path, cpu: str, level: str, compiler: str, cc: str, root: str, cache: Path, work: Path) -> dict[str, object]:
    env = sweep.tool_env(root)
    tools = f"{root}/usr/bin/m68k-linux-gnu-"
    asm = work / f"{source.stem}.{compiler}.s"
    flags = o0_sweep.flags_of(source.read_text(), f"m68k-{cpu}")
    if compiler == "gcc":
        cmd = [f"{tools}gcc", f"-m{cpu}", f"-{level}", "-msoft-float", "-ffreestanding", "-S", "-o", str(asm), str(source)]
        run_env = env
    else:
        cmd = [cc, f"-{level}", "-w", "-S", *flags, "-I", str(ROOT / "runtime/shared/posix"), "-o", str(asm), str(source)]
        run_env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": f"m68k-{cpu}"}
    built = subprocess.run(cmd, capture_output=True, text=True, env=run_env, timeout=sweep.TIMEOUT)
    if built.returncode != 0:
        raise SystemExit(f"m68k_program_size: {compiler} does not compile {source.name}: {sweep.first_error(built.stderr)}")
    glob = work / f"{source.stem}.{compiler}.g.s"
    glob.write_text(globalize(asm.read_text()))
    obj = work / f"{source.stem}.{compiler}.o"
    subprocess.run([f"{tools}as", f"-m{cpu}", "-o", str(obj), str(glob)], check=True, capture_output=True, env=env)
    crt, archive = support(cc, root, cpu, level, cache)
    exe, mapfile = work / f"{source.stem}.{compiler}.exe", work / f"{source.stem}.{compiler}.map"
    link = subprocess.run(
        [f"{tools}gcc", f"-m{cpu}", "-msoft-float", "-nostdlib", "-static", "-ffreestanding", f"-Wl,-Map={mapfile}", "-o", str(exe), str(obj), crt, archive],
        capture_output=True, text=True, env=env,
    )
    if link.returncode != 0:
        raise SystemExit(f"m68k_program_size: {compiler} {source.name} does not link: {sweep.first_error(link.stderr)}")
    sizes = text_bytes(mapfile.read_text())
    library = members_of(sizes)
    program = sizes.get(str(obj), 0)
    return {"program": program, "library": sum(library.values()), "members": library, "whole": program + sum(library.values())}


def support(cc: str, root: str, cpu: str, level: str, cache: Path) -> tuple[str, str]:
    """The crt object and the compiler-rt archive at LEVEL: the only objects a program is linked with here (no memcpy or OS stubs)."""
    args = sweep.build_support(cc, root, cpu, cache, int(os.environ.get("JOBS", "4")), level)
    return args[0], args[2]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpu", default="68020")
    parser.add_argument("--level", default="Os")
    parser.add_argument("--compiler", default="both", choices=["ours", "gcc", "both"])
    parser.add_argument("names", nargs="+")
    args = parser.parse_args()
    root = os.environ.get("GCC_M68K")
    if not root:
        print("m68k_program_size: GCC_M68K is missing", file=sys.stderr)
        return 2
    cc = o0_sweep.compiler()
    cache = sweep.cache_dir(cc)
    columns = ["ours", "gcc"] if args.compiler == "both" else [args.compiler]
    print("| program | cpu | level | compiler | program B | library B | whole B | members |")
    print("|---|---|---|---|---|---|---|---|")
    with tempfile.TemporaryDirectory() as tmp:
        for name in args.names:
            for compiler in columns:
                got = measure(ROOT / "tests/run/c" / f"{name}.c", args.cpu, args.level, compiler, cc, root, cache, Path(tmp))
                members = " ".join(f"{short(m)}:{b}" for m, b in sorted(got["members"].items()))
                print(f"| {name} | {args.cpu} | {args.level} | {compiler} | {got['program']} | {got['library']} | {got['whole']} | {members} |")
    return 0


if __name__ == "__main__":
    sys.exit(main())

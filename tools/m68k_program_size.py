#!/usr/bin/env python3
"""Whole-program bytes of a tests/run/c program on m68k: the program object, and the library members it pulls in.

m68k_program_size.py [--cpu 68020] [--level Os] [--compiler ours|gcc|clang|both|all] NAME...

The program is compiled by llrm-cc (`ours`), by m68k-linux-gnu-gcc (`gcc`, the call-the-library column) or by the pin's clang to IR and its M68k
llc (`clang`), assembled, and linked with the crt stub and the compiler-rt archive llrm-cc builds at the same level (every column uses it:
libgcc's m68k routines are not in the gcc root). Only the members the program references link, as in a real link. Every row prints both numbers, `program` and `library`, and their sum
`whole`; a column that shows one of them hides the other (a libcall column linked with every routine, or none, was off by hundreds of bytes).
Environment: LLRM_BIN (llrm-cc), GCC_M68K, LLRM_PIN_CLANG and LLRM_PIN_LLC (the clang column; defaults as `pinned_tool`), M68K_EXEC_CACHE.
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
import external_flags  # noqa: E402
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


def pinned_tool(name: str) -> str:
    """The pin's clang or its M68k llc: LLRM_PIN_CLANG / LLRM_PIN_LLC, else the clang build llrm-cc uses (~/.cache/llrm/clang-<pin commit>) and the
    M68k-enabled llc build in ~/scratch/llvm-build-m68k (the cached build has no M68k target)."""
    given = os.environ.get("LLRM_PIN_CLANG" if name == "clang" else "LLRM_PIN_LLC")
    if given:
        return given
    if name == "llc":
        return str(Path.home() / "scratch/llvm-build-m68k/bin/llc")
    commit = (ROOT / "toolchain/clang/pin").read_text().splitlines()[0].split()[0][:12]
    return str(Path.home() / ".cache/llrm" / f"clang-{commit}" / "bin" / name)


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


def gcc_command(tools: str, cpu: str, level: str, asm: Path, source: Path, flags: list[str]) -> list[str]:
    """gcc's line with the program's own flags in gcc's spelling: `-fno-inline-functions` left out, gcc inlined what ours and the pin keep calls."""
    return [f"{tools}gcc", f"-m{cpu}", f"-{level}", "-msoft-float", "-ffreestanding", *external_flags.gcc_program_flags(flags), "-S", "-o", str(asm), str(source)]


def measure(source: Path, cpu: str, level: str, compiler: str, cc: str, root: str, cache: Path, work: Path) -> dict[str, object]:
    env = sweep.tool_env(root)
    tools = f"{root}/usr/bin/m68k-linux-gnu-"
    asm = work / f"{source.stem}.{compiler}.s"
    flags = o0_sweep.flags_of(source.read_text(), f"m68k-{cpu}")
    if compiler == "gcc":
        cmd = gcc_command(tools, cpu, level, asm, source, flags)
        run_env = env
    elif compiler == "clang":
        pin, llc = pinned_tool("clang"), pinned_tool("llc")
        if not Path(pin).exists() or not Path(llc).exists():
            raise SystemExit(f"m68k_program_size: the clang column needs the pin's clang ({pin}) and its llc ({llc}); LLRM_PIN_CLANG and LLRM_PIN_LLC name them")
        ir = work / f"{source.stem}.clang.ll"
        front = subprocess.run(
            [pin, "--target=m68k-unknown-linux-gnu", f"-mcpu={cpu}", f"-{level}", "-w", "-S", "-emit-llvm", "-I", str(ROOT / "runtime/shared/posix"), "-o", str(ir), str(source)],
            capture_output=True, text=True, timeout=sweep.TIMEOUT,
        )
        if front.returncode != 0:
            raise SystemExit(f"m68k_program_size: clang does not compile {source.name}: {sweep.first_error(front.stderr)}")
        cmd = [llc, f"-mcpu=M{cpu}", "-O2", "-filetype=obj", str(ir), "-o", str(work / f"{source.stem}.clang.o")]  # its text is not GNU as syntax
        run_env = env
    else:
        cmd = [cc, f"-{level}", "-w", "-S", *flags, "-I", str(ROOT / "runtime/shared/posix"), "-o", str(asm), str(source)]
        run_env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": f"m68k-{cpu}"}
    built = subprocess.run(cmd, capture_output=True, text=True, env=run_env, timeout=sweep.TIMEOUT)
    if built.returncode != 0:
        raise SystemExit(f"m68k_program_size: {compiler} does not compile {source.name}: {sweep.first_error(built.stderr)}")
    obj = work / f"{source.stem}.{compiler}.o"
    if compiler != "clang":
        glob = work / f"{source.stem}.{compiler}.g.s"
        glob.write_text(globalize(asm.read_text()))
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
    args = sweep.build_support(cc, root, cpu, cache, int(os.environ.get("JOBS", "4")), level, runnable=False)
    return args[0], args[2]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpu", default="68020")
    parser.add_argument("--level", default="Os")
    parser.add_argument("--compiler", default="both", choices=["ours", "gcc", "clang", "both", "all"])
    parser.add_argument("names", nargs="+")
    args = parser.parse_args()
    root = os.environ.get("GCC_M68K")
    if not root:
        print("m68k_program_size: GCC_M68K is missing", file=sys.stderr)
        return 2
    cc = o0_sweep.compiler()
    cache = sweep.cache_dir(cc)
    columns = {"both": ["ours", "gcc"], "all": ["ours", "gcc", "clang"]}.get(args.compiler, [args.compiler])
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

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
    """The compiler-rt members of a link, by member name. The harness stubs (`stubs_<cpu>.a`: memset, memcpy, memmove, memcmp, the crt) stand in for the platform's libc, which is not
    program bytes on any target but DOS: counted, a program that called memset carried the stub's 454 B (mem_length_boundaries_m32 -Os 68000: gcc 152 + 454 against ours 142, the call's
    bytes against a byte loop's)."""
    return {re.search(r"\(([^)]+)\)", path).group(1): size for path, size in sizes.items() if "(" in path and not Path(path.split("(")[0]).name.startswith("stubs_")}


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


def in_text(text: str) -> str:
    """The compiler's assembly under `.text`, its own `.globl` lines unchanged. Every label global (as this once did) made each call to a `static` function a 32-bit relocation: ours, which calls pc-relative, read 192 B larger on fcmp_all (5530 against 5338) and gcc, which calls absolute, did not."""
    return ".text\n" + text


def ours_command(cc: str, level: str, flags: list[str], obj: Path, source: Path) -> list[str]:
    """llrm-cc's own object: its encoder, not GNU as on the printed text. `lea (sym,%pc)` is 4 B in our object (`R_68K_PC16`, as the pin's) and 8 B when GNU as reads the text at the 68020 (a 32-bit base displacement): fcmp_all 68020 Os read 5338 B by text and is 4794 B as an object."""
    return [cc, f"-{level}", "-w", "-c", *flags, "-I", str(ROOT / "runtime/shared/posix"), "-o", str(obj), str(source)]


def gcc_command(tools: str, cpu: str, level: str, asm: Path, source: Path, flags: list[str], pcrel: bool = False) -> list[str]:
    """gcc's line with the program's own flags in gcc's spelling: `-fno-inline-functions` left out, gcc inlined what ours and the pin keep calls."""
    return [f"{tools}gcc", f"-m{cpu}", f"-{level}", "-msoft-float", *(["-mpcrel"] if pcrel else []), *external_flags.GCC_DIALECT, *external_flags.gcc_source_defines(source.read_text()), "-I", str(ROOT / "runtime/shared/posix"), *[flag for flag in external_flags.gcc_program_flags(flags) if not flag.startswith("-mcmodel")], "-S", "-o", str(asm), str(source)]


def measure(source: Path, cpu: str, level: str, compiler: str, cc: str, root: str, cache: Path, work: Path) -> dict[str, object]:
    env = sweep.tool_env(root)
    tools = f"{root}/usr/bin/m68k-linux-gnu-"
    asm = work / f"{source.stem}.{compiler}.s"
    flags = o0_sweep.flags_of(source.read_text(), f"m68k-{cpu}")
    if compiler in ("gcc", "gcc-pcrel"):
        cmd = gcc_command(tools, cpu, level, asm, source, flags, pcrel=compiler == "gcc-pcrel")
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
        cmd = ours_command(cc, level, flags, work / f"{source.stem}.ours.o", source)
        run_env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": f"m68k-{cpu}"}
    built = subprocess.run(cmd, capture_output=True, text=True, env=run_env, timeout=sweep.TIMEOUT)
    if built.returncode != 0:
        raise SystemExit(f"m68k_program_size: {compiler} does not compile {source.name}: {sweep.first_error(built.stderr)}")
    obj = work / f"{source.stem}.{compiler}.o"
    if compiler not in ("clang", "ours"):
        glob = work / f"{source.stem}.{compiler}.g.s"
        glob.write_text(in_text(asm.read_text()))
        assembled = subprocess.run([f"{tools}as", f"-m{cpu}", "-o", str(obj), str(glob)], capture_output=True, text=True, env=env)
        if assembled.returncode != 0:  # gcc -mpcrel on a data section over 32 KB: a 16-bit displacement does not reach
            raise SystemExit(f"m68k_program_size: {compiler} {source.name} does not assemble: {sweep.first_error(assembled.stderr)}")
    crt, archive, stubs = support(cc, root, cpu, level, cache)
    exe, mapfile = work / f"{source.stem}.{compiler}.exe", work / f"{source.stem}.{compiler}.map"
    link = subprocess.run(link_command(tools, cpu, obj, exe, mapfile, crt, archive, stubs), capture_output=True, text=True, env=env)
    if link.returncode != 0:
        raise SystemExit(f"m68k_program_size: {compiler} {source.name} does not link: {sweep.first_error(link.stderr)}")
    sizes = text_bytes(mapfile.read_text())
    library = members_of(sizes)
    program = sizes.get(str(obj), 0)
    return {"program": program, "library": sum(library.values()), "members": library, "whole": program + sum(library.values())}


def support(cc: str, root: str, cpu: str, level: str, cache: Path) -> tuple[str, str, str]:
    """The crt object, the compiler-rt archive at LEVEL and the libc stub archive (`memcpy`, `memset`, the `llrm_os_*` calls): one stub source for both columns, so a program either column calls them for pays the same member bytes."""
    args = sweep.build_support(cc, root, cpu, cache, int(os.environ.get("JOBS", "4")), level, runnable=False)
    stubs = cache / f"stubs_{cpu}.a"
    if not stubs.exists():
        tmp = stubs.with_suffix(".tmp")
        tmp.unlink(missing_ok=True)
        subprocess.run([f"{root}/usr/bin/m68k-linux-gnu-ar", "rcs", str(tmp), args[3], args[4]], check=True, env=sweep.tool_env(root))
        tmp.rename(stubs)
    return args[0], args[2], str(stubs)


def link_command(tools: str, cpu: str, obj: Path, exe: Path, mapfile: Path, crt: str, archive: str, stubs: str) -> list[str]:
    """The program, the crt and the two archives; the group lets the stubs reach back into compiler-rt."""
    return [f"{tools}gcc", f"-m{cpu}", "-msoft-float", "-nostdlib", "-static", "-ffreestanding", f"-Wl,-Map={mapfile}", "-o", str(exe), str(obj), crt,
            "-Wl,--start-group", archive, stubs, "-Wl,--end-group"]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cpu", default="68020")
    parser.add_argument("--level", default="Os")
    parser.add_argument("--compiler", default="both", choices=["ours", "gcc", "gcc-pcrel", "clang", "both", "like", "all"])
    parser.add_argument("names", nargs="+")
    args = parser.parse_args()
    root = os.environ.get("GCC_M68K")
    if not root:
        print("m68k_program_size: GCC_M68K is missing", file=sys.stderr)
        return 2
    cc = o0_sweep.compiler()
    cache = sweep.cache_dir(cc)
    columns = {"both": ["ours", "gcc"], "like": ["ours", "gcc-pcrel"], "all": ["ours", "gcc", "clang"]}.get(args.compiler, [args.compiler])
    print("| program | cpu | level | compiler | program B | library B | whole B | members |")
    print("|---|---|---|---|---|---|---|---|")
    keep = os.environ.get("M68K_KEEP")  # a directory that keeps the linked .exe files for `cycles --exe`
    if keep:
        Path(keep).mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        tmp = keep or tmp
        for name in args.names:
            report(name, columns, args, cc, root, cache, Path(tmp))
    return 0


def report(name: str, columns: list[str], args: argparse.Namespace, cc: str, root: str, cache: Path, work: Path) -> None:
    """One row per column. A program a column cannot build or link (an x86-only source, a libc call: the link has no libc) is one SKIP row and the next program runs: it measured nothing, and it must not end the sweep."""
    for compiler in columns:
        try:
            got = measure(ROOT / "tests/run/c" / f"{name}.c", args.cpu, args.level, compiler, cc, root, cache, work)
        except SystemExit as refused:
            print(f"| {name} | {args.cpu} | {args.level} | {compiler} | SKIP | | | {str(refused.code or refused).removeprefix('m68k_program_size: ')[:200]} |")
            return
        members = " ".join(f"{short(m)}:{b}" for m, b in sorted(got["members"].items()))
        print(f"| {name} | {args.cpu} | {args.level} | {compiler} | {got['program']} | {got['library']} | {got['whole']} | {members} |")


if __name__ == "__main__":
    sys.exit(main())

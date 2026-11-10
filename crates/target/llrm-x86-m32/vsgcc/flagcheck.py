"""The m32 table's flag parity: refuses a gcc or clang column, or an llrm-cc front end, whose codegen flags differ from the others'.

  flagcheck.py externals gcc|clang LEVEL FLAG...   the external's flags against the program's (build.sh calls this per column)
  flagcheck.py ours LEVEL OURS_CLANG_LINE FLAG...   the clang line llrm-cc ran (logged through a wrapper) against an external's
  flagcheck.py source FILE                          refuses a program that uses wchar_t or a wide literal

The external column goes through `external_flags.check_gcc_flags`: the program's tokens (-O level, -march) must be in it, and
beyond the shared list nothing may be (a distro default on one side moved arm64's cells 4..16 B). The llrm-cc line differs from an
external's by plumbing and by documented front-end settings only (`OURS_ONLY`, `EXTERNAL_ONLY`); a fast-math or excess-precision
flag on either side is refused. m32 is i486: no SSE, so every column is x87 (-mfpmath=387 is the CPU's only choice).
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[4] / "tools"))
import external_flags  # noqa: E402

# Flags only the externals carry: the target spelled as -m32, the shared list's
# distro defaults (the same tokens `external_flags.flags` returns), -Dfar= (the DOS keyword the kernels never use in C).
EXTERNAL_ONLY = ("-m32", "-Dfar=", "-c")
# Front-end settings of llrm-cc's clang that are not codegen: the Watcom headers, the DOS ABI spelling, the IR-only run. The
# one codegen flag is -funroll-loops at -O1 (DEVIATE UNROLL-O1, llrm-cc lib.rs: the pin's -O1 marks every loop unroll.disable).
# Ours-only front-end settings and what they change: -fshort-wchar makes wchar_t 2 bytes (gcc/clang on Linux: 4), so a program
# using wchar_t or a wide literal would be a different program per column: `uses_wide_chars` refuses it (none of bench/, tests/run/c
# or kernels/ does). -std=gnu99 (gcc/clang default gnu17) changes language rules (C11 atomics, _Generic, anonymous-struct details),
# not codegen of C99 code; -fdeclspec, -fhosted, the Watcom headers (-nostdinc/-isystem) and -fdefault-calling-conv=regcall are the
# DOS front end: the calling convention of external functions is -mabi=sysv's (stack arguments, as gcc and clang), stamped in
# $O/abi by build.sh; this guard does not compare conventions.
OURS_ONLY = ("-fhosted", "-fshort-wchar", "-std=gnu99", "-fdeclspec", "-nostdinc", "-S", "-emit-llvm", "-mlong-double-64")
PAIRS = ("-o", "-isystem", "-idirafter", "-include", "-Xclang", "-target")
FP_CODEGEN = ("-ffast-math", "-Ofast", "-ffp-contract", "-fexcess-precision", "-mfpmath", "-mno-sse", "-msse", "-ffinite-math-only", "-funsafe-math")


def uses_wide_chars(text):
    return re.search(r"\bwchar_t\b|(?<![A-Za-z0-9_])L[\"']", text) is not None


def shared(level):
    """The program's own tokens: what both sides are asked for."""
    return [f"-{level}", "-march=i486"]


def check_external(kind, level, tokens):
    # `check_gcc_flags` requires each of these: the column is built with exactly them (build.sh `F`).
    allowed = [*external_flags.flags("x86", ("-fno-pic",)), "-m32", "-Dfar="]
    try:
        external_flags.check_gcc_flags(shared(level), [t for t in tokens if t != "-mlong-double-64"], allowed_extra=allowed)
    except ValueError as e:
        return f"{kind} -{level}: {e}"
    fp = [t for t in tokens if t.startswith(FP_CODEGEN)]
    return f"{kind} -{level}: floating-point flags {fp}" if fp else ""


def codegen_flags(line, level):
    """The codegen flags of an llrm-cc clang line, the plumbing and documented front-end settings dropped."""
    tokens, out, i = line.split(), set(), 0
    while i < len(tokens):
        t = tokens[i]
        if t in PAIRS:
            i += 1
        elif t.startswith(("-I", "-D", "-W", "-fdefault-calling-conv")) or t in OURS_ONLY or t.endswith((".c", ".cpp")):
            pass
        elif t == "-funroll-loops" and level == "O1":
            pass
        elif t.startswith("--target="):
            pass
        else:
            out.add(t)
        i += 1
    return out


def check_ours(level, line, tokens):
    ours = codegen_flags(line, level)
    theirs = {t for t in tokens if t not in EXTERNAL_ONLY and t not in external_flags.flags("x86", ("-fno-pic",))}
    theirs.discard("-fomit-frame-pointer")
    ours -= {"-fno-pic", "-fno-stack-protector", "-fno-asynchronous-unwind-tables"}
    theirs -= {"-fno-pic"}
    if ours != theirs:
        return f"ours -{level}: only ours {sorted(ours - theirs)}, only external {sorted(theirs - ours)}"
    fp = [t for t in ours | theirs if t.startswith(FP_CODEGEN)]
    return f"floating-point flags {fp}" if fp else ""


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "source":
        if uses_wide_chars(Path(sys.argv[2]).read_text()):
            print(f"flagcheck: {sys.argv[2]} uses wchar_t or a wide literal: ours is 2 bytes, gcc and clang 4 (cell n/a)", file=sys.stderr)
            sys.exit(2)
        sys.exit(0)
    why = check_external(sys.argv[2], sys.argv[3], sys.argv[4:]) if mode == "externals" else check_ours(sys.argv[2], sys.argv[3], sys.argv[4:])
    if why:
        print(f"flagcheck: {why}", file=sys.stderr)
        sys.exit(2)

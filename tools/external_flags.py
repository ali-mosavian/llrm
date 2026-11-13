"""The one flag set every external bar (gcc, clang, the pin) is compiled with; per-target fields are explicit, never forked.

A distro compiler changes codegen by default: Ubuntu's gcc keeps the frame pointer at -Os (x_fir read 176 B against 166 B
for upstream gcc), gcc -m64 adds endbr64 and a canary, and a distro builds PIE. The bar is the compiler's own code, so every
default that is the distro's is named here, in one place.

  python3 tools/external_flags.py [--target x86|arm64|none] [--reloc=FLAG]... [--gcc-major N] [--hash]

Relocation model is not in the shared list: it is what OUR compiler emits for the target (m32: -fno-pic -fno-pie; m64: ours is
PIE, -fPIE; -fno-pie faked +10.6% on hanoi2). The target's owner passes it with --reloc and asserts it with a relocation-kind probe.
"""

import argparse
import collections
import hashlib

COMMON = (
    "-fomit-frame-pointer",
    "-fno-stack-protector",
    "-fno-stack-clash-protection",
    "-U_FORTIFY_SOURCE",
    "-fno-asynchronous-unwind-tables",  # last: build.sh swaps it for the .eh_frame column
)
TARGET = {
    "x86": ("-fcf-protection=none",),  # no endbr
    "arm64": ("-mbranch-protection=none",),  # no PAC/BTI
    "none": (),  # targets with no branch-protection option (m68k, wasm, 6502, z80)
}
ZERO_INIT_PADDING = "-fzero-init-padding-bits=standard"  # gcc 15's upstream default, pinned; gcc < 15 rejects the option


def flags(target, reloc=(), gcc_major=None):
    return (*reloc, *TARGET[target], *COMMON, *((ZERO_INIT_PADDING,) if gcc_major and gcc_major >= 15 else ()))


# A test's own flags go to clang and to ours as written, where `-fno-inline-functions` means no inlining. gcc's flag only stops
# -finline-functions: called-once and small functions still inline at -Os (frameless x86_32 -Os: gcc 304 B with or without it; ours 378,
# the pin 625, gcc -fno-inline 501). gcc has no mode that keeps `inline`-declared functions only, so the whole of it is -fno-inline.
GCC_PROGRAM_FLAGS = {"-fno-inline-functions": "-fno-inline"}
# What gcc has no flag for. Left in, `-fno-inline=NAME` and `-fsanitize=stack` fail the compile, but `-ffixed-point` is read as
# `-ffixed-REG` and only warns ("unknown register name: point"): a column that compiled a different program.
# gcc 15 defaults to C23, clang and ours to gnu17: a program with an old-style call ("none(1, 2)" to `int none()`) is an error to gcc
# alone (old_style_extra_args had no gcc cell). The dialect every gcc column builds the corpus in.
GCC_DIALECT = ("-std=gnu17",)
CLANG_ONLY = ("-fno-inline=", "-fsanitize=stack", "-ffixed-point")


def gcc_program_flags(tokens):
    """The tokens of a test's `// flags:` as gcc must take them for the same meaning; ValueError for one gcc cannot take (the cell is n/a for gcc)."""
    for token in tokens:
        if token.startswith(CLANG_ONLY):
            raise ValueError(f"gcc has no {token}: this cell has no gcc column")
    return [GCC_PROGRAM_FLAGS.get(token, token) for token in tokens]


def check_gcc_flags(program_tokens, gcc_tokens, allowed_extra=()):
    """Refuses a gcc flag list that differs from the test's own flags by anything but `GCC_PROGRAM_FLAGS` (and `allowed_extra`).

    A column built with a flag set nobody compared moves its cells silently (arm64: ours without -fomit-frame-pointer against
    externals with it, 24 of 72 cells -4..-16 B). ValueError names every difference, or that the cell has no gcc column.
    The lists are compared as multisets: a flag missing, added or repeated is refused, but ORDER is not checked, so a pair whose
    meaning depends on order (`-O2 -O0`, `-fno-inline -finline-functions`) passes when both lists hold the same tokens."""
    want = collections.Counter([*gcc_program_flags(program_tokens), *allowed_extra])
    have = collections.Counter(gcc_tokens)
    missing, extra = sorted((want - have).elements()), sorted((have - want).elements())
    if missing or extra:
        raise ValueError(f"gcc's flags differ from the test's beyond GCC_PROGRAM_FLAGS: missing {missing}, extra {extra}")


def config_hash():
    text = repr((COMMON, sorted(TARGET.items()), ZERO_INIT_PADDING))
    return hashlib.sha256(text.encode()).hexdigest()[:12]


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", choices=sorted(TARGET), default="x86")
    ap.add_argument("--reloc", action="append", default=[])
    ap.add_argument("--gcc-major", type=int)
    ap.add_argument("--hash", action="store_true")
    args = ap.parse_args()
    print(config_hash() if args.hash else " ".join(flags(args.target, args.reloc, args.gcc_major)))

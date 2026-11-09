"""The codegen flags of one 6502 compile line, and the guard that refuses a cell whose two sides differ.

`program_flags` is the `// flags:` -f/-D options of a test; ours (clang spelling) takes them as written, gcc through
`external_flags.gcc_program_flags` (the one translation table). `mismatch` compares both lines in clang spelling.
The gcc progs column ran `-fno-tree-loop-distribute-patterns -fpack-struct=1` and no `// flags:` of the program: 9 programs
changed code with the first (ours turns the same loops into memset/memcpy calls), 11 inlined what the test forbids, and
the second was a no-op (gcc's natural alignment on the 6502 is 1) (2026-11-09).
"""

import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
import external_flags  # noqa: E402

# Both sides of the corpus matrix: plain char is unsigned on llrm-cc, gcc, cc65, SDCC and llvm-mos; the tests expect signed.
CORPUS_COMMON = ("-fsigned-char",)
X86_ONLY = re.compile(r"-(m32|m16|march=.*|mabi=.*|mlong-double-80|mregparm=.*)$")
# What names the cell or the plumbing, not the code: level, output, includes, cc1's own predefined types, the default dialect.
PLUMBING = re.compile(r"-(O\w*|I.*|o|S|c|w|W.*|quiet|std=gnu17|D__\w+_TYPE__=.*|-target=.*|target)$")
PAIRS = {"-I", "-o", "-include", "-x"}  # option and its separate argument
# gcc's spelling of a clang option, from the one table
FROM_GCC = {gcc: clang for clang, gcc in external_flags.GCC_PROGRAM_FLAGS.items()}


def program_flags(path):
    """The `// flags:` -f options of a program (alternatives split by `|`), x86-only -m options dropped.
    Any other token is an error: a flag the harness cannot honor must not be measured as if it were."""
    first = open(path).readline()
    if not first.startswith("// flags:"):
        return []
    out = []
    alts = first[len("// flags:") :].split("|")
    # a sanitizer alternative is another build of the program (no runtime here): measured, `idioms` read with bounds checks
    plain = [a for a in alts if "-fsanitize" not in a]
    for alt in plain or alts:
        for t in alt.split():
            if re.fullmatch(r"-O\w+", t) or t.startswith("-W") or X86_ONLY.fullmatch(t):
                continue
            if not t.startswith(("-f", "-D")):
                raise SystemExit(f"{path}: header flag {t} is not handled by the harness")
            if t not in out:
                out.append(t)
    return out


def canonical(tokens):
    """The codegen flags of one command line, in clang's spelling, as a sorted tuple; the level is checked on its own."""
    kept, skip = [], False
    for t in tokens:
        if skip:
            skip = False
        elif t in PAIRS:
            skip = True
        elif not PLUMBING.fullmatch(t) and not t.endswith((".c", ".cpp", ".i")):
            kept.append(FROM_GCC.get(t, t))
    return tuple(sorted(kept))


def level(tokens):
    return next((t for t in tokens if re.fullmatch(r"-O\w+", t)), None)


def mismatch(ours, bar):
    """Empty when the sides agree, else a one-line reason."""
    if level(ours) != level(bar):
        return f"level differs: ours {level(ours)}, bar {level(bar)}"
    (o, b) = canonical(ours), canonical(bar)
    if o != b:
        return f"flags differ: only ours {sorted(set(o) - set(b))}, only bar {sorted(set(b) - set(o))}"
    return ""


if __name__ == "__main__":
    why = mismatch(sys.argv[1].split(), sys.argv[2:])
    if why:
        print(f"flagset: {why}", file=sys.stderr)
        sys.exit(2)

"""The codegen-affecting flag set of one compile line, canonical, so a judge table can refuse columns built differently.

`canonical(tokens)` keeps what changes the code a compiler emits (-f*, -m*) and drops what names the cell or the
column: the level (-O), the target, include/define options, and the vectorizer switches (a column's kind, not a
disagreement). `-fzero-init-padding-bits=` is gcc's alone (clang and ours reject it). A table whose ours and bar
columns differ here is not a comparison: "ours" without -fomit-frame-pointer against a bar with it read 4-16 B
worse on 24 of 72 cells (2026-11-09).
"""

import sys

DROPPED_PREFIXES = ("-O", "-I", "-D", "-U", "-W", "-g", "-target", "--target", "-include", "-c", "-o", "-l", "-L", "-std")
COLUMN_KIND = ("-fno-vectorize", "-fno-slp-vectorize", "-fno-tree-vectorize")
GCC_ONLY = ("-fzero-init-padding-bits=",)


def canonical(tokens):
    kept = set()
    for token in tokens:
        if token in COLUMN_KIND or token.startswith(GCC_ONLY) or token.startswith(DROPPED_PREFIXES):
            continue
        if token.startswith(("-f", "-m")):
            kept.add(token)
    return " ".join(sorted(kept))


def check_gcc(tokens):
    """The gcc column's flags against the neutral set, by `external_flags.check_gcc_flags` (membership, no table of its own): the bench's own
    flags are none (the matrix gives -O itself), the allowed extras are the neutral set and the vectorizer switch of a no-vector column."""
    sys.path.insert(0, __file__.rsplit("/", 2)[0])
    import external_flags

    neutral = external_flags.flags("arm64", ["-fno-pic", "-fno-pie"], 15)
    external_flags.check_gcc_flags([], tokens, allowed_extra=(*neutral, *(t for t in tokens if t in COLUMN_KIND)))


if __name__ == "__main__":
    if sys.argv[1] == "--check-gcc":
        try:
            check_gcc(open(sys.argv[2]).read().split())
        except ValueError as why:
            sys.exit(str(why))
    else:
        print(canonical(open(sys.argv[1]).read().split()))

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


if __name__ == "__main__":
    print(canonical(open(sys.argv[1]).read().split()))

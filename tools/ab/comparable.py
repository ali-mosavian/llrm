"""Which tests/run/c programs an external-compiler column (gcc, clang) can be quoted for.

A source that picks its code by `__clang__` or `__GNUC__` is a different program for each compiler: ours and clang's
build one branch, gcc's the other, so a byte or clock gap between the columns is the source, not the code generator.
variadic.c read 662 B against gcc's 219 B at x86_64 -Os for exactly this reason.
"""
import re
from pathlib import Path

SPLIT = re.compile(r"^\s*#\s*(?:if|ifdef|ifndef|elif)\b.*\b(?:__clang__|__GNUC__)\b", re.M)


def compiler_split(source: str) -> bool:
    """Whether the source compiles to a different program under gcc than under clang."""
    return SPLIT.search(source) is not None


def not_comparable(corpus: Path) -> list[str]:
    """The names in `corpus` (a directory of .c files) no external column may be quoted for."""
    return sorted(one.name for one in corpus.glob("*.c") if compiler_split(one.read_text()))

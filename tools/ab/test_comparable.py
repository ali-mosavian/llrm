"""An external-compiler column for a program that splits on the compiler is a different program, not a gap."""
from pathlib import Path

import comparable

CORPUS = Path(__file__).resolve().parents[2] / "tests" / "run" / "c"


def test_a_source_that_picks_its_code_by_the_compiler_is_not_comparable():
    assert comparable.compiler_split("#ifdef __clang__\nint a;\n#else\nint b;\n#endif\n")
    assert comparable.compiler_split("  # if defined(__GNUC__) && !defined(__clang__)\n")
    assert not comparable.compiler_split("int clang_value;  /* __clang__ in a comment */\n")


def test_two_corpus_programs_split_on_the_compiler_and_no_more():
    """variadic.c and long_double_variadic.c: x86_64 -Os read 662 B against gcc's 219 B for the first. A third one must be listed here."""
    assert comparable.not_comparable(CORPUS) == ["long_double_variadic.c", "variadic.c"]

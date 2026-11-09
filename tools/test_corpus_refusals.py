import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import corpus_refusals as cr  # noqa: E402


def needs(token):
    return lambda flags: (0, "") if token in " ".join(flags) else (1, f"x.c:1:1: error: needs {token}")


def test_a_program_is_compiled_with_its_own_flags_not_the_tables():
    """fixed_point.c (`-ffixed-point`) and 3 ms_struct programs were counted as refusals because the sweep passed the row's flags only."""
    assert cr.classify("x86_64-linux", "// flags: -O1 -ffixed-point | -O2 -ffixed-point\nint x;\n", needs("-ffixed-point")) == ("ok",)


def test_the_runtime_include_dirs_are_always_given():
    """os_*.c and bench_io.c (13 at m16, 3 at m64) failed on `'llrm_os.h' file not found`."""
    assert cr.classify("x86_16-dos", "int x;\n", needs("runtime/shared/posix")) == ("ok",)


def test_a_program_the_header_excludes_is_not_applicable_never_refused():
    assert cr.classify("x86_64-linux", "// targets: dos\nint x;\n", lambda f: (1, "boom"))[0] == "na"


def test_a_real_refusal_names_the_first_error():
    assert cr.classify("x86_64-linux", "int x;\n", lambda f: (1, "note\nx.c:3:1: error: bad\n")) == ("refused", "x.c:3:1: error: bad")


def test_the_level_and_the_x86_mode_are_not_taken_from_the_header():
    seen = []
    cr.classify("x86_64-linux", "// flags: -O1 -m32 -march=i486 -DA\nint x;\n", lambda f: seen.append(f) or (0, ""))
    assert seen[0][0] == "-DA" and "-O1" not in seen[0] and "-m32" not in seen[0]

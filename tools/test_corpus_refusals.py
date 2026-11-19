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


def test_register_upper_half_is_not_run_at_m64_where_there_is_no_watcom_abi():
    """`-mabi=watcom` is not an m64 ABI (`no ABI "watcom": this target has sysv, win64`): m64 refused the program. Only the m64 row changes."""
    import corpus_targets

    source = (Path(__file__).parent.parent / "tests/run/c/register_upper_half.c").read_text()
    assert not corpus_targets.applies("x86_64-linux", source)
    assert all(corpus_targets.applies(t, source) for t in ("x86_32-linux", "x86_32-dos", "x86_16-dos", "arm64-linux", "m68k-68020", "wasm32"))


def test_bench_io_passes_no_void_to_report():
    """Since #9424's twins `bench_close` is `((void)...)` where __INT_MAX__ is defined (every compiler but the DOS ones): `report(bench_close(handle))` was
    "passing void to long" and the program was refused at all targets (36 cells). The program closes, then reports the 0 its `.out` expects."""
    import subprocess

    root = Path(__file__).parent.parent
    done = subprocess.run(["gcc", "-fsyntax-only", "-I", str(root / "runtime/shared/posix"), str(root / "tests/run/c/bench_io.c")], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr


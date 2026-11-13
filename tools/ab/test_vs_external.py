"""The external columns of the x86_64 size matrix are built with one flag set, none of it the distro's."""
import subprocess

import external_flags
import vs_external

PLAIN = "// flags: -fno-inline-functions\nint f(void) { return 1; }\n"
DISTRO_DEFAULTS = ("-fcf-protection=none", "-fno-stack-protector", "-fomit-frame-pointer")


def test_both_columns_drop_the_distro_defaults():
    """gcc -Os with the distro's canary and endbr64 read memory_expansion 24554 B against 21552 B: -2750 B stack protector, -252 B endbr64."""
    lines = vs_external.external_commands(PLAIN, "-Os")
    for name in ("gcc", "clang"):
        assert all(flag in lines[name] for flag in DISTRO_DEFAULTS), name
        assert "-fPIE" in lines[name], name


def test_gcc_takes_the_program_flags_in_its_own_spelling_and_clang_as_written():
    lines = vs_external.external_commands(PLAIN, "-Os")
    assert "-fno-inline" in lines["gcc"] and "-fno-inline-functions" not in lines["gcc"]
    assert "-fno-inline-functions" in lines["clang"]


def test_the_two_lines_differ_only_by_the_program_flag_translation():
    lines = vs_external.external_commands(PLAIN, "-Os")
    gcc = [flag for flag in lines["gcc"][1:] if flag != "-fno-inline"]
    clang = [flag for flag in lines["clang"][1:] if flag != "-fno-inline-functions"]
    assert gcc == clang


def test_a_program_that_splits_on_the_compiler_has_no_gcc_column():
    """variadic.c at -Os: 662 B against gcc's 168 B, gcc compiling a different program."""
    split = "#ifdef __clang__\nint a;\n#else\nint b;\n#endif\n"
    lines = vs_external.external_commands(split, "-Os")
    assert lines["gcc"] is None and lines["clang"] is not None


def test_a_flag_gcc_cannot_take_is_no_gcc_column_not_a_crash():
    """`-ffixed-point` raised out of the whole matrix and truncated it to 158 of 374 cells."""
    lines = vs_external.external_commands("// flags: -ffixed-point\nint f;\n", "-O1")
    assert lines["gcc"] is None
    assert external_flags.COMMON[0] in lines["clang"]


def test_the_helpers_an_object_calls_are_the_undefined_symbols_libgcc_defines():
    nm_u = "                 U __popcountdi2\n                 U report\n                 U __stack_chk_fail\n"
    libgcc = "libgcc.a:_popcountsi2.o:0000000000000000 T __popcountdi2\nlibgcc.a:_clzdi2.o:0000000000000000 T __clzdi2\n"
    helpers = vs_external.libgcc_members(libgcc)
    assert helpers == {"__popcountdi2": "_popcountsi2.o", "__clzdi2": "_clzdi2.o"}
    assert vs_external.undefined_helpers(nm_u, helpers) == ["__popcountdi2"]


def test_gcc_popcount_calls_libgcc_and_its_object_says_so(tmp_path):
    """bitcount_u64 -Os: gcc 245 B against ours 308 B, the 8 B `ones` a call of `__popcountdi2` whose code is not in the object."""
    source = tmp_path / "ones.c"
    source.write_text("int ones(unsigned long long x) { return __builtin_popcountll(x); }\n")
    obj = tmp_path / "ones.o"
    subprocess.run(["gcc", "-Os", "-c", str(source), "-o", str(obj)], check=True)
    calls = vs_external.library_calls(obj)
    assert calls.startswith("__popcountdi2:") and int(calls.split(":")[1]) > 8, calls


def test_a_helper_the_external_object_calls_is_charged_to_its_column():
    """gcc -Os popcount of `bitcount_u64`: an 8 B call to `__popcountdi2` against 90 B inline was a 82 B gap; the routine (94 B
    with its table) is part of the program, so the gcc column is 8 + 94 and ours is not above it."""
    assert vs_external.charged(8, "__popcountdi2:94") == 102
    assert vs_external.charged(8, "-") == 8
    assert vs_external.charged(10, "__udivti3:300,__popcountdi2:94") == 404
    inline, call = 90, vs_external.charged(8, "__popcountdi2:94")
    assert inline <= call

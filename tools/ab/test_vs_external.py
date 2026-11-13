"""The external columns of the x86_64 size matrix are built with one flag set, none of it the distro's."""
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

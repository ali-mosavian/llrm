"""An A/B cell is built with its target family's own `// flags-FAMILY:` line, and only that family's."""
import cell_flags

SOURCE = "// flags: -O2 -fno-inline-functions -m32 | -Os\n// flags-m68k: -mcmodel=medium\nint main(void) { return 0; }\n"


def test_the_m68k_family_flag_reaches_an_m68k_cell():
    """memcpy_runtime overflowed a pc-relative displacement on m68k without it: 8 cells EMPTY in the 2026-11-13 identity sweep."""
    assert cell_flags.cell_flags(SOURCE, "m68k-68000") == ["-fno-inline-functions", "-mcmodel=medium"]


def test_another_family_and_a_cell_without_a_target_get_the_program_flags_only():
    assert cell_flags.cell_flags(SOURCE, "x86_64-linux") == ["-fno-inline-functions"]
    assert cell_flags.cell_flags(SOURCE) == ["-fno-inline-functions"]

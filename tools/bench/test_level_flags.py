import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import level_flags  # noqa: E402

O2_ONLY = "// flags: -O2 -march=i486 -fno-inline-functions | -O2 -march=i486 -m32\nint main(void){return 0;}\n"


def test_a_levels_cell_is_built_with_the_levels_flags_not_the_headers():
    """The 34 O2-only programs had no m64 O1 cell: the matrix refused them. A LEVELS cell takes the level and the header's -f words, never its -O2 or -march."""
    assert level_flags.cell(O2_ONLY, "O1") == ("LEVELS", ["-fno-inline-functions"])


def test_a_header_alternative_that_names_the_level_is_the_headers_cell():
    text = "// flags: -O1 -march=i486 | -O2 -march=i486\n"
    assert level_flags.cell(text, "O1") == ("header", [])


def test_a_machine_alternative_is_not_an_m64_cell():
    assert level_flags.cell("// flags: -O1 -m32\n", "O1")[0] == "LEVELS"


def test_a_program_without_a_header_is_a_levels_cell():
    assert level_flags.cell("int x;\n", "Os") == ("LEVELS", [])

import shutil
import subprocess

import pytest

import elfsize

SIZE = """x.o  :
section              size   addr
.text                  40      0
.text.startup          12      0
.text.unlikely         30      0
.text.hot               8      0
.rela.text             24      0
.data                   4      0
.comment               44      0
Total                  162
"""


def test_every_text_section_counts_and_relocations_do_not():
    """`.text`-only sums dropped `.text.startup` (main), `.text.unlikely` and `.text.hot`: gcc -Os bytes read up to 3x low."""
    assert elfsize.text_bytes(SIZE) == 90


def test_main_in_text_startup_of_a_real_gcc_object_is_counted(tmp_path):
    gcc = shutil.which("gcc")
    if not gcc:
        pytest.skip("no gcc")
    (tmp_path / "m.c").write_text("int main(void) { return 7; }\n")
    subprocess.run([gcc, "-O2", "-c", str(tmp_path / "m.c"), "-o", str(tmp_path / "m.o")], check=True)
    out = subprocess.run(["size", "-A", str(tmp_path / "m.o")], capture_output=True, text=True, check=True).stdout
    assert elfsize.text_bytes(out) > 0


def gcc_main_object(tmp_path):
    gcc = shutil.which("gcc")
    if not gcc:
        pytest.skip("no gcc")
    (tmp_path / "m.c").write_text("int main(void) { return 7; }\n")
    subprocess.run([gcc, "-O2", "-c", str(tmp_path / "m.c"), "-o", str(tmp_path / "m.o")], check=True)
    return tmp_path / "m.o"


def test_the_pin_comparison_tools_count_main_in_text_startup(tmp_path):
    """cc_cost, m16_bytes and pin_cg_ablate read the first or the exact `.text`: an object whose code is `main` (.text.startup) read 0 bytes."""
    import sys
    from pathlib import Path

    sys.path.insert(0, str(Path(__file__).parent / "pairs"))
    sys.path.insert(0, str(Path(__file__).parent / "dagisel"))
    import cc_cost
    import m16_bytes
    import pin_cg_ablate

    obj = gcc_main_object(tmp_path)
    size = subprocess.run(["size", "-A", str(obj)], capture_output=True, text=True, check=True).stdout
    want = elfsize.text_bytes(size)
    assert want > 0
    assert cc_cost.text_bytes(obj) == want
    assert m16_bytes.pin_text(size) == want
    assert pin_cg_ablate.text_bytes(str(obj)) == want

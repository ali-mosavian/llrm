import importlib.util
import shutil
import subprocess

import cfi_diff as d


def test_an_object_without_an_unwind_table_has_no_function_tables(tmp_path):
    """The comparator crashed on an object stripped of `.eh_frame`, so a missing table could not show as a hole."""
    if not (shutil.which("cc") and shutil.which("objcopy")) or importlib.util.find_spec("elftools") is None:
        return
    (tmp_path / "x.c").write_text("extern int g(int);\nint f(int a) { return g(a) + 1; }\n")
    kept, stripped = tmp_path / "kept.o", tmp_path / "stripped.o"
    subprocess.run(["cc", "-c", "-O1", "-fasynchronous-unwind-tables", str(tmp_path / "x.c"), "-o", str(kept)], check=True)
    subprocess.run(["objcopy", "-R", ".eh_frame", str(kept), str(stripped)], check=True)
    assert "f" in d.tables_of(str(kept))
    assert d.tables_of(str(stripped)) == {}


def test_rows_at_other_offsets_have_the_same_shape():
    pin = ((0, 7, 8, ()), (1, 7, 16, ((16, 5, -16),)), (4, 6, 16, ((16, 5, -16),)))
    ours = ((0, 7, 8, ()), (2, 7, 16, ((16, 5, -16),)), (7, 6, 16, ((16, 5, -16),)))
    assert d.shape(pin) == d.shape(ours)
    assert d.shape(pin) != d.shape(ours[:2])
    assert d.shape(None) is None

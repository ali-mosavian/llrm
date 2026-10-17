import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("bench_table", Path(__file__).parent / "bench_table.py")
bench_table = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench_table)


def cell(instructions, code):
    return {"instructions": instructions, "code_bytes": code}


def test_a_cell_of_any_front_end_whose_bytes_moved_is_named():
    """A BASIC cell +27% in code is a cell the vsgcc rows (C only) never see; every language's cells are compared."""
    base = {"scroll/bq32/Os": cell(100, 200), "scroll/c/Os": cell(100, 200), "sieve/nib/O2": cell(50, 80)}
    main = {"scroll/bq32/Os": cell(100, 254), "scroll/c/Os": cell(100, 200), "sieve/nib/O2": cell(50, 80)}
    text = bench_table.render(base, main)
    assert "scroll/bq32/Os code_bytes: 200 -> 254 (+27.00%)" in text
    assert "| bq32 | Os | 1 | 1 |" in text
    assert "| nib | O2 | 1 | 0 |" in text

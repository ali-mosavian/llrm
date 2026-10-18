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


def test_an_error_cell_stops_the_table_and_is_named():
    """A cell recorded as {"error": ...} had no counter and was skipped by the table: the table read as if it were not there."""
    import pytest

    base = {"sieve/c/O2": cell(50, 80), "scroll/c/Os": cell(100, 200)}
    main = {"sieve/c/O2": cell(50, 80), "scroll/c/Os": {"error": "prints 3, want 4"}}
    with pytest.raises(SystemExit, match=r"1 rows failed, 0 missing\nrows total 2 / with counters 3 of 4 / failed 1 / missing 0(.|\n)*scroll/c/Os \(failed in main: prints 3, want 4\)"):
        bench_table.render(base, main)
    text = bench_table.render(base, main, allow_failed=True)
    assert text.startswith("1 rows failed") and "allowed: scroll/c/Os" in text


def test_a_size_only_cell_is_failed_unless_its_output_was_checked():
    """A timed-only program was sized and never run in the gate: its row had no counters and read as fine. Sized only = unchecked = failed;
    `checked: true` (run, output compared) is not."""
    import pytest

    size_only = {"code_bytes": 10, "data_bytes": 0, "bss_bytes": 0}
    with pytest.raises(SystemExit, match=r"2 rows failed(.|\n)*a/c/O2 \(failed in base: sized only, never run\)"):
        bench_table.render({"a/c/O2": size_only}, {"a/c/O2": size_only})
    ran = size_only | {"checked": True}
    text = bench_table.render({"a/c/O2": ran}, {"a/c/O2": ran})
    assert text.startswith("rows total 1 / with counters 2 of 2 / failed 0")
    assert "base: 1 rows with no instructions (run and output-checked): a/c/O2" in text


def test_a_kernel_folded_away_is_an_unmeasured_row_counted_missing_and_named_not_failed():
    """crc/nib -O3 compiles to `mov eax, <the crc>`: no kernel to find. It reads 'not in the map', which looked like any failure."""
    import pytest

    folded = {"error": "unmeasured: kernel folded away: bench_crc is not in the map, and main does not call one function of its own"}
    base = {"crc/nib/O3": cell(10, 20)}
    main = {"crc/nib/O3": folded}
    with pytest.raises(SystemExit, match=r"0 rows failed, 1 missing(.|\n)*failed 0 / missing 1(.|\n)*crc/nib/O3 \(unmeasured in main: kernel folded away"):
        bench_table.render(base, main)

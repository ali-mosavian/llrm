import shutil
from pathlib import Path

import bench


FIXTURES = Path(__file__).with_name("fixtures") / "basic_exe"


def measured_in_emulator(monkeypatch, tmp_path, program):
    """`program` of the fixtures, built for our runtime at -O2 and run in the emulator with its output checked against its .out."""
    monkeypatch.setattr(bench, "BENCH", tmp_path / "bench")
    shutil.copytree(FIXTURES / program, tmp_path / "bench" / program)
    return bench.measure_all([program], ["O2"], tmp_path / "work")[(program, "bq16", "O2")]


def test_clear_and_erase_zero_the_variables_llrm_keeps_in_bss(monkeypatch, tmp_path):
    """CLEAR and ERASE print 0 for an array element, a scalar and a LONG array element set before them: the variables live in `_BSS`, which the
    start-up zeroes and the linker does not store (822 B of bss, 47 of data). A row that printed its values stayed 5, 7 and 9 if CLEAR missed `_BSS`."""
    counts = measured_in_emulator(monkeypatch, tmp_path, "zclear")
    assert "prints" not in counts.get("error", ""), counts
    assert counts["bss_bytes"] >= 800 and counts["data_bytes"] < 100, counts


def test_read_parses_the_data_text_as_bc_stores_it(monkeypatch, tmp_path):
    """DATA text is stored as written (tabs, blank runs, `1,,2`, `+2`, an unquoted string with blanks, a trailing `' note`): the 19 items read
    back exactly as Microsoft's BC and runtime read them (zprobe.out is BC 4.5's own output)."""
    counts = measured_in_emulator(monkeypatch, tmp_path, "zprobe")
    assert "prints" not in counts.get("error", ""), counts

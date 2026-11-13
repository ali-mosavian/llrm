"""The A/B cell list: every program x target x level, the not-for-this-target ones left for `cells.sh` to count as N/A."""
from pathlib import Path

import corpus_list


def test_a_line_per_program_target_and_level_in_cells_sh_form(tmp_path: Path):
    (tmp_path / "a.c").write_text("int a;\n")
    (tmp_path / "b.c").write_text("// targets: dos\nint b;\n")
    corpus = corpus_list.programs(tmp_path, tmp_path / "no-bench")
    out = corpus_list.lines(["O2", "Os"], ["m68k-68000", "m68k-68020"], corpus)
    assert len(out) == 2 * 2 * 2
    assert f"run/a m68k-68000 -O2 {tmp_path / 'a.c'}" in out
    assert any(line.startswith("run/b m68k-68020 -Os ") for line in out), "a DOS-only program is still listed: cells.sh says N/A"


def test_the_corpus_has_the_run_programs_and_the_benchmarks():
    ids = [ident for ident, _ in corpus_list.programs()]
    assert "run/known_loads_walks" in ids and any(ident.startswith("bench/") for ident in ids)

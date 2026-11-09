import z80_owner_table as t

SDCC = {"sdcc-size": (100, 900), "sdcc-speed": (130, 500)}


def test_ok_when_ours_is_at_both_bars_strict():
    assert t.verdict("Os", (100, 500), SDCC) == "ok"


def test_strict_levels_judge_each_axis_alone():
    assert t.verdict("Os", (101, 500), SDCC) == "B-worse"
    assert t.verdict("O1", (100, 501), SDCC) == "T-worse"
    assert t.verdict("Oz", (101, 501), SDCC) == "B-worse T-worse"


def test_o2_o3_judge_at_three_clocks_per_byte():
    assert t.verdict("O2", (101, 497), SDCC) == "ok"  # +1 B, -3 T net 0
    assert t.verdict("O3", (101, 500), SDCC) == "worse"
    assert t.verdict("O2", (90, 520), SDCC) == "ok"


def test_a_missing_ours_row_is_unmeasured_never_ok():
    assert t.verdict("Os", None, SDCC) == "UNMEASURED"


def test_a_cell_without_sdcc_rows_is_counted_not_ok():
    assert t.verdict("Os", (100, 500), {}) == "NO-SDCC"


def test_a_double_program_is_unranked_when_ours_is_wider_than_sdcc():
    assert t.verdict("Os", (1000, 9000), SDCC, double=True) == t.UNRANKED
    assert t.is_double("double x;") and not t.is_double("float x; // doubled")


def test_a_cell_no_variant_attains_and_ours_on_the_front_is_virtual():
    assert t.is_virtual_cell((110, 600), SDCC)
    assert not t.is_virtual_cell((140, 950), SDCC)  # dominated
    assert not t.is_virtual_cell((100, 500), {"sdcc-a": (100, 500)})  # one variant attains both bars


def test_rows_carry_unmeasured_and_nosdcc_cells():
    ours = {"s": {"p": {"ours": (1, 2)}}}
    sdcc = {"s": {"q": {"sdcc-size": (3, 4)}}}
    got = {r[0]: r[10] for r in t.rows(ours, sdcc, {"p": "int x;", "q": "int y;"}) if r[1] == "Os"}
    assert got == {"p": "NO-SDCC", "q": "UNMEASURED"}


def test_a_program_for_other_targets_is_not_a_row():
    sources = {"p": "int x;", "dosonly": "// targets: dos\nint y;\n", "wide": "// flags: -O2 -m32\nint z;\n"}
    ours = {"s": {"dosonly": {"ours": (1, 2)}}}
    assert {r[0] for r in t.rows(ours, {}, sources)} == {"p"}


def test_a_variant_with_other_flags_never_sets_a_bar():
    """An unknown SDCC column (flags we cannot vouch for) was a bar like any other; now it is refused and the cell is NO-SDCC."""
    refused = t.REFUSED = getattr(t, "REFUSED", [])
    refused.clear()
    sdcc = {"2": {"p": {"sdcc-weird": (50, 100)}}}
    ours = {"2": {"p": {"ours": (100, 500)}}}
    out = t.rows(ours, sdcc, {"p": "int main(void) { return 0; }"})
    assert [r[10] for r in out if r[1] == "O2"] == ["NO-SDCC"]
    assert refused and "unknown SDCC variant" in refused[0][3]


def test_a_program_flag_sdcc_cannot_take_refuses_every_variant():
    refused = t.REFUSED = getattr(t, "REFUSED", [])
    refused.clear()
    sdcc = {"2": {"p": {"sdcc-size": (50, 100), "sdcc-speed": (60, 90)}}}
    ours = {"2": {"p": {"ours": (100, 500)}}}
    out = t.rows(ours, sdcc, {"p": "// flags: -O2 -ffixed-point\nint main(void) { return 0; }"})
    assert [r[10] for r in out if r[1] == "O2"] == ["NO-SDCC"] and len(refused) == 2


def test_the_real_variants_are_kept():
    refused = t.REFUSED = getattr(t, "REFUSED", [])
    refused.clear()
    sdcc = {"2": {"p": {k: (50, 100) for k in ("sdcc-size", "sdcc-size-ma", "sdcc-speed", "sdcc-speed-ma")}}}
    ours = {"2": {"p": {"ours": (100, 500)}}}
    out = t.rows(ours, sdcc, {"p": "int main(void) { return 0; }"})
    assert [r[12] for r in out if r[1] == "O2"] == [4] and not refused


def test_an_oz_row_bigger_than_its_os_row_is_stale():
    """loop_fusion Oz read 953 B beside Os 907 B in the 2026-11-09 table; one binary gives Oz no bigger than Os."""
    ours = {"s": {"a": {"ours": (907, 1)}, "b": {"ours": (5, 1)}}, "z": {"a": {"ours": (953, 1)}, "b": {"ours": (5, 1)}}}
    assert t.stale_oz(ours) == [("a", 907, 953)]
    assert t.stale_oz({"s": {"a": {"ours": (907, 1)}}, "z": {"a": {"ours": (901, 1)}}}) == []


def test_the_table_is_not_written_from_a_stale_oz_column(tmp_path):
    for lv, b in (("s", 907), ("z", 953)):
        d = tmp_path / "ours"
        d.mkdir(exist_ok=True)
        (d / f"full.{lv}.tsv").write_text(f"a\tours\t{b}\t1\n")
    (tmp_path / "sdcc").mkdir()
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "a.c").write_text("int x;")
    args = [str(tmp_path / "out.tsv"), "--ours", str(tmp_path / "ours"), "--sdcc", str(tmp_path / "sdcc"), "--programs", str(tmp_path / "src")]
    try:
        t.main(args)
        raised = False
    except SystemExit as stop:
        raised = stop.code == 1
    assert raised and not (tmp_path / "out.tsv").exists()


def test_an_allowed_stale_oz_table_says_so_in_its_header(tmp_path):
    for lv, b in (("s", 907), ("z", 953)):
        d = tmp_path / "ours"
        d.mkdir(exist_ok=True)
        (d / f"full.{lv}.tsv").write_text(f"a\tours\t{b}\t1\n")
    (tmp_path / "sdcc").mkdir()
    (tmp_path / "src").mkdir()
    (tmp_path / "src" / "a.c").write_text("int x;")
    out = tmp_path / "out.tsv"
    t.main([str(out), "--ours", str(tmp_path / "ours"), "--sdcc", str(tmp_path / "sdcc"), "--programs", str(tmp_path / "src"), "--allow-stale-oz"])
    assert "# STALE-OZ ALLOWED: a" in out.read_text().splitlines()


def _stale_dirs(tmp_path, programs):
    for lv, bytes_ in (("s", 907), ("z", 953)):
        d = tmp_path / "ours"
        d.mkdir(exist_ok=True)
        (d / f"full.{lv}.tsv").write_text("".join(f"{p}\tours\t{bytes_}\t1\n" for p in programs))
    (tmp_path / "sdcc").mkdir(exist_ok=True)
    (tmp_path / "src").mkdir(exist_ok=True)
    for p in programs:
        (tmp_path / "src" / f"{p}.c").write_text("int x;")
    return ["--ours", str(tmp_path / "ours"), "--sdcc", str(tmp_path / "sdcc"), "--programs", str(tmp_path / "src")]


def test_a_gap_named_with_its_row_is_written_and_listed_and_any_other_still_refuses(tmp_path):
    out = tmp_path / "out.tsv"
    args = _stale_dirs(tmp_path, ["a", "b"])
    try:
        t.main([str(out), *args, "--known-oz-gap", "a=Z80-OZ-1"])
        raised = False
    except SystemExit as stop:
        raised = stop.code == 1
    assert raised and not out.exists()  # b is not named
    t.main([str(out), *args, "--known-oz-gap", "a=Z80-OZ-1", "--known-oz-gap", "b=Z80-OZ-2"])
    assert "# OZ>OS KNOWN: a=Z80-OZ-1 b=Z80-OZ-2" in out.read_text().splitlines()

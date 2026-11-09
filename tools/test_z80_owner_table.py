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

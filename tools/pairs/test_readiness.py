"""The readiness table lists a program larger or slower than main, with its owner column blank, and counts them in the headline."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import readiness  # noqa: E402


def test_growers_are_the_programs_larger_in_new_worst_first_and_an_uncompiled_one_is_not_compared():
    base = {"a.c": 100, "b.c": 100, "c.c": 100, "d.c": 0}
    new = {"a.c": 150, "b.c": 90, "c.c": 100, "d.c": 50}
    larger, count, was, now = readiness.growers(base, new)
    assert larger == [("a.c", 100, 150)] and (count, was, now) == (3, 300, 340)


def test_slower_counts_only_programs_measured_on_both_sides():
    base = {"rows": {"a": {"clocks": 100}, "b": {"clocks": 100}, "c": {"error": "x"}}}
    new = {"rows": {"a": {"clocks": 130}, "b": {"clocks": 90}, "c": {"clocks": 1}}}
    assert readiness.slower(base, new) == ([("a", 100, 130)], 2)


def test_the_headline_counts_and_every_row_has_an_empty_owner_column():
    sized = {"-Os": ([("quicksort.nib", 24, 39)], 2, 50, 70)}
    lines = readiness.report(["h"], sized, ([("fib.bas", 100, 110)], 5))
    assert "HEADLINE m16 -Os: 50 -> 70 B (+20); 1 of 2 programs larger" in lines
    assert any(line.startswith("HEADLINE m16 -Os clocks: 1 of 5 programs slower") for line in lines)
    rows = [line for line in lines if line.startswith("  ")]
    assert len(rows) == 2 and all(row.endswith("|") for row in rows) and "+15" in rows[0]


def test_a_table_is_measured_once_per_dist(tmp_path):
    calls = []
    path = tmp_path / "t.json"
    for _ in range(2):
        assert readiness.cached(path, lambda: calls.append(1) or {"a": 1}) == {"a": 1}
    assert len(calls) == 1


def test_cells_counts_the_distinct_bp_cells_of_one_function_only():
    text = "_sort proc near\n mov [bp-2], ax\n mov [bp-2], bx\n mov [bp-4], cx\n_sort endp\n_other proc near\n mov [bp-6], ax\n_other endp\n"
    assert readiness.cells(text, "_sort") == 2


def test_a_program_folded_on_main_is_marked_tiny_not_listed_as_a_12x_slowdown():
    """x87_arguments.c: main folded the whole program (237 clocks, the start-up), the tip keeps the loop like the pin (2893): the table read it as +1120%."""
    rows = {name: {"clocks": 1, "instructions": n, "language": ".c"} for name, n in {"floor.c": 100, "folded.c": 150, "real.c": 5000}.items()}
    assert readiness.tiny({"rows": rows}) == {"floor.c", "folded.c"}
    lines = readiness.report(["h"], {}, ([("folded.c", 237, 2893)], 3), {"folded.c"})
    assert any(line.startswith("  folded.c tiny") for line in lines) and "1 of them tiny" in " ".join(lines)


def test_the_larger_list_is_grouped_by_owner_from_the_causes_file_then_the_flag_owner_and_blank_rows_stay_visible():
    """The coordinator asked for the 139 larger programs per owner; a program nobody holds is listed under (none), never dropped."""
    rules = [{"match": "nbody_fixed/*", "owner": "w-10"}, {"match": "*/x.c", "owner": "unassigned"}]
    larger = [("nbody_fixed/a.nib", 10, 30), ("m/x.c", 10, 20), ("q/y.bas", 10, 15), ("z/w.c", 1, 2)]
    groups = readiness.by_owner(larger, rules, {"q/y.bas": "tree-ch"})
    assert list(groups) == ["w10", "(none)", "w5"] and [r[0] for r in groups["(none)"]] == ["m/x.c", "z/w.c"]
    lines = readiness.report(["h"], {"-Os": (larger, 4, 0, 0)}, None, owners=(rules, {"q/y.bas": "tree-ch"}))
    assert "  w10: 1 programs, +20 B" in lines and "  (none): 2 programs, +11 B" in lines


def test_flag_first_reads_both_ablation_json_layouts():
    """The cache m16burndown writes is flag -> program -> bytes; w13's earlier file was program -> {tip, flag: bytes}. Read as the wrong one, no program got a flag owner (104 of 138 blank)."""
    flag_first = {"-fno-tree-ch": {"a.c": 90}, "-fno-lsr": {"a.c": 99}}
    program_first = {"a.c": {"base": 80, "tip": 100, "-fno-tree-ch": 90, "-fno-lsr": 99}}
    assert readiness.flag_first(flag_first) == flag_first
    assert readiness.flag_first(program_first) == flag_first


def test_a_two_flag_cause_goes_to_the_first_flag_with_an_owner_and_a_program_no_flag_moves_is_ra_peephole_not_none():
    """104 of 138 growers read `(none)`; 69 of them were programs no pass flag moves, 14 had two flags moving them equally."""
    ablation = {"a": "multi: sroa+tree-pre", "b": "multi: gcse+tree-ch", "c": "RA/peephole", "d": "gcse"}
    assert [readiness.owner_of(name, [], ablation) for name in "abcd"] == ["w14", "w5", readiness.RA_OWNER, ""]


def test_the_causes_file_spelling_w_dash_n_is_the_same_owner_as_wn():
    assert readiness.owner_of("a", [{"match": "a", "owner": "w-2"}], {}) == "w2" == readiness.owner_of("b", [], {"b": "move-loop-invariants"})


def test_owner_deltas_list_only_the_owners_that_moved_new_and_gone_owners_count_from_zero():
    before = {"w2": [18, 788], "w5": [9, 388], "w3": [4, 75]}
    after = {"w2": [16, 700], "w5": [9, 388], "w6": [1, 14]}
    assert readiness.owner_deltas(before, after) == [
        "  w2: programs 18 -> 16 (-2), bytes +788 -> +700 (-88)",
        "  w3: programs 4 -> 0 (-4), bytes +75 -> +0 (-75)",
        "  w6: programs 0 -> 1 (+1), bytes +0 -> +14 (+14)",
    ]


def test_a_dist_built_under_a_shortened_name_is_reused_only_when_its_ok_names_the_full_sha(tmp_path):
    """w9 built sbp-36ad593b8; looking only for sbp-<full> queued a second 40-minute build of the same commit."""
    full = "36ad593b8f77b17352ad5e1f9df4191f599fd8ab"
    (tmp_path / "sbp-36ad593b8" / "dist").mkdir(parents=True)
    (tmp_path / "sbp-36ad593b8" / "dist" / "llrm-llc").write_text("x")
    (tmp_path / "sbp-36ad593b8" / "ok").write_text(f"sha: {full}\n")
    assert readiness.existing_dist(full, tmp_path) == tmp_path / "sbp-36ad593b8" / "dist"
    (tmp_path / "sbp-36ad593b8" / "ok").write_text("sha: 0000000000000000000000000000000000000000\n")
    assert readiness.existing_dist(full, tmp_path) is None


def test_attribution_uses_the_sizes_of_the_commit_the_ablation_was_taken_on():
    new, old = {"a.c": 90, "b.c": 0}, {"a.c": 100, "b.c": 50}
    assert readiness.ablation_baseline(new) == {"a.c": 90}
    assert readiness.ablation_baseline(new, lambda: old) == old

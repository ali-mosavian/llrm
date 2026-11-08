"""wasm-ab.sh counted only five-field rows as worse, so a cell the head refuses and the base compiles read as not worse; wasm-sweep.sh
wrote `ours=NA pin=NA` for four programs and the table ended clean (sum_three, os_execute, tail_calls_edges, widening_multiply)."""
import wasm_rows as w


def test_a_cell_only_the_head_refuses_is_not_a_pass():
    worse, head, base, both = w.ab(["# stamp", "fft O2 100 100 0", "idioms O2 FAIL head", "end 12:00Z"])
    assert worse == [] and head == ["idioms O2"] and base == [] and both == []


def test_a_row_that_does_not_say_which_side_failed_is_the_heads():
    assert w.ab(["idioms O2 FAIL"])[1] == ["idioms O2"]


def test_a_larger_head_is_worse_and_a_refusing_base_is_not_the_heads_fault():
    worse, head, base, both = w.ab(["a O2 10 12 2", "b O2 FAIL base", "c O2 FAIL both", "d O2 10 9 -1"])
    assert worse == ["a O2"] and head == [] and base == ["b O2"] and both == ["c O2"]


def test_a_missing_reference_cell_is_listed_unmeasured_not_zero():
    worse, unmeasured = w.sweep(
        ["O2 tests/run/c/a.c ours=10 pin=9 flags=-", "O2 tests/run/c/b.c ours=NA pin=NA flags=-", "Os tests/run/c/c.c ours=5 pin=NA flags=-", "end"]
    )
    assert worse == ["tests/run/c/a.c O2"] and unmeasured == ["tests/run/c/b.c O2", "tests/run/c/c.c Os"]

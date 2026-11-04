"""cellcheck judges with bench.against and bench.rate_for, not its own copy: a copy priced every non-bq16 cell at 312 (m16 bas/nib/c are 500)."""
import json
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import bench  # noqa: E402
import cellcheck  # noqa: E402


def row(clocks, size, instructions=100, memops=10):
    return {"instructions": instructions, "memory_operands": memops, "clocks": clocks, "code_bytes": size}


# (key, head row, base row, fails)
TABLE = [
    ("p/bas/O2", row(1001, 98), row(1000, 100), False),  # m16 500 mc/B: +1 clk for -2 B is a tie
    ("p/nib/O3", row(1001, 98), row(1000, 100), False),
    ("p/c/O2", row(1002, 98), row(1000, 100), True),
    ("p/bq32/O2", row(1001, 98), row(1000, 100), True),  # flat 312 mc/B: +1 clk for -2 B is worse
    ("p/bq16/O2", row(1001, 98), row(1000, 100), False),
    ("p/bas/Os", row(1000, 100, 150), row(1000, 100), True),  # -Os: a tie in bytes and clocks gates instructions
    ("p/bas/Os", row(999, 100, 150), row(1000, 100), False),  # clocks lower: instructions do not gate
    ("p/bq32/Os", row(1000, 101), row(1000, 100), True),
    ("p/bq16/Os", row(1001, 100), row(1000, 100), True),  # -Os: clocks alone worse, bytes and instructions equal
    ("p/bas/O2", row(10, 10), row(0, 10, instructions=0), True),  # a base that executed nothing is no base
]


@pytest.mark.parametrize("key,head,base,fails", TABLE)
def test_cellcheck_agrees_with_bench_against(key, head, base, fails):
    _, language, level = key.rsplit("/", 2)
    assert bool(bench.against(key, head, base, rate=bench.rate_for(language, level))) is fails
    assert bool(cellcheck.check({key: head}, {key: base})) is fails


def test_a_cell_the_head_lacks_fails_and_o1_is_not_gated():
    assert cellcheck.check({}, {"p/bas/O2": row(1, 1)}) == ["p/bas/O2: NOHEAD"]
    assert cellcheck.check({"p/bas/O1": row(9, 9)}, {"p/bas/O1": row(1, 1)}) == []


def test_two_harness_digests_are_all_nobase(tmp_path, capsys):
    files = []
    for name, digest in (("head", "aaa"), ("base", "bbb")):
        (tmp_path / f"{name}.json").write_text(json.dumps({"p/bas/O2": row(1, 1)}))
        (tmp_path / f"{name}.json.harness").write_text(digest)
        files.append(str(tmp_path / f"{name}.json"))
    assert cellcheck.main(files) == 1
    assert "ALL NOBASE: harness digests differ (aaa vs bbb)" in capsys.readouterr().out


def test_equal_bytes_and_instructions_with_other_clocks_is_a_pad_parity_candidate_and_still_fails():
    key, head, base = "p/bq16/Os", row(1001, 100), row(1000, 100)
    assert cellcheck.pad_parity({key: head}, {key: base}) == {key}
    assert cellcheck.check({key: head}, {key: base})
    assert cellcheck.pad_parity({key: row(1001, 101)}, {key: base}) == set()

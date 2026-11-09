import pytest

import bench
import worse_cells


def folder(root, name, expected=None, mark=None):
    where = root / name
    where.mkdir(parents=True)
    if mark is not None:
        (where / "bench.toml").write_text(f"references = {mark}\n")
    if expected is not None:
        (where / "expected.toml").write_text(expected)


def test_a_program_under_parity_finds_its_references(tmp_path):
    """The first table skipped the 11 parity programs: it looked for bench/parity/expected.toml, not bench/parity/<name>/."""
    folder(tmp_path, "parity/loop", "[ow.O2]\ncode_bytes = 171\nclocks = 900\n")
    rows, none, _ = worse_cells.worse({"parity/loop/c/O2": {"code_bytes": 202, "clocks": 900}}, tmp_path)
    assert rows == [("parity/loop/c/O2", ["+0 clk +31 B vs ow"])] and none == []


def test_a_program_without_references_is_listed_as_no_ref(tmp_path):
    folder(tmp_path, "sfm16_dbg", mark="false")
    rows, none, _ = worse_cells.worse({"sfm16_dbg/c/O2": {"code_bytes": 10, "clocks": 10}}, tmp_path)
    assert rows == [] and none == ["sfm16_dbg"]


def test_a_program_with_no_lookup_and_no_mark_fails_loudly(tmp_path):
    folder(tmp_path, "mystery")
    with pytest.raises(worse_cells.NoReferenceLookup):
        worse_cells.worse({"mystery/c/O2": {"code_bytes": 10, "clocks": 10}}, tmp_path)


def test_at_o2_a_byte_for_two_clocks_is_a_trade_not_a_loss(tmp_path):
    """parity/qlight O2 read +80.5% bytes (388 vs 215) beside the best clocks: -229 clk for +172 B passes at 500 milliclocks per byte."""
    folder(tmp_path, "parity/qlight", "[bcc.O2]\ncode_bytes = 216\nclocks = 617\n[bcc.Os]\ncode_bytes = 216\nclocks = 617\n")
    ours = {"code_bytes": 388, "clocks": 388}
    assert worse_cells.worse({"parity/qlight/c/O2": ours}, tmp_path, 500)[0] == []
    assert worse_cells.worse({"parity/qlight/c/Os": ours}, tmp_path, 500)[0] == [("parity/qlight/c/Os", ["B 388 vs 216(bcc) +79.6%"])]


def test_the_byte_price_is_the_one_bench_uses(tmp_path, monkeypatch):
    """worse_cells carried its own 500: a changed timing sheet would have moved the gate and not this table."""
    assert worse_cells.rate() == bench.rate_for("c", "O2") > 0
    sheet = tmp_path / "timings.times"
    sheet.write_text("speed_milliclocks_per_byte 2000 2000\n")
    monkeypatch.setattr(bench, "M16_TIMINGS", sheet)
    assert worse_cells.rate() == 2000
    folder(tmp_path, "parity/qlight", "[bcc.O2]\ncode_bytes = 216\nclocks = 617\n")
    assert worse_cells.worse({"parity/qlight/c/O2": {"code_bytes": 388, "clocks": 388}}, tmp_path, worse_cells.rate())[0] != []


def test_a_missing_reference_column_is_reported_not_skipped(tmp_path):
    """z80's table took the next-best SDCC column when -ma was missing and understated the gap by up to 1.7 kB. Here tc has no stored cell: the
    best of ow, bcc and tcpp stands in, and the row says so."""
    folder(tmp_path, "huge", "[ow.Os]\ncode_bytes = 200\nclocks = 10\n[bcc.Os]\ncode_bytes = 190\nclocks = 10\n[tcpp.Os]\ncode_bytes = 250\nclocks = 10\n")
    rows, _, missing = worse_cells.worse({"huge/c/Os": {"code_bytes": 195, "clocks": 10}}, tmp_path, 500)
    assert rows == [("huge/c/Os", ["B 195 vs 190(bcc) +2.6%"])]
    assert missing == ["huge/c/Os: no stored tc"]


def test_a_constant_table_kept_outside_the_code_counts(tmp_path):
    """Code bytes alone hid a table in data: ours 100 B code + 300 B data read as better than a reference with 150 B code + 0 B data. m16's worse counts
    (c O1 4, O2 7, Os 2) were text-only until data and static (data + bss) joined the judgement."""
    folder(tmp_path, "tab", "[bcc.O1]\ncode_bytes = 150\nclocks = 10\ndata_bytes = 0\nbss_bytes = 0\n[bcc.O2]\ncode_bytes = 150\nclocks = 10\ndata_bytes = 0\nbss_bytes = 0\n")
    ours = {"code_bytes": 100, "clocks": 10, "data_bytes": 300, "bss_bytes": 0}
    for level in ("O1", "O2"):
        rows = worse_cells.worse({f"tab/c/{level}": ours}, tmp_path, 500)[0]
        assert rows and "data B 300 vs 0(bcc)" in "; ".join(rows[0][1]), level


def test_bss_counts_only_through_static(tmp_path):
    """A zero table moved from data to bss leaves static equal: not worse; extra bss is worse as static, not as its own axis."""
    folder(tmp_path, "z", "[ow.Os]\ncode_bytes = 100\nclocks = 10\ndata_bytes = 40\nbss_bytes = 0\n")
    moved = {"code_bytes": 100, "clocks": 10, "data_bytes": 0, "bss_bytes": 40}
    assert worse_cells.worse({"z/c/Os": moved}, tmp_path, 500)[0] == []
    grown = {**moved, "bss_bytes": 60}
    findings = worse_cells.worse({"z/c/Os": grown}, tmp_path, 500)[0][0][1]
    assert findings == ["static B 60 vs 40(ow) +50.0%"]

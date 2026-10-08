"""The vsgcc report's summary: the geomean hid hanoi's 2.6x behind twenty programs near 1.0."""
import pytest

import table


def programs(ratios):
    """llrm at `ratio` times gcc and clang on every counter, at both levels."""
    one = lambda n: {'ins': n, 'clocks': n, 'code': n}
    return {name: {'llrm': one(100 * r), 'llrmOs': one(100 * r), 'gccO2': one(100), 'clangO2': one(200), 'gccOs': one(100), 'clangOs': one(200)} for name, r in ratios.items()}


def test_the_summary_names_the_worst_program_beside_each_geomean():
    lines = table.summary(programs({'a': 1.0, 'b': 4.0, 'c': 1.0}))
    assert "geomean llrm/best O2 clocks: 1.59" in lines
    assert "worst llrm/best O2 clocks: 4.00 (b)" in lines
    assert "worst llrm/best Os code: 4.00 (b)" in lines
    assert len(lines) == 12


def test_a_program_without_a_variant_is_refused_not_averaged():
    """An unresolved symbol left the harness without a row, and the table averaged what remained."""
    P = programs({'a': 1.0, 'b': 2.0})
    del P['b']['clangOs']
    with pytest.raises(AssertionError, match="clangOs"):
        table.complete(P)


def test_a_stub_built_from_another_stub_s_is_refused(tmp_path, monkeypatch):
    """A work directory kept from before stub.s grew `report_` linked nothing: unresolved report_, and a stale table.
    Its age is no test (a checkout dates stub.s after any build): the source it was built from is."""
    import harness
    (tmp_path / "stub.elf").write_bytes(b"")
    monkeypatch.setattr(harness, "OUT", tmp_path)
    with pytest.raises(AssertionError, match="not built from this stub.s"):
        harness.fresh_stub()
    (tmp_path / "stub.elf.src").write_text("an older stub.s")
    with pytest.raises(AssertionError, match="not built from this stub.s"):
        harness.fresh_stub()
    harness.record_stub(tmp_path)
    assert harness.fresh_stub() == tmp_path / "stub.elf"

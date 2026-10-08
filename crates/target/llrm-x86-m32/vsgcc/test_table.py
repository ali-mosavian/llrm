"""The vsgcc report's summary: the geomean hid hanoi's 2.6x behind twenty programs near 1.0."""
import pytest

import table


def programs(ratios):
    """llrm at `ratio` times gcc (and half of clang) on every counter, at every level."""
    one = lambda n: {'ins': n, 'clocks': n, 'code': n}
    return {name: {table.llrm(l): one(100 * r) for l in table.LEVELS} | {f'gcc{l}': one(100) for l in table.LEVELS} | {f'clang{l}': one(200) for l in table.LEVELS} for name, r in ratios.items()}


def test_the_summary_names_the_worst_program_beside_each_geomean():
    lines = table.summary(programs({'a': 1.0, 'b': 4.0, 'c': 1.0}))
    assert "geomean llrm/gcc O2 clocks: 1.59" in lines
    assert "worst llrm/gcc O2 clocks: 4.00 (b)" in lines
    assert "worst llrm/gcc Os code: 4.00 (b)" in lines
    assert "worst llrm/gcc O3 ins: 4.00 (b)" in lines
    assert len(lines) == 48


def test_a_program_without_a_variant_is_refused_not_averaged():
    """An unresolved symbol left the harness without a row, and the table averaged what remained."""
    P = programs({'a': 1.0, 'b': 2.0})
    del P['b']['clangO3']
    with pytest.raises(AssertionError, match="clangO3"):
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


def test_the_x_kernels_are_summarised_apart_from_the_bench_programs():
    """One summary over both hid the kernels' worst rows behind the bench programs' geomean (or the reverse)."""
    lines = table.grouped(programs({'a': 1.0, 'b': 2.0, 'x_c': 4.0, 'x_d': 1.0}))
    assert "bench (n=2) worst llrm/gcc O2 ins: 2.00 (b)" in lines
    assert "x_ kernels (n=2) worst llrm/gcc O2 ins: 4.00 (x_c)" in lines
    assert len(lines) == 96


def test_the_ratio_is_against_one_compiler_not_the_minimum_of_two():
    """x_iir read 2.36 against the best of gcc and clang: clang's three imuls had the fewest instructions and gcc's chains the fewest
    clocks (clang runs twice gcc's), so the 'best' row was the instructions of one compiler beside the clocks of the other."""
    mixed = {'x': {**{table.llrm(l): {'ins': 150, 'clocks': 150, 'code': 100} for l in table.LEVELS},
                   **{f'gcc{l}': {'ins': 200, 'clocks': 100, 'code': 100} for l in table.LEVELS},
                   **{f'clang{l}': {'ins': 100, 'clocks': 200, 'code': 100} for l in table.LEVELS}}}
    lines = table.summary(mixed)
    assert "worst llrm/gcc O2 ins: 0.75 (x)" in lines
    assert "worst llrm/gcc O2 clocks: 1.50 (x)" in lines
    assert "worst llrm/clang O2 ins: 1.50 (x)" in lines
    assert not any("best" in line for line in lines)
    assert lines.index("geomean llrm/gcc O2 clocks: 1.50") < lines.index("geomean llrm/gcc O2 ins: 0.75"), "clocks first"

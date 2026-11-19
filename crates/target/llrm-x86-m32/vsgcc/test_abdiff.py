"""abdiff: two result sets of the same programs, a row worse in any one counter is listed."""
import abdiff


def row(clocks=100, ins=50, code=40):
    return {'clocks': clocks, 'ins': ins, 'code': code}


def test_a_row_whose_clocks_alone_regress_is_listed_worse():
    """A bytes-only comparison printed nothing for a clock-only regression (6502, #6582)."""
    base = {('p', 'llrmOs'): row()}
    new = {('p', 'llrmOs'): row(clocks=106)}
    assert abdiff.worse(base, new) == [(('p', 'llrmOs'), 'clocks', 100, 106)]


def test_a_row_whose_instructions_alone_regress_is_listed_worse():
    assert abdiff.worse({('p', 'v'): row()}, {('p', 'v'): row(ins=51)}) == [(('p', 'v'), 'ins', 50, 51)]


def test_a_row_better_in_one_counter_and_worse_in_another_is_listed_for_the_worse_one():
    got = abdiff.worse({('p', 'v'): row()}, {('p', 'v'): row(code=30, clocks=101)})
    assert got == [(('p', 'v'), 'clocks', 100, 101)]


def test_an_unchanged_row_and_a_row_with_an_error_are_not_listed():
    assert abdiff.worse({('p', 'v'): row(), ('q', 'v'): {'error': 'x'}}, {('p', 'v'): row(), ('q', 'v'): row()}) == []


def _repo(tmp_path):
    import subprocess

    def git(*a):
        return subprocess.run(['git', '-C', str(tmp_path), *a], check=True, capture_output=True, text=True).stdout.strip()

    git('init', '-q')
    git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '--allow-empty', '-m', 'base')
    base = git('rev-parse', 'HEAD')
    git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '--allow-empty', '-m', 'head')
    return git, base, git('rev-parse', 'HEAD')


def test_a_head_not_sitting_on_its_base_is_refused(tmp_path):
    """w12's CGP-MATH-1 numbers compared heads on 4e5bb10309 with a base at 82acead13f: all retracted."""
    git, base, head = _repo(tmp_path)
    (tmp_path / 'a').write_bytes(b'1')
    (tmp_path / 'b').write_bytes(b'2')
    rows = {('p', 'v'): row()}
    assert abdiff.refusals(str(tmp_path), base, head, tmp_path / 'a', tmp_path / 'b', rows, rows) == []
    git('-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '--allow-empty', '-m', 'later')
    later = git('rev-parse', 'HEAD')
    assert abdiff.refusals(str(tmp_path), later, head, tmp_path / 'a', tmp_path / 'b', rows, rows)


def test_identical_binaries_and_empty_counts_are_refused(tmp_path):
    git, base, head = _repo(tmp_path)
    (tmp_path / 'a').write_bytes(b'1')
    (tmp_path / 'b').write_bytes(b'1')
    rows = {('p', 'v'): row()}
    assert abdiff.refusals(str(tmp_path), base, head, tmp_path / 'a', tmp_path / 'b', rows, rows)
    (tmp_path / 'b').write_bytes(b'2')
    assert abdiff.refusals(str(tmp_path), base, head, tmp_path / 'a', tmp_path / 'b', {}, rows)

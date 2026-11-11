"""matrix: the scoreboard TSV's bytes are the gate's (.text plus .data/.rodata), the same for every compiler column."""
import matrix


def r(prog, variant, code, data):
    return {'prog': prog, 'variant': variant, 'code': code, 'data': data, 'clocks': 10, 'ins': 5}


def test_a_rows_bytes_count_the_constants_the_kernel_loads():
    """The m32 matrix of 2026-11-05 listed `code` alone: nbody -Os ours 420 B against gcc 356 B, where text+data is 444 against 392."""
    rows = matrix.rows([r('nbody', 'llrmOs', 420, 24), r('nbody', 'gccOs', 356, 36)])
    assert [(x[3], x[4]) for x in rows] == [('gcc', '392'), ('ours', '444')]


def test_a_row_without_data_counts_its_code_alone():
    assert matrix.rows([r('p', 'clangO2', 100, 0)])[0][4] == '100'


def test_the_eh_frame_is_a_column_of_its_own_and_not_in_the_bytes():
    """The .eh_frame of float_bits (ours 84 B against the pin's 48) was seen only in a card: the matrix reports it and never adds it to `code`."""
    row = matrix.rows([{**r('p', 'gccO2', 100, 8), 'eh_frame': 52}, {**r('p', 'llrm', 100, 8)}])
    assert [(x[3], x[4], x[8]) for x in row] == [('gcc', '108', '52'), ('ours', '108', '')]

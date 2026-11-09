import bench6502_pads as b

LOG = "| kernel | ours |\n|---|---|\n| crc16 | 40 B, 1000 cy, 10 in |\n| lz4 | 479 B linked, 90 B body, 2000 cy |\n"


def test_cycles_are_the_mean_of_the_layouts():
    other = LOG.replace("1000 cy", "1100 cy").replace("2000 cy", "2200 cy")
    out = b.merged([LOG, other])
    assert "40 B, 1050 cy, 10 in" in out and "479 B linked, 90 B body, 2100 cy" in out, out


def test_a_single_layout_is_its_own_mean():
    assert b.merged([LOG]) == LOG


def test_the_least_layout_is_available_next_to_the_mean():
    other = LOG.replace("1000 cy", "1100 cy").replace("2000 cy", "1900 cy")
    out = b.merged([LOG, other], min)
    assert "40 B, 1000 cy, 10 in" in out and "479 B linked, 90 B body, 1900 cy" in out, out


def test_logs_that_print_their_tables_in_another_order_merge_by_kernel_and_column():
    """Tests run in parallel and print a table when they finish: the O2 variant logs of one run listed their tables in
    two orders, and the merge by position averaged the cycles of different kernels (lz4's SDCC clocks read 10,034 against
    39,000 in every log)."""
    one = "| kernel | a |\n|---|---|\n| lz4 | 448 B, 39000 cy |\n| crc | 66 B, 3000 cy |\n"
    other = "| kernel | a |\n|---|---|\n| crc | 66 B, 3200 cy |\n| lz4 | 448 B, 39200 cy |\n"
    out = b.merged([one, other])
    assert "448 B, 39100 cy" in out and "66 B, 3100 cy" in out, out


def test_a_cell_whose_bytes_move_between_logs_is_refused():
    import pytest
    one = "| kernel | a |\n|---|---|\n| lz4 | 448 B, 39000 cy |\n"
    with pytest.raises(AssertionError, match="bytes differ"):
        b.merged([one, one.replace("448 B", "449 B")])

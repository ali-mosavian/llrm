import bench6502_pads as b

LOG = "| kernel | ours |\n|---|---|\n| crc16 | 40 B, 1000 cy, 10 in |\n| lz4 | 479 B linked, 90 B body, 2000 cy |\n"


def test_cycles_are_the_mean_of_the_layouts():
    other = LOG.replace("1000 cy", "1100 cy").replace("2000 cy", "2200 cy")
    out = b.merged([LOG, other])
    assert "40 B, 1050 cy, 10 in" in out and "479 B linked, 90 B body, 2100 cy" in out, out


def test_a_single_layout_is_its_own_mean():
    assert b.merged([LOG]) == LOG

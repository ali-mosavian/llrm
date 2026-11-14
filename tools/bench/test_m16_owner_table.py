import datetime
from pathlib import Path

import m16_owner_table as table


def cell(code, clocks, data=0, bss=0):
    return {"code_bytes": code, "clocks": clocks, "data_bytes": data, "bss_bytes": bss}


def refs(program, level, code, clocks):
    return {f"{program}/{name}/{level}": cell(code + at, clocks + at) for at, name in enumerate(table.C_REFS)}


def matmul(level="O1"):
    """matmul at -O1 as measured at -march=i386 (2026-11-10): Nib 334 B, the C compilers 298 (bcc) without the init and 352 with it."""
    out = {f"matmul/nib/{level}": cell(334, 24150), f"matmul/c/{level}": cell(293, 23467), f"matmul~init/c/{level}": cell(339, 24151)}
    out |= {f"matmul/bcc/{level}": cell(298, 35138), f"matmul/ow/{level}": cell(401, 56145), f"matmul/tc/{level}": cell(398, 69951), f"matmul/tcpp/{level}": cell(339, 66900)}
    out |= {f"matmul~init/bcc/{level}": cell(352, 36092), f"matmul~init/ow/{level}": cell(443, 56515), f"matmul~init/tc/{level}": cell(455, 70334), f"matmul~init/tcpp/{level}": cell(393, 67854)}
    return out


def by_row(measured):
    found, _ = table.rows(measured)
    return {(r[0], r[1], r[2]): r for r in found}


def test_a_nib_row_is_judged_against_the_c_compilers_on_the_twin_with_its_semantics():
    """matmul nib -O1 read WORSE (334 > 298, bcc) for a bar that never zero-initialised the arrays Nib does; with the init (bcc 352) it is OK."""
    row = by_row(matmul())[("matmul", "nib", "O1")]
    assert row[10] == "OK", row
    assert (row[7], row[8]) == (352, 36092), "the best external is the twin's"
    assert (row[12], row[13]) == (298, 35138), "the plain source's best stays beside it"
    assert "[bar matmul~init]" in row[11]


def test_a_nib_program_without_a_twin_source_keeps_the_plain_bar():
    """sieve has no bench/sieve/twin: its Nib row is judged on the plain source."""
    only = {"sieve/nib/O1": cell(120, 5000), "sieve/c/O1": cell(110, 4900), **refs("sieve", "O1", 111, 4800)}
    row = by_row(only)[("sieve", "nib", "O1")]
    assert row[10] == "WORSE" and "clocks 5000>4800(ow)" in row[11], row


def test_a_nib_program_whose_twin_is_not_measured_is_not_judged_on_the_plain_source():
    """jpeg_single, stft_single and four more Nib cells read WORSE (code 3601>2784, ow) on 2026-11-13 because the table held no twin rows: the plain
    bar lacks the bounds checks and the I/O functions the Nib object holds. Against the measured twin (3980) the cell is better."""
    nib, plain = {"jpeg_single/nib/O1": cell(3542, 90_000_000), "jpeg_single/c/O1": cell(2833, 84_000_000)}, refs("jpeg_single", "O1", 2784, 118_000_000)
    assert by_row(nib | plain)[("jpeg_single", "nib", "O1")][10] == "NO_TWIN"
    twin = {"jpeg_single~bounds/c/O1": cell(3134, 87_000_000), **refs("jpeg_single~bounds", "O1", 3980, 156_000_000)}
    row = by_row(nib | plain | twin)[("jpeg_single", "nib", "O1")]
    assert row[10] == "OK" and row[7] == 3980, row


def test_the_c_row_keeps_its_own_bar_and_a_twin_row_is_not_judged():
    found = by_row(matmul())
    assert found[("matmul", "c", "O1")][10] == "OK"
    twin = found[("matmul~init", "c", "O1")]
    assert twin[10] == "TWIN" and (twin[7], twin[8]) == ("", ""), twin


def test_the_header_names_the_dist_its_md5_the_harness_and_the_time(tmp_path):
    for name in ("llrm-llc", "llrm-cc", "llrm-nib"):
        (tmp_path / name).write_bytes(name.encode())
    text = table.render(matmul(), tmp_path, "abc123", datetime.datetime(2026, 11, 10, 9, 30, tzinfo=datetime.UTC))
    head = text.splitlines()
    assert head[0] == "# x86-m16 owner table 2026-11-10T09:30Z (date -u)"
    assert head[1].startswith(f"# dist {tmp_path}; md5: ") and head[1].count("  ") == 3, head[1]
    assert head[2].startswith("# harness commit abc123;")


def stft(level="O1", skeleton=None):
    """stft_fixed at -O1 on the pair d7f7c5758f (2026-11-13): Nib 1592 B gross, bcc plain C 1224, the twin's own bar higher."""
    nib = cell(1592, 149_000_000) | ({"io_skeleton_bytes": skeleton} if skeleton else {})
    out = {f"stft_fixed/nib/{level}": nib, f"stft_fixed~init/c/{level}": cell(1250, 143_000_000)}
    out |= {f"stft_fixed/bcc/{level}": cell(1224, 294_000_000), f"stft_fixed~init/bcc/{level}": cell(1500, 296_000_000)}
    return out


def test_a_nib_cell_with_file_io_is_read_net_of_the_fixed_std_io_skeleton_against_plain_c():
    """m16-b netted stft_fixed by hand with the skeleton its own dist measured (383, 358, 203 on one pair, 391/366/202 on another): the cell moved with the
    compiler under test. Net is the gross less the stored bytes, 1592 - 391 = 1201 against bcc's 1224: -23."""
    row = by_row(stft())[("stft_fixed", "nib", "O1")]
    assert (row[14], row[15]) == (1201, -23), row


def test_inlining_a_skeleton_function_does_not_move_the_net_cell():
    """The build's own skeleton (io_skeleton_bytes) falls from 391 to 340 when a std.io function is inlined into its callers; the net cell is gross less the
    stored bytes either way, so only what the inlining does to the program's bytes (here none) moves it. Judging from the build's measure made it 1592 - 340."""
    kept = by_row(stft(skeleton=391))[("stft_fixed", "nib", "O1")]
    inlined = by_row(stft(skeleton=340))[("stft_fixed", "nib", "O1")]
    assert kept[14] == inlined[14] == 1201


def test_a_nib_cell_without_file_io_has_no_net():
    measured = {"fib/nib/O1": cell(90, 1000), "fib/ow/O1": cell(80, 1000), "fib/bcc/O1": cell(70, 1000)}
    row = by_row(measured)[("fib", "nib", "O1")]
    assert (row[14], row[15]) == ("", "")


def test_the_stored_skeleton_is_refused_once_the_skeleton_program_changed(tmp_path, monkeypatch):
    """Stored bytes of a program that was edited since are another program's: re-measure, never subtract them."""
    import shutil

    shutil.copytree(table.bench.IO_SKELETON, tmp_path / "s")
    (tmp_path / "s" / "io_skeleton.nib").write_text((tmp_path / "s" / "io_skeleton.nib").read_text() + "\n")
    monkeypatch.setattr(table.bench, "IO_SKELETON", tmp_path / "s")
    try:
        table.bench.fixed_io_skeleton()
    except ValueError as error:
        assert "measure again" in str(error)
    else:
        raise AssertionError("an edited skeleton program kept its stored bytes")


def test_at_o1_a_cell_over_the_bar_in_bytes_only_is_ok_and_reports_the_bytes():
    """Ruling 2026-11-14: O1/O2/O3 are judged on clocks. A Nib cell 10 B over the best bar and 100 clocks under it was WORSE (strict code); it is OK with the bytes reported."""
    only = {"sieve/nib/O1": cell(121, 4700), "sieve/c/O1": cell(110, 4900), **refs("sieve", "O1", 111, 4800)}
    row = by_row(only)[("sieve", "nib", "O1")]
    assert row[10] == "OK" and "bytes +10 (reported)" in row[11], row


def test_at_o2_a_clock_gain_no_longer_buys_bytes_and_a_clock_loss_is_worse():
    """The 1 clk = 2 B pricing is gone: O2 over the bar in clocks is WORSE however many bytes it saves."""
    only = {"sieve/nib/O2": cell(90, 4900), "sieve/c/O2": cell(110, 4900), **refs("sieve", "O2", 111, 4800)}
    row = by_row(only)[("sieve", "nib", "O2")]
    assert row[10] == "WORSE" and row[11].startswith("clocks 4900>4800(ow)"), row


def test_at_os_both_bytes_and_clocks_must_match_the_bar():
    only = {"sieve/nib/Os": cell(112, 4700), "sieve/c/Os": cell(110, 4900), **refs("sieve", "Os", 111, 4800)}
    row = by_row(only)[("sieve", "nib", "Os")]
    assert row[10] == "WORSE" and "total 112>111(ow)" in row[11] and "clocks" not in row[11], row


def test_at_os_a_data_table_that_replaces_code_is_total_bytes_not_a_data_miss():
    """switch at -Os: 288 B of code and a 12 B lookup table against bcc's 559 B of code and no data. Graded per axis the 12 > 0 data made four cells WORSE; the ruling's bytes are
    code + data (oversight, 2026-11-14): 300 <= 559, OK."""
    only = {"sieve/c/Os": cell(288, 500000, 12), **refs("sieve", "Os", 559, 960000)}
    row = by_row(only)[("sieve", "c", "Os")]
    assert row[10] == "OK", row

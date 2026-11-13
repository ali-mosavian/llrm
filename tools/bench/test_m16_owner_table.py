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
    assert row[10] == "WORSE" and "code 120>111(ow)" in row[11], row


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

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


def test_the_table_labels_the_default_486_and_no_march_flag(tmp_path):
    """The 2026-11-16 census ran with no -march (the m16 default CPU, the 486: the gate's) and its table said `flags -march=i386` and `ours_isa i386`:
    an instrument that mislabels its own run (-march=i386 prices the 386 table and voided a census)."""
    for name in ("llrm-llc", "llrm-cc", "llrm-nib"):
        (tmp_path / name).write_bytes(name.encode())
    text = table.render(matmul(), tmp_path, "abc123", datetime.datetime(2026, 11, 17, 9, 30, tzinfo=datetime.UTC))
    lines = text.splitlines()
    assert "-march=i386" not in lines[2] and "default CPU" in lines[2], lines[2]
    columns = lines[next(at for at, line in enumerate(lines) if line.startswith("prog\t"))].split("\t")
    rows = [line.split("\t") for line in lines if line and not line.startswith(("#", "prog\t"))]
    assert {row[columns.index("ours_isa")] for row in rows} == {"i486"}, rows


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


def test_at_os_a_basic_cell_is_code_plus_static_so_a_table_that_replaces_code_is_ok():
    """switch bas at -Os: 396 B of code and 27 B static against bc's 1128 B and 19 B: total 423 vs 1147. Graded on static alone, 27 > 19 made it WORSE (oversight, 2026-11-14)."""
    measured = {"switch/bas/Os": cell(396, 604295, 27), "switch/bc/O2": cell(1128, 4221878, 19), "switch/pds71/O2": cell(1070, 4060681, 19), "switch/vbdos/O2": cell(1080, 4164707, 19)}
    row = by_row(measured)[("switch", "bas", "Os")]
    assert row[10] == "OK", row


def test_the_header_counts_the_reference_cells_measured_of_those_asked(tmp_path):
    """The 18de C table had `ow` bars only (bcc, tc, tcpp: "no object" in every cell) and nothing in it said so."""
    for name in ("llrm-llc", "llrm-cc", "llrm-nib"):
        (tmp_path / name).write_bytes(name.encode())
    measured = {"sieve/c/Os": cell(110, 4900), "sieve/ow/Os": cell(111, 4800), "sieve/bcc/Os": {"error": "bcc: no object"}}
    text = table.render(measured, tmp_path, "abc123", datetime.datetime(2026, 11, 10, 9, 30, tzinfo=datetime.UTC))
    line = next(row for row in text.splitlines() if row.startswith("# reference cells"))
    assert "ow=1/1" in line and "bcc=0/1" in line and "tc=0/1" in line, line


def test_a_table_is_refused_when_a_reference_measured_no_cell_at_all(tmp_path):
    import json

    run = tmp_path / "run.json"
    run.write_text(json.dumps({"sieve/c/Os": cell(110, 4900), "sieve/ow/Os": cell(111, 4800), "sieve/bcc/Os": {"error": "bcc: no object"}}))
    out = tmp_path / "out.tsv"
    assert table.main([str(run), str(tmp_path), str(out), "abc123"]) == 3
    assert not out.exists()


def test_a_borland_job_that_built_nothing_says_why(tmp_path):
    """A timed-out or silent job wrote "bcc: no object" for every program; now the cell and stderr carry the job's own reason."""
    import subprocess

    import bench

    assert bench.borland_job_failure("timed out after 1800 s: bcc.sh", tmp_path, ["R000"]) == "timed out after 1800 s: bcc.sh"
    silent = subprocess.CompletedProcess(["bcc.sh"], 1, "", "dosbox: no display")
    assert "exited 1" in bench.borland_job_failure(silent, tmp_path, ["R000"]) and "no display" in bench.borland_job_failure(silent, tmp_path, ["R000"])
    (tmp_path / "R000.OBJ").write_bytes(b"x")
    assert bench.borland_job_failure(silent, tmp_path, ["R000", "R001"]) is None


def test_a_big_zero_array_stored_by_a_reference_and_held_in_bss_by_ours_is_a_convention_cell_not_a_win():
    """`huge`: bcc, tcpp and ow store 80,400 B of zeros as data, ours holds them in bss; the Os byte gap is the object format, not code quality."""
    measured = {"huge/c/Os": cell(253, 5000, data=12, bss=80400), "huge/ow/Os": cell(511, 6000, data=80400), "huge/bcc/Os": cell(338, 6100, data=80400)}
    row = by_row(measured)[("huge", "c", "Os")]
    assert row[10] == "CONVENTION" and "object-format convention" in row[11], row
    small = {"z/c/Os": cell(50, 100, data=0, bss=100), "z/ow/Os": cell(60, 100, data=100)}
    assert by_row(small)[("z", "c", "Os")][10] == "OK"


LIMIT = {"error": "stopped at the 300000000-instruction limit (LLRM_ICOUNT_LIMIT raises it)"}


def test_a_reference_stopped_at_the_instruction_limit_is_printed_as_a_bound_and_never_judged():
    """The Os C table read a reference cell stopped at 3e8 instructions as no data: the bar looked complete. It is a bound: shown, not a bar."""
    measured = {"p/c/Os": cell(300, 1000)} | refs("p", "Os", 400, 2000) | {"p/bcc/Os": LIMIT}
    row = by_row(measured)[("p", "c", "Os")]
    assert row[10] == "OK" and ">= 3e8" in row[11] and "bcc" in row[11], row


def test_a_cell_of_ours_stopped_at_the_limit_is_a_bound_never_ok_or_worse():
    row = by_row({"p/c/Os": LIMIT} | refs("p", "Os", 400, 2000))[("p", "c", "Os")]
    assert row[10] not in ("OK", "WORSE") and ">= 3e8" in row[11], row


def test_a_reference_that_miscompiled_is_named_in_the_note_as_no_bar_not_read_as_a_win():
    """c-Os: six reference cells printed wrong values and one failed to compile; they were only a count in refs_missing."""
    wrong = {"error": "prints ['64'], want ['38421']"}
    row = by_row({"p/c/Os": cell(300, 1000)} | refs("p", "Os", 400, 2000) | {"p/tc/Os": wrong})[("p", "c", "Os")]
    assert "wrong output, not a bar: tc (prints ['64'], want ['38421'])" in row[11], row


def test_a_bounds_twin_whose_references_all_print_wrong_values_has_no_bar_and_a_dated_cause():
    """stft_fixed~bounds: bcc, tc and tcpp print another value than the host oracle. They are listed as wrong output, with the cause's date, owner and exit; the cell has no bar."""
    wrong = {"error": "prints ['219192480'], want ['374810179']"}
    measured = {"stft_fixed/nib/Os": cell(300, 1000), "stft_fixed~bounds/c/Os": cell(310, 1100)} | {f"stft_fixed~bounds/{name}/Os": wrong for name in ("bcc", "tc", "tcpp")}
    row = by_row(measured)[("stft_fixed", "nib", "Os")]
    assert row[10] == "NO_BAR", row
    assert "wrong output, not a bar: bcc (prints ['219192480'], want ['374810179'])" in row[11] and "owner w12, exit 2026-12-13" in row[11], row


def test_the_header_names_the_levels_the_table_holds_not_a_fixed_list(tmp_path):
    """The merged O1/O2/Os table said `--opt O1 O2 O3 Os`: a reader would take O3 for covered."""
    text = table.render(matmul("O1") | matmul("Os"), tmp_path, "abc123", datetime.datetime(2026, 11, 10, tzinfo=datetime.UTC))
    harness = text.splitlines()[2]
    assert "--opt O1 Os " in harness and "O3" not in harness and "O2" not in harness, harness


def test_a_program_marked_references_false_is_no_bar_by_construction_not_a_missing_bar():
    """sfm16_float_add (bench.toml: `references = false`: the soft-float library is ours, no reference compiler builds it) read NO_BAR like a cell whose
    references failed to measure: 27 of the table's NO_BAR rows were this. A program with no reference by design is listed, never counted."""
    only = {"sfm16_float_add/c/Os": cell(700, 9000)}
    assert by_row(only)[("sfm16_float_add", "c", "Os")][10] == "NO_BAR_BY_CONSTRUCTION"
    assert by_row({"sieve/c/Os": cell(100, 900)})[("sieve", "c", "Os")][10] == "NO_BAR", "a program that should have references and has none stays NO_BAR"

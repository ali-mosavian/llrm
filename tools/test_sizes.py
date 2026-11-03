"""`uv run --project tools --with pytest python -m pytest tools/test_sizes.py`

sizes.py's instrument: it counts code, not the records the linker consumes (rule 3)."""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import pytest  # noqa: E402
import sizes  # noqa: E402
from sizes import code_bytes, code_size, data_bytes, lib_bytes, selected  # noqa: E402


def record(kind: int, body: bytes) -> bytes:
    return bytes([kind]) + (len(body) + 1).to_bytes(2, "little") + body + b"\0"


def name(text: str) -> bytes:
    return bytes([len(text)]) + text.encode()


def object_with(code: int, extra: bytes = b"") -> bytes:
    """THEADR, LNAMES (_TEXT, CODE, _DATA, DATA), a code and a data SEGDEF, then `extra`."""
    lnames = name("_TEXT") + name("CODE") + name("_DATA") + name("DATA")
    segment = lambda size, at: bytes([0x48]) + size.to_bytes(2, "little") + bytes([at, at + 1, 1])  # noqa: E731
    return record(0x80, name("x.c")) + record(0x96, lnames) + record(0x98, segment(code, 1)) + record(0x98, segment(40, 3)) + extra + record(0x8A, b"\0")


def test_the_code_segment_is_counted_and_the_data_segment_is_not(tmp_path):
    path = tmp_path / "a.obj"
    path.write_bytes(object_with(123))
    assert code_bytes(path) == 123


def test_a_basic_objects_code_class_counts(tmp_path):
    """The QB frontend's objects name the class BC_CODE: they read 0 bytes while only CODE counted."""
    path = tmp_path / "b.obj"
    lnames = name("") + name("BC_CODE") + name("DIVMOD_CODE") + name("DATA")
    segment = bytes([0x48]) + (77).to_bytes(2, "little") + bytes([3, 2, 1])
    path.write_bytes(record(0x80, name("x")) + record(0x96, lnames) + record(0x98, segment))
    assert code_bytes(path) == 77


def test_a_change_to_fixups_alone_reads_zero_bytes(tmp_path):
    """sizes.py read the file's size: an inlined body naming a global once more grew lru.c by 78 bytes of
    FIXUPP records, with 18 more bytes of code."""
    plain, fixed = tmp_path / "plain.obj", tmp_path / "fixed.obj"
    plain.write_bytes(object_with(100))
    fixed.write_bytes(object_with(100, record(0x9C, b"\xc4\x01\x01\x54\x01" * 20)))
    assert fixed.stat().st_size > plain.stat().st_size
    assert code_bytes(fixed) - code_bytes(plain) == 0


def test_a_class_index_past_127_takes_two_bytes(tmp_path):
    """OMF indices from 0x80 up are two bytes: with 130 names the class index read one byte and the
    overlay index landed in its place."""
    path = tmp_path / "w.obj"
    lnames = b"".join(name(f"N{n}") for n in range(128)) + name("BC_CODE") + name("SEG")
    # class BC_CODE is name 129 (0x81 0x00 | 1: high bit set, then the low byte), segment SEG name 130.
    segment = bytes([0x48]) + (55).to_bytes(2, "little") + bytes([0x80, 130, 0x80, 129, 1])
    path.write_bytes(record(0x80, name("x")) + record(0x96, lnames) + record(0x98, segment))
    assert code_bytes(path) == 55


def segdef(size: int, segment: int, klass: int) -> bytes:
    return record(0x98, bytes([0x48]) + size.to_bytes(2, "little") + bytes([segment, klass, 1]))


def ledata(segment: int, length: int) -> bytes:
    return record(0xA0, bytes([segment, 0, 0]) + bytes(length))


def lnames(*texts: str) -> bytes:
    return record(0x96, b"".join(name(one) for one in texts))


def test_initialised_bytes_are_the_data_records_and_the_rest_of_the_segment_is_uninitialised(tmp_path):
    """Names 1..: _TEXT CODE _DATA DATA _BSS BSS. A 40-byte data segment loaded with 25 bytes and a 16-byte BSS
    with none read 40 initialised and 0 uninitialised when the SEGDEF length alone was counted."""
    path = tmp_path / "d.obj"
    path.write_bytes(record(0x80, name("x")) + lnames("_TEXT", "CODE", "_DATA", "DATA", "_BSS", "BSS") + segdef(10, 1, 2) + segdef(40, 3, 4) + segdef(16, 5, 6) + ledata(1, 10) + ledata(2, 25))
    assert data_bytes(path) == (25, 31)
    assert code_bytes(path) == 10


def test_a_basic_objects_runtime_cells_are_the_programs_data(tmp_path):
    """BC and llrm-qb put BC_DATA (class BC_VARS) and BC_DS, BC_SA (class BC_SEGS) in the object: the runtime reads
    them, but they are the program's bytes in the image."""
    path = tmp_path / "b.obj"
    path.write_bytes(record(0x80, name("x")) + lnames("R_CODE", "BC_CODE", "BC_DATA", "BC_VARS", "BC_DS", "BC_SEGS") + segdef(50, 1, 2) + segdef(14, 3, 4) + segdef(3, 5, 6) + ledata(2, 14) + ledata(3, 3))
    assert data_bytes(path) == (17, 0)


def test_the_stack_and_debug_records_are_not_the_programs_data(tmp_path):
    path = tmp_path / "s.obj"
    path.write_bytes(record(0x80, name("x")) + lnames("STACK", "STACK", "$$SYMBOLS", "DEBSYM", "$$TYPES", "DEBTYP") + segdef(4096, 1, 2) + segdef(300, 3, 4) + segdef(90, 5, 6))
    assert data_bytes(path) == (0, 0)


def test_iterated_data_counts_what_it_expands_to(tmp_path):
    """An LIDATA of 100 repeats of 4 bytes loads 400 bytes."""
    path = tmp_path / "i.obj"
    block = (100).to_bytes(2, "little") + (0).to_bytes(2, "little") + bytes([4, 1, 2, 3, 4])
    path.write_bytes(record(0x80, name("x")) + lnames("_DATA", "DATA") + segdef(400, 1, 2) + record(0xA2, bytes([1, 0, 0]) + block))
    assert data_bytes(path) == (400, 0)


def comdat(kind: int, segment: int, function: int, length: int, flags: int = 0, attributes: int = 0) -> bytes:
    """A COMDAT of `length` bytes, placed in `segment` and named by LNAMES entry `function`."""
    wide = kind == 0xC3
    return record(kind, bytes([flags, attributes, 0]) + bytes(4 if wide else 2) + bytes([0, 0, segment, function]) + bytes(length))


def test_a_function_per_comdat_counts_in_the_segment_it_is_placed_in(tmp_path):
    """PerProcedure writes each function as a COMDAT into an empty code SEGDEF; the segment read 0 code bytes and the
    bench's sizes of a flat program were the start-up program's (`COMDAT records, which segments() does not read`).
    A function in two records counts both; a 32-bit COMDAT is read as a 16-bit one."""
    path = tmp_path / "c.obj"
    path.write_bytes(
        record(0x80, name("x")) + lnames("_TEXT", "CODE", "f", "g", "h") + segdef(0, 1, 2) + comdat(0xC2, 1, 3, 30) + comdat(0xC2, 1, 3, 12, flags=1) + comdat(0xC3, 1, 4, 8) + comdat(0xC2, 1, 5, 5)
    )
    assert code_bytes(path) == 55


def test_a_library_function_counts_in_lib_bytes_and_not_in_code_bytes(tmp_path):
    """stft_fixed O2 read 2284 code bytes for a program of 1724: 560 of them were the standard library's instantiations in the Nib object
    (`G$7$std$io$File$open` and the rest), which C's extern I/O never shows. code_bytes is the program's own functions; lib_bytes the library's."""
    path = tmp_path / "lib.obj"
    path.write_bytes(
        record(0x80, name("x"))
        + lnames("_TEXT", "CODE", "G$7$_std$io$File$open$ABCD", "G$8$_std$os$write$ABCD", "_fft$ABCD", "G$9$helper$ABCD")
        + segdef(0, 1, 2)
        + comdat(0xC2, 1, 3, 90)
        + comdat(0xC2, 1, 4, 46)
        + comdat(0xC2, 1, 5, 656)
        + comdat(0xC2, 1, 6, 30)
    )
    assert (code_bytes(path), lib_bytes(path)) == (686, 136)


def test_an_object_without_library_functions_has_a_zero_lib_bytes_and_an_elf_one_has_none(tmp_path):
    path = tmp_path / "own.obj"
    path.write_bytes(record(0x80, name("x")) + lnames("_TEXT", "CODE", "f") + segdef(0, 1, 2) + comdat(0xC2, 1, 3, 12))
    assert (code_bytes(path), lib_bytes(path)) == (12, 0)
    elf = tmp_path / "a.o"
    elf.write_bytes(b"\x7fELF" + bytes(60))
    assert lib_bytes(elf) is None


def test_a_comdat_the_linker_places_is_refused_not_missed(tmp_path):
    """An allocation other than explicit leaves the segment to the linker: its bytes cannot be attributed."""
    path = tmp_path / "f.obj"
    path.write_bytes(record(0x80, name("x")) + lnames("_TEXT", "CODE", "f") + segdef(0, 1, 2) + record(0xC2, bytes([0, 1, 0, 0, 0, 0, 0, 3]) + bytes(9)))
    with pytest.raises(ValueError):
        code_bytes(path)


def test_a_linked_image_is_not_an_object(tmp_path):
    """Sizes come from the program's own object. An EXE holds the start-up and the libraries too, and reads as a size
    that is not the program's: it is refused."""
    path = tmp_path / "p.exe"
    path.write_bytes(b"MZ" + bytes(500))
    with pytest.raises(ValueError):
        code_bytes(path)


def test_a_basic_benchmark_without_a_dialect_is_built_as_qb45():
    """bench/grep read 1133 code bytes before and after inlining ASC and MID$: sizes.py built it with llrm-qb's own default,
    VBDOS, which keeps the calls, while tests/run and bench build it as QB 4.5."""
    from sizes import programs

    built = dict(programs(Path("/x"), demos=False))
    arguments = built["grep/grep.bas"]
    assert arguments[arguments.index("--dialect") + 1] == "qb45"
    assert "--dialect" not in built["grep/grep.c"] and "--dialect" not in built["grep/grep.nib"]


def test_sizes_reports_bytes_and_no_cost_estimate(monkeypatch, tmp_path, capsys):
    """The instruction and memory-operand columns were the backend's per-call `cost` estimate, summed. Across two builds they read
    40M -> 137M where the executed counts moved by under 1% (#390), inlining into a loop inflated them (#433), and tally.nib's was
    20x off (#481). Bytes are the only column left; executed counts are tools/bench's."""
    import sizes

    seen = {}

    def run(command, timeout, **options):
        seen.update(options.get("env", {}))
        return type("Done", (), {"stderr": ""})()

    monkeypatch.delenv("LLRM_DEBUG", raising=False)
    monkeypatch.setattr(sizes.cpulimit, "run", run)
    assert sizes.measure(["llrm-cc", "x.c"], "-O2") is None
    assert "LLRM_DEBUG" not in seen, "measure asked the compiler for its cost estimate"

    tables = iter([{"a/a.c": 100, "b/b.c": 50}, {"a/a.c": 120, "b/b.c": 50}])
    monkeypatch.setattr(sizes, "table", lambda *args, **kwargs: next(tables))
    monkeypatch.setattr(sys, "argv", ["sizes.py", "base", "new", "-O2"])
    with pytest.raises(SystemExit):
        sizes.main()
    out = capsys.readouterr().out
    assert re.search(r"a/a\.c\s+bytes\s+100 ->\s+120 \(\+20\)", out), out
    assert "b/b.c" not in out, "an unchanged program is not listed"
    assert " ins " not in out and " mem " not in out and "TOTAL bytes 150 -> 170" in out, out


def test_the_isel_variable_reaches_the_compiler_command(monkeypatch):
    """`LLRM_ISEL=dag python3 tools/sizes.py` measured the old selector twice (185238 -> 185238 bytes): the variable was never passed on."""
    command = ["llrm-c", "x.c", "-Os"]
    monkeypatch.delenv("LLRM_ISEL", raising=False)
    assert selected(command) == command
    monkeypatch.setenv("LLRM_ISEL", "dag")
    assert selected(command) == ["llrm-c", "--isel=dag", "x.c", "-Os"]


def test_a_basic_program_takes_the_excess_precision_a_pair_names(tmp_path, monkeypatch):
    """main compiled SINGLE as fast and the tip as standard: a pair on the defaults read +39 B on stft_single.bas as growth. SIZES_EXCESS sets both sides."""
    monkeypatch.setenv("SIZES_EXCESS", "fast")
    listed = dict(sizes.programs(tmp_path, demos=False))
    assert "-fexcess-precision=fast" in listed["stft_single/stft_single.bas"]
    assert not any("-fexcess-precision" in word for word in listed["stft_single/stft_single.c"])


def test_a_c_program_is_built_by_llrm_cc_to_an_object_on_a_watcom_row(tmp_path):
    """sizes.py ran bins/llrm-c, which is gone: every C program read as failing to compile and tests/test_programs_compile.py went red once
    its binary list named llrm-cc instead."""
    import sizes

    command = sizes.c_command(str(tmp_path / "llrm-cc"), str(sizes.ROOT / "bench/bintree/bintree.c"))
    assert Path(command[0]).name == "llrm-cc" and "--target=x86_16-wat" in command
    assert sizes.stop(command) == ["-c"] and sizes.stop(["llrm-qb", "x.bas"]) == []


def code_object(tmp_path, content: bytes, use32: bool = False) -> Path:
    path = tmp_path / "pad.obj"
    attributes = 0x49 if use32 else 0x48
    path.write_bytes(
        record(0x80, name("x")) + lnames("_TEXT", "CODE") + record(0x98, bytes([attributes]) + len(content).to_bytes(2, "little") + bytes([1, 2, 1]))
        + record(0xA0, bytes([1, 0, 0]) + content) + record(0x8A, b"\0")
    )
    return path


RET, NOP = b"\xc3", b"\x90"


def test_a_layout_shift_before_an_aligned_loop_leaves_code_bytes_unchanged(tmp_path):
    """x86-m32 quicksort bq32 -O2: the nops before the first align-16 loop went 2 -> 14 B because a 4-byte mov moved, and code_bytes (SEGDEF sizes) read +12 B of growth."""
    near = code_size(code_object(tmp_path, RET + NOP * 3 + RET))  # pad to 4
    far = code_size(code_object(tmp_path, RET + NOP * 15 + RET))  # pad to 16
    assert (near.code, far.code) == (2, 2)
    assert (near.align, far.align) == (3, 15)


def test_multibyte_nops_and_register_self_moves_are_padding_in_32_bit_code(tmp_path):
    pad = b"\x0f\x1f\x44\x00\x00" + b"\x89\xf6" + b"\x8d\x76\x00"  # nop dword [eax+eax], mov esi,esi, lea esi,[esi+0]: 10 B, ends at 11
    got = code_size(code_object(tmp_path, RET + pad + b"\x90\x90\x90\x90\x90" + RET, use32=True))  # 1 + 10 + 5 = 16
    assert (got.code, got.align) == (2, 15)


def test_a_nop_run_that_does_not_end_on_a_boundary_or_fills_one_stays_counted(tmp_path):
    """An instruction run that looks like padding is code unless it ends where an alignment would end it and is shorter than that alignment."""
    assert code_size(code_object(tmp_path, RET + NOP * 4 + RET)) == sizes.CodeSize(6, 0, 0)  # ends at 5
    assert code_size(code_object(tmp_path, NOP * 4 + RET)) == sizes.CodeSize(5, 0, 0)  # as long as the alignment it ends on


def test_bytes_that_start_no_instruction_are_skipped_counted_and_the_padding_after_them_is_still_found(tmp_path):
    """BASIC objects carry a 48-byte module header ahead of the code in the same segment, and an instruction can straddle two LEDATA records: decoding
    the segment to its end stopped at the header, and 22 of 41 flat cells read no padding at all."""
    header, code = b"\xc4", RET + NOP * 2 + RET
    path = tmp_path / "two.obj"
    path.write_bytes(
        record(0x80, name("x")) + lnames("_TEXT", "CODE")
        + record(0x98, bytes([0x48]) + (len(header) + len(code)).to_bytes(2, "little") + bytes([1, 2, 1]))
        + record(0xA0, bytes([1, 0, 0]) + header) + record(0xA0, bytes([1, len(header), 0]) + code) + record(0x8A, b"\0")
    )
    assert code_size(path) == sizes.CodeSize(len(header) + 2, 2, 1)

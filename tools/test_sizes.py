"""`uv run --project tools --with pytest python -m pytest tools/test_sizes.py`

sizes.py's instrument: it counts code, not the records the linker consumes (rule 3)."""

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import pytest  # noqa: E402
import sizes  # noqa: E402
from sizes import code_bytes, data_bytes, selected  # noqa: E402


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

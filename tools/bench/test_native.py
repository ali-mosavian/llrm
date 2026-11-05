"""The x86_64 rows: which programs run natively, and that a row is a count of the program's own run."""

import shutil
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
import native  # noqa: E402

PROGRAM = "extern void report(long value);\nlong bench_f(int n) { long s = 0; for (int i = 0; i < n; i++) s += i * i; return s; }\nint main(void) { report(bench_f(20)); return 0; }\n"


def program(tmp_path, name="f", source=PROGRAM, out="2470\n"):
    directory = tmp_path / name
    directory.mkdir()
    (directory / f"{name}.c").write_text(source)
    if out is not None:
        (directory / f"{name}.out").write_text(out)
    return directory


def test_a_program_runs_natively_only_with_main_an_expected_output_and_no_far_pointer(tmp_path):
    """DOS-only sources (far pointers) have no x86_64 reading: textfill and lru are not-applicable, not compile failures."""
    assert native.candidates(program(tmp_path)) is not None
    assert native.candidates(program(tmp_path, "g", out=None)) is None
    assert native.candidates(program(tmp_path, "h", "char far *p; int main(void) { return 0; }\n")) is None
    assert native.candidates(program(tmp_path, "i", "int f(void) { return 0; }\n")) is None
    assert native.candidates(program(tmp_path, "j", "char __huge *p; int main(void) { return 0; }\n")) is None
    assert native.candidates(program(tmp_path, "k", "// data: photo.jpg\n" + PROGRAM)) is None  # jpeg and stft printed the file name where a number was expected


def test_a_row_counts_one_run_of_the_program_not_the_start_up_or_the_driver(tmp_path):
    """Instructions are (11 runs - 1 run) / 10 of `bench_main_`: a loop of n iterations must grow with n, and the build of the same source twice reads equal."""
    if shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs gcc and perf")
    bench = tmp_path / "inc"
    bench.mkdir()
    one, two = program(tmp_path), program(tmp_path, "f2")
    small = native.measure_one("gcc64", tmp_path, one, one / "f.c", "O1", tmp_path / "w", bench)
    again = native.measure_one("gcc64", tmp_path, two, two / "f2.c", "O1", tmp_path / "w", bench)
    big_dir = program(tmp_path, "big", PROGRAM.replace("bench_f(20)", "bench_f(200)"), out="2646700\n")
    big = native.measure_one("gcc64", tmp_path, big_dir, big_dir / "big.c", "O1", tmp_path / "w", bench)
    assert "error" not in small and "error" not in big, (small, big)
    assert abs(small["instructions"] - again["instructions"]) <= 1  # the rounding of a mean
    assert big["instructions"] - small["instructions"] > 3 * 180  # 180 more iterations of a loop of at least three instructions


def test_a_program_that_prints_the_wrong_value_is_an_error_not_a_count(tmp_path):
    if shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs gcc and perf")
    directory = program(tmp_path, out="1\n")
    row = native.measure_one("gcc64", tmp_path, directory, directory / "f.c", "O1", tmp_path / "w", tmp_path)
    assert "prints" in row.get("error", "") and "instructions" not in row


def test_the_gated_column_is_flagless_and_the_columns_beside_it_are_no_pic(tmp_path):
    """llrm-cc -m64 without -fno-pic writes rip-relative code (memcpy_runtime -Os 235 B, 209 B with it) and main ignores the flag: the gate compares flagless head to flagless
    base, and the column that compares to gcc and clang (both -fno-pic) is ours with the flag, reported only."""
    def command(column):
        return native.compile_command(column, tmp_path, tmp_path / "a.c", "O2", tmp_path / "a.o", tmp_path)[0]

    assert "-fno-pic" not in command(native.OURS)
    assert all("-fno-pic" in command(column) for column in native.REPORTED)
    assert command(native.NOPIC)[0] == command(native.OURS)[0]


def test_one_binary_measured_twice_reads_the_same_count_to_the_rounding(tmp_path):
    """The first driver timed each run with rdtsc and insertion-sorted the timings: its own instruction count followed the noise, and one binary read 507, 499, 494 and 492 (173 without it)."""
    if shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs gcc and perf")
    directory = program(tmp_path)
    native.measure_one("gcc64", tmp_path, directory, directory / "f.c", "O1", tmp_path / "w", tmp_path)
    exe = tmp_path / "w" / "f.gcc64.O1"
    counts = {round((native.instructions_of(exe, "101", directory) - native.instructions_of(exe, "1", directory)) / 100) for _ in range(5)}
    assert max(counts) - min(counts) <= 1  # the count is a mean over 100 runs, rounded: one instruction is its resolution


def test_a_program_gcc_and_clang_cannot_build_either_is_skipped_not_an_error_of_ours(tmp_path, monkeypatch):
    """parity/scalar and parity/parity call helpers only the DOS harness defines: every column failed to link and the judge read ours as an error."""
    directory = program(tmp_path)
    monkeypatch.setattr(native, "perf_missing", lambda: None)
    monkeypatch.setattr(native, "measure_one", lambda column, *rest: {"error": "link: undefined reference to `keep'"})
    rows = native.measure([("f", directory)], tmp_path, ["O2"], tmp_path / "w", tmp_path)
    assert {key[1] for key in rows} == {native.OURS, *native.REPORTED}
    assert all("skip" in row and "error" not in row for row in rows.values())
    monkeypatch.setattr(native, "measure_one", lambda column, *rest: {"error": "compile: x"} if column == native.OURS else {"instructions": 1})
    assert "error" in native.measure([("f", directory)], tmp_path, ["O2"], tmp_path / "w", tmp_path)[("f", native.OURS, "O2")]


def test_the_pin_column_compiles_with_the_pinned_clang(tmp_path, monkeypatch):
    """The scoreboard compares ours to gcc and clang; the pinned LLVM clang is the compiler llrm tracks, and its column is named by LLRM_PIN_CLANG."""
    monkeypatch.setenv("LLRM_PIN_CLANG", "/pin/bin/clang")
    command, _ = native.compile_command("pin64", tmp_path, tmp_path / "a.c", "O2", tmp_path / "a.o", tmp_path)
    assert command[0] == "/pin/bin/clang" and "-fno-pic" in command


def test_nbody_single_prints_its_checksum_on_x86_64(tmp_path):
    """main 65098fd950's llrm-cc -m64 printed 3731452 for bench/nbody_single (gcc and clang 86016); release/next prints 86016. A float program must not change its answer with the compiler."""
    if not (native.BIN / "llrm-cc").is_file() or shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs llrm-cc, gcc and perf")
    directory = native.ROOT / "bench" / "nbody_single"
    row = native.measure_one(native.OURS, native.BIN, directory, directory / "nbody_single.c", "O2", tmp_path, native.ROOT / "bench")
    assert "error" not in row, row


def test_the_counted_source_runs_and_the_sized_source_is_sized(tmp_path):
    """crc's bench_crc(0) folded into main: llrm and clang read 21 instructions where the loop runs 600+ (gcc 665). The harness gives a call-keeping counted source and a noinline sized one."""
    if shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs gcc and perf")
    directory = program(tmp_path, "g", PROGRAM)
    (directory / "sized.c").write_text(PROGRAM + "\nlong pad_(long n) { return n * 3 + n / 7; }\n")
    plain = native.measure_one("gcc64", tmp_path, directory, directory / "g.c", "O2", tmp_path / "w", tmp_path)
    both = native.measure_one("gcc64", tmp_path, directory, directory / "g.c", "O2", tmp_path / "w2", tmp_path, directory / "sized.c")
    assert "error" not in both, both
    assert both["instructions"] == plain["instructions"]
    assert both["code_bytes"] > plain["code_bytes"]


def test_expected_output_is_read_from_the_program_not_the_counted_source(tmp_path):
    """The counted source lives in the harness's folder, beside no `.out`: every c64 row with a call-keeping pair died on FileNotFoundError."""
    if shutil.which("gcc") is None or native.perf_missing():
        pytest.skip("needs gcc and perf")
    directory = program(tmp_path, "g", PROGRAM)
    elsewhere = tmp_path / "kept"
    elsewhere.mkdir()
    (elsewhere / "g.wrapped.c").write_text(PROGRAM)
    row = native.measure_one("gcc64", tmp_path, directory, elsewhere / "g.wrapped.c", "O2", tmp_path / "w", tmp_path)
    assert "error" not in row, row


def test_the_per_run_count_does_not_flip_with_the_counters_jitter(monkeypatch):
    """crc c64 -O2 read 482 on most runs and 483 on some: the difference of two counts over 100 runs is 482.4 (cold first run), the counter's ~15
    instructions of jitter moved it to 482.55, and rounding flipped the row. Over a wider window the same jitter stays under .5."""
    for jitter in (0, 15, -15):
        def count(exe, argument, cwd, repeats=7, jitter=jitter):
            runs = int(argument)
            return 190656 + (jitter if runs == 1 else 0) + round(482.4 * (runs - 1))

        monkeypatch.setattr(native, "instructions_of", count)
        assert native.instructions_per_run(Path("x"), Path(".")) == 482, jitter


def test_pic_mode_reads_the_objects_relocations_not_the_flag(tmp_path):
    """hanoi2 -O2 read 10.6% behind clang when ours was pc-relative and clang absolute; like for like ours was 16% ahead. The mode is the object's, and a column of
    the other mode is an error row, not a number."""
    if shutil.which("gcc") is None or shutil.which("readelf") is None:
        pytest.skip("no gcc or readelf")
    source = tmp_path / "t.c"
    source.write_text("static int a[100];\nint f(int i) { return a[i]++; }\n")
    modes = {}
    for flag in ("-fPIE", "-fno-pic"):
        obj = tmp_path / f"t{flag}.o"
        assert __import__("subprocess").run(["gcc", "-O2", flag, "-c", str(source), "-o", str(obj)]).returncode == 0
        modes[flag] = native.pic_mode(obj)
    assert modes == {"-fPIE": "pic", "-fno-pic": "nopic"}
    row = {"instructions": 1}
    assert native.mismatch(row, "clang64", tmp_path / "t-fno-pic.o", tmp_path / "t-fPIE.o") == {"error": "pic mode: clang64 is nopic, ours is pic"}
    assert native.mismatch(row, "clang64", tmp_path / "t-fno-pic.o", tmp_path / "t-fno-pic.o") == row


def test_pic_mode_is_read_from_the_sized_object_not_the_wrapper_pointer_beside_it(tmp_path):
    """hanoi gcc64/clang64 O2 were error rows (`pic mode: gcc64 is pic, ours is mixed`) for 2 cells: the check read the counted object, where the harness's global pointer is a pc-relative data reference in every column. It reads the sized one."""
    for name in ("hanoi.gcc64.O2.o", "hanoi.gcc64.O2.sized.o", "hanoi.c64nopic.O2.o"):
        (tmp_path / name).write_bytes(b"")
    assert native.checked_object(tmp_path, "hanoi", "gcc64", "O2").name == "hanoi.gcc64.O2.sized.o"
    assert native.checked_object(tmp_path, "hanoi", native.NOPIC, "O2").name == "hanoi.c64nopic.O2.o"


@pytest.mark.parametrize("compiler", ("gcc", "clang", "ours"))
def test_pic_mode_reads_a_known_object_of_each_compiler_as_its_mode(compiler, tmp_path):
    """Positive controls: -fPIC and -fPIE read pic, -fno-pic reads nopic, per compiler, on code with a .bss load beside an indexed .rodata one. gcc -fno-pic wrote an
    absolute .rodata reference beside a pc-relative .bss one and read `mixed`, so 16 sfm16 gcc64/clang64 O2 cells were error rows against ours (nopic)."""
    import subprocess

    import llrmbin

    source = tmp_path / "t.c"
    source.write_text("static int a[100];\nstatic int b;\nstatic const float k[4] = {1.5f, 2.5f, 3.5f, 4.5f};\nint f(int i) { return a[i]++ + b; }\nfloat g(int i) { return k[i & 3]; }\n")
    if compiler == "ours":
        program, env = [str(llrmbin.compiler(llrmbin.bin_dir())), "-m64"], {**__import__("os").environ, "LLRM_CC_TARGET": "x86_64-linux"}
    else:
        if shutil.which(compiler) is None or shutil.which("readelf") is None:
            pytest.skip(f"no {compiler} or readelf")
        program, env = [compiler], None
    for flag, want in (("-fPIC", "pic"), ("-fPIE", "pic"), ("-fno-pic", "nopic")):
        obj = tmp_path / f"t{flag}.o"
        assert subprocess.run([*program, "-O2", flag, "-c", str(source), "-o", str(obj)], env=env).returncode == 0
        assert native.pic_mode(obj) == want, (compiler, flag)

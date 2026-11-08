"""A program tagged `// targets: dos` read as FAIL on z80, wasm and m68k: three harnesses kept their own lists of the programs
they cannot build (far, huge, watcom, regparm, fs/gs, os_*), and each cell counted as a hole."""
import re
import subprocess
import sys
from pathlib import Path

import corpus_targets as ct

HERE = Path(__file__).resolve().parent
RUN = HERE.parent / "tests/run/c"


def test_a_dos_program_is_not_applicable_elsewhere():
    source = "// flags: -O2 -mabi=watcom\n// targets: dos\nint main(void){return 0;}\n"
    assert ct.applies("x86_16-wat", source) and ct.applies("x86_32-dos", source)
    assert not any(ct.applies(t, source) for t in ("z80", "wasm32", "m68k-68020", "x86_64-linux", "6502-sim65", "arm64-linux"))
    assert ct.reason(source) == "n/a, DOS-only"


def test_an_untagged_program_is_for_every_target():
    assert ct.applies("wasm32", "int main(void){return 0;}\n")


def test_a_family_and_a_full_name_both_select():
    source = "// targets: x86-m16 wasm32\n"
    assert ct.applies("x86_16-dos", source) and ct.applies("wasm32", source) and not ct.applies("x86_32-dos", source)


def test_the_command_prints_na_and_exits_3_not_a_failure(tmp_path):
    program = tmp_path / "p.c"
    program.write_text("// targets: dos\n")
    done = subprocess.run([sys.executable, str(HERE / "corpus_targets.py"), "check", "z80", str(program)], capture_output=True, text=True)
    assert (done.returncode, done.stdout.strip()) == (3, "n/a, DOS-only")


def test_the_shipped_dos_programs_are_tagged():
    for name in ("watcom_far_sum", "huge_fill", "regparm3_matrix_4", "fsgs_call", "os_screen", "far_return"):
        assert not ct.applies("z80", (RUN / f"{name}.c").read_text()), name


def test_a_family_flags_line_is_read_only_for_its_family():
    source = "// flags: -O2 -m32\n// flags-m68k: -mcmodel=medium\nint main(void){return 0;}\n"
    assert ct.flags_for("m68k", source) == ["-mcmodel=medium"]
    assert ct.flags_for("x86", source) == []


def test_the_big_data_programs_ask_m68k_for_the_medium_model():
    """memcpy_runtime, memfuncs_runtime and memory_expansion address arrays past 32 KiB: 16-bit PC-relative displacements
    cannot reach them (gas: value too large for field of 2 bytes; ld: R_68K_PC16 truncated)."""
    for name in ("memcpy_runtime", "memfuncs_runtime", "memory_expansion"):
        assert ct.flags_for("m68k", (RUN / f"{name}.c").read_text()) == ["-mcmodel=medium"], name


def test_an_override_names_the_abi_fact_it_rests_on():
    """An expected-output override with no reason, or equal to the shared file, hides a difference or nothing."""
    overrides = sorted(RUN.glob("*.m68k.out"))
    assert overrides, "the m68k overrides are gone"
    for out in overrides:
        why = out.with_suffix(".why")
        assert why.exists() and len(why.read_text().strip().splitlines()) == 1, f"{out.name}: one line of why"
        assert why.read_text().startswith("ABI: "), f"{out.name}: an override is for an ABI difference; fix the program otherwise"
        shared = out.with_name(out.name.replace(".m68k.out", ".out"))
        assert shared.exists() and shared.read_bytes() != out.read_bytes(), f"{out.name}: stale, equal to {shared.name}"


def test_the_expected_output_is_the_targets_override_or_the_shared_file():
    source = RUN / "long_double_stride.c"
    assert ct.expected_output(source, "m68k-68020").name == "long_double_stride.m68k.out"
    assert ct.expected_output(source, "x86_32-dos").name == "long_double_stride.out"
    assert ct.expected_output(RUN / "sum.c", "m68k-68000").name == "sum.out"


def test_the_word_order_programs_pick_words_by_significance_not_index():
    """double_to_int64, float_bits and float_conversions_loop read word[0] as the low word: right on x86, the high word on
    m68k, so their .out (written on x86) mismatched at every m68k cell. One .out serves both byte orders."""
    for name in ("double_to_int64", "float_bits", "float_conversions_loop"):
        text = (RUN / f"{name}.c").read_text()
        assert "__BYTE_ORDER__" in text, name
        code = [line for line in text.splitlines() if not line.lstrip().startswith("//")]
        assert not any(re.search(r"\b(word|w)\[[01]\]", line) for line in code), name


def test_a_program_written_for_one_abi_is_tagged_for_it():
    source = (RUN / "i64_param_home.c").read_text()
    assert ct.applies("x86_32-dos", source) and not ct.applies("m68k-68000", source)


def test_the_os_layer_programs_whose_out_is_dos_semantics_are_dos_only():
    """os_files, os_find, os_names and os_console_in print the DOS layer's answers (error codes, ordering): on the POSIX layer ours
    and host gcc agree with each other and both differ from the .out. os_execute runs COMSPEC. Unmarked, x86_32-linux read them as miscompiles."""
    for name in ("os_files", "os_find", "os_names", "os_console_in", "os_execute"):
        source = (RUN / f"{name}.c").read_text()
        assert not ct.applies("x86_32-linux", source) and not ct.applies("m68k-68020", source) and ct.applies("x86_32-dos", source)


def test_an_lp64_program_is_only_for_targets_with_a_64_bit_long():
    """mulconst_i64's .out is LP64; the m68k executed sweep counted it as a failure on every compiler."""
    source = "// targets: lp64\nlong f(void);\n"
    assert ct.applies("x86_64-linux", source) and ct.applies("arm64-linux", source)
    assert not any(ct.applies(t, source) for t in ("m68k-68020", "x86_32-linux", "x86_16-dos", "wasm32", "z80-sdcc", "6502-sim65"))


def test_the_mulconst_i64_program_is_not_run_on_m68k():
    source = (Path(__file__).parent.parent / "tests/run/c/mulconst_i64.c").read_text()
    assert not ct.applies("m68k-68020", source) and ct.applies("x86_64-linux", source)


def test_a_negated_token_excludes_only_those_targets():
    source = "// targets: !linux\nlong double f(void);\n"
    assert not ct.applies("x86_32-linux", source) and not ct.applies("x86_64-linux", source)
    assert ct.applies("x86_32-dos", source) and ct.applies("m68k-68020", source) and ct.applies("x86_16-wat", source)


def test_long_double_stride_is_not_run_on_the_sysv_rows():
    """Its .out is Watcom's 8-byte long double; x86_32-linux got 12/24/20/12 (gcc's too) and read as a miscompile."""
    source = (RUN / "long_double_stride.c").read_text()
    assert not ct.applies("x86_32-linux", source) and ct.applies("x86_32-dos", source) and ct.applies("m68k-68020", source)


def test_the_stack_sanitizer_programs_are_dos_only():
    """stackdeep and stackover link the DOS runtime's LL$STACK_LOW and __STKOVERFLOW; tagged x86-m32 they ran on x86_32-linux and failed to link."""
    for name in ("stackdeep", "stackover"):
        source = (RUN / f"{name}.c").read_text()
        assert not ct.applies("x86_32-linux", source) and ct.applies("x86_32-dos", source) and ct.applies("x86_16-dos", source)




def test_stft_single_has_an_expected_output_per_float_model():
    """The DOS .out is x87 excess precision. Strict single (m68k, m64) and arm64's fused multiply-add give other checksums; each target
    compared with the x87 .out read as a miscompile."""
    source = HERE.parent / "bench/stft_single/stft_single.c"
    want = {"m68k-68020": "502187692", "x86_64-linux": "502187692", "arm64-linux": "40144774", "x86_32-dos": "78803551", "wasm32": "502187692"}
    for target, value in want.items():
        assert ct.expected_output(source, target).read_text().strip() == value, target

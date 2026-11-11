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


def test_programs_that_assume_a_32_bit_int_are_not_run_at_m16():
    """select_of_constants and shl_of_a_zero_extended_dividend hash values wider than m16's 16-bit int: the m16 run read as a miscompile (6528 for 880220544; a 200000-iteration run past the 120 s limit). Host gcc -m32 matches their .out."""
    for name in ("select_of_constants", "shl_of_a_zero_extended_dividend"):
        source = (RUN / f"{name}.c").read_text()
        assert not ct.applies("x86_16-dos", source) and ct.applies("x86_32-linux", source) and ct.applies("x86_64-linux", source)




SMALL_INT_TARGETS = ("x86_16-dos", "x86_16-wat", "z80-sdcc", "6502-sim65")


def test_a_program_marked_x86_widths_is_not_run_on_any_16_bit_int_target():
    """select_of_constants was tagged `!x86-m16`, which left z80 and 6502 in: their correct 16-bit output (6528, 11412, ...) read as WRONG against the 32-bit-int .out. The marker `// x86-widths:` says the .out depends on the x86 widths; every target with a 16-bit int must be excluded by the tags."""
    marked = [p for p in sorted(RUN.glob("*.c")) if "\n// x86-widths:" in p.read_text()]
    assert len(marked) >= 3
    for path in marked:
        source = path.read_text()
        assert not any(ct.applies(t, source) for t in SMALL_INT_TARGETS), f"{path.name} is for a 16-bit-int target"


def test_a_program_run_on_z80_or_6502_is_also_run_at_m16():
    """m16 executes the corpus with a 16-bit int against the shared .out, so a program it does not run has no executed proof that its .out is width-independent; select_of_constants and shl_of_a_zero_extended_dividend were such programs and z80/6502 reported WRONG on them."""
    for path in sorted(RUN.glob("*.c")):
        source = path.read_text()
        small = [t for t in ("z80-sdcc", "6502-sim65") if ct.applies(t, source)]
        assert not small or ct.applies("x86_16-wat", source), f"{path.name} is for {small} but not for m16"


def test_tail_calls_edges_is_not_run_where_the_backend_takes_no_fno_inline_names():
    """tail_calls_edges is callee-pop x86 code (every variant -march=i486 or -mabi=...) and its `-fno-inline=NAME` flags are backend-only: llrm-wasm-llc refused them (build failure, 3 cells). It is for the x86 rows."""
    source = (RUN / "tail_calls_edges.c").read_text()
    assert ct.applies("x86_16-dos", source) and ct.applies("x86_32-linux", source)
    assert not ct.applies("wasm32", source) and not ct.applies("arm64-linux", source) and not ct.applies("x86_64-linux", source)


def test_programs_with_over_64k_data_are_not_run_in_a_16_bit_address_space():
    """memcpy_runtime and memfuncs_runtime keep 70 KB arrays: x86_16 failed to link (segment 140032 > 65536), z80 and 6502 died in clang
    ("array is too large"); the sweep listed them as compile refusals of the target."""
    for name in ("memcpy_runtime", "memfuncs_runtime"):
        source = (RUN / f"{name}.c").read_text()
        assert not any(ct.applies(t, source) for t in ("x86_16-wat", "z80-sdcc", "6502-sim65"))
        assert all(ct.applies(t, source) for t in ("x86_32-linux", "x86_64-linux", "wasm32", "arm64-linux"))


def test_a_24_bit_bit_field_program_is_not_run_with_a_16_bit_int():
    """bitfield_overhang_init declares `int b : 24`: clang rejects it on x86_16, z80 and 6502 (width exceeds the type's 16 bits)."""
    source = (RUN / "bitfield_overhang_init.c").read_text()
    assert not any(ct.applies(t, source) for t in ("x86_16-wat", "z80-sdcc", "6502-sim65"))
    assert ct.applies("x86_32-linux", source) and ct.applies("wasm32", source)


def test_a_program_including_stdio_is_not_run_on_a_freestanding_target():
    """widening_multiply includes <stdio.h>: wasm32, z80 and 6502 have none ('stdio.h' file not found), x86_16 and the sysv rows do."""
    source = (RUN / "widening_multiply.c").read_text()
    assert not any(ct.applies(t, source) for t in ("wasm32", "z80-sdcc", "6502-sim65"))
    assert ct.applies("x86_16-wat", source) and ct.applies("x86_64-linux", source)


def test_long_double_variadic_runs_on_m68k_and_its_row_is_closed():
    """The pin prints 0 for it (M68K-VA-DOUBLE); ours reads the double at the ABI slot (#8453), so it is a cell again, not a skip."""
    source = (RUN / "long_double_variadic.c").read_text()
    assert all(ct.applies(t, source) for t in ("m68k-68000", "m68k-68020", "x86_32-linux"))
    row = (RUN.parents[2] / "docs/working/m68k-va-double-row.md").read_text()
    assert "CLOSED by #8453" in row and "Owner: m68k" in row and "Review: 2026-12-20" in row

def test_a_program_whose_flags_all_force_a_32_bit_x86_mode_is_not_run_on_z80_or_6502():
    """urem_eq (`-m32 -march=i486`) and spill_store_order (`-m32`) were run on z80 and 6502 and read WRONG: their .out is the 32-bit-int result (74938 for a 16-bit 57373), and spill_store_order shifts a 16-bit unsigned by 16 (poison)."""
    for name in ("urem_eq", "spill_store_order"):
        source = (RUN / f"{name}.c").read_text()
        assert ct.x86_32_only(source)
        assert not any(ct.applies(t, source) for t in ("z80-sdcc", "6502-sim65"))
        assert ct.applies("arm64-linux", source) and ct.applies("x86_32-linux", source)
    assert not ct.x86_32_only("// flags: -O2 -m32 | -O2 -m16\nint x;\n")


def test_a_program_aligning_a_local_past_a_byte_stack_is_not_run_on_z80():
    """z80 passed over_aligned_local_after_even_pushes on a folded `ptr & 15` (frames keep one byte): now a named refusal."""
    source = (RUN / "over_aligned_local_after_even_pushes.c").read_text()
    assert not ct.applies("z80-sdcc", source)
    assert ct.applies("x86_64-linux", source) and ct.applies("m68k-68020", source) and ct.applies("wasm32", source)


def test_programs_that_assume_a_32_bit_int_or_an_80_bit_long_double_are_not_run_on_6502_or_z80():
    """6502 reported WRONG for bitcount_u32 (15 want 31), bitcount_idioms and long_double_decimal_literal (0x999A want 0xCCCD, the
    double's low word) at both levels: a 16-bit `unsigned` and a 64-bit long double answered correctly for their types, and the
    sweep counted three unmeasured cells as miscompiles. Only 6502 and z80 leave long_double_decimal_literal: m68k measures it
    against its own `.m68k.out` and arm64 and wasm32 keep it as before."""
    for name in ("bitcount_u32", "bitcount_idioms", "long_double_decimal_literal"):
        source = (RUN / f"{name}.c").read_text()
        assert not any(ct.applies(t, source) for t in ("6502-sim65", "z80-sdcc")), name
        assert ct.applies("x86_64-linux", source), name
    literal = (RUN / "long_double_decimal_literal.c").read_text()
    assert all(ct.applies(t, literal) for t in ("m68k-68020", "arm64-linux", "wasm32", "x86_16-dos", "x86_32-linux"))
    assert ct.applies("arm64-linux", (RUN / "bitcount_u32.c").read_text())


NON_X86 = ("arm64-linux", "m68k-68020", "wasm32", "z80-sdcc", "6502-cc65", "x86_64-linux")


def test_an_untagged_program_in_a_dos_only_directory_is_for_dos_only():
    """arm64's sweep counted the 21 untagged tests/fixtures/callconv files, runtime/qb/dos and tests/cc/models programs as failures: only 58 of 861 C files carry a targets line."""
    source = "int f(void){return 0;}\n"
    for path in ("tests/fixtures/callconv/c/pncallee.c", "runtime/qb/dos/m16/far.c", "tests/cc/models/pointers.c"):
        assert not any(ct.applies(t, source, Path(path)) for t in NON_X86), path
        assert ct.applies("x86_16-dos", source, Path(path)) and ct.applies("x86_32-wat", source, Path(path)), path


def test_a_dos_marker_in_an_untagged_source_makes_it_dos_only():
    for source in ("int f(void){ __asm { mov ax, 1 } return 0; }\n", "QB_FAR char *p;\n", "char __far *p;\n", "#include <dos.h>\n", "void f(void){ __emit__(0x90); }\n"):
        assert not any(ct.applies(t, source) for t in NON_X86), source
        assert ct.applies("x86_16-dos", source), source


def test_an_explicit_targets_line_beats_the_directory_default():
    source = "// targets: !z80-sdcc\nint f(void){return 0;}\n"
    assert ct.applies("arm64-linux", source, Path("tests/fixtures/callconv/c/x.c"))
    assert not ct.applies("z80-sdcc", source, Path("tests/fixtures/callconv/c/x.c"))


def test_a_plain_untagged_program_outside_those_directories_still_applies_everywhere():
    assert all(ct.applies(t, "int f(void){return 0;}\n", Path("tests/run/c/plain.c")) for t in NON_X86)


def test_an_lp64_target_reads_the_lp64_expected_output(tmp_path):
    """A `long` of 64 bits printed 3421780262 where the 32-bit-long .out wanted -873187034: 108 cells of wasm-b's x86_64 run read as
    differences, in 36 programs where llrm matched host clang -m64. The shared .out stays the ILP32 truth; LP64 targets read NAME.lp64.out."""
    source = tmp_path / "p.c"
    for name in ("p.out", "p.lp64.out"):
        (tmp_path / name).write_text("x\n")
    for target in ("x86_64-linux", "arm64-linux", "arm64-macos"):
        assert ct.expected_output(source, target).name == "p.lp64.out", target
    for target in ("x86_32-linux", "m68k-68020", "wasm32", "z80", "x86_16-dos"):
        assert ct.expected_output(source, target).name == "p.out", target
    (tmp_path / "p.x86_64.out").write_text("y\n")
    assert ct.expected_output(source, "x86_64-linux").name == "p.x86_64.out", "a family file is more specific than the width file"
    assert ct.expected_output(tmp_path / "q.c", "x86_64-linux").name == "q.out"


def test_a_cell_that_differs_from_the_oracle_on_base_and_head_is_a_stale_oracle():
    """wasm-b's identity run printed 108 cells as differences from .out where base and head agreed with each other: the oracle was stale (or
    the harness fed no input, or kept CRLF), not the compiler. An agreement of base and head is not a pass or a fail against it."""
    assert ct.oracle_verdict("1\n", base="1\n", head="1\n") == "PASS"
    assert ct.oracle_verdict("1\n", base="1\n", head="2\n") == "FAIL"
    assert ct.oracle_verdict("1\n", base="2\n", head="2\n") == "STALE-ORACLE"
    assert ct.oracle_verdict("1\n", base="2\n", head="3\n") == "STALE-ORACLE"
    assert ct.oracle_verdict("1\n", base="2\n", head="1\n") == "PASS", "a fix of the base is a pass"
    assert ct.oracle_verdict("ab\ncd\n", base="ab\r\ncd\r\n", head="ab\r\ncd\r\n") == "PASS", "a DOS console's CRLF is the same text"


def test_an_lp64_override_names_the_abi_fact_it_rests_on():
    """An LP64 expected output with no reason, or equal to the shared file, hides a difference or nothing."""
    overrides = sorted(HERE.parent.glob("**/*.lp64.out"))
    assert overrides, "the lp64 overrides are gone"
    for out in overrides:
        why = out.with_suffix(".why")
        assert why.exists() and len(why.read_text().strip().splitlines()) == 1, f"{out.name}: one line of why"
        assert why.read_text().startswith("ABI: "), f"{out.name}: an override is for an ABI difference; fix the program otherwise"
        shared = out.with_name(out.name.replace(".lp64.out", ".out"))
        assert shared.exists() and shared.read_bytes() != out.read_bytes(), f"{out.name}: stale, equal to {shared.name}"


def test_programs_that_pun_a_32_bit_word_do_not_type_it_long():
    """sfm16_float* typed their 32-bit words `long` and punned them through `union { unsigned long u; float f; }`, and fixed_point read a
    32-bit _Accum through `unsigned long`: on LP64 the upper four bytes of the union are uninitialised, so gcc -O2 printed 54'305'724'024'370'529
    where clang printed 954'867'168'609 for the same sfm16_float_add source, and fixed_point printed stack addresses at -O0."""
    for source in sorted((HERE.parent / "bench").glob("sfm16_float*/*.c")):
        text = source.read_text()
        assert "typedef long sfi32" not in text and "unsigned long u; float f" not in text, source.name
    text = (RUN / "fixed_point.c").read_text()
    assert "unsigned long," not in text and "report((long)u.i)" not in text


def test_a_65c02_row_is_as_not_applicable_as_a_6502_row():
    """The width and freestanding regexes spelled `6502` and missed `65c02`: bitfield_overhang_init, memcpy_runtime, memfuncs_runtime and
    widening_multiply (`!int16`, `!ptr16`, `!freestanding`) were run and refused on 65c02, 12 rows."""
    for name in ("bitfield_overhang_init", "memcpy_runtime", "memfuncs_runtime", "widening_multiply"):
        source = (RUN / f"{name}.c").read_text()
        for target in ("6502-sim65", "65c02-sim65"):
            assert not ct.applies(target, source), (name, target)
    assert not ct.applies("65c02-sim65", "// flags: -O2 -m32\nint x;\n"), "a 32-bit-x86-only program is no 65c02 program either"


def test_a_program_with_a_far_pointer_is_dos_only_without_a_tag():
    """sum_three declares `short far *data` with no `// targets: dos`: 30 rows were refused by a clang front-end error on every other target."""
    source = (HERE.parent / "bench/parity/sum_three/sum_three.c").read_text()
    for target in ("x86_64-linux", "arm64-linux", "m68k-68020", "z80", "6502-sim65", "wasm32", "x86_32-linux"):
        assert not ct.applies(target, source), target
    assert ct.applies("x86_32-dos", source) and ct.applies("x86_16-dos", source)
    assert ct.applies("x86_64-linux", "/* as far as we know */\nint x; int far_away;\n"), "prose and names are not a far pointer"


def test_a_program_that_prints_through_the_hosted_libc_is_not_run_for_macos():
    """widening_multiply includes <stdio.h>; the arm64-macos row has no macOS sysroot, so it parsed the host's /usr/include/stdio.h and died."""
    source = (RUN / "widening_multiply.c").read_text()
    assert not ct.applies("arm64-macos", source)
    assert ct.applies("arm64-linux", source) and ct.applies("x86_64-linux", source)
    assert not ct.applies("arm64-macos", "// targets: !macos\nint x;\n") and ct.applies("arm64-linux", "// targets: !macos\nint x;\n")


def test_a_vla_program_is_not_run_where_the_backend_has_no_dynamic_alloca():
    """vla_callee_constant is in the sweeps for the targets that build it; m68k, 6502, z80 and wasm refuse a dynamic alloca (rc 1) and read n/a, not FAIL."""
    source = (RUN / "vla_callee_constant.c").read_text()
    for target in ("m68k-68000", "m68k-68020", "6502-sim65", "65c02-sim65", "z80-sdcc", "wasm32"):
        assert not ct.applies(target, source), target
    for target in ("arm64-linux", "x86_64-linux", "x86_32-linux", "x86_16-dos"):
        assert ct.applies(target, source), target

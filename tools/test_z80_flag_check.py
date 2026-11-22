import z80_flag_check as z

# The command line of a z80 corpus cell: CLANG_FLAGS (run.py) plus the program's -f flags; and the four SDCC variants (z80-abf-lci.py).
OURS = ["-O2", "-target", "msp430", "-fsigned-char", "-fpack-struct=1", "-fno-vectorize", "-fno-slp-vectorize", "-Xclang", "-disable-llvm-passes",
        "-Xclang", "-disable-O0-optnone", "-fomit-frame-pointer", "-fno-stack-protector", "-fno-asynchronous-unwind-tables", "-S", "-emit-llvm"]
VARIANTS = [["--fsigned-char", "--opt-code-size"], ["--fsigned-char", "--opt-code-size", "--max-allocs-per-node", "200000"],
            ["--fsigned-char", "--opt-code-speed"], ["--fsigned-char", "--opt-code-speed", "--max-allocs-per-node", "200000"]]


def test_the_real_cell_flags_agree():
    assert all(z.sdcc_mismatch(OURS, v) == "" for v in VARIANTS)
    assert z.sdcc_mismatch(OURS + ["-fno-inline-functions", "-fwrapv"], VARIANTS[0]) == ""


def test_an_sdcc_only_flag_is_refused_not_measured():
    assert "--stack-auto" in z.sdcc_mismatch(OURS, ["--fsigned-char", "--opt-code-size", "--stack-auto"])


def test_a_char_signedness_on_one_side_only_is_refused():
    assert "signedness" in z.sdcc_mismatch(OURS, ["--opt-code-size"])
    assert "signedness" in z.sdcc_mismatch([f for f in OURS if f != "-fsigned-char"], VARIANTS[0])


def test_an_ours_only_flag_without_a_listed_counterpart_is_refused():
    """-funroll-loops would change ours and not SDCC; -ffixed-point has no SDCC type at all."""
    assert "-funroll-loops" in z.sdcc_mismatch(OURS + ["-funroll-loops"], VARIANTS[0])
    assert "-ffixed-point" in z.sdcc_mismatch(OURS + ["-ffixed-point"], VARIANTS[0])


def test_a_variant_needs_exactly_one_goal():
    assert z.sdcc_mismatch(OURS, ["--fsigned-char"])
    assert z.sdcc_mismatch(OURS, ["--fsigned-char", "--opt-code-size", "--opt-code-speed"])


GCC = ["-O2", "-fsigned-char", "-fpack-struct=1", "-fno-tree-loop-distribute-patterns", "-w", "-std=gnu17"]


def test_gcc_gets_the_programs_f_flags_too():
    ours = OURS + ["-fno-inline-functions", "-fwrapv"]
    assert z.gcc_mismatch(ours, GCC + ["-fno-inline-functions", "-fwrapv"]) == ""
    assert "program flags" in z.gcc_mismatch(ours, GCC)
    assert "level" in z.gcc_mismatch(OURS, ["-Os"] + GCC[1:])

    assert "program flags" in z.gcc_mismatch(OURS, ["-O2", "-fno-ipa-cp", "-fsigned-char", "-fpack-struct=1"])


FIVE = ["sfm16_float", "sfm16_float_add", "sfm16_float_sub", "sfm16_float_mul", "sfm16_float_div"]


def test_the_sdcc_command_carries_the_clang_type_macros():
    """The five sfm16_float programs name `__INT32_TYPE__`; SDCC does not define it, so they had no bar (counted as no-bar)."""
    cmd = z.sdcc_command(VARIANTS[0], "p.c", "p.rel")
    assert "-D__INT32_TYPE__=long" in cmd and "-D__UINT32_TYPE__=unsigned long" in cmd and cmd[-3:] == ["p.c", "-o", "p.rel"]


def test_the_five_sfm16_float_programs_compile_with_sdcc(tmp_path):
    import shutil
    import subprocess
    from pathlib import Path
    import pytest
    if not shutil.which("sdcc"):
        pytest.skip("no sdcc")
    root = Path(__file__).resolve().parent.parent / "bench"
    for name in FIVE:
        src = root / name / f"{name}.c"
        bare = subprocess.run(["sdcc", "-mz80", "-c", "-D__attribute__(x)=", str(src), "-o", str(tmp_path / "bare.rel")], capture_output=True)
        assert bare.returncode != 0, f"{name} builds without the macros: the test no longer shows the symptom"
        got = subprocess.run(z.sdcc_command(VARIANTS[0], src, tmp_path / f"{name}.rel"), capture_output=True)
        assert got.returncode == 0, f"{name}: {got.stderr[-300:]!r}"
